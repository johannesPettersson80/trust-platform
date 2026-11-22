#[test]
fn a_runtime_on_the_computers_clock_refuses_simulator_advance() {
    // Specification 11 §6.8.1, "Simulator for machine tests": every runtime but a simulator runs on
    // the wall clock and refuses `simulator.advance` as `clock_not_manual`, so that every runtime
    // answers it alike (specification 34 STATE-08) and no request moves a running plant's time.
    // Catches a computer's runtime that does not route the request, which would refuse it as
    // unsupported, and one that answers it with cycles.
    let state = control_test_state(
        r#"
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
END_PROGRAM
"#,
    );
    let response = handle_request_value(
        json!({"id": 1, "type": "simulator.advance", "params": {"cycles": 1}}),
        &state,
        None,
    );
    assert!(!response.ok);
    assert_eq!(response.error_code.as_deref(), Some("clock_not_manual"));
}
