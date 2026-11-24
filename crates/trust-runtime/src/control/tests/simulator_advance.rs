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

#[test]
fn a_runtime_on_the_computers_clock_takes_a_trace_and_refuses_its_replay() {
    // Specification 11 §6.8.1, "Replaying a trace": every runtime but a simulator runs on the wall
    // clock and refuses `simulator.replay` as `clock_not_manual`, after taking the trace's upload
    // (`"for": "trace"`), so that every runtime answers it alike (specification 34 STATE-08).
    // Catches an upload for a trace refused as an unknown purpose, and a computer's runtime that
    // does not route the request or replays on its wall clock.
    use base64::Engine as _;

    let state = control_test_state(
        "PROGRAM Main VAR count : INT := 0; END_VAR count := count + 1; END_PROGRAM",
    );
    let trace = b"TRSTT001 a trace";
    let digest: String = trust_runtime_core::deployment::sha256(trace)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let begun = handle_request_value(
        json!({"id": 1, "type": "transfer.begin", "params": {
            "bytes": trace.len(), "sha256": digest, "for": "trace"}}),
        &state,
        None,
    );
    assert!(begun.ok, "{begun:?}");
    let transfer = begun.result.as_ref().expect("a begun upload")["transfer"].clone();
    let sent = handle_request_value(
        json!({"id": 2, "type": "transfer.part", "params": {
            "transfer": transfer, "offset": 0,
            "data": base64::engine::general_purpose::STANDARD.encode(trace),
            "crc32": format!("{:08x}", trust_runtime_core::crc32::checksum(trace))}}),
        &state,
        None,
    );
    assert!(sent.ok, "{sent:?}");
    let replayed = handle_request_value(
        json!({"id": 3, "type": "simulator.replay", "params": {"transfer": transfer}}),
        &state,
        None,
    );
    assert!(!replayed.ok);
    assert_eq!(replayed.error_code.as_deref(), Some("clock_not_manual"));
}
