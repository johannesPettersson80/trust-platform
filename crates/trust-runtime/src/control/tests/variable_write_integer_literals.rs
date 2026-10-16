// Spec 11, "An admitted integer is an untyped literal": a queued set or var.force takes the
// declared integer type of its global at the scan boundary, as io.write does. A value the type
// cannot take is rejected alone, as write_rejected, and the PLC keeps running.

const VAR_LITERAL_SOURCE: &str = r#"
VAR_GLOBAL
    Counter : INT := 0;
END_VAR
PROGRAM MainProgram
END_PROGRAM
"#;

fn var_literal_fixture() -> (ControlState, crate::Runtime) {
    let state = hmi_test_state(VAR_LITERAL_SOURCE);
    let mut runtime = crate::harness::CompileSession::from_source(VAR_LITERAL_SOURCE)
        .build_runtime()
        .expect("variable literal fixture compiles");
    runtime.set_debug_control(state.debug.clone());
    (state, runtime)
}

fn var_literal_request(state: &ControlState, id: u64, kind: &str, target: &str, value: &str) {
    let response = handle_request_value(
        json!({"id": id, "type": kind, "params": {"target": target, "value": value}}),
        state,
        None,
    );
    assert!(
        response.ok,
        "{kind} {target} {value} should queue: {:?}",
        response.error
    );
}

fn var_literal_counter(runtime: &crate::Runtime) -> crate::value::Value {
    runtime
        .storage()
        .get_global("Counter")
        .cloned()
        .expect("Counter")
}

/// Drain the write_rejected events raised since the previous call.
fn var_literal_rejections(state: &ControlState) -> usize {
    state
        .debug
        .drain_runtime_events()
        .iter()
        .filter(|event| matches!(event, crate::debug::RuntimeEvent::WriteRejected { .. }))
        .count()
}

#[test]
fn set_integer_literal_takes_the_declared_int_type() {
    use crate::value::Value;
    let (state, mut runtime) = var_literal_fixture();
    var_literal_request(&state, 1, "set", "global:Counter", "41");

    runtime
        .execute_cycle()
        .expect("the INT literal is accepted at the scan boundary");

    assert_eq!(var_literal_counter(&runtime), Value::Int(41));
    assert!(
        !runtime.faulted(),
        "an accepted literal must not fault the PLC"
    );
    assert_eq!(var_literal_rejections(&state), 0);
}

#[test]
fn set_integer_literal_outside_the_declared_type_is_rejected_without_a_fault() {
    use crate::value::Value;
    let (state, mut runtime) = var_literal_fixture();
    var_literal_request(&state, 2, "set", "global:Counter", "40000");

    runtime
        .execute_cycle()
        .expect("a rejected write must not fault the scan");

    assert_eq!(
        var_literal_rejections(&state),
        1,
        "one write_rejected event"
    );
    assert_eq!(
        var_literal_counter(&runtime),
        Value::Int(0),
        "the rejected write leaves Counter unchanged"
    );
    assert!(!runtime.faulted());

    runtime
        .execute_cycle()
        .expect("the PLC keeps scanning after the rejection");
    assert_eq!(runtime.cycle_counter(), 2);
}

#[test]
fn var_force_integer_literal_forces_the_declared_int_global() {
    use crate::value::Value;
    let (state, mut runtime) = var_literal_fixture();
    var_literal_request(&state, 3, "var.force", "global:Counter", "41");

    runtime
        .execute_cycle()
        .expect("the forced INT literal is applied at the scan boundary");

    assert_eq!(var_literal_counter(&runtime), Value::Int(41));
    assert_eq!(
        state.debug.forced_snapshot().vars.len(),
        1,
        "the force stays active for the next scan"
    );
    assert_eq!(var_literal_rejections(&state), 0);
    assert!(!runtime.faulted());
}

#[test]
fn var_force_integer_literal_outside_the_declared_type_is_rejected_without_a_fault() {
    use crate::value::Value;
    let (state, mut runtime) = var_literal_fixture();
    var_literal_request(&state, 4, "var.force", "global:Counter", "40000");

    runtime
        .execute_cycle()
        .expect("a rejected force must not fault the scan");

    assert!(
        var_literal_rejections(&state) >= 1,
        "the rejected force reports write_rejected"
    );
    assert_eq!(
        var_literal_counter(&runtime),
        Value::Int(0),
        "the rejected force leaves Counter unchanged"
    );
    assert!(!runtime.faulted());
}
