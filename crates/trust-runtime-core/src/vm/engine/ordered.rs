//! Sorted pairs for the engine's small live identity collections; growth is charged as bytes.
use super::{EngineState, RuntimeError, Vec};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Entries<K, V>(Vec<(K, V)>);

impl<K, V> Default for Entries<K, V> {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl<K: Ord, V> Entries<K, V> {
    #[cfg(test)]
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
                .ok_or_else(|| RuntimeError::Overflow)?,
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
        // Four searches bound the common insertion path. The work component is
        // bookkeeping and is never charged (TIME-08); allocation bytes are.
        let count = self.0.len();
        let comparisons = if count <= 1 {
            1
        } else {
            (count - 1).ilog2() as usize + 2
        };
        let search = 4 * comparisons;
        let index = match self.0.binary_search_by(|(candidate, _)| candidate.cmp(key)) {
            Err(index) => index,
            Ok(_) => return Ok((0, search)),
        };
        let (bytes, moved) = growth_demand::<(K, V)>(&self.0)?;
        let Some(work) = search
            .checked_add(self.0.len() - index)
            .and_then(|n| n.checked_add(moved))
        else {
            return Err(RuntimeError::Overflow);
        };
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
        .ok_or_else(|| RuntimeError::Overflow)?
        .max(4);
    Ok((
        capacity
            .checked_mul(core::mem::size_of::<T>())
            .ok_or_else(|| RuntimeError::Overflow)?,
        entries.len(),
    ))
}
pub(super) fn reserve_growth<T>(entries: &mut Vec<T>) -> Result<(), RuntimeError> {
    if entries.len() == entries.capacity() {
        let capacity = entries
            .capacity()
            .checked_mul(2)
            .ok_or_else(|| RuntimeError::Overflow)?
            .max(4);
        entries
            .try_reserve_exact(capacity - entries.len())
            .map_err(|_| RuntimeError::Overflow)?;
    }
    Ok(())
}
impl EngineState<'_> {
    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    pub(super) fn charge_sorted_growth(
        &self,
        demand: Result<(usize, usize), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        // The moved-entry component is bookkeeping (TIME-08) and is never charged.
        let (bytes, _work) = demand?;
        self.charge_allocation_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::StorageOwner;
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
    fn prepared_insertion_publishes_at_the_sorted_slot_without_growth() {
        let mut entries = Entries::default();
        entries.insert(2u32, 20u32).unwrap();
        entries.insert(6, 60).unwrap();
        entries.prepare_insert(&4).unwrap();
        let capacity = entries.capacity();
        assert!(capacity > entries.len());
        assert_eq!(entries.insert(4, 40), Ok(None));
        assert_eq!(entries.capacity(), capacity);
        assert_eq!(
            entries.iter().collect::<Vec<_>>(),
            alloc::vec![(&2, &20), (&4, &40), (&6, &60)]
        );
        // An existing key reserves nothing and is replaced in place.
        entries.prepare_insert(&6).unwrap();
        assert_eq!(entries.insert(6, 61), Ok(Some(60)));
    }

    #[test]
    fn demand_allocates_only_for_growth_and_not_for_replacement() {
        let mut entries = Entries::default();
        let slot = core::mem::size_of::<(u32, u32)>();
        assert_eq!(
            entries.insertion_demand(&3).map(|demand| demand.0),
            Ok(4 * slot)
        );
        for key in [3, 5, 7, 9] {
            entries.insert(key, key).unwrap();
        }
        let (bytes, _) = entries.insertion_demand(&1).unwrap();
        assert_eq!(bytes, 8 * slot);
        entries.prepare_insert(&1).unwrap();
        let capacity = entries.0.capacity();
        entries.insert(1, 10).unwrap();
        assert_eq!(entries.0.capacity(), capacity);
        // An existing key replaces its value without allocating.
        assert_eq!(entries.insertion_demand(&1).map(|demand| demand.0), Ok(0));
        assert_eq!(entries.insert(1, 11).unwrap(), Some(10));
        assert_eq!(entries.0.capacity(), capacity);
        entries.retain(|_, _| false);
        assert_eq!(
            entries.insertion_demand(&u32::MAX).map(|demand| demand.0),
            Ok(0)
        );
    }

    #[test]
    fn insertion_demand_allocates_nothing_for_replacement_or_spare_capacity() {
        for count in [3usize, 5] {
            let mut entries = Entries::default();
            for key in 1..=count as u32 {
                entries.insert(key * 2, key).unwrap();
            }
            let (bytes, _) = entries.insertion_demand(&(count as u32 * 2)).unwrap();
            assert_eq!(bytes, 0, "count={count}");

            let capacity = entries.0.capacity();
            assert!(capacity > count, "fixture needs spare capacity");
            let (bytes, _) = entries.insertion_demand(&0).unwrap();
            assert_eq!(bytes, 0, "count={count}");
        }
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
        let mut state_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        let before = state.data.storage.instances().len();
        let templates = state.data.construction.instance_templates.len();
        let lifetimes = state.data.lifetimes.instance_lifetimes.len();
        let owner = state.data.storage.reserve_execution_frame().unwrap();
        state.data.storage.activate_execution_frame(owner).unwrap();
        let pou = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
        // The entry node and the first declaration preflight are the work boundary.
        // TIME-08 VERIFY: both remain CONSTRUCT units charged by reserve_instance.
        state.data.resources.work_budget.reset(1);
        assert!(matches!(
            state.reserve_instance(pou, Some(owner)),
            Err(RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::Work,
                ..
            })
        ));
        assert_eq!(state.data.storage.instances().len(), before);
        assert_eq!(state.data.construction.instance_templates.len(), templates);
        assert_eq!(state.data.lifetimes.instance_lifetimes.len(), lifetimes);
        assert_eq!(state.data.lifetimes.owned_instances.count(owner), 0);
        // Metadata growth is bookkeeping under TIME-08 and is charged only as allocation
        // bytes. Each stage admits the bytes of the steps before it and refuses the next
        // growth, which must leave no instance, template, lifetime or owner entry behind.
        let limit = prepared.limits.max_construction_bytes;
        state
            .data
            .resources
            .work_budget
            .reset(prepared.limits.max_work);
        let mut preflight = state.data.storage.instance_insertion_demand().unwrap().0;
        for &id in prepared.declarations(StorageOwner::Instance, Some(pou)) {
            if let Some(value) = prepared.layout.entries[id as usize]
                .default_const_idx
                .and_then(|index| prepared.vm.consts.get(index as usize))
            {
                preflight += state.value_clone_charge(value, 0).unwrap();
            }
        }
        let future = crate::memory::InstanceId(u32::MAX);
        let template_bytes = state
            .data
            .construction
            .instance_templates
            .insertion_demand(future)
            .unwrap()
            .0;
        let lifetime_bytes = state
            .data
            .lifetimes
            .instance_lifetimes
            .insertion_demand(future)
            .unwrap()
            .0;
        // Stop before templates, before lifetimes, and before owner registration.
        // The latter two exercise cleanup after earlier metadata was inserted.
        for (completed, allowed) in [
            (0, preflight),
            (1, preflight + template_bytes),
            (2, preflight + template_bytes + lifetime_bytes),
        ] {
            state.data.resources.construction_bytes.set(limit - allowed);
            let error = state.reserve_instance(pou, Some(owner)).unwrap_err();
            assert!(
                matches!(
                    error,
                    RuntimeError::ExecutionLimit {
                        kind: crate::error::ExecutionLimitKind::AllocationBytes,
                        ..
                    }
                ),
                "completed={completed}: {error:?}"
            );
            assert_eq!(state.data.storage.instances().len(), before);
            assert_eq!(state.data.construction.instance_templates.len(), templates);
            assert_eq!(state.data.lifetimes.instance_lifetimes.len(), lifetimes);
            assert_eq!(state.data.lifetimes.owned_instances.count(owner), 0);
        }
        let mut marks = super::super::LifecycleMarks::default();
        let key = (0, Some(crate::memory::InstanceId(u32::MAX)));
        // An exhausted allocation allowance refuses the sorted growth for a new mark.
        state.data.resources.construction_bytes.set(limit);
        assert!(state
            .charge_sorted_growth(marks.insertion_demand(key))
            .is_err());
        assert!(!marks.contains(&key).unwrap());
        // Retiring a failed candidate needs no fuel and leaves the old state intact.
        marks.retain(|_| false);
        assert_eq!(marks.len(), 0);
        assert_eq!(state.data.storage.instances().len(), before);
    }
}

#[cfg(test)]
#[path = "ordered/work_allowance_tests.rs"]
mod work_allowance_tests;
