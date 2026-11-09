/// The status of the fixture's experiment, `experiment-1`.
fn guarded_status(state: &ControlState) -> serde_json::Value {
    let status = handle_request_value(
        json!({
            "id": 8101,
            "type": "control.experiment.status",
            "params": { "experiment_id": "experiment-1" }
        }),
        state,
        None,
    );
    status.result.expect("the experiment's status")
}

/// Sets the fixture's measured output, `%MW2`, for the next published scan.
fn set_measurement(state: &ControlState, value: i64) {
    state.io_snapshot.lock().unwrap().as_mut().unwrap().memory[1].value =
        crate::io::IoSnapshotValue::Value(Value::LInt(value));
}

/// Runtime protocol annex E5.4: the measured output `%MW2` reads 50 at the start and must stay within 0 to 80. It reads 85
/// at the second execution, so the experiment fails with `measurement_outside_limits`, the restoring 10 is written at that
/// execution, the target's reading of 10 confirms it at the next, and the status keeps 85 as the value that stopped it.
/// Wrong behaviour caught: a runtime that watches only its written target and holds the step while the measurement runs
/// out of its band (the CODESYS toolkit's S12).
#[test]
fn control_studio_measurement_guard_stops_the_experiment_and_restores_its_target() {
    let state = experiment_state(10, false);
    let started = start_experiment(
        &state,
        json!({
            "duration_ms": 2_000,
            "measurement_guard": { "address": "%MW2", "minimum": 0, "maximum": 80 }
        }),
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    assert_eq!(
        guarded_status(&state)["measurement_guard"],
        json!({ "address": "%MW2", "minimum": 0.0, "maximum": 80.0, "value": 50.0 }),
        "the start reads the measurement"
    );
    experiment_scan(&state, 0);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(
            crate::io::IoAddress::parse("%MW0").unwrap(),
            Value::LInt(15),
        )],
        "inside its band the step is written"
    );
    set_measurement(&state, 85);
    experiment_scan(&state, 10);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(
            crate::io::IoAddress::parse("%MW0").unwrap(),
            Value::LInt(10),
        )],
        "the stop writes the restoration"
    );
    experiment_scan(&state, 20);
    let status = guarded_status(&state);
    assert_eq!(status["status"], json!("failed"));
    assert_eq!(status["stop_reason"], json!("measurement_outside_limits"));
    assert_eq!(status["restoration"], json!("restored"));
    assert_eq!(
        status["measurement_guard"],
        json!({ "address": "%MW2", "minimum": 0.0, "maximum": 80.0, "value": 85.0 })
    );
}

/// E5.4: a guarded measurement that has no value any more stops the experiment as bad runtime data. Wrong behaviour
/// caught: a guard that reads a missing measurement as inside its band.
#[test]
fn control_studio_unreadable_guarded_measurement_stops_as_bad_runtime_data() {
    let state = experiment_state(10, false);
    assert!(
        start_experiment(
            &state,
            json!({
                "duration_ms": 2_000,
                "measurement_guard": { "address": "%MW2", "minimum": 0, "maximum": 80 }
            }),
        )
        .ok
    );
    experiment_scan(&state, 0);
    let _ = state.debug.drain_io_writes();
    state.io_snapshot.lock().unwrap().as_mut().unwrap().memory[1].value =
        crate::io::IoSnapshotValue::Unresolved;
    experiment_scan(&state, 10);
    let status = guarded_status(&state);
    assert_eq!(status["stop_reason"], json!("bad_runtime_data"));
    assert_eq!(status["measurement_guard"]["value"], json!(null));
}

/// E5.3 check 8, worked from the fixture (the measurement reads 50): a band of 60 to 80 already excludes it
/// (`outside_limits`); a band whose minimum is not below its maximum, and a guard naming both an address and a symbol, are
/// refused; an address the scan does not hold has no value (`invalid_target`). A guard within its band starts, and an
/// experiment without one reports none. Wrong behaviour caught: an experiment started while its measurement is already
/// outside the band, or with a guard it can never read.
#[test]
fn control_studio_measurement_guard_is_checked_at_the_start() {
    let refusal = |guard: serde_json::Value| {
        let response = start_experiment(
            &experiment_state(10, false),
            json!({ "measurement_guard": guard }),
        );
        assert!(!response.ok, "{guard} must be refused");
        response.error_code
    };
    assert_eq!(
        refusal(json!({ "address": "%MW2", "minimum": 60, "maximum": 80 })).as_deref(),
        Some("outside_limits")
    );
    assert_eq!(
        refusal(json!({ "address": "%MW2", "minimum": 80, "maximum": 80 })).as_deref(),
        Some("invalid_experiment")
    );
    assert_eq!(
        refusal(json!({ "address": "%MW2", "symbol": "Main.Manual", "minimum": 0, "maximum": 80 }))
            .as_deref(),
        Some("invalid_target")
    );
    assert_eq!(
        refusal(json!({ "address": "%MW40", "minimum": 0, "maximum": 80 })).as_deref(),
        Some("invalid_target")
    );
    let unguarded = experiment_state(10, false);
    assert!(start_experiment(&unguarded, json!({})).ok);
    assert_eq!(guarded_status(&unguarded)["measurement_guard"], json!(null));
}
