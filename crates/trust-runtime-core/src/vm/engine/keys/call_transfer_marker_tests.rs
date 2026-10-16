use super::*;

/// Spec 34 §4.3: markers iterate in ascending (declaration, owner) order with u32::MAX as its own
/// owner key, and a refused insertion with that key leaves the markers unchanged.
#[test]
fn call_transfer_u32_max_owner_marks_iterate_in_order_and_refusal_changes_nothing() {
    let mut marks = LifecycleMarks::default();
    for owner in [Some(InstanceId(u32::MAX)), Some(InstanceId(0)), None] {
        assert!(marks.insert((9, owner)).unwrap());
    }
    assert_eq!(
        marks.iter().collect::<Vec<_>>(),
        vec![
            (9, None),
            (9, Some(InstanceId(0))),
            (9, Some(InstanceId(u32::MAX))),
        ]
    );

    let before = marks.iter().collect::<Vec<_>>();
    assert_eq!(
        marks.insert((65_536, Some(InstanceId(u32::MAX)))),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(marks.len(), 3);
    assert_eq!(marks.iter().collect::<Vec<_>>(), before);
    assert!(marks.contains(&(9, Some(InstanceId(u32::MAX)))).unwrap());
}
