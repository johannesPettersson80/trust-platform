fn experiment_state(value: i64, forced: bool) -> ControlState {
    // `Main.Manual` is the loop's mode: TRUE is the Manual value for output kinds and not Manual for setpoint kinds.
    let state = control_test_state("PROGRAM Main\nVAR Manual : BOOL := TRUE; END_VAR\nEND_PROGRAM\n");
    state.connect_experiments();
    // The target is simulated, as the scan of this fixture reports (Specification 35, A2.4).
    state.experiments.record_simulated_target(true);
    let address = crate::io::IoAddress::parse("%MW0").expect("experiment address");
    *state.io_snapshot.lock().expect("I/O snapshot") = Some(crate::io::IoSnapshot {
        scan: Some(7),
        forced: if forced {
            vec![address.clone()]
        } else {
            Vec::new()
        },
        memory: vec![
            crate::io::IoSnapshotEntry {
                name: Some(SmolStr::new("Setpoint")),
                address,
                value_type: Some(trust_hir::TypeId::LINT),
                value_type_name: None,
                value: crate::io::IoSnapshotValue::Value(Value::LInt(value)),
                source: None,
            },
            crate::io::IoSnapshotEntry {
                name: Some(SmolStr::new("ProcessValue")),
                address: crate::io::IoAddress::parse("%MW2").expect("relay feedback address"),
                value_type: Some(trust_hir::TypeId::LINT),
                value_type_name: None,
                value: crate::io::IoSnapshotValue::Value(Value::LInt(50)),
                source: None,
            },
        ],
        ..crate::io::IoSnapshot::default()
    });
    state
}

fn start_experiment(state: &ControlState, overrides: serde_json::Value) -> ControlResponse {
    let mut params = json!({
        "experiment_id": "experiment-1",
        "source": "simulation",
        "kind": "setpoint_step",
        "address": "%MW0",
        "value": 15,
        "restoration_value": 10,
        "minimum": 0,
        "maximum": 100,
        "duration_ms": 5_000,
        "expected_scan": 7,
        "task": "$background"
    });
    if let (Some(base), Some(changes)) = (params.as_object_mut(), overrides.as_object()) {
        base.extend(changes.clone());
    }
    // Output kinds need Manual (`Main.Manual` equal to the Manual value); setpoint kinds need Auto.
    let manual_required = matches!(
        params["kind"].as_str(),
        Some("output_step" | "mv_step" | "mv_pulse" | "relay" | "prbs" | "multisine")
    );
    params["mode_binding"] = json!({ "symbol": "Main.Manual", "manual_value": manual_required });
    handle_request_value(
        json!({
            "id": 8001,
            "type": "control.experiment.start",
            "params": params
        }),
        state,
        None,
    )
}

#[test]
fn control_studio_experiment_starts_once_and_abort_restores_exact_value() {
    let state = experiment_state(10, false);
    let started = start_experiment(&state, json!({}));
    assert!(started.ok, "start failed: {:?}", started.error);
    assert_eq!(
        started
            .result
            .as_ref()
            .and_then(|value| value.get("status"))
            .and_then(serde_json::Value::as_str),
        Some("running")
    );
    assert!(
        state.debug.drain_io_writes().is_empty(),
        "the start request writes nothing"
    );
    experiment_scan(&state, 0);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(
            crate::io::IoAddress::parse("%MW0").unwrap(),
            Value::LInt(15),
        )]
    );

    let duplicate = start_experiment(&state, json!({}));
    assert!(!duplicate.ok);
    assert_eq!(duplicate.error_code.as_deref(), Some("experiment_active"));

    let aborted = handle_request_value(
        json!({
            "id": 8002,
            "type": "control.experiment.abort",
            "params": { "experiment_id": "experiment-1" }
        }),
        &state,
        None,
    );
    assert!(aborted.ok, "abort failed: {:?}", aborted.error);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(
            crate::io::IoAddress::parse("%MW0").unwrap(),
            Value::LInt(10),
        )]
    );
    assert_eq!(
        aborted
            .result
            .as_ref()
            .and_then(|value| value.get("stop_reason"))
            .and_then(serde_json::Value::as_str),
        Some("user_abort")
    );
}

#[test]
fn control_studio_project_sequence_uses_and_restores_boolean_authority() {
    let state = experiment_state(10, false);
    let address = crate::io::IoAddress::parse("%MX8.0").unwrap();
    state
        .io_snapshot
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .memory
        .push(crate::io::IoSnapshotEntry {
            name: Some(SmolStr::new("ExperimentEnabled")),
            address: address.clone(),
            value_type: Some(trust_hir::TypeId::BOOL),
            value_type_name: None,
            value: crate::io::IoSnapshotValue::Value(Value::Bool(false)),
            source: None,
        });
    let started = start_experiment(
        &state,
        json!({
            "kind": "project_sequence",
            "address": "%MX8.0",
            "value": true,
            "restoration_value": false,
            "value_type": "BOOL",
            "minimum": 0,
            "maximum": 1,
            "duration_ms": 2_000
        }),
    );
    assert!(
        started.ok,
        "project sequence start failed: {:?}",
        started.error
    );
    assert!(
        state.debug.drain_io_writes().is_empty(),
        "the start request writes nothing"
    );
    experiment_scan(&state, 0);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(address.clone(), Value::Bool(true))]
    );

    let aborted = handle_request_value(
        json!({
            "id": 8004,
            "type": "control.experiment.abort",
            "params": { "experiment_id": "experiment-1" }
        }),
        &state,
        None,
    );
    assert!(
        aborted.ok,
        "project sequence abort failed: {:?}",
        aborted.error
    );
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(address, Value::Bool(false))]
    );
}

#[test]
fn control_studio_experiment_rejects_unknown_or_unsafe_preconditions() {
    let advanced = start_experiment(&experiment_state(10, false), json!({ "expected_scan": 6 }));
    assert!(
        advanced.ok,
        "a newer unchanged scan must remain valid: {:?}",
        advanced.error
    );

    let forced = start_experiment(&experiment_state(10, true), json!({}));
    assert!(!forced.ok);
    assert_eq!(forced.error_code.as_deref(), Some("forced_signal"));

    let outside = start_experiment(&experiment_state(10, false), json!({ "value": 101 }));
    assert!(!outside.ok);
    assert_eq!(outside.error_code.as_deref(), Some("outside_limits"));

    // The fixture's target is simulated, so a physical label contradicts the runtime's fact (A2.4).
    let physical = start_experiment(
        &experiment_state(10, false),
        json!({ "source": "physical" }),
    );
    assert!(!physical.ok);
    assert_eq!(physical.error_code.as_deref(), Some("invalid_target"));
}

#[test]
fn control_studio_experiment_timeout_restores_without_an_editor_request() {
    let state = experiment_state(10, false);
    let started = start_experiment(&state, json!({ "duration_ms": 1_000 }));
    assert!(started.ok, "start failed: {:?}", started.error);
    let _ = state.debug.drain_io_writes();
    for time in (0..=990).step_by(10) {
        experiment_scan(&state, time);
    }
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![
            (crate::io::IoAddress::parse("%MW0").unwrap(), Value::LInt(15)),
            (crate::io::IoAddress::parse("%MW0").unwrap(), Value::LInt(10)),
        ]
    );
    let status = handle_request_value(
        json!({
            "id": 8003,
            "type": "control.experiment.status",
            "params": { "experiment_id": "experiment-1" }
        }),
        &state,
        None,
    );
    assert!(status.ok, "status failed: {:?}", status.error);
    assert_eq!(
        status
            .result
            .as_ref()
            .and_then(|value| value.get("status"))
            .and_then(serde_json::Value::as_str),
        Some("completed")
    );
    assert_eq!(
        status
            .result
            .as_ref()
            .and_then(|value| value.get("stop_reason"))
            .and_then(serde_json::Value::as_str),
        Some("duration_complete")
    );
}

#[test]
fn control_studio_experiment_accepts_bounded_slow_process_duration() {
    let state = experiment_state(10, false);
    let started = start_experiment(&state, json!({ "duration_ms": 1_200_000 }));
    assert!(
        started.ok,
        "slow-process experiment was rejected: {:?}",
        started.error
    );

    let aborted = handle_request_value(
        json!({
            "id": 2,
            "type": "control.experiment.abort",
            "params": { "experiment_id": "experiment-1" }
        }),
        &state,
        None,
    );
    assert!(
        aborted.ok,
        "slow-process experiment abort failed: {:?}",
        aborted.error
    );

    let rejected = start_experiment(&state, json!({ "duration_ms": 86_400_001 }));
    assert!(!rejected.ok);
    assert_eq!(rejected.error_code.as_deref(), Some("invalid_experiment"));
}

#[test]
fn control_studio_experiment_preserves_declared_lreal_signal_type() {
    let state = control_test_state("PROGRAM Main\nVAR Manual : BOOL := TRUE; END_VAR\nEND_PROGRAM\n");
    state.connect_experiments();
    state.experiments.record_simulated_target(true);
    let address = crate::io::IoAddress::parse("%ML0").expect("experiment address");
    *state.io_snapshot.lock().expect("I/O snapshot") = Some(crate::io::IoSnapshot {
        scan: Some(11),
        memory: vec![crate::io::IoSnapshotEntry {
            name: Some(SmolStr::new("Setpoint")),
            address: address.clone(),
            value_type: Some(trust_hir::TypeId::LREAL),
            value_type_name: None,
            value: crate::io::IoSnapshotValue::Value(Value::LReal(0.0)),
            source: None,
        }],
        ..crate::io::IoSnapshot::default()
    });
    let started = handle_request_value(
        json!({
            "id": 8010,
            "type": "control.experiment.start",
            "params": {
                "experiment_id": "lreal-step",
                "source": "simulation",
                "kind": "setpoint_step",
                "address": "%ML0",
                "value": 5,
                "restoration_value": 0,
                "value_type": "LREAL",
                "minimum": 0,
                "maximum": 100,
                "duration_ms": 5_000,
                "expected_scan": 11,
                "task": "$background",
                "mode_binding": { "symbol": "Main.Manual", "manual_value": false }
            }
        }),
        &state,
        None,
    );

    assert!(started.ok, "typed LREAL start failed: {:?}", started.error);
    assert!(
        state.debug.drain_io_writes().is_empty(),
        "the start request writes nothing"
    );
    experiment_scan(&state, 0);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(address.clone(), Value::LWord(5.0_f64.to_bits()))]
    );

    let aborted = handle_request_value(
        json!({
            "id": 8011,
            "type": "control.experiment.abort",
            "params": { "experiment_id": "lreal-step" }
        }),
        &state,
        None,
    );
    assert!(aborted.ok, "typed LREAL abort failed: {:?}", aborted.error);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(address, Value::LWord(0.0_f64.to_bits()))]
    );
}

#[test]
fn control_studio_experiment_executes_bounded_trajectory_and_restores_once() {
    let state = experiment_state(10, false);
    let started = start_experiment(
        &state,
        json!({
            "kind": "prbs",
            "duration_ms": 1_000,
            "trajectory": [
                { "at_ms": 0, "value": 15 },
                { "at_ms": 30, "value": 5 },
                { "at_ms": 60, "value": 20 }
            ]
        }),
    );
    assert!(started.ok, "trajectory start failed: {:?}", started.error);
    for time in (0..=50).step_by(10) {
        experiment_scan(&state, time);
    }
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![
            (
                crate::io::IoAddress::parse("%MW0").unwrap(),
                Value::LInt(15)
            ),
            (crate::io::IoAddress::parse("%MW0").unwrap(), Value::LInt(5)),
            (
                crate::io::IoAddress::parse("%MW0").unwrap(),
                Value::LInt(20)
            ),
        ]
    );
    let aborted = handle_request_value(
        json!({
            "id": 8012,
            "type": "control.experiment.abort",
            "params": { "experiment_id": "experiment-1" }
        }),
        &state,
        None,
    );
    assert!(aborted.ok);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(
            crate::io::IoAddress::parse("%MW0").unwrap(),
            Value::LInt(10),
        )]
    );
}

#[test]
fn control_studio_mv_pulse_requires_a_timed_return_to_its_baseline() {
    let state = experiment_state(10, false);
    let rejected = start_experiment(
        &state,
        json!({
            "kind": "mv_pulse",
            "trajectory": [{ "at_ms": 0, "value": 15 }]
        }),
    );
    assert!(!rejected.ok);
    assert_eq!(rejected.error_code.as_deref(), Some("invalid_experiment"));

    let started = start_experiment(
        &state,
        json!({
            "kind": "mv_pulse",
            "trajectory": [
                { "at_ms": 0, "value": 15 },
                { "at_ms": 2500, "value": 10 }
            ]
        }),
    );
    assert!(started.ok, "pulse start failed: {:?}", started.error);
}

#[test]
fn control_studio_relay_switches_from_process_feedback_and_never_from_elapsed_dwell() {
    let state = experiment_state(10, false);
    let started = start_experiment(
        &state,
        json!({
            "kind": "relay",
            "value": 15,
            "duration_ms": 1_000,
            "relay_feedback": {
                "address": "%MW2",
                "value_type": "LINT",
                "center": 50,
                "hysteresis": 2,
                "low_value": 5,
                "high_value": 15
            }
        }),
    );
    assert!(
        started.ok,
        "feedback relay start failed: {:?}",
        started.error
    );
    assert!(
        state.debug.drain_io_writes().is_empty(),
        "the start request writes nothing"
    );
    experiment_scan(&state, 0);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(
            crate::io::IoAddress::parse("%MW0").unwrap(),
            Value::LInt(15),
        )]
    );

    experiment_scan(&state, 10);
    assert!(
        state.debug.drain_io_writes().is_empty(),
        "elapsed time alone must not switch a feedback relay"
    );

    state.io_snapshot.lock().unwrap().as_mut().unwrap().memory[1].value =
        crate::io::IoSnapshotValue::Value(Value::LInt(53));
    experiment_scan(&state, 20);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(crate::io::IoAddress::parse("%MW0").unwrap(), Value::LInt(5),)]
    );

    state.io_snapshot.lock().unwrap().as_mut().unwrap().memory[1].value =
        crate::io::IoSnapshotValue::Value(Value::LInt(51));
    experiment_scan(&state, 30);
    assert!(
        state.debug.drain_io_writes().is_empty(),
        "the relay must retain its low state inside hysteresis"
    );

    state.io_snapshot.lock().unwrap().as_mut().unwrap().memory[1].value =
        crate::io::IoSnapshotValue::Value(Value::LInt(47));
    experiment_scan(&state, 40);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(
            crate::io::IoAddress::parse("%MW0").unwrap(),
            Value::LInt(15),
        )]
    );

    let aborted = handle_request_value(
        json!({
            "id": 80120,
            "type": "control.experiment.abort",
            "params": { "experiment_id": "experiment-1" }
        }),
        &state,
        None,
    );
    assert!(
        aborted.ok,
        "feedback relay abort failed: {:?}",
        aborted.error
    );
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(
            crate::io::IoAddress::parse("%MW0").unwrap(),
            Value::LInt(10),
        )]
    );
}

#[test]
fn control_studio_observation_never_writes_or_restores_a_signal() {
    let state = experiment_state(10, false);
    let started = start_experiment(
        &state,
        json!({
            "kind": "observation",
            "value": 10,
            "trajectory": []
        }),
    );
    assert!(started.ok, "observation start failed: {:?}", started.error);
    assert!(state.debug.drain_io_writes().is_empty());
    let aborted = handle_request_value(
        json!({
            "id": 8013,
            "type": "control.experiment.abort",
            "params": { "experiment_id": "experiment-1" }
        }),
        &state,
        None,
    );
    assert!(aborted.ok);
    assert!(state.debug.drain_io_writes().is_empty());
}

#[test]
fn control_studio_observation_accepts_a_process_value_that_changed_after_review() {
    let state = experiment_state(11, false);
    let started = start_experiment(
        &state,
        json!({
            "kind": "observation",
            "value": 10,
            "restoration_value": 10,
            "trajectory": []
        }),
    );

    assert!(
        started.ok,
        "a read-only observation must not reject a normally changing process value: {:?}",
        started.error
    );
    assert!(state.debug.drain_io_writes().is_empty());
}

#[test]
fn control_studio_abort_preempts_future_excitation_and_confirms_restoration_from_io() {
    let state = experiment_state(10, false);
    let started = start_experiment(
        &state,
        json!({
            "duration_ms": 1_000,
            "trajectory": [
                { "at_ms": 0, "value": 15 },
                { "at_ms": 100, "value": 20 }
            ]
        }),
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    assert!(
        state.debug.drain_io_writes().is_empty(),
        "the start request writes nothing"
    );
    experiment_scan(&state, 0);
    assert_eq!(state.debug.drain_io_writes().len(), 1);

    let aborted = handle_request_value(
        json!({
            "id": 8014,
            "type": "control.experiment.abort",
            "params": { "experiment_id": "experiment-1" }
        }),
        &state,
        None,
    );
    assert!(aborted.ok, "abort failed: {:?}", aborted.error);
    assert_eq!(
        aborted
            .result
            .as_ref()
            .and_then(|value| value.get("restoration"))
            .and_then(serde_json::Value::as_str),
        Some("pending")
    );
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(
            crate::io::IoAddress::parse("%MW0").unwrap(),
            Value::LInt(10),
        )]
    );

    experiment_scan(&state, 10);
    assert!(
        state.debug.drain_io_writes().is_empty(),
        "no trajectory point may be queued after abort is accepted"
    );
    state.io_snapshot.lock().unwrap().as_mut().unwrap().memory[0].value =
        crate::io::IoSnapshotValue::Value(Value::LInt(10));
    experiment_scan(&state, 20);
    let status = handle_request_value(
        json!({
            "id": 8015,
            "type": "control.experiment.status",
            "params": { "experiment_id": "experiment-1" }
        }),
        &state,
        None,
    );
    assert_eq!(
        status
            .result
            .as_ref()
            .and_then(|value| value.get("status"))
            .and_then(serde_json::Value::as_str),
        Some("aborted")
    );
    assert_eq!(
        status
            .result
            .as_ref()
            .and_then(|value| value.get("restoration"))
            .and_then(serde_json::Value::as_str),
        Some("restored")
    );
}

#[test]
fn control_studio_runtime_data_trip_stops_and_failed_restoration_is_explicit() {
    let state = experiment_state(10, false);
    let started = start_experiment(
        &state,
        json!({
            "duration_ms": 2_000
        }),
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    assert!(
        state.debug.drain_io_writes().is_empty(),
        "the start request writes nothing"
    );
    experiment_scan(&state, 0);
    let _ = state.debug.drain_io_writes();
    state.io_snapshot.lock().unwrap().as_mut().unwrap().memory[0].value =
        crate::io::IoSnapshotValue::Value(Value::LInt(101));

    experiment_scan(&state, 10);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(
            crate::io::IoAddress::parse("%MW0").unwrap(),
            Value::LInt(10),
        )]
    );
    for time in [20, 30, 40] {
        experiment_scan(&state, time);
    }
    let status = handle_request_value(
        json!({
            "id": 8016,
            "type": "control.experiment.status",
            "params": { "experiment_id": "experiment-1" }
        }),
        &state,
        None,
    );
    assert_eq!(
        status
            .result
            .as_ref()
            .and_then(|value| value.get("status"))
            .and_then(serde_json::Value::as_str),
        Some("failed_to_restore")
    );
    assert_eq!(
        status
            .result
            .as_ref()
            .and_then(|value| value.get("stop_reason"))
            .and_then(serde_json::Value::as_str),
        Some("signal_outside_limits")
    );
    assert_eq!(
        status
            .result
            .as_ref()
            .and_then(|value| value.get("restoration"))
            .and_then(serde_json::Value::as_str),
        Some("failed")
    );
}

#[test]
fn control_studio_unresolved_runtime_value_trips_the_running_experiment() {
    let state = experiment_state(10, false);
    assert!(start_experiment(&state, json!({ "duration_ms": 2_000 })).ok);
    experiment_scan(&state, 0);
    let _ = state.debug.drain_io_writes();
    state.io_snapshot.lock().unwrap().as_mut().unwrap().memory[0].value =
        crate::io::IoSnapshotValue::Unresolved;
    experiment_scan(&state, 10);
    let status = handle_request_value(
        json!({
            "id": 8017,
            "type": "control.experiment.status",
            "params": { "experiment_id": "experiment-1" }
        }),
        &state,
        None,
    );
    assert_eq!(
        status
            .result
            .as_ref()
            .and_then(|value| value.get("stop_reason"))
            .and_then(serde_json::Value::as_str),
        Some("bad_runtime_data")
    );
}

#[test]
fn control_studio_running_experiment_trips_on_force() {
    let forced_state = experiment_state(10, false);
    assert!(start_experiment(&forced_state, json!({ "duration_ms": 2_000 })).ok);
    assert!(
        forced_state.debug.drain_io_writes().is_empty(),
        "the start request writes nothing"
    );
    experiment_scan(&forced_state, 0);
    let _ = forced_state.debug.drain_io_writes();
    forced_state
        .io_snapshot
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .forced = vec![crate::io::IoAddress::parse("%MW0").unwrap()];
    experiment_scan(&forced_state, 10);
    experiment_scan(&forced_state, 20);
    let forced_status = handle_request_value(
        json!({
            "id": 8018,
            "type": "control.experiment.status",
            "params": { "experiment_id": "experiment-1" }
        }),
        &forced_state,
        None,
    );
    assert_eq!(
        forced_status
            .result
            .as_ref()
            .and_then(|value| value.get("stop_reason"))
            .and_then(serde_json::Value::as_str),
        Some("signal_forced")
    );
    assert_eq!(
        forced_status
            .result
            .as_ref()
            .and_then(|value| value.get("restoration"))
            .and_then(serde_json::Value::as_str),
        Some("restored")
    );
}

/// Publishes one scan of the fixture's I/O at `time_ms` of process time, as the runtime's cycle publishes it. The scan runs
/// the experiments, so a test advances them by publishing scans rather than by waiting (Specification 35, A3.5).
fn experiment_scan(state: &ControlState, time_ms: i64) {
    let io = state
        .io_snapshot
        .lock()
        .expect("I/O snapshot")
        .clone()
        .expect("fixture I/O");
    let storage = state
        .debug
        .with_snapshot(|snapshot| snapshot.storage.clone())
        .expect("debug snapshot");
    state
        .debug
        .push_control_snapshot_at(io, time_ms * 1_000_000, &storage, &[], true, true);
}
