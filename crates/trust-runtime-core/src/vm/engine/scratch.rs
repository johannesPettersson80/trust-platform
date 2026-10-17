//! Execution scratch that construction reserves and every scan reuses (spec 34 MEM-03,
//! MEM-09): the stack executor's execution buffer, the local slots of its activations, the
//! initializer stacks, the output copy-back journal and the process-image staging.
use super::io::staging::StagingDemand;
use super::output_transaction::SavedDestinations;
use super::*;
use crate::vm::construction::values::ValueOperation;
use crate::vm::VmTrap;

/// Local-slot vectors for stack activations. Each has room for the largest POU's locals, so
/// handing one out never allocates.
#[derive(Debug, Default)]
pub(super) struct FrameLocals {
    spares: Vec<Vec<Value>>,
    slots: usize,
}

impl FrameLocals {
    /// Locals for an activation with `count` slots, all NULL.
    pub(super) fn take(&mut self, count: usize) -> Vec<Value> {
        let mut locals = self.spares.pop().unwrap_or_else(|| {
            // Without the register tier every finished call returns its locals here, so a
            // new vector gets the payload size and is kept. The register tier keeps or drops
            // call results itself, so its vectors are as large as the call needs.
            #[cfg(not(feature = "register-ir"))]
            let capacity = self.slots.max(count);
            #[cfg(feature = "register-ir")]
            let capacity = count;
            Vec::with_capacity(capacity)
        });
        locals.resize(count, Value::Null);
        locals
    }

    /// Keep a finished activation's locals for the next activation, emptied, when they have
    /// the payload size and the reserved room holds them; otherwise they are dropped.
    pub(super) fn recycle(&mut self, mut locals: Vec<Value>) {
        if locals.capacity() >= self.slots && self.spares.len() < self.spares.capacity() {
            locals.clear();
            self.spares.push(locals);
        }
    }

    /// Room for `levels` vectors of `slots` locals, and the program activation's vector.
    fn reserve(&mut self, levels: usize, slots: usize) -> Result<(), RuntimeError> {
        self.slots = slots;
        self.spares
            .try_reserve_exact(levels)
            .map_err(|_| VmTrap::BudgetExceeded.into_runtime_error())?;
        let mut locals = Vec::new();
        locals
            .try_reserve_exact(slots)
            .map_err(|_| VmTrap::BudgetExceeded.into_runtime_error())?;
        self.spares.push(locals);
        Ok(())
    }
}

/// The scratch sizes of one prepared module under its admitted limits.
struct ScratchPlan {
    /// Call levels of the stack executor: the module's static call depth capped at the
    /// admitted ceiling, and at least one for the entry activation (spec 34 MEM-02, MEM-09).
    levels: usize,
    /// Operand values across the levels: the deepest body, plus for every further level the
    /// most operands a body keeps below the call that started it.
    operands: usize,
    /// The largest local-slot count of any POU.
    slots: usize,
    /// Nested initializer evaluations and construction frames.
    initializers: usize,
    /// Nested typed value operations.
    types: usize,
    /// Destinations of one output copy-back group: the most parameters of any POU.
    journal: usize,
    /// The most edge inputs of any POU: one edge-input list and one restore list.
    edges: usize,
    staging: StagingDemand,
}

impl ScratchPlan {
    fn of(prepared: &PreparedModule) -> Result<Self, RuntimeError> {
        Self::with_levels(prepared, prepared.reserved_call_levels())
    }

    fn with_levels(prepared: &PreparedModule, levels: usize) -> Result<Self, RuntimeError> {
        let residue = prepared.max_call_operand_residue.unwrap_or(0);
        let operands = levels
            .saturating_sub(1)
            .checked_mul(residue)
            .and_then(|below| below.checked_add(prepared.max_operand_depth))
            .ok_or(RuntimeError::Overflow)?;
        let slots = prepared
            .vm
            .pou_by_id
            .values()
            .map(|pou| pou.local_ref_count as usize)
            .max()
            .unwrap_or(0);
        let journal = prepared
            .vm
            .pou_params
            .values()
            .map(Vec::len)
            .max()
            .unwrap_or(0);
        // `PouMap::keys` exists only with the register feature, so iterate the identity and
        // payload pairs and ignore the payloads; clippy's `keys()` suggestion does not
        // compile on the chips.
        #[allow(
            clippy::iter_kv_map,
            reason = "PouMap::keys exists only with register-ir"
        )]
        let edges = prepared
            .vm
            .pou_by_id
            .iter()
            .map(|(pou, _)| prepared.edge_inputs(*pou).len())
            .max()
            .unwrap_or(0);
        Ok(Self {
            levels,
            operands,
            slots,
            initializers: levels,
            types: crate::bytecode::BYTECODE_MAX_TYPE_NESTING + 1,
            journal,
            edges,
            staging: StagingDemand::of(&prepared.io_bindings).ok_or(RuntimeError::Overflow)?,
        })
    }

    /// Every byte the plan reserves.
    fn bytes(&self) -> Option<usize> {
        let buffer = ExecutionBuffers::reservation_bytes(self.operands, self.levels)?
            .checked_add(core::mem::size_of::<ExecutionBuffers>())?;
        let frame_locals = self
            .levels
            .checked_mul(core::mem::size_of::<Vec<Value>>())?
            .checked_add(self.slots.checked_mul(core::mem::size_of::<Value>())?)?;
        // One frame payload per call depth, `levels` local slots and presence bits wide.
        let frame_payloads =
            crate::vm::frames::payloads::FramePayloads::reservation_bytes(self.levels, self.slots)?;
        // One edge-input list and one restore list, the most edge inputs of any POU.
        let edge_input = core::mem::size_of::<crate::vm::call::context::VmEdgeInput>();
        let edge_restore =
            core::mem::size_of::<smol_str::SmolStr>().checked_add(core::mem::size_of::<Value>())?;
        let edges = self
            .edges
            .checked_mul(edge_input.checked_add(edge_restore)?)?;
        let initializers = self.initializers.checked_mul(
            core::mem::size_of::<ActiveInitializer>() + core::mem::size_of::<VmFrame>(),
        )?;
        let types = self
            .types
            .checked_mul(core::mem::size_of::<(u32, ValueOperation)>())?;
        let journal = SavedDestinations::allocation_bytes(self.journal).ok()?;
        buffer
            .checked_add(frame_locals)?
            .checked_add(frame_payloads)?
            .checked_add(edges)?
            .checked_add(initializers)?
            .checked_add(types)?
            .checked_add(journal)?
            .checked_add(self.staging.reservation_bytes()?)
    }
}

impl EngineState<'_> {
    /// Reserve the scratch that every scan reuses, charged in full against the construction
    /// allowance before any of it is reserved (spec 34 MEM-09). Each later activation and
    /// staging is still charged as before (TIME-08); only the allocation goes.
    pub(super) fn reserve_execution_scratch(&mut self) -> Result<(), RuntimeError> {
        let plan = ScratchPlan::of(self.prepared)?;
        self.charge_allocation_bytes(plan.bytes().ok_or(RuntimeError::Overflow)?)?;
        let refused = |_| VmTrap::BudgetExceeded.into_runtime_error();
        let mut buffers = ExecutionBuffers::default();
        buffers
            .reserve(plan.operands, plan.levels)
            .map_err(VmTrap::into_runtime_error)?;
        // One frame payload per call depth, so a stack activation takes one instead of
        // allocating (spec 34 MEM-03; census A3, A4).
        buffers
            .frames
            .reserve_frame_payloads(plan.levels, plan.slots)?;
        let resources = &mut self.data.resources;
        resources.buffers.try_reserve_exact(1).map_err(refused)?;
        resources.buffers.push(buffers);
        resources.frame_locals.reserve(plan.levels, plan.slots)?;
        // One edge-input list and one restore list, reused by every edge-affected call
        // (spec 34 MEM-09; census A5).
        resources
            .edge_inputs
            .try_reserve_exact(plan.edges)
            .map_err(refused)?;
        resources
            .edge_restores
            .try_reserve_exact(plan.edges)
            .map_err(refused)?;
        resources.spare_journal.reserve_storage(plan.journal)?;
        resources.staging.reserve(plan.staging)?;
        let construction = &mut self.data.construction;
        construction
            .initializers
            .try_reserve_exact(plan.initializers)
            .map_err(refused)?;
        construction
            .frames
            .try_reserve_exact(plan.initializers)
            .map_err(refused)?;
        construction
            .types
            .try_reserve_exact(plan.types)
            .map_err(refused)
    }
}

#[cfg(test)]
#[path = "scratch/tests.rs"]
mod tests;
