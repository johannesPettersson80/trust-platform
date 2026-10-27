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
