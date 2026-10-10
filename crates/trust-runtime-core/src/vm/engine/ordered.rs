//! Charged sorted pairs for the engine's small live identity collections.
use super::{EngineState, RuntimeError, Vec};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Entries<K, V>(Vec<(K, V)>);
impl<K, V> Default for Entries<K, V> {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl<K: Ord, V> Entries<K, V> {
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn reservation_demand(
        &self,
        capacity: usize,
    ) -> Result<(usize, usize), RuntimeError> {
        if capacity <= self.0.capacity() {
            return Ok((0, 0));
        }
        Ok((
            capacity
                .checked_mul(core::mem::size_of::<(K, V)>())
                .ok_or(RuntimeError::Overflow)?,
            1,
        ))
    }
    pub(super) fn reserve_capacity(&mut self, capacity: usize) -> Result<(), RuntimeError> {
        if capacity > self.0.capacity() {
            self.0
                .try_reserve_exact(capacity - self.0.len())
                .map_err(|_| RuntimeError::Overflow)?;
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.0.capacity()
    }
    pub(super) fn get(&self, key: &K) -> Option<&V> {
        self.0
            .binary_search_by(|(candidate, _)| candidate.cmp(key))
            .ok()
            .map(|index| &self.0[index].1)
    }
    pub(super) fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let index = self
            .0
            .binary_search_by(|(candidate, _)| candidate.cmp(key))
            .ok()?;
        Some(&mut self.0[index].1)
    }
    pub(super) fn insertion_demand(&self, key: &K) -> Result<(usize, usize), RuntimeError> {
        // Demand, optional preparation, lookup and insertion may each search.
        // Four logarithmic traversals bound every wrapper, including owner lists.
        let search = 4 * (self.0.len().max(1).ilog2() as usize + 1);
        let Err(index) = self.0.binary_search_by(|(candidate, _)| candidate.cmp(key)) else {
            return Ok((0, search));
        };
        let (bytes, moved) = growth_demand::<(K, V)>(&self.0)?;
        let work = search
            .checked_add(self.0.len() - index)
            .and_then(|n| n.checked_add(moved))
            .ok_or(RuntimeError::Overflow)?;
        Ok((bytes, work))
    }
    pub(super) fn prepare_insert(&mut self, key: &K) -> Result<(), RuntimeError> {
        if self.get(key).is_none() {
            reserve_growth(&mut self.0)?;
        }
        Ok(())
    }
    pub(super) fn insert(&mut self, key: K, value: V) -> Result<Option<V>, RuntimeError> {
        match self
            .0
            .binary_search_by(|(candidate, _)| candidate.cmp(&key))
        {
            Ok(index) => Ok(Some(core::mem::replace(&mut self.0[index].1, value))),
            Err(index) => {
                reserve_growth(&mut self.0)?;
                self.0.insert(index, (key, value));
                Ok(None)
            }
        }
    }
    pub(super) fn remove(&mut self, key: &K) -> Option<V> {
        let index = self
            .0
            .binary_search_by(|(candidate, _)| candidate.cmp(key))
            .ok()?;
        Some(self.0.remove(index).1)
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.iter().map(|(key, value)| (key, value))
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(&K, &V) -> bool) {
        self.0.retain(|(key, value)| keep(key, value));
    }
    pub(super) fn into_entries(self) -> alloc::vec::IntoIter<(K, V)> {
        self.0.into_iter()
    }
}

pub(super) fn growth_demand<T>(entries: &Vec<T>) -> Result<(usize, usize), RuntimeError> {
    if entries.len() < entries.capacity() {
        return Ok((0, 0));
    }
    let capacity = entries
        .capacity()
        .checked_mul(2)
        .ok_or(RuntimeError::Overflow)?
        .max(4);
    Ok((
        capacity
            .checked_mul(core::mem::size_of::<T>())
            .ok_or(RuntimeError::Overflow)?,
        entries.len(),
    ))
}
pub(super) fn reserve_growth<T>(entries: &mut Vec<T>) -> Result<(), RuntimeError> {
    if entries.len() == entries.capacity() {
        let capacity = entries
            .capacity()
            .checked_mul(2)
            .ok_or(RuntimeError::Overflow)?
            .max(4);
        entries
            .try_reserve_exact(capacity - entries.len())
            .map_err(|_| RuntimeError::Overflow)?;
    }
    Ok(())
}
impl EngineState<'_> {
    pub(super) fn charge_sorted_growth(
        &self,
        demand: Result<(usize, usize), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        let (bytes, work) = demand?;
        self.charge_work_units(work)?;
        self.charge_allocation_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;

    #[test]
    fn compact_pairs_preserve_map_order_replacements_and_middle_removal() {
        let mut compact = Entries::default();
        let mut baseline = BTreeMap::new();
        for key in [u32::MAX, 0, 7, 1, u32::MAX - 1, 7, 0] {
            let value = key.wrapping_add(10);
            assert_eq!(
                compact.insert(key, value).unwrap(),
                baseline.insert(key, value)
            );
            assert_eq!(
                compact.iter().collect::<Vec<_>>(),
                baseline.iter().collect::<Vec<_>>()
            );
        }
        assert_eq!(compact.remove(&7), baseline.remove(&7));
        compact.retain(|key, _| *key != 0);
        baseline.retain(|key, _| *key != 0);
        assert_eq!(
            compact.into_entries().collect::<Vec<_>>(),
            baseline.into_iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn demand_covers_each_growth_search_and_shift_without_allocating_on_replacement() {
        let mut entries = Entries::default();
        let slot = core::mem::size_of::<(u32, u32)>();
        assert_eq!(entries.insertion_demand(&3), Ok((4 * slot, 4)));
        for key in [3, 5, 7, 9] {
            entries.insert(key, key).unwrap();
        }
        let (bytes, work) = entries.insertion_demand(&1).unwrap();
        assert_eq!(bytes, 8 * slot);
        assert_eq!(
            work,
            4 * 3 + 4 + 4,
            "four searches plus relocation and front shift"
        );
        entries.prepare_insert(&1).unwrap();
        let capacity = entries.0.capacity();
        entries.insert(1, 10).unwrap();
        assert_eq!(entries.0.capacity(), capacity);
        assert_eq!(entries.insertion_demand(&1), Ok((0, 4 * 3)));
        assert_eq!(entries.insert(1, 11).unwrap(), Some(10));
        assert_eq!(entries.0.capacity(), capacity);
        entries.retain(|_, _| false);
        assert_eq!(entries.insertion_demand(&u32::MAX), Ok((0, 4)));
    }

    #[test]
    fn failed_metadata_charge_does_not_strand_a_new_instance_or_publish_a_mark() {
        let prepared = super::super::PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state =
            EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        let before = state.storage.instances().len();
        let templates = state.construction.instance_templates.len();
        let lifetimes = state.lifetimes.instance_lifetimes.len();
        let owner = state.storage.reserve_execution_frame().unwrap();
        state.lifetimes.live_activations.push(owner);
        let pou = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
        // Stop before templates, before lifetimes, and before owner registration.
        // The latter two exercise cleanup after earlier metadata was inserted.
        for completed in 0..3 {
            let moved = state.storage.instance_insertion_demand().unwrap().1;
            let future = crate::memory::InstanceId(u32::MAX);
            let mut fuel = 1 + moved;
            if completed >= 1 {
                fuel += state
                    .construction
                    .instance_templates
                    .insertion_demand(future)
                    .unwrap()
                    .1;
            }
            if completed >= 2 {
                fuel += state
                    .lifetimes
                    .instance_lifetimes
                    .insertion_demand(future)
                    .unwrap()
                    .1;
            }
            state.resources.work_budget.reset(fuel);
            let error = state.reserve_instance(pou, Some(owner)).unwrap_err();
            assert_eq!(
                error.stable_code(),
                crate::error::StableErrorCode::RuntimeExecutionTimeout
            );
            assert_eq!(state.storage.instances().len(), before);
            assert_eq!(state.construction.instance_templates.len(), templates);
            assert_eq!(state.lifetimes.instance_lifetimes.len(), lifetimes);
            assert_eq!(state.lifetimes.owned_instances.count(owner), 0);
        }
        let mut marks = super::super::LifecycleMarks::default();
        let key = (0, Some(crate::memory::InstanceId(u32::MAX)));
        state.resources.work_budget.reset(0);
        assert!(state
            .charge_sorted_growth(marks.insertion_demand(key))
            .is_err());
        assert!(!marks.contains(&key).unwrap());
        // Retiring a failed candidate needs no fuel and leaves the old state intact.
        marks.retain(|_| false);
        assert_eq!(marks.len(), 0);
        assert_eq!(state.storage.instances().len(), before);
    }
}
