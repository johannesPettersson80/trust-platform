//! Typed identity boundaries over compact sorted live entries.
use super::ordered::{growth_demand, reserve_growth, Entries};
use super::{FrameId, InstanceId, RuntimeError, Vec};

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct InstanceSet(Entries<u32, ()>);
impl InstanceSet {
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn insertion_demand(&self, id: InstanceId) -> Result<(usize, usize), RuntimeError> {
        self.0.insertion_demand(&id.0)
    }
    pub(super) fn insert(&mut self, id: InstanceId) -> Result<bool, RuntimeError> {
        Ok(self.0.insert(id.0, ())?.is_none())
    }
    pub(super) fn contains(&self, id: &InstanceId) -> bool {
        self.0.get(&id.0).is_some()
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(InstanceId) -> bool) {
        self.0.retain(|id, _| keep(InstanceId(*id)));
    }
    #[cfg(test)]
    pub(super) fn remove(&mut self, id: &InstanceId) -> bool {
        self.0.remove(&id.0).is_some()
    }
    #[cfg(test)]
    pub(super) fn iter(&self) -> impl Iterator<Item = InstanceId> + '_ {
        self.0.iter().map(|(id, _)| InstanceId(*id))
    }
}
impl IntoIterator for InstanceSet {
    type Item = InstanceId;
    type IntoIter = core::iter::Map<alloc::vec::IntoIter<(u32, ())>, fn((u32, ())) -> InstanceId>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_entries().map(|(id, _)| InstanceId(id))
    }
}

#[derive(Default)]
pub(super) struct InstanceTemplates(Entries<u32, u32>);
impl InstanceTemplates {
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn get(&self, id: &InstanceId) -> Option<&u32> {
        self.0.get(&id.0)
    }
    pub(super) fn insertion_demand(&self, id: InstanceId) -> Result<(usize, usize), RuntimeError> {
        self.0.insertion_demand(&id.0)
    }
    pub(super) fn insert(&mut self, id: InstanceId, pou: u32) -> Result<(), RuntimeError> {
        self.0.insert(id.0, pou)?;
        Ok(())
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(InstanceId) -> bool) {
        self.0.retain(|id, _| keep(InstanceId(*id)));
    }
    pub(super) fn remove(&mut self, id: &InstanceId) {
        self.0.remove(&id.0);
    }
}
#[derive(Default)]
pub(super) struct ActivationPous(Entries<u32, u32>);
impl ActivationPous {
    pub(super) fn reservation_demand(
        &self,
        capacity: usize,
    ) -> Result<(usize, usize), RuntimeError> {
        self.0.reservation_demand(capacity)
    }
    pub(super) fn reserve_capacity(&mut self, capacity: usize) -> Result<(), RuntimeError> {
        self.0.reserve_capacity(capacity)
    }
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn get(&self, id: &FrameId) -> Option<&u32> {
        self.0.get(&id.0)
    }
    pub(super) fn insertion_demand(&self, id: FrameId) -> Result<(usize, usize), RuntimeError> {
        self.0.insertion_demand(&id.0)
    }
    pub(super) fn insert(&mut self, id: FrameId, pou: u32) -> Result<(), RuntimeError> {
        self.0.insert(id.0, pou)?;
        Ok(())
    }
    pub(super) fn remove(&mut self, id: &FrameId) {
        self.0.remove(&id.0);
    }
}
#[derive(Default)]
pub(super) struct InstanceLifetimes(Entries<u32, Option<u32>>);
impl InstanceLifetimes {
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn get(&self, id: &InstanceId) -> Option<Option<FrameId>> {
        self.0.get(&id.0).map(|owner| owner.map(FrameId))
    }
    pub(super) fn contains_key(&self, id: &InstanceId) -> bool {
        self.0.get(&id.0).is_some()
    }
    pub(super) fn insertion_demand(&self, id: InstanceId) -> Result<(usize, usize), RuntimeError> {
        self.0.insertion_demand(&id.0)
    }
    pub(super) fn insert(
        &mut self,
        id: InstanceId,
        owner: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        self.0.insert(id.0, owner.map(|id| id.0))?;
        Ok(())
    }
    /// Rollback only restores an existing association, without allocation/fuel.
    pub(super) fn restore(&mut self, id: InstanceId, owner: Option<FrameId>) {
        if let Some(slot) = self.0.get_mut(&id.0) {
            *slot = owner.map(|id| id.0);
        }
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(InstanceId, Option<FrameId>) -> bool) {
        self.0
            .retain(|id, owner| keep(InstanceId(*id), owner.map(FrameId)));
    }
    pub(super) fn remove(&mut self, id: &InstanceId) {
        self.0.remove(&id.0);
    }
}
#[derive(Default)]
pub(super) struct OwnedInstances(Entries<u32, Vec<u32>>);
impl OwnedInstances {
    pub(super) fn reservation_demand(
        &self,
        capacity: usize,
    ) -> Result<(usize, usize), RuntimeError> {
        self.0.reservation_demand(capacity)
    }
    pub(super) fn reserve_capacity(&mut self, capacity: usize) -> Result<(), RuntimeError> {
        self.0.reserve_capacity(capacity)
    }
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn count(&self, owner: FrameId) -> usize {
        self.0.get(&owner.0).map_or(0, Vec::len)
    }
    pub(super) fn push_demand(&self, owner: FrameId) -> Result<(usize, usize), RuntimeError> {
        let (outer_bytes, outer_work) = self.0.insertion_demand(&owner.0)?;
        let (bytes, moved) = match self.0.get(&owner.0) {
            Some(entries) => growth_demand(entries)?,
            None => (4 * core::mem::size_of::<u32>(), 0),
        };
        Ok((
            outer_bytes
                .checked_add(bytes)
                .ok_or(RuntimeError::Overflow)?,
            outer_work
                .checked_add(moved)
                .ok_or(RuntimeError::Overflow)?,
        ))
    }
    pub(super) fn push(
        &mut self,
        owner: FrameId,
        instance: InstanceId,
    ) -> Result<(), RuntimeError> {
        if let Some(entries) = self.0.get_mut(&owner.0) {
            reserve_growth(entries)?;
            entries.push(instance.0);
        } else {
            let mut entries = Vec::new();
            reserve_growth(&mut entries)?;
            entries.push(instance.0);
            self.0.insert(owner.0, entries)?;
        }
        Ok(())
    }
    pub(super) fn snapshot(&self, owner: FrameId) -> impl Iterator<Item = InstanceId> + use<> {
        self.0
            .get(&owner.0)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(InstanceId)
    }
    pub(super) fn take(&mut self, owner: FrameId) -> impl Iterator<Item = InstanceId> + use<> {
        self.0
            .remove(&owner.0)
            .unwrap_or_default()
            .into_iter()
            .map(InstanceId)
    }
}

/// Reuse ordered destination slots without packing or narrowing their keys.
#[derive(Default)]
pub(super) struct RetainedGlobals(super::output_transaction::SavedDestinations);
impl RetainedGlobals {
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn get(&self, declaration: &u32) -> Option<&super::Value> {
        let slot = usize::try_from(*declaration).ok()?;
        self.0.get(&(crate::memory::MemoryLocation::Global, slot))
    }
    pub(super) fn contains_key(&self, declaration: &u32) -> bool {
        self.get(declaration).is_some()
    }
    pub(super) fn insert(
        &mut self,
        declaration: u32,
        value: super::Value,
    ) -> Result<(), super::RuntimeError> {
        let slot =
            usize::try_from(declaration).map_err(|_| super::RuntimeError::InvalidExecutionState)?;
        self.0
            .insert((crate::memory::MemoryLocation::Global, slot), value)?;
        Ok(())
    }
    pub(super) fn growth_charge(&self, declaration: u32) -> Result<(usize, usize), RuntimeError> {
        let slot = usize::try_from(declaration).map_err(|_| RuntimeError::InvalidExecutionState)?;
        self.0
            .insertion_demand(&(crate::memory::MemoryLocation::Global, slot))
    }
    /// Keep the former conservative two-tree charge and add the wider key's cost.
    pub(super) fn insertion_charge() -> usize {
        let old_entry = core::mem::size_of::<(u32, super::Value)>();
        let shared_entry =
            core::mem::size_of::<((crate::memory::MemoryLocation, usize), super::Value)>();
        8 * old_entry + 4 * shared_entry.saturating_sub(old_entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{collections::BTreeSet, vec};

    #[test]
    fn bounded_frame_metadata_reserves_outer_entries_before_run() {
        let mut pous = ActivationPous::default();
        let mut owners = OwnedInstances::default();
        let ordinary = 4;
        let all_frames = 2 * ordinary;
        assert_eq!(
            pous.reservation_demand(ordinary),
            Ok((ordinary * core::mem::size_of::<(u32, u32)>(), 1))
        );
        assert_eq!(
            owners.reservation_demand(all_frames),
            Ok((all_frames * core::mem::size_of::<(u32, Vec<u32>)>(), 1))
        );
        pous.reserve_capacity(ordinary).unwrap();
        owners.reserve_capacity(all_frames).unwrap();
        let capacities = (pous.0.capacity(), owners.0.capacity());
        for offset in 0..all_frames {
            let frame = FrameId(u32::MAX - offset as u32);
            if offset < ordinary {
                assert_eq!(pous.insertion_demand(frame).unwrap().0, 0);
                pous.insert(frame, 7).unwrap();
            }
            // Only the nested instance payload needs allocation during this push.
            assert_eq!(
                owners.push_demand(frame).unwrap().0,
                4 * core::mem::size_of::<u32>()
            );
            owners.push(frame, InstanceId(offset as u32)).unwrap();
            assert_eq!((pous.0.capacity(), owners.0.capacity()), capacities);
        }
        assert_eq!(
            owners.take(FrameId(u32::MAX)).collect::<Vec<_>>(),
            vec![InstanceId(0)]
        );
        assert_eq!(owners.0.capacity(), capacities.1);
        assert!(pous.reservation_demand(usize::MAX).is_err());
        assert!(owners.reservation_demand(usize::MAX).is_err());
    }

    #[test]
    fn primitive_sets_keep_typed_order_deduplication_and_full_identity_bits() {
        let ids = [
            InstanceId(u32::MAX),
            InstanceId(0),
            InstanceId(1),
            InstanceId(u32::MAX - 1),
        ];
        let mut expected = BTreeSet::new();
        let mut actual = InstanceSet::default();
        for id in ids {
            assert_eq!(actual.insert(id).unwrap(), expected.insert(id));
            assert!(!actual.insert(id).unwrap());
            assert!(actual.contains(&id));
        }
        assert_eq!(
            actual.iter().collect::<Vec<_>>(),
            expected.iter().copied().collect::<Vec<_>>()
        );
        assert_eq!(
            actual.remove(&InstanceId(1)),
            expected.remove(&InstanceId(1))
        );
        assert!(!actual.remove(&InstanceId(1)));
        assert_eq!(
            actual.into_iter().collect::<Vec<_>>(),
            expected.into_iter().collect::<Vec<_>>()
        );
    }

    #[test]
    fn primitive_maps_keep_frame_instance_domains_and_persistent_lifetimes_distinct() {
        let mut activations = ActivationPous::default();
        let mut templates = InstanceTemplates::default();
        let mut lifetimes = InstanceLifetimes::default();
        for raw in [0, 1, u32::MAX] {
            activations.insert(FrameId(raw), 10).unwrap();
            templates.insert(InstanceId(raw), 20).unwrap();
            assert_eq!(activations.get(&FrameId(raw)), Some(&10));
            assert_eq!(templates.get(&InstanceId(raw)), Some(&20));
            assert_eq!(lifetimes.get(&InstanceId(raw)), None);
            lifetimes.insert(InstanceId(raw), None).unwrap();
            assert_eq!(lifetimes.get(&InstanceId(raw)), Some(None));
            lifetimes
                .insert(InstanceId(raw), Some(FrameId(u32::MAX)))
                .unwrap();
            assert_eq!(
                lifetimes.get(&InstanceId(raw)),
                Some(Some(FrameId(u32::MAX)))
            );
        }
        activations.remove(&FrameId(1));
        templates.insert(InstanceId(1), 21).unwrap();
        assert_eq!(activations.get(&FrameId(1)), None);
        assert_eq!(templates.get(&InstanceId(1)), Some(&21));
        lifetimes.remove(&InstanceId(1));
        assert!(!lifetimes.contains_key(&InstanceId(1)));
        assert_eq!(lifetimes.get(&InstanceId(0)), Some(Some(FrameId(u32::MAX))));
    }

    #[test]
    fn owned_instance_payloads_keep_order_duplicates_and_independent_rollback_snapshot() {
        let mut owners = OwnedInstances::default();
        let owner = FrameId(u32::MAX);
        for id in [0, u32::MAX, 0] {
            owners.push(owner, InstanceId(id)).unwrap();
        }
        owners.push(FrameId(0), InstanceId(9)).unwrap();
        let snapshot = owners.snapshot(owner);
        owners.push(owner, InstanceId(3)).unwrap();
        assert_eq!(
            snapshot.collect::<Vec<_>>(),
            vec![InstanceId(0), InstanceId(u32::MAX), InstanceId(0)]
        );
        assert_eq!(
            owners.take(owner).collect::<Vec<_>>(),
            vec![
                InstanceId(0),
                InstanceId(u32::MAX),
                InstanceId(0),
                InstanceId(3)
            ]
        );
        assert_eq!(
            owners.take(FrameId(0)).collect::<Vec<_>>(),
            vec![InstanceId(9)]
        );
        assert_eq!(owners.take(owner).count(), 0);
    }
    #[test]
    fn retained_values_use_ordered_global_keys_without_narrowing_journal_domains() {
        use super::super::Value;
        use crate::memory::{IoArea, MemoryLocation};
        let mut retained = RetainedGlobals::default();
        for id in [u32::MAX, 1, 0] {
            retained.insert(id, Value::UDInt(id)).unwrap();
        }
        assert_eq!(
            retained.0.keys().copied().collect::<Vec<_>>(),
            vec![
                (MemoryLocation::Global, 0),
                (MemoryLocation::Global, 1),
                (MemoryLocation::Global, u32::MAX as usize),
            ]
        );
        retained.insert(1, Value::Int(7)).unwrap();
        assert_eq!(retained.get(&1), Some(&Value::Int(7)));
        assert_eq!(retained.len(), 3);
        assert!(!retained.contains_key(&2));
        assert!(RetainedGlobals::insertion_charge() >= 8 * core::mem::size_of::<(u32, Value)>());
        let locations = [
            MemoryLocation::Global,
            MemoryLocation::Local(FrameId(0)),
            MemoryLocation::Local(FrameId(u32::MAX)),
            MemoryLocation::Instance(InstanceId(0)),
            MemoryLocation::Instance(InstanceId(u32::MAX)),
            MemoryLocation::Io(IoArea::Input),
            MemoryLocation::Io(IoArea::Output),
            MemoryLocation::Io(IoArea::Memory),
            MemoryLocation::Retain,
        ];
        let mut journal = super::super::output_transaction::SavedDestinations::new();
        for location in locations {
            for slot in [0, usize::MAX] {
                assert!(journal
                    .insert((location, slot), Value::Bool(true))
                    .unwrap()
                    .is_none());
            }
        }
        assert_eq!(journal.len(), locations.len() * 2);
        assert_eq!(
            journal.keys().copied().collect::<Vec<_>>(),
            locations
                .into_iter()
                .flat_map(|location| [(location, 0), (location, usize::MAX)])
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn promoted_instances_survive_old_frame_retirement_then_retire_with_their_new_owner() {
        use super::super::{EngineState, PreparedModule, Value};
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut state =
            EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        let destination = state.storage.reserve_execution_frame().unwrap();
        let source = state.storage.reserve_execution_frame().unwrap();
        state
            .lifetimes
            .live_activations
            .extend([destination, source]);
        let pou = *prepared.vm.function_block_ids.get("COUNTER").unwrap();
        let instance = state.reserve_instance(pou, Some(source)).unwrap();
        state
            .construction
            .initialized_instances
            .insert(instance)
            .unwrap();
        state
            .construction
            .initialized_declarations
            .insert((0, Some(instance)))
            .unwrap();
        state.construction.once.insert((0, Some(instance))).unwrap();
        assert_eq!(
            state.lifetimes.instance_lifetimes.get(&instance),
            Some(Some(source))
        );
        state
            .promote_constructed_instances(&Value::Instance(instance), source, Some(destination))
            .unwrap();
        state.remove_owned_instances(source).unwrap();
        assert!(state.storage.get_instance(instance).is_some());
        assert_eq!(
            state.lifetimes.instance_lifetimes.get(&instance),
            Some(Some(destination))
        );
        assert!(state
            .construction
            .initialized_declarations
            .contains(&(0, Some(instance)))
            .unwrap());
        state.remove_owned_instances(destination).unwrap();
        assert!(state.storage.get_instance(instance).is_none());
        assert!(!state.lifetimes.instance_lifetimes.contains_key(&instance));
        assert!(state
            .construction
            .instance_templates
            .get(&instance)
            .is_none());
        assert!(!state.construction.initialized_instances.contains(&instance));
        assert!(!state
            .construction
            .initialized_declarations
            .contains(&(0, Some(instance)))
            .unwrap());
        assert!(!state
            .construction
            .once
            .contains(&(0, Some(instance)))
            .unwrap());
    }
}
