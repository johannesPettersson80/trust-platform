use super::*;
use alloc::collections::BTreeSet;

#[test]
fn lifecycle_key_is_injective_ordered_and_keeps_all_instance_bits() {
    let owners = [
        None,
        Some(InstanceId(0)),
        Some(InstanceId(1)),
        Some(InstanceId(u32::MAX - 1)),
        Some(InstanceId(u32::MAX)),
    ];
    let mut expected = BTreeSet::new();
    let mut marks = LifecycleMarks::default();
    for declaration in [u16::MAX as u32, 1, 0] {
        for owner in owners.into_iter().rev() {
            let key = (declaration, owner);
            assert_eq!(decode_mark(encode_mark(declaration, owner).unwrap()), key);
            assert!(marks.insert(key).unwrap());
            assert!(!marks.insert(key).unwrap());
            assert!(marks.contains(&key).unwrap());
            expected.insert(key);
        }
    }
    assert_eq!(
        marks.iter().collect::<Vec<_>>(),
        expected.into_iter().collect::<Vec<_>>()
    );
    let before = marks.len();
    assert_eq!(
        marks.insert((65_536, None)),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(
        marks.contains(&(u32::MAX, Some(InstanceId(0)))),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(marks.len(), before);
}

#[test]
fn retiring_an_owner_keeps_global_and_other_generation_marks() {
    let mut marks = LifecycleMarks::default();
    for owner in [None, Some(InstanceId(0)), Some(InstanceId(u32::MAX))] {
        marks.insert((7, owner)).unwrap();
    }
    marks.retain(|(_, owner)| *owner != Some(InstanceId(0)));
    assert!(marks.contains(&(7, None)).unwrap());
    assert!(!marks.contains(&(7, Some(InstanceId(0)))).unwrap());
    assert!(marks.contains(&(7, Some(InstanceId(u32::MAX)))).unwrap());
}

#[test]
fn planned_marker_reserves_before_publication_and_publishes_once() {
    let mut marks = LifecycleMarks::default();
    let key = (3, Some(InstanceId(9)));
    let plan = marks.insertion_plan(key).unwrap();
    assert!(
        plan.demand().0 > 0,
        "an empty map needs its first allocation"
    );
    let prepared = marks.prepare_insert(plan).unwrap();
    prepared.publish();
    assert!(marks.contains(&key).unwrap());
    let capacity = marks.capacity_state_for_test();
    // A present marker needs no storage; publishing it again changes nothing.
    let plan = marks.insertion_plan(key).unwrap();
    assert_eq!(plan.demand().0, 0);
    marks.prepare_insert(plan).unwrap().publish();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks.capacity_state_for_test(), capacity);
}

#[test]
fn planning_a_marker_publishes_nothing() {
    let marks = LifecycleMarks::default();
    marks.insertion_plan((1, None)).unwrap();
    assert_eq!(marks.len(), 0);
    assert_eq!(marks.capacity_state_for_test(), 0);
    assert_eq!(
        marks.insertion_plan((70_000, None)).err(),
        Some(RuntimeError::InvalidExecutionState)
    );
}

#[test]
fn root_index_conversion_never_truncates() {
    assert_eq!(root_index(0), Ok(0));
    assert_eq!(root_index(u32::MAX as usize), Ok(u32::MAX));
    if let Some(too_large) = (u32::MAX as usize).checked_add(1) {
        assert_eq!(
            root_index(too_large),
            Err(RuntimeError::InvalidExecutionState)
        );
    }
}
