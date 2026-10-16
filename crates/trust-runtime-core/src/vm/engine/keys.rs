//! Compact internal keys preserve admitted declaration and full instance identity.
use super::*;

// Source-free admission enforces this cap before EngineState exists. Checking at
// insertion also protects internal callers; growing the format cap must revisit
// this representation rather than silently truncate an identity.
const _: () = assert!(crate::bytecode::BYTECODE_MAX_CONSTRUCTION_RECORDS <= 65_536);
const OWNER_MASK: u64 = (1u64 << 33) - 1;

fn encode_mark(declaration: u32, owner: Option<InstanceId>) -> Result<u64, RuntimeError> {
    let declaration =
        u16::try_from(declaration).map_err(|_| RuntimeError::InvalidExecutionState)?;
    let owner = owner.map_or(0, |owner| u64::from(owner.0) + 1);
    Ok((u64::from(declaration) << 33) | owner)
}

fn decode_mark(key: u64) -> (u32, Option<InstanceId>) {
    let owner = key & OWNER_MASK;
    (
        (key >> 33) as u32,
        (owner != 0).then(|| InstanceId((owner - 1) as u32)),
    )
}

/// A checked marker key and the allocation its insertion may need.
#[derive(Clone, Copy)]
pub(super) struct MarkerInsertPlan {
    key: u64,
    demand: (usize, usize),
}

impl MarkerInsertPlan {
    pub(super) fn demand(self) -> (usize, usize) {
        self.demand
    }
}

/// Exclusive prepared insertion. Capacity is reserved and the exclusive borrow
/// prevents intervening changes, so publication cannot allocate or fail.
pub(super) struct PreparedMarkerInsert<'a> {
    marks: &'a mut LifecycleMarks,
    key: u64,
}

/// Initialized-declaration and once markers in ascending `(declaration, owner)` order.
#[derive(Default)]
pub(super) struct LifecycleMarks(super::ordered::Entries<u64, ()>);

impl LifecycleMarks {
    #[cfg(test)]
    pub(super) fn insertion_demand(
        &self,
        key: (u32, Option<InstanceId>),
    ) -> Result<(usize, usize), RuntimeError> {
        Ok(self.insertion_plan(key)?.demand())
    }

    pub(super) fn insertion_plan(
        &self,
        key: (u32, Option<InstanceId>),
    ) -> Result<MarkerInsertPlan, RuntimeError> {
        let key = encode_mark(key.0, key.1)?;
        Ok(MarkerInsertPlan {
            key,
            demand: self.0.insertion_demand(&key)?,
        })
    }

    /// Reserve storage for the planned marker before any destination write.
    pub(super) fn prepare_insert(
        &mut self,
        plan: MarkerInsertPlan,
    ) -> Result<PreparedMarkerInsert<'_>, RuntimeError> {
        self.0.prepare_insert(&plan.key)?;
        Ok(PreparedMarkerInsert {
            marks: self,
            key: plan.key,
        })
    }

    #[cfg(test)]
    pub(super) fn insert(&mut self, key: (u32, Option<InstanceId>)) -> Result<bool, RuntimeError> {
        let inserted = !self.contains(&key)?;
        let plan = self.insertion_plan(key)?;
        self.prepare_insert(plan)?.publish();
        Ok(inserted)
    }

    pub(super) fn contains(&self, key: &(u32, Option<InstanceId>)) -> Result<bool, RuntimeError> {
        Ok(self.0.get(&encode_mark(key.0, key.1)?).is_some())
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    #[cfg(test)]
    pub(super) fn capacity_state_for_test(&self) -> usize {
        self.0.capacity()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (u32, Option<InstanceId>)> + '_ {
        self.0.iter().map(|(key, _)| decode_mark(*key))
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(&(u32, Option<InstanceId>)) -> bool) {
        self.0.retain(|key, _| keep(&decode_mark(*key)));
    }
}

impl PreparedMarkerInsert<'_> {
    /// Publish after all fallible admission and destination writes have succeeded.
    pub(super) fn publish(self) {
        let inserted = self.marks.0.insert(self.key, ());
        debug_assert!(inserted.is_ok(), "prepared marker capacity was reserved");
    }
}

pub(super) fn root_index(index: usize) -> Result<u32, RuntimeError> {
    u32::try_from(index).map_err(|_| RuntimeError::InvalidExecutionState)
}

#[cfg(test)]
#[path = "keys/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "keys/call_transfer_marker_tests.rs"]
mod call_transfer_marker_tests;
