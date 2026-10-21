//! Typed identity boundaries over compact sorted live entries.
use super::ordered::{fresh_growth_demand, reserve_growth, Entries};
use super::{EngineState, FrameId, InstanceId, RuntimeError, Vec};

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct InstanceSet(Entries<u32, ()>);
impl InstanceSet {
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.0.len()
    }
    pub(super) fn insertion_demand(&self, id: InstanceId) -> Result<(usize, usize), RuntimeError> {
        self.0.insertion_demand(&id.0)
    }
    /// The demand of inserting `id` into a set reused from the reserved lists, charged as a set
    /// that grew on demand from empty (spec 34 TIME-08; C1-11c).
    pub(super) fn fresh_insertion_demand(
        &self,
        id: InstanceId,
    ) -> Result<(usize, usize), RuntimeError> {
        self.0.fresh_insertion_demand(&id.0)
    }
    pub(super) fn insert(&mut self, id: InstanceId) -> Result<bool, RuntimeError> {
        Ok(self.0.insert(id.0, ())?.is_none())
    }
    /// Room for `capacity` identities, so that inserting as many allocates nothing (C1-19c).
    pub(super) fn reserve_capacity(&mut self, capacity: usize) -> Result<(), RuntimeError> {
        self.0.reserve_capacity(capacity)
    }
    /// Room for `capacity` entries that changes no charge (C1-21).
    pub(super) fn reserve_room(&mut self, capacity: usize) -> Result<(), RuntimeError> {
        self.0.reserve_room(capacity)
    }
    /// Remove every identity and keep the room they took.
    pub(super) fn clear(&mut self) {
        self.0.clear();
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
    #[cfg(test)]
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
    /// Room for `capacity` entries that changes no charge (C1-21).
    pub(super) fn reserve_room(&mut self, capacity: usize) -> Result<(), RuntimeError> {
        self.0.reserve_room(capacity)
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(InstanceId) -> bool) {
        self.0.retain(|id, _| keep(InstanceId(*id)));
    }
    pub(super) fn remove(&mut self, id: &InstanceId) {
        self.0.remove(&id.0);
    }
}
#[derive(Default)]
pub(super) struct InstanceLifetimes(Entries<u32, Option<u32>>);
impl InstanceLifetimes {
    #[cfg(test)]
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
    /// Room for `capacity` entries that changes no charge (C1-21).
    pub(super) fn reserve_room(&mut self, capacity: usize) -> Result<(), RuntimeError> {
        self.0.reserve_room(capacity)
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
pub(super) struct OwnedInstances(
    Entries<u32, Vec<u32>>,
    /// The emptied lists of owners that ended, kept for the next owner, so that the instances of a
    /// call take no allocation once the engine is warm (spec 34 MEM-09; C1-11c).
    Vec<Vec<u32>>,
    #[cfg(test)] core::cell::Cell<usize>,
);
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
    /// Keep `count` emptied lists with room for `room` instances each, so that the owners a chain
    /// of calls holds at once take their lists without allocating (spec 34 MEM-09; C1-19c).
    pub(super) fn reserve_lists(&mut self, count: usize, room: usize) -> Result<(), RuntimeError> {
        let refused = |_| RuntimeError::Overflow;
        self.1
            .try_reserve_exact(count.saturating_sub(self.1.len()))
            .map_err(refused)?;
        while self.1.len() < count {
            let mut list = Vec::new();
            list.try_reserve_exact(room).map_err(refused)?;
            self.1.push(list);
        }
        Ok(())
    }
    /// The bytes [`OwnedInstances::reserve_lists`] takes for `count` lists of `room` instances.
    pub(super) fn list_reservation_bytes(count: usize, room: usize) -> Option<usize> {
        room.checked_mul(core::mem::size_of::<u32>())?
            .checked_add(core::mem::size_of::<Vec<u32>>())?
            .checked_mul(count)
    }
    pub(super) fn count(&self, owner: FrameId) -> usize {
        self.0.get(&owner.0).map_or(0, Vec::len)
    }
    pub(super) fn push_demand(&self, owner: FrameId) -> Result<(usize, usize), RuntimeError> {
        let (outer_bytes, outer_work) = self.0.insertion_demand(&owner.0)?;
        // An owner's list is charged as a list that grows from empty, also when it reuses the
        // list of an owner that ended (TIME-08).
        let held = self.0.get(&owner.0).map_or(0, Vec::len);
        let (bytes, moved) = fresh_growth_demand::<u32>(held)?;
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
            let mut entries = self.1.pop().unwrap_or_default();
            reserve_growth(&mut entries)?;
            entries.push(instance.0);
            self.0.insert(owner.0, entries)?;
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn snapshot(&self, owner: FrameId) -> impl Iterator<Item = InstanceId> + use<> {
        self.note_snapshot_copy();
        self.0
            .get(&owner.0)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(InstanceId)
    }
    pub(super) fn copy_snapshot(&self, owner: FrameId, output: &mut Vec<InstanceId>) {
        self.note_snapshot_copy();
        if let Some(entries) = self.0.get(&owner.0) {
            output.extend(entries.iter().copied().map(InstanceId));
        }
    }
    #[cfg(test)]
    fn note_snapshot_copy(&self) {
        self.2.set(self.2.get() + 1);
    }
    #[cfg(not(test))]
    fn note_snapshot_copy(&self) {}
    #[cfg(test)]
    pub(super) fn snapshot_copy_count(&self) -> usize {
        self.2.get()
    }
    /// The instances `owner`'s list holds, in the order they were pushed, taken off the owner. The
    /// emptied list is kept for the next owner when the iterator is dropped.
    pub(super) fn take(&mut self, owner: FrameId) -> TakenInstances<'_> {
        TakenInstances {
            entries: self.0.remove(&owner.0).unwrap_or_default(),
            next: 0,
            spare: &mut self.1,
        }
    }
}

/// The instances taken off an owner (see [`OwnedInstances::take`]).
pub(super) struct TakenInstances<'a> {
    entries: Vec<u32>,
    next: usize,
    spare: &'a mut Vec<Vec<u32>>,
}
impl Iterator for TakenInstances<'_> {
    type Item = InstanceId;
    fn next(&mut self) -> Option<InstanceId> {
        let id = *self.entries.get(self.next)?;
        self.next += 1;
        Some(InstanceId(id))
    }
}
impl Drop for TakenInstances<'_> {
    fn drop(&mut self) {
        let mut entries = core::mem::take(&mut self.entries);
        // A list that never held an instance has no room to keep. The spare lists grow only while
        // the engine warms up; a list they have no room for is freed.
        if entries.capacity() == 0 || self.spare.try_reserve(1).is_err() {
            return;
        }
        entries.clear();
        self.spare.push(entries);
    }
}

/// The retained globals a warm restart imported into this generation, by declaration index, sorted
/// (C1-14a). Their values live in the store; the set only tells construction which globals' actions
/// to skip. Its growth is charged as the table it replaces charged it, which kept a copy of each
/// global's value under its destination key: an entry of `((MemoryLocation, usize), Value)` and a
/// capacity that doubles from 4 (spec 34 TIME-08).
#[derive(Default)]
pub(super) struct RetainedGlobals {
    declarations: Vec<u32>,
    charged_capacity: usize,
}
impl RetainedGlobals {
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.declarations.len()
    }
    pub(super) fn contains_key(&self, declaration: &u32) -> bool {
        self.declarations.binary_search(declaration).is_ok()
    }
    pub(super) fn insert(&mut self, declaration: u32) -> Result<(), super::RuntimeError> {
        let Err(at) = self.declarations.binary_search(&declaration) else {
            return Ok(());
        };
        if self.declarations.len() == self.charged_capacity {
            self.charged_capacity = self.next_capacity()?;
        }
        self.declarations
            .try_reserve(1)
            .map_err(|_| RuntimeError::Overflow)?;
        self.declarations.insert(at, declaration);
        Ok(())
    }
    /// The allocation and the work the insertion of `declaration` charges: those of the table of
    /// values it replaces, a sorted list that grows on demand to twice its capacity, at least 4.
    pub(super) fn growth_charge(&self, declaration: u32) -> Result<(usize, usize), RuntimeError> {
        let at = match self.declarations.binary_search(&declaration) {
            Ok(_) => return Ok((0, 0)),
            Err(at) => at,
        };
        let len = self.declarations.len();
        let shifts = len - at;
        if len < self.charged_capacity {
            return Ok((0, shifts));
        }
        let bytes = self
            .next_capacity()?
            .checked_mul(core::mem::size_of::<(
                (crate::memory::MemoryLocation, usize),
                super::Value,
            )>())
            .ok_or(RuntimeError::Overflow)?;
        let work = len.checked_add(shifts).ok_or(RuntimeError::Overflow)?;
        Ok((bytes, work))
    }
    fn next_capacity(&self) -> Result<usize, RuntimeError> {
        self.charged_capacity
            .checked_mul(2)
            .map(|capacity| capacity.max(4))
            .ok_or(RuntimeError::Overflow)
    }
    /// Keep the former conservative two-tree charge and add the wider key's cost.
    pub(super) fn insertion_charge() -> usize {
        let old_entry = core::mem::size_of::<(u32, super::Value)>();
        let shared_entry =
            core::mem::size_of::<((crate::memory::MemoryLocation, usize), super::Value)>();
        8 * old_entry + 4 * shared_entry.saturating_sub(old_entry)
    }
}

impl super::EngineState<'_> {
    /// Reserve the next activation identity for an invocation, an initializer result
    /// or an engineering scope. After construction, a reservation made while no
    /// activation is reserved begins a top-level execution entry: one task program or
    /// function-block invocation, or one engineering operation. Its activation
    /// identities start again at zero, and the instance identities issued since the
    /// previous entry began are issued again once none of those instances is live
    /// (specification 34 §4.3.1, HOST-02). The lifetime checks keep every reference to
    /// an activation, and every instance it owns, inside its entry, so a reissued
    /// identity cannot name anything of an earlier entry. Construction keeps a single
    /// increasing sequence.
    pub(super) fn reserve_activation_identity(&mut self) -> Result<FrameId, RuntimeError> {
        self.begin_entry_before_first_frame()?;
        self.data.storage.reserve_execution_frame()
    }

    /// Start a top-level execution entry when no frame of one is active, as its first frame's
    /// identity is reserved, and as a program that runs without its frame begins (checklist P1-9).
    pub(super) fn begin_entry_before_first_frame(&mut self) -> Result<(), RuntimeError> {
        if self.data.construction_complete && self.data.storage.execution_frame_count() == 0 {
            self.begin_execution_entry()?;
        }
        Ok(())
    }

    /// Refuse a frame at call depth `depth` that the program's call levels do not hold.
    pub(super) fn check_frame_depth(&self, depth: u32) -> Result<(), RuntimeError> {
        if depth as usize >= self.prepared.limits.max_call_depth {
            return Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error());
        }
        Ok(())
    }
}

/// The bytes of heap that the bookkeeping of `bounds` takes once reserved
/// ([`EngineState::reserve_instance_bookkeeping`]): for each instance its template, its lifetime,
/// its initialized flag and its slot in the lifetime check's visited set; each mark; each claimed
/// root (specification 34 MEM-12; C1-21).
pub(super) fn instance_bookkeeping_bytes(
    bounds: &crate::vm::prepared::InstanceBookkeeping,
) -> Option<usize> {
    let instance = core::mem::size_of::<(u32, u32)>()
        + core::mem::size_of::<(u32, Option<u32>)>()
        + core::mem::size_of::<(u32, ())>();
    let mark = core::mem::size_of::<(u64, ())>();
    bounds
        .instances
        .checked_mul(instance)?
        .checked_add(visited_capacity(bounds.instances)?.checked_mul(core::mem::size_of::<u32>())?)?
        .checked_add(bounds.marks.checked_add(bounds.once)?.checked_mul(mark)?)?
        .checked_add(
            bounds
                .claimed_roots
                .checked_mul(core::mem::size_of::<u32>())?,
        )
}

/// The room of the lifetime check's visited set for `instances` instances: the capacity that a set
/// which grew on demand to hold them has, 0, then 4, then each power of two, which its charge counts
/// and its growth reserves (spec 34 TIME-08).
fn visited_capacity(instances: usize) -> Option<usize> {
    match instances {
        0 => Some(0),
        _ => Some(instances.checked_next_power_of_two()?.max(4)),
    }
}

impl EngineState<'_> {
    /// Reserve, before construction creates any instance, the bookkeeping of every instance the
    /// program holds at once: its persistent instances and those the deepest chain of calls creates
    /// on the activation stack, with their marks, the claimed roots and the lifetime check's visited
    /// set, charged as construction memory (specification 34 MEM-09; C1-21). Neither construction
    /// nor a scan grows it then, and each insertion is still charged the growth of a list without
    /// the room (TIME-08).
    pub(super) fn reserve_instance_bookkeeping(&mut self) -> Result<(), RuntimeError> {
        let bounds = self.prepared.instance_bookkeeping()?;
        self.charge_allocation_bytes(
            instance_bookkeeping_bytes(&bounds).ok_or(RuntimeError::Overflow)?,
        )?;
        let construction = &mut self.data.construction;
        construction
            .instance_templates
            .reserve_room(bounds.instances)?;
        construction
            .initialized_instances
            .reserve_room(bounds.instances)?;
        construction
            .initialized_declarations
            .reserve_room(bounds.marks)?;
        construction.once.reserve_room(bounds.once)?;
        construction.claimed_roots =
            crate::collections::SortedSet::try_with_capacity(bounds.claimed_roots)
                .map_err(|_| RuntimeError::Overflow)?;
        self.data
            .lifetimes
            .instance_lifetimes
            .reserve_room(bounds.instances)?;
        self.data.resources.bookkeeping.reserve_lifetime_visited(
            visited_capacity(bounds.instances).ok_or(RuntimeError::Overflow)?,
        )
    }
}

#[cfg(test)]
#[path = "identities/reuse_tests.rs"]
mod reuse_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{collections::BTreeSet, vec};

    #[test]
    fn bounded_frame_metadata_reserves_outer_entries_before_run() {
        let mut storage = crate::memory::VariableStorage::default();
        let mut owners = OwnedInstances::default();
        let ordinary = 4;
        let all_frames = 2 * ordinary;
        assert_eq!(
            owners.reservation_demand(all_frames),
            Ok((all_frames * core::mem::size_of::<(u32, Vec<u32>)>(), 1))
        );
        storage.reserve_execution_frames(all_frames).unwrap();
        owners.reserve_capacity(all_frames).unwrap();
        let capacities = (storage.execution_frame_capacity(), owners.0.capacity());
        let mut first = None;
        for offset in 0..all_frames {
            let frame = storage.reserve_execution_frame().unwrap();
            first.get_or_insert(frame);
            storage.activate_execution_frame(frame).unwrap();
            if offset < ordinary {
                storage.set_execution_frame_owner(frame, 7, None).unwrap();
            }
            // Only the nested instance payload needs allocation during this push.
            assert_eq!(
                owners.push_demand(frame).unwrap().0,
                4 * core::mem::size_of::<u32>()
            );
            owners.push(frame, InstanceId(offset as u32)).unwrap();
            assert_eq!(
                (storage.execution_frame_capacity(), owners.0.capacity()),
                capacities
            );
        }
        assert_eq!(storage.execution_frame_owner_count(), ordinary);
        assert_eq!(
            owners.take(first.unwrap()).collect::<Vec<_>>(),
            vec![InstanceId(0)]
        );
        assert_eq!(owners.0.capacity(), capacities.1);
        assert!(
            crate::memory::VariableStorage::execution_frame_reservation_charge(usize::MAX)
                .is_none()
        );
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
        let mut storage = crate::memory::VariableStorage::default();
        let mut templates = InstanceTemplates::default();
        let mut lifetimes = InstanceLifetimes::default();
        let mut frames = Vec::new();
        for _ in 0..3 {
            let frame = storage.reserve_execution_frame().unwrap();
            storage.activate_execution_frame(frame).unwrap();
            storage.set_execution_frame_owner(frame, 10, None).unwrap();
            let instance = InstanceId(frame.0);
            frames.push(frame);
            templates.insert(instance, 20).unwrap();
            assert_eq!(storage.execution_frame_owner(frame).unwrap().pou_id, 10);
            assert_eq!(templates.get(&instance), Some(&20));
            assert_eq!(lifetimes.get(&instance), None);
            lifetimes.insert(instance, None).unwrap();
            assert_eq!(lifetimes.get(&instance), Some(None));
            lifetimes.insert(instance, Some(FrameId(u32::MAX))).unwrap();
            assert_eq!(lifetimes.get(&instance), Some(Some(FrameId(u32::MAX))));
        }
        templates.insert(InstanceId(u32::MAX), 99).unwrap();
        assert_eq!(templates.get(&InstanceId(u32::MAX)), Some(&99));
        let retired = frames[1];
        storage.release_execution_frame(retired);
        templates.insert(InstanceId(retired.0), 21).unwrap();
        assert!(storage.execution_frame_owner(retired).is_none());
        assert_eq!(templates.get(&InstanceId(retired.0)), Some(&21));
        lifetimes.remove(&InstanceId(retired.0));
        assert!(!lifetimes.contains_key(&InstanceId(retired.0)));
        assert_eq!(
            lifetimes.get(&InstanceId(frames[0].0)),
            Some(Some(FrameId(u32::MAX)))
        );
    }

    /// Spec 34 MEM-09 and TIME-08: the list of an owner that ended is kept for the next owner, and
    /// the next owner is charged as for a new list. Five instances grow a list to 8 entries; reused,
    /// its first instance is charged a new list's 4 entries of 4 bytes (16), and its fifth the
    /// growth from 4 to 8 entries (32), as a list growing from empty is. Catches a reused list
    /// charged by the room it kept (nothing for the fifth instance), and a taken list that is freed
    /// instead of kept.
    #[test]
    fn the_list_of_an_ended_owner_is_reused_and_charged_as_a_new_one() {
        let mut owners = OwnedInstances::default();
        for id in 0..5 {
            owners.push(FrameId(1), InstanceId(id)).unwrap();
        }
        assert_eq!(owners.take(FrameId(1)).count(), 5);
        assert_eq!(owners.1.len(), 1);
        assert_eq!(owners.1[0].capacity(), 8);
        assert_eq!(
            owners.push_demand(FrameId(2)).map(|(bytes, _)| bytes),
            Ok(16)
        );
        for id in 0..4 {
            owners.push(FrameId(2), InstanceId(id)).unwrap();
        }
        assert!(owners.1.is_empty());
        assert_eq!(
            owners.push_demand(FrameId(2)).map(|(bytes, _)| bytes),
            Ok(32)
        );
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
        // Spec 34 TIME-08 (C1-14a): the retained-global set keeps declaration indices in order,
        // the widest included, and charges its growth as the table of values it replaces charged
        // it: the first entry 4 entries of `((MemoryLocation, usize), Value)`, the fifth 8, and the
        // entries between them nothing. Catches a set charged by its own 4-byte entries and one
        // that charges every insertion.
        use super::super::Value;
        use crate::memory::{IoArea, MemoryLocation};
        let entry = core::mem::size_of::<((MemoryLocation, usize), Value)>();
        let mut retained = RetainedGlobals::default();
        assert_eq!(retained.growth_charge(u32::MAX).unwrap(), (4 * entry, 0));
        for id in [u32::MAX, 1, 0] {
            retained.insert(id).unwrap();
        }
        assert_eq!(retained.declarations, vec![0, 1, u32::MAX]);
        // A fourth entry still fits the charged capacity: only the shift of the larger one.
        assert_eq!(retained.growth_charge(2).unwrap(), (0, 1));
        retained.insert(2).unwrap();
        // The fifth doubles the capacity to 8, and its work is the four entries moved and the one
        // it shifts.
        assert_eq!(retained.growth_charge(3).unwrap(), (8 * entry, 5));
        // A declaration already there charges nothing and is kept once.
        assert_eq!(retained.growth_charge(1).unwrap(), (0, 0));
        retained.insert(1).unwrap();
        assert_eq!(retained.len(), 4);
        assert!(retained.contains_key(&2));
        assert!(!retained.contains_key(&3));
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
        let mut state_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        let destination = state.data.storage.reserve_execution_frame().unwrap();
        let source = state.data.storage.reserve_execution_frame().unwrap();
        state
            .data
            .storage
            .activate_execution_frame(destination)
            .unwrap();
        state.data.storage.activate_execution_frame(source).unwrap();
        let pou = prepared
            .vm
            .function_block_ids
            .get(&prepared.vm.strings, "COUNTER")
            .unwrap();
        let instance = state.reserve_instance(pou, Some(source)).unwrap();
        state
            .data
            .construction
            .initialized_instances
            .insert(instance)
            .unwrap();
        state
            .data
            .construction
            .initialized_declarations
            .insert((0, Some(instance)))
            .unwrap();
        state
            .data
            .construction
            .once
            .insert((0, Some(instance)))
            .unwrap();
        assert_eq!(
            state.data.lifetimes.instance_lifetimes.get(&instance),
            Some(Some(source))
        );
        state
            .promote_constructed_instances(&Value::Instance(instance), source, Some(destination))
            .unwrap();
        state.remove_owned_instances(source).unwrap();
        assert!(state.data.storage.get_instance(instance).is_some());
        assert_eq!(
            state.data.lifetimes.instance_lifetimes.get(&instance),
            Some(Some(destination))
        );
        assert!(state
            .data
            .construction
            .initialized_declarations
            .contains(&(0, Some(instance)))
            .unwrap());
        state.remove_owned_instances(destination).unwrap();
        assert!(state.data.storage.get_instance(instance).is_none());
        assert!(!state
            .data
            .lifetimes
            .instance_lifetimes
            .contains_key(&instance));
        assert!(state
            .data
            .construction
            .instance_templates
            .get(&instance)
            .is_none());
        assert!(!state
            .data
            .construction
            .initialized_instances
            .contains(&instance));
        assert!(!state
            .data
            .construction
            .initialized_declarations
            .contains(&(0, Some(instance)))
            .unwrap());
        assert!(!state
            .data
            .construction
            .once
            .contains(&(0, Some(instance)))
            .unwrap());
    }
}
