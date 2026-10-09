/// `control.status` reports the runtime's own session epoch, task intervals and cycle interval
/// (specification 35, A2.2 and A2.3), so Control Studio never takes them from editor settings.
#[test]
fn control_status_reports_the_session_epoch_task_intervals_and_cycle_interval() {
    let source = r#"
CONFIGURATION Cell
RESOURCE Cpu ON PLC
TASK Loop (INTERVAL := T#50ms, PRIORITY := 1);
PROGRAM Main WITH Loop : Counter;
END_RESOURCE
END_CONFIGURATION
PROGRAM Counter
VAR Output AT %QD0 : DINT := 0; END_VAR
Output := Output + 1;
END_PROGRAM
"#;
    let state = control_test_state(source);
    let status = |state: &ControlState| {
        let response = handle_request_value(json!({"id": 9301, "type": "status"}), state, None);
        assert!(response.ok, "status failed: {:?}", response.error);
        response.result.expect("status result")
    };
    let result = status(&state);
    assert_eq!(result["session_epoch"], json!(state.debug.runtime_epoch()), "{result}");
    assert_eq!(result["cycle_interval_ms"], json!(10), "{result}");
    let tasks = result["tasks"].as_array().expect("tasks");
    assert_eq!(tasks.len(), 1, "{result}");
    assert!(
        tasks[0]["name"]
            .as_str()
            .is_some_and(|name| name.eq_ignore_ascii_case("loop")),
        "{result}"
    );
    assert_eq!(tasks[0]["interval_ms"], json!(50), "{result}");
    // A restart or a program reload begins a new session epoch (A2.2).
    state.debug.clear_runtime_mutations();
    let after = status(&state);
    assert_ne!(after["session_epoch"], result["session_epoch"], "{after}");
}
