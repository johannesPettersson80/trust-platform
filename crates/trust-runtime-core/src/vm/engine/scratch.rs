//! Execution scratch that construction reserves and every scan reuses (spec 34 MEM-03,
//! MEM-09): the stack executor's execution buffer, the local slots of its activations, the
//! initializer stacks, the output copy-back journal and the process-image staging.
use super::io::staging::StagingDemand;
use super::output_transaction::SavedDestinations;
use super::*;
use crate::vm::construction::values::ValueOperation;
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
        if self.slots.len() < self.levels {
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
    pub(super) fn return_visited(&mut self, visited: InstanceSet) {
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
        Ok(Self {
            levels,
            operands,
            slots,
            initializers: levels,
            types: crate::bytecode::BYTECODE_MAX_TYPE_NESTING + 1,
            journal,
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
        let initializers = self.initializers.checked_mul(
            core::mem::size_of::<ActiveInitializer>() + core::mem::size_of::<VmFrame>(),
        )?;
        let types = self
            .types
            .checked_mul(core::mem::size_of::<(u32, ValueOperation)>())?;
        let journal = SavedDestinations::allocation_bytes(self.journal).ok()?;
        let bookkeeping = BookkeepingScratch::reservation_bytes(self.initializers)?;
        buffer
            .checked_add(frame_locals)?
            .checked_add(initializers)?
            .checked_add(types)?
            .checked_add(journal)?
            .checked_add(bookkeeping)?
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
        let resources = &mut self.data.resources;
        resources.buffers.try_reserve_exact(1).map_err(refused)?;
        resources.buffers.push(buffers);
        resources.frame_locals.reserve(plan.levels, plan.slots)?;
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
            .map_err(refused)?;
        self.data.resources.bookkeeping.reserve(plan.initializers)
    }
}

#[cfg(test)]
#[path = "scratch/tests.rs"]
mod tests;
