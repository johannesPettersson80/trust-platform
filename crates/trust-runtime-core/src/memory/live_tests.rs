//! Live-array identity, admission and retirement boundaries (MEM-03/10/11).
use super::*;
use crate::error::RuntimeError;
use alloc::vec;

#[test]
fn live_array_keeps_full_keys_sorted_and_supports_non_lifo_retirement() {
    let mut entries = live::LiveEntries::default();
    for id in [0, 1, u32::MAX - 1, u32::MAX] {
        entries.try_append(InstanceId(id), id).unwrap();
    }
    assert!(!entries.is_empty());
    assert_eq!(entries.remove(&InstanceId(1)), Some(1));
    assert_eq!(entries.remove(&InstanceId(1)), None);
    assert_eq!(entries.get(&InstanceId(u32::MAX)), Some(&u32::MAX));
    assert_eq!(
        entries.iter().map(|(key, _)| key.0).collect::<Vec<_>>(),
        vec![0, u32::MAX - 1, u32::MAX]
    );
    assert_eq!(
        entries.try_append(InstanceId(0), 99),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(entries.get(&InstanceId(0)), Some(&0));
    entries.retain(|id, _| id.0 == u32::MAX);
    assert_eq!(entries.len(), 1);
    assert!(
        entries.capacity() < 16,
        "capacity follows live entries, not numeric identity"
    );
}

#[test]
fn execution_frame_capacity_failure_preserves_ids_locals_and_reservation_on_clone() {
    let mut storage = VariableStorage::new();
    storage.reserve_execution_frames(2).unwrap();
    let first = storage.reserve_execution_frame().unwrap();
    let second = storage.reserve_execution_frame().unwrap();
    let next = storage.activations.next_frame_id;
    let capacity = storage.activations.execution_frames.capacity();
    assert_eq!(
        storage.reserve_execution_frame().unwrap_err().stable_code(),
        crate::error_code::StableErrorCode::VmCallStackOverflow
    );
    assert_eq!(storage.activations.next_frame_id, next);
    let mut locals = vec![Value::Int(17)];
    storage
        .suspend_execution_frame(second, &mut locals)
        .unwrap();
    assert!(locals.is_empty());
    assert_eq!(storage.local_slot(second, 0), Some(&Value::Int(17)));
    assert!(storage.release_execution_frame(first));
    assert!(!storage.execution_frame_is_live(first));
    let third = storage.reserve_execution_frame().unwrap();
    assert!(third.0 > second.0);
    let clone = storage.clone();
    assert_eq!(clone.activations.execution_frames.capacity(), capacity);
    assert_eq!(clone.activations.execution_frame_limit, Some(2));
    assert_eq!(clone.local_slot(second, 0), Some(&Value::Int(17)));
    storage.resume_execution_frame(second, &mut locals).unwrap();
    assert_eq!(locals, vec![Value::Int(17)]);
    assert!(storage.release_execution_frame(second));
    assert!(storage.release_execution_frame(third));
    assert_eq!(storage.execution_frame_count(), 0);
    assert_eq!(storage.activations.execution_frames.capacity(), capacity);
}

/// Specification 34 §4.3.1 and HOST-02: within one top-level entry identities only increase,
/// so exhaustion refuses with `Overflow` instead of wrapping; a later entry restarts activation
/// identities but never issues the identity of a live instance, or of one that outlived an entry.
#[test]
fn frame_and_instance_id_exhaustion_never_wrap_or_replace_live_state() {
    let mut storage = VariableStorage::new();
    storage.activations.next_frame_id = u32::MAX - 1;
    storage.identities.next_instance_id = u32::MAX - 1;
    let frame = storage.reserve_execution_frame().unwrap();
    let instance = storage.try_create_instance("LAST").unwrap();
    assert_eq!(frame.0, u32::MAX - 1);
    assert_eq!(instance.0, u32::MAX - 1);
    assert_eq!(
        storage.reserve_execution_frame(),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        storage.try_create_instance("WRAPPED"),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(storage.get_instance(instance).unwrap().type_name, "LAST");
    // A reserved activation keeps its entry open: no new entry, so nothing restarts.
    assert_eq!(
        storage.begin_execution_entry(),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(
        storage.reserve_execution_frame(),
        Err(RuntimeError::Overflow)
    );
    assert!(storage.execution_frame_is_live(frame));
    // Within the entry a released identity is never issued again.
    storage.release_execution_frame(frame);
    storage.remove_instance(instance);
    assert_eq!(
        storage.reserve_execution_frame(),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        storage.try_create_instance("REUSED"),
        Err(RuntimeError::Overflow)
    );
    // The next entry restarts activations at zero. Nothing the finished entry issued is
    // live, so its instance identities are issued again from where that entry started.
    storage.begin_execution_entry().unwrap();
    let restarted = storage.reserve_execution_frame().unwrap();
    assert_eq!(restarted, FrameId(0));
    assert_eq!(storage.try_create_instance("FIRST"), Ok(InstanceId(0)));
    // An instance that holds the last identity when its entry ends is never replaced:
    // the exhausted sequence refuses instead.
    storage.identities.next_instance_id = u32::MAX - 1;
    let last = storage.try_create_instance("LAST").unwrap();
    assert!(storage.release_execution_frame(restarted));
    storage.begin_execution_entry().unwrap();
    assert_eq!(
        storage.try_create_instance("REPLACED"),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(storage.get_instance(last).unwrap().type_name, "LAST");
    assert_eq!(storage.instances().len(), 2);
    // LAST outlived an entry, so even after its removal no later entry issues its identity.
    storage.remove_instance(last);
    storage.begin_execution_entry().unwrap();
    assert_eq!(
        storage.try_create_instance("REUSED"),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(storage.instances().len(), 1);
    assert!(VariableStorage::execution_frame_reservation_charge(usize::MAX).is_none());
    let mut empty = VariableStorage::new();
    assert!(empty.reserve_execution_frames(usize::MAX).is_err());
    assert_eq!(empty.execution_frame_count(), 0);
    assert_eq!(empty.activations.next_frame_id, 0);
}

/// Specification 34 §4.3.1: within an entry identities increase in ancestry order and a
/// released one is not issued again; the next entry restarts activations at zero and issues
/// again exactly the instance identities the finished entry released, never the identity of
/// an instance that was live when an entry ended.
#[test]
fn entry_boundary_reissues_only_identities_released_inside_the_finished_entry() {
    let mut storage = VariableStorage::new();
    let root = storage.try_create_instance("ROOT").unwrap();
    storage.begin_execution_entry().unwrap();
    let caller = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(caller).unwrap();
    let first = storage.try_create_instance("LOCAL").unwrap();
    let callee = storage.reserve_execution_frame().unwrap();
    storage.activate_execution_frame(callee).unwrap();
    assert_eq!((caller, callee), (FrameId(0), FrameId(1)));
    assert_eq!((root, first), (InstanceId(0), InstanceId(1)));
    assert_eq!(storage.previous_execution_activation(callee), Some(caller));
    assert!(storage.release_execution_frame(callee));
    let later = storage.reserve_execution_frame().unwrap();
    assert_eq!(
        later,
        FrameId(2),
        "no identity is reissued within its entry"
    );
    assert!(storage.release_execution_frame(later));
    assert!(storage.release_execution_frame(caller));
    storage.remove_instance(first);
    let second = storage.try_create_instance("LOCAL").unwrap();
    assert_eq!(
        second,
        InstanceId(2),
        "no instance identity is reissued within its entry"
    );
    assert_eq!(storage.retain_instances(|id| id != second), 1);

    storage.begin_execution_entry().unwrap();
    let restarted = storage.reserve_execution_frame().unwrap();
    assert_eq!(restarted, FrameId(0));
    let reissued = storage.try_create_instance("LOCAL").unwrap();
    assert_eq!(reissued, InstanceId(1));
    assert_eq!(storage.get_instance(root).unwrap().type_name, "ROOT");
    assert!(storage.release_execution_frame(restarted));

    // `reissued` is live when its entry ends, so its identity is never issued again.
    storage.begin_execution_entry().unwrap();
    let beyond = storage.try_create_instance("LOCAL").unwrap();
    assert_eq!(beyond, InstanceId(2));
    storage.remove_instance(reissued);
    storage.remove_instance(beyond);
    storage.begin_execution_entry().unwrap();
    assert_eq!(storage.try_create_instance("LOCAL"), Ok(InstanceId(2)));
    assert_eq!(storage.get_instance(root).unwrap().type_name, "ROOT");
    assert_eq!(storage.instances().len(), 2);
}

#[test]
fn owner_batch_retirement_visits_each_instance_once_and_preserves_survivor_values() {
    let mut storage = VariableStorage::new();
    let ids: Vec<_> = (0..6)
        .map(|value| {
            let id = storage.try_create_instance("FB").unwrap();
            storage.set_instance_var(id, "VALUE", Value::Int(value));
            id
        })
        .collect();
    let mut visits = vec![0; ids.len()];
    let removed = storage.retain_instances(|id| {
        visits[id.0 as usize] += 1;
        id == ids[0] || id == ids[4] // persistent root and promoted invocation instance
    });
    assert_eq!(removed, 4);
    assert_eq!(visits, vec![1; 6]);
    assert_eq!(
        storage.get_instance_var(ids[0], "VALUE"),
        Some(&Value::Int(0))
    );
    assert_eq!(
        storage.get_instance_var(ids[4], "VALUE"),
        Some(&Value::Int(4))
    );
    assert!(storage.get_instance(ids[1]).is_none());
    let later = storage.try_create_instance("LATER").unwrap();
    assert!(later.0 > ids[5].0);
    assert_eq!(
        storage.remove_instance(ids[0]).unwrap().variables["VALUE"],
        Value::Int(0)
    );
    assert_eq!(
        storage.get_instance_var(ids[4], "VALUE"),
        Some(&Value::Int(4))
    );
}

#[cfg(not(feature = "std"))]
#[test]
fn portable_hash_lookup_preserves_misses_parent_changes_shadowing_and_reset() {
    let mut storage = VariableStorage::new();
    let parent = storage.try_create_instance("BASE").unwrap();
    let child = storage.try_create_instance("CHILD").unwrap();
    storage.get_instance_mut(child).unwrap().parent = Some(parent);
    let charge = storage.clone_container_charge();
    for _ in 0..8 {
        assert!(storage
            .ref_for_instance(
                child,
                "unknown long field name beyond inline string capacity"
            )
            .is_none());
        assert!(storage.ref_for_instance_recursive(child, "LATE").is_none());
    }
    assert_eq!(
        storage.clone_container_charge(),
        charge,
        "unknown names create no cache entries"
    );
    storage.set_instance_var(parent, "LATE", Value::Int(3));
    assert_eq!(
        storage
            .ref_for_instance_recursive(child, "LATE")
            .unwrap()
            .location,
        MemoryLocation::Instance(parent)
    );
    storage.set_instance_var(child, "FIRST", Value::Int(0));
    storage.set_instance_var(child, "LATE", Value::Int(4));
    let direct = storage.resolved_instance_field_ref(child, "LATE").unwrap();
    assert_eq!(
        (direct.location, direct.offset),
        (MemoryLocation::Instance(child), 1)
    );
    assert_eq!(storage.read_by_ref(direct), Some(&Value::Int(4)));
    assert!(
        storage.ref_for_instance(child, "late").is_none(),
        "exact-case keys are preserved"
    );
    storage
        .get_instance_mut(child)
        .unwrap()
        .variables
        .shift_remove("LATE");
    assert_eq!(
        storage
            .resolved_instance_field_ref(child, "LATE")
            .unwrap()
            .location,
        MemoryLocation::Instance(parent)
    );
    let clone = storage.clone();
    assert_eq!(
        clone.activations.instances.capacity(),
        storage.activations.instances.capacity()
    );
    assert_eq!(
        clone.declared_instance_field_offset(child, "FIRST"),
        Some(0)
    );
    storage.reset_runtime_values(false);
    assert!(storage.ref_for_instance_recursive(child, "LATE").is_none());
    assert!(storage.try_create_instance("NEW").unwrap().0 > child.0);
}

#[cfg(not(feature = "std"))]
#[test]
fn instance_growth_demand_matches_actual_reserved_array_and_survives_non_lifo_removal() {
    let mut storage = VariableStorage::new();
    let slot = core::mem::size_of::<(InstanceId, InstanceData)>();
    assert_eq!(storage.instance_insertion_demand(), Some((4 * slot, 0)));
    let first = storage.try_create_instance("FB").unwrap();
    assert_eq!(storage.activations.instances.capacity(), 4);
    assert_eq!(storage.instance_insertion_demand(), Some((0, 0)));
    for _ in 0..3 {
        storage.try_create_instance("FB").unwrap();
    }
    assert_eq!(storage.instance_insertion_demand(), Some((8 * slot, 4)));
    storage.try_create_instance("FB").unwrap();
    assert_eq!(storage.activations.instances.capacity(), 8);
    storage.remove_instance(first);
    assert_eq!(storage.instance_insertion_demand(), Some((0, 0)));
    assert!(storage.get_instance(first).is_none());
}
