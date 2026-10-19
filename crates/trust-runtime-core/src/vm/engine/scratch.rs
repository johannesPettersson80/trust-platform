//! Execution scratch that construction reserves and every scan reuses (spec 34 MEM-03,
//! MEM-09): the stack executor's execution buffer, the local slots of its activations, the
//! initializer stacks, the output copy-back journal and the process-image staging.
use super::io::staging::StagingDemand;
use super::output_transaction::SavedDestinations;
use super::*;
use crate::value::{RefIndices, RefPath, RefSegment};
use crate::vm::construction::values::ValueOperation;
use crate::vm::module::VmNativeSymbolSpec;
use crate::vm::VmTrap;

/// Initializer, construction-frame and lifetime-check bookkeeping that construction reserves and
/// every scan reuses (spec 34 MEM-03, MEM-09, census N1-N4 and I10): one staged result slot, one
/// pending-identity list, one seed visited set, one seed backup list and one owned-instance
/// snapshot per callback level, plus the lifetime check's visited set. Every buffer is taken from
/// its list and returned, so no evaluation or check allocates it again; growth is still charged at
/// the call site against the capacity a buffer growing on demand would have (spec 34 TIME-08).
#[derive(Default)]
pub(super) struct BookkeepingScratch {
    slots: Vec<Vec<Value>>,
    pending: Vec<Vec<u32>>,
    visited: Vec<InstanceSet>,
    backups: Vec<Vec<(InstanceId, crate::memory::InstanceData)>>,
    snapshots: Vec<Vec<InstanceId>>,
    /// The lifetime check's visited set, reserved for the live instance count and cleared per
    /// check (spec 34 MEM-09, census I10).
    pub(super) lifetime_visited: core::cell::RefCell<super::policy::LifetimeVisited>,
    /// The number of call levels reserved. A buffer returned above it is dropped.
    levels: usize,
}

impl BookkeepingScratch {
    /// The bytes this scratch reserves for `levels` callback levels.
    fn reservation_bytes(levels: usize) -> Option<usize> {
        let per_level = core::mem::size_of::<Vec<Value>>()
            .checked_add(core::mem::size_of::<Value>())?
            .checked_add(core::mem::size_of::<Vec<u32>>())?
            .checked_add(core::mem::size_of::<InstanceSet>())?
            .checked_add(core::mem::size_of::<
                Vec<(InstanceId, crate::memory::InstanceData)>,
            >())?
            .checked_add(core::mem::size_of::<Vec<InstanceId>>())?;
        levels
            .checked_mul(per_level)?
            .checked_add(core::mem::size_of::<Self>())
    }

    /// Reserve the per-level lists and the fixed one-element result slots (spec 34 MEM-09).
    fn reserve(&mut self, levels: usize) -> Result<(), RuntimeError> {
        let refused = |_| VmTrap::BudgetExceeded.into_runtime_error();
        self.levels = levels;
        self.slots.try_reserve_exact(levels).map_err(refused)?;
        self.pending.try_reserve_exact(levels).map_err(refused)?;
        self.visited.try_reserve_exact(levels).map_err(refused)?;
        self.backups.try_reserve_exact(levels).map_err(refused)?;
        self.snapshots.try_reserve_exact(levels).map_err(refused)?;
        for _ in 0..levels {
            let mut slot = Vec::new();
            slot.try_reserve_exact(1).map_err(refused)?;
            self.slots.push(slot);
            self.pending.push(Vec::new());
            self.visited.push(InstanceSet::default());
            self.backups.push(Vec::new());
            self.snapshots.push(Vec::new());
        }
        Ok(())
    }

    /// Reserve the lifetime check's visited set for `instances` live identities. Called when the
    /// live instance count grows, never during a scan of an unchanged program (spec 34 MEM-09).
    pub(super) fn reserve_lifetime_visited(&self, instances: usize) -> Result<(), RuntimeError> {
        self.lifetime_visited.borrow_mut().reserve(instances)
    }

    pub(super) fn take_slot(&mut self) -> Vec<Value> {
        self.slots.pop().unwrap_or_default()
    }
    pub(super) fn return_slot(&mut self, mut slot: Vec<Value>) {
        slot.clear();
        // A slot whose buffer a suspended evaluation took has no room to keep; the buffer comes
        // back when the evaluation finishes (C1-13).
        if slot.capacity() > 0 && self.slots.len() < self.levels {
            self.slots.push(slot);
        }
    }
    pub(super) fn take_pending(&mut self) -> Vec<u32> {
        self.pending.pop().unwrap_or_default()
    }
    pub(super) fn return_pending(&mut self, mut pending: Vec<u32>) {
        pending.clear();
        if self.pending.len() < self.levels {
            self.pending.push(pending);
        }
    }
    pub(super) fn take_visited(&mut self) -> InstanceSet {
        self.visited.pop().unwrap_or_default()
    }
    pub(super) fn return_visited(&mut self, mut visited: InstanceSet) {
        // A set comes back empty, so that no later initializer may write the instances of the
        // one that used it before, nor skip their backups.
        visited.clear();
        if self.visited.len() < self.levels {
            self.visited.push(visited);
        }
    }
    pub(super) fn take_backups(&mut self) -> Vec<(InstanceId, crate::memory::InstanceData)> {
        self.backups.pop().unwrap_or_default()
    }
    pub(super) fn return_backups(
        &mut self,
        mut backups: Vec<(InstanceId, crate::memory::InstanceData)>,
    ) {
        backups.clear();
        if self.backups.len() < self.levels {
            self.backups.push(backups);
        }
    }
    pub(super) fn take_snapshot(&mut self) -> Vec<InstanceId> {
        self.snapshots.pop().unwrap_or_default()
    }
    pub(super) fn return_snapshot(&mut self, mut snapshot: Vec<InstanceId>) {
        snapshot.clear();
        if self.snapshots.len() < self.levels {
            self.snapshots.push(snapshot);
        }
    }

    /// Whether every reserved buffer has been returned to its list (spec 34 MEM-09; test support).
    #[cfg(test)]
    pub(super) fn all_buffers_returned(&self) -> bool {
        self.slots.len() == self.levels
            && self.pending.len() == self.levels
            && self.visited.len() == self.levels
            && self.backups.len() == self.levels
            && self.snapshots.len() == self.levels
    }
}

/// Local-slot vectors for stack activations. Each has room for the largest POU's locals, so
/// handing one out never allocates. Each activation's parameter-presence bits come from the same
/// store, so a call with presence bits allocates none either (census A4).
#[derive(Debug, Default)]
pub(super) struct FrameLocals {
    spares: Vec<Vec<Value>>,
    presence: Vec<Vec<bool>>,
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

    /// Parameter-presence bits for a new activation, none set; the activation writes its own.
    pub(super) fn take_presence(&mut self) -> Vec<bool> {
        self.presence
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(self.slots))
    }

    /// Keep a finished activation's presence bits for the next activation, emptied, as
    /// [`FrameLocals::recycle`] keeps its locals.
    pub(super) fn recycle_presence(&mut self, mut present: Vec<bool>) {
        if present.capacity() >= self.slots && self.presence.len() < self.presence.capacity() {
            present.clear();
            self.presence.push(present);
        }
    }

    /// Room for `levels` vectors of `slots` locals and as many presence vectors, and the program
    /// activation's vector of each.
    fn reserve(&mut self, levels: usize, slots: usize) -> Result<(), RuntimeError> {
        let refused = |_| VmTrap::BudgetExceeded.into_runtime_error();
        self.slots = slots;
        self.spares.try_reserve_exact(levels).map_err(refused)?;
        self.presence.try_reserve_exact(levels).map_err(refused)?;
        let mut locals = Vec::new();
        locals.try_reserve_exact(slots).map_err(refused)?;
        self.spares.push(locals);
        let mut present = Vec::new();
        present.try_reserve_exact(slots).map_err(refused)?;
        self.presence.push(present);
        Ok(())
    }

    /// The bytes [`FrameLocals::reserve`] takes for `levels` levels of `slots` slots.
    fn reservation_bytes(levels: usize, slots: usize) -> Option<usize> {
        let locals = levels
            .checked_mul(core::mem::size_of::<Vec<Value>>())?
            .checked_add(slots.checked_mul(core::mem::size_of::<Value>())?)?;
        let presence = levels
            .checked_mul(core::mem::size_of::<Vec<bool>>())?
            .checked_add(slots.checked_mul(core::mem::size_of::<bool>())?)?;
        locals.checked_add(presence)
    }
}

impl EngineState<'_> {
    /// Give back the path buffers of the references a finished activation's `locals` hold, before
    /// the locals are kept for the next activation (spec 34 ARCH-05): a call-time reference
    /// initializer then takes the same buffers on every call (C1-13).
    pub(super) fn recycle_local_references(&self, locals: &mut [Value]) {
        for local in locals {
            if let Value::Reference(Some(_)) = local
                && let Value::Reference(Some(reference)) = core::mem::replace(local, Value::Null)
            {
                self.data.resources.references.recycle(reference.path);
            }
        }
    }
}

/// The path and index buffers of the references a scan builds (spec 34 ARCH-05). A reference
/// whose path grows takes a kept buffer, and one the execution has used gives its buffers back,
/// so after the first scan a program's references allocate nothing. The charges stay those of an
/// owned path (TIME-08): a buffer's capacity changes no charge. The references borrow the engine
/// immutably while they grow, so the kept buffers are behind a `RefCell`.
#[derive(Debug, Default)]
pub(super) struct ReferenceBuffers {
    paths: core::cell::RefCell<Vec<RefPath>>,
    indices: core::cell::RefCell<Vec<RefIndices>>,
}

/// The most buffers of each kind kept; the references a program holds beyond them allocate.
const KEPT_REFERENCE_BUFFERS: usize = 64;

/// The room of a new path buffer and of a new index buffer: the segments and the dimensions of
/// the deepest paths and arrays programs commonly build. A buffer that has to grow is kept grown.
const REFERENCE_BUFFER_ROOM: usize = 4;

impl ReferenceBuffers {
    /// An empty path buffer: a kept one, else a new one.
    pub(super) fn take_path(&self) -> RefPath {
        self.paths
            .borrow_mut()
            .pop()
            .unwrap_or_else(|| RefPath::with_capacity(REFERENCE_BUFFER_ROOM))
    }

    /// An empty index buffer: a kept one, else a new one.
    pub(super) fn take_indices(&self) -> RefIndices {
        self.indices
            .borrow_mut()
            .pop()
            .unwrap_or_else(|| RefIndices::with_capacity(REFERENCE_BUFFER_ROOM))
    }

    /// Keep the buffers of a path the execution has used: the path's and each index segment's.
    pub(super) fn recycle(&self, mut path: RefPath) {
        for segment in path.drain(..) {
            if let RefSegment::Index(mut indices) = segment {
                indices.clear();
                keep(&self.indices, indices);
            }
        }
        keep(&self.paths, path);
    }
}

/// Keep an emptied buffer with room in it, unless [`KEPT_REFERENCE_BUFFERS`] are kept already.
fn keep<T>(spares: &core::cell::RefCell<Vec<Vec<T>>>, buffer: Vec<T>) {
    let mut spares = spares.borrow_mut();
    if buffer.capacity() > 0 && spares.len() < KEPT_REFERENCE_BUFFERS {
        spares.push(buffer);
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
    /// Reusable per-depth call-binding scratch (spec 34 MEM-09; census C1, C3, C4, C5, C7).
    call_scratch: crate::vm::call::scratch::CallScratchSizes,
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
        // The most actual arguments of any call site sizes the argument and flag vectors
        // (spec 34 MEM-09; census C1, C3).
        let call_args = prepared
            .vm
            .native_symbol_specs
            .iter()
            .map(|spec| match spec {
                VmNativeSymbolSpec::Parsed { arg_specs, .. } => arg_specs.len(),
                VmNativeSymbolSpec::ParseError(_) => 0,
            })
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
            call_scratch: crate::vm::call::scratch::CallScratchSizes {
                args: call_args,
                params: journal,
                slots,
            },
            edges,
            staging: StagingDemand::of(&prepared.io_bindings).ok_or(RuntimeError::Overflow)?,
        })
    }

    /// Every byte the plan reserves.
    fn bytes(&self) -> Option<usize> {
        let buffer = ExecutionBuffers::reservation_bytes(self.operands, self.levels)?
            .checked_add(core::mem::size_of::<ExecutionBuffers>())?;
        let frame_locals = FrameLocals::reservation_bytes(self.levels, self.slots)?;
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
        let bookkeeping = BookkeepingScratch::reservation_bytes(self.initializers)?;
        let call_scratch = crate::vm::call::scratch::CallScratch::reservation_bytes(
            self.levels,
            self.call_scratch,
        )?;
        buffer
            .checked_add(frame_locals)?
            .checked_add(frame_payloads)?
            .checked_add(edges)?
            .checked_add(initializers)?
            .checked_add(types)?
            .checked_add(journal)?
            .checked_add(bookkeeping)?
            .checked_add(call_scratch)?
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
        // One reusable call-binding vector of each kind per call depth, so a call takes one
        // instead of allocating (spec 34 MEM-09; census C1, C3, C4, C5, C7).
        resources
            .call_scratch
            .reserve(plan.levels, plan.call_scratch)?;
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
            .map_err(refused)?;
        self.data.resources.bookkeeping.reserve(plan.initializers)
    }
}

#[cfg(test)]
#[path = "scratch/tests.rs"]
mod tests;
