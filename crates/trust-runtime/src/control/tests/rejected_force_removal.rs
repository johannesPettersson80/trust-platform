// Spec 11, "changes neither the destination nor the force set": a force whose application the scan
// boundary rejects is removed from the force set, and its rejection is reported once. The rejected
// value is not applied again on the next scan.

#[test]
fn io_force_integer_literal_outside_the_declared_type_leaves_no_force_after_two_scans() {
    use crate::value::Value;
    let (state, mut runtime) = io_literal_fixture();
    io_literal_request(&state, 1, "io.force", "%MW0", "40000");

    runtime
        .execute_cycle()
        .expect("a rejected force must not fault the scan");
    runtime
        .execute_cycle()
        .expect("the PLC keeps scanning after the rejection");

    assert!(
        state.debug.forced_snapshot().io.is_empty(),
        "the rejected force must not remain in the force set"
    );
    assert_eq!(
        io_literal_rejections(&state),
        1,
        "exactly one write_rejected over two scans"
    );
    assert_eq!(
        io_literal_image(&runtime, "%MW0"),
        Value::Word(0),
        "the rejected force leaves the destination unchanged"
    );
    assert!(!runtime.faulted());
}

#[test]
fn var_force_integer_literal_outside_the_declared_type_leaves_no_force_after_two_scans() {
    use crate::value::Value;
    let (state, mut runtime) = var_literal_fixture();
    var_literal_request(&state, 2, "var.force", "global:Counter", "40000");

    runtime
        .execute_cycle()
        .expect("a rejected force must not fault the scan");
    runtime
        .execute_cycle()
        .expect("the PLC keeps scanning after the rejection");

    assert!(
        state.debug.forced_snapshot().vars.is_empty(),
        "the rejected force must not remain in the force set"
    );
    assert_eq!(
        var_literal_rejections(&state),
        1,
        "exactly one write_rejected over two scans"
    );
    assert_eq!(
        var_literal_counter(&runtime),
        Value::Int(0),
        "the rejected force leaves Counter unchanged"
    );
    assert!(!runtime.faulted());
}
