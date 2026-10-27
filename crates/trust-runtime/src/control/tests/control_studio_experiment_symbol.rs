// Experiments by variable name (Specification 35): Control Studio steps a program variable, such
// as a loop's manual output or setpoint, without a process-image address. The variable is written
// between scans and restored like an address.

const SYMBOL_EXPERIMENT_SOURCE: &str = r#"
PROGRAM Main
VAR
    ManualOutput : LREAL := 12.5;
    Setpoint : REAL := 20.0;
    Count : DINT := 3;
    Wired AT %MD8 : DINT;
    Flag : BOOL;
    Mode : BOOL := TRUE;
END_VAR
VAR CONSTANT
    Fixed : LREAL := 1.0;
END_VAR
END_PROGRAM
"#;

fn with_fresh_scan(state: &ControlState) {
    *state.io_snapshot.lock().expect("I/O snapshot") = Some(crate::io::IoSnapshot {
        scan: Some(7),
        ..crate::io::IoSnapshot::default()
    });
}

fn symbol_experiment_state() -> ControlState {
    let state = control_test_state(SYMBOL_EXPERIMENT_SOURCE);
    state.connect_experiments();
    state.experiments.record_simulated_target(true);
    with_fresh_scan(&state);
    state
}

fn start_symbol_experiment(state: &ControlState, overrides: serde_json::Value) -> ControlResponse {
    let mut params = json!({
        "experiment_id": "symbol-1",
        "source": "simulation",
        "kind": "output_step",
        "task": "$background",
        "symbol": "Main.ManualOutput",
        "value": 40.0,
        "restoration_value": 12.5,
        "minimum": 0,
        "maximum": 100,
        "duration_ms": 5_000,
        "expected_scan": 7
    });
    if let (Some(base), Some(changes)) = (params.as_object_mut(), overrides.as_object()) {
        base.extend(changes.clone());
    }
    // The loop's mode: `Main.Mode` is TRUE, the Manual value for output kinds and not Manual for setpoint kinds.
    let manual_required = matches!(
        params["kind"].as_str(),
        Some("output_step" | "mv_step" | "mv_pulse" | "relay" | "prbs" | "multisine")
    );
    params["mode_binding"] = json!({ "symbol": "Main.Mode", "manual_value": manual_required });
    handle_request_value(
        json!({"id": 8301, "type": "control.experiment.start", "params": params}),
        state,
        None,
    )
}

#[test]
fn a_symbolic_experiment_writes_the_variable_and_restores_it_on_abort() {
    let state = symbol_experiment_state();
    let started = start_symbol_experiment(&state, json!({}));
    assert!(started.ok, "start failed: {:?}", started.error);
    let result = started.result.as_ref().unwrap();
    assert_eq!(result["status"], "running");
    assert_eq!(result["symbol"], "Main.ManualOutput");
    assert!(result.get("address").is_none());
    assert!(
        state.debug.drain_control_writes().is_empty(),
        "the start request writes nothing"
    );
    experiment_scan(&state, 0);
    let writes = state.debug.drain_control_writes();
    assert_eq!(writes.len(), 1, "{writes:?}");
    // A write of the variable at the place its path resolved to, no process image.
    assert_eq!(writes[0].label, "Main.ManualOutput");
    assert_eq!(writes[0].value, Value::LReal(40.0));
    assert!(
        state.debug.drain_io_writes().is_empty(),
        "a variable target writes no process image"
    );

    let aborted = handle_request_value(
        json!({"id": 8302, "type": "control.experiment.abort", "params": {"experiment_id": "symbol-1"}}),
        &state,
        None,
    );
    assert!(aborted.ok, "abort failed: {:?}", aborted.error);
    let aborted = aborted.result.unwrap();
    assert_eq!(aborted["symbol"], "Main.ManualOutput");
    assert_eq!(aborted["stop_reason"], "user_abort");
    let restored = state.debug.drain_control_writes();
    assert_eq!(restored.len(), 1, "{restored:?}");
    assert_eq!(restored[0].value, Value::LReal(12.5));
}

#[test]
fn a_symbolic_experiment_writes_in_the_variables_own_type() {
    let state = symbol_experiment_state();
    let started = start_symbol_experiment(
        &state,
        json!({"kind": "setpoint_step", "symbol": "Main.Setpoint", "value": 22.5, "restoration_value": 20.0}),
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    experiment_scan(&state, 0);
    let writes = state.debug.drain_control_writes();
    assert_eq!(writes[0].value, Value::Real(22.5));
    let aborted = handle_request_value(
        json!({"id": 8303, "type": "control.experiment.abort", "params": {"experiment_id": "symbol-1"}}),
        &state,
        None,
    );
    assert!(aborted.ok);

    let state = symbol_experiment_state();
    let started = start_symbol_experiment(
        &state,
        json!({"symbol": "Main.Count", "value": 5, "restoration_value": 3}),
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    experiment_scan(&state, 0);
    assert_eq!(state.debug.drain_control_writes()[0].value, Value::DInt(5));
}

#[test]
fn a_symbolic_experiment_is_refused_with_a_plain_reason() {
    for (overrides, code) in [
        (json!({"address": "%MW0"}), "invalid_target"),
        (json!({"symbol": null}), "invalid_target"),
        (json!({"symbol": "Main.Wired"}), "invalid_target"),
        (json!({"symbol": "Main.Fixed", "restoration_value": 1.0}), "invalid_target"),
        (json!({"symbol": "Main.Flag", "value": 1, "restoration_value": 0}), "invalid_target"),
        (json!({"symbol": "Main.Missing"}), "invalid_target"),
        (json!({"restoration_value": 99.0}), "restoration_mismatch"),
        (json!({"value_type": "INT", "value": 40, "restoration_value": 12}), "incompatible_binding"),
    ] {
        let state = symbol_experiment_state();
        let response = start_symbol_experiment(&state, overrides.clone());
        assert!(!response.ok, "{overrides} must be refused");
        assert_eq!(
            response.error_code.as_deref(),
            Some(code),
            "{overrides}: {:?}",
            response.error
        );
        assert!(state.debug.drain_var_writes().is_empty());
        assert!(state.debug.drain_control_writes().is_empty());
    }
}

#[test]
fn a_forced_variable_takes_no_experiment() {
    let state = symbol_experiment_state();
    state
        .debug
        .try_force_global("Main.ManualOutput", Value::LReal(1.0))
        .expect("force the variable");
    let response = start_symbol_experiment(&state, json!({}));
    assert!(!response.ok);
    assert_eq!(response.error_code.as_deref(), Some("forced_signal"));
}

#[test]
fn a_symbolic_experiment_changes_the_running_variable_and_its_restoration_is_confirmed() {
    let mut state = symbol_experiment_state();
    let mut harness = TestHarness::from_source(SYMBOL_EXPERIMENT_SOURCE).unwrap();
    state.debug = studio_debug(&mut harness);
    harness.runtime_mut().set_simulation_enabled(true);
    state.connect_experiments();
    harness.cycle();
    let started = start_symbol_experiment(&state, json!({}));
    assert!(started.ok, "start failed: {:?}", started.error);
    // The first scan queues the step, and the next scan applies it before the program runs.
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        studio_value(&harness, "Main.ManualOutput"),
        Value::LReal(40.0),
        "the step reaches the running program at the next scan"
    );

    with_fresh_scan(&state);
    let aborted = handle_request_value(
        json!({"id": 8304, "type": "control.experiment.abort", "params": {"experiment_id": "symbol-1"}}),
        &state,
        None,
    );
    assert!(aborted.ok, "abort failed: {:?}", aborted.error);
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(studio_value(&harness, "Main.ManualOutput"), Value::LReal(12.5));
    // The restoration is applied before the program runs in this cycle, and confirmed by this cycle's scan.
    let status = handle_request_value(
        json!({"id": 8305, "type": "control.experiment.status", "params": {"experiment_id": "symbol-1"}}),
        &state,
        None,
    );
    let status = status.result.expect("the experiment has a status");
    assert_eq!(status["status"], "aborted");
    assert_eq!(status["restoration"], "restored", "{status}");
}

/// An experiment runs on the runtime's clock: in a simulation that runs faster its duration passes
/// faster, so the excitation lasts the process time the plan reviewed (specification 35, §17.3).
#[test]
fn an_experiment_lasts_its_duration_on_the_runtimes_clock() {
    let state = symbol_experiment_state();
    let started = start_symbol_experiment(&state, json!({"duration_ms": 60_000}));
    assert!(started.ok, "start failed: {:?}", started.error);
    let status = || {
        handle_request_value(
            json!({"id": 8310, "type": "control.experiment.status", "params": {"experiment_id": "symbol-1"}}),
            &state,
            None,
        )
        .result
        .expect("experiment status")
    };
    experiment_scan(&state, 0);
    experiment_scan(&state, 200);
    assert_eq!(status()["status"], "running", "200 ms of process time does not end a 60 s experiment");
    // The runtime's clock runs past the duration, as a faster simulation's does: the next scan is at 61 s.
    experiment_scan(&state, 61_000);
    let last = status();
    assert_eq!(last["status"], "completed", "{last}");
    assert_eq!(last["stop_reason"], "duration_complete", "{last}");
    assert!(
        last["elapsed_ms"]
            .as_u64()
            .is_some_and(|elapsed| elapsed >= 60_000),
        "the elapsed time is the runtime's: {last}"
    );
}
