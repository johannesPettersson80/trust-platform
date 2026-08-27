fn experiment_state(value: i64, forced: bool) -> ControlState {
    let state = hmi_test_state("PROGRAM Main\nEND_PROGRAM\n");
    let address = crate::io::IoAddress::parse("%MW0").expect("experiment address");
    *state.io_snapshot.lock().expect("I/O snapshot") = Some(crate::io::IoSnapshot {
        scan: Some(7),
        forced: if forced { vec![address.clone()] } else { Vec::new() },
        memory: vec![crate::io::IoSnapshotEntry {
            name: Some(SmolStr::new("Setpoint")),
            address,
            value_type: None,
            value: crate::io::IoSnapshotValue::Value(Value::LInt(value)),
            source: None,
        }],
        ..crate::io::IoSnapshot::default()
    });
    state.io_snapshot_seen_ms.store(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_millis() as u64,
        std::sync::atomic::Ordering::Relaxed,
    );
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
        "stop_on_stale_ms": 500
    });
    if let (Some(base), Some(changes)) = (params.as_object_mut(), overrides.as_object()) {
        base.extend(changes.clone());
    }
    handle_request_value(json!({
        "id": 8001,
        "type": "control.experiment.start",
        "params": params
    }), state, None)
}

#[test]
fn control_studio_experiment_starts_once_and_abort_restores_exact_value() {
    let state = experiment_state(10, false);
    let started = start_experiment(&state, json!({}));
    assert!(started.ok, "start failed: {:?}", started.error);
    assert_eq!(
        started.result.as_ref().and_then(|value| value.get("status")).and_then(serde_json::Value::as_str),
        Some("running")
    );
    assert_eq!(state.debug.drain_io_writes(), vec![(
        crate::io::IoAddress::parse("%MW0").unwrap(),
        Value::LInt(15),
    )]);

    let duplicate = start_experiment(&state, json!({}));
    assert!(!duplicate.ok);
    assert_eq!(duplicate.error_code.as_deref(), Some("experiment_active"));

    let aborted = handle_request_value(json!({
        "id": 8002,
        "type": "control.experiment.abort",
        "params": { "experiment_id": "experiment-1" }
    }), &state, None);
    assert!(aborted.ok, "abort failed: {:?}", aborted.error);
    assert_eq!(state.debug.drain_io_writes(), vec![(
        crate::io::IoAddress::parse("%MW0").unwrap(),
        Value::LInt(10),
    )]);
    assert_eq!(
        aborted.result.as_ref().and_then(|value| value.get("stop_reason")).and_then(serde_json::Value::as_str),
        Some("user_abort")
    );
}

#[test]
fn control_studio_experiment_rejects_unknown_or_unsafe_preconditions() {
    let advanced = start_experiment(&experiment_state(10, false), json!({ "expected_scan": 6 }));
    assert!(advanced.ok, "a newer unchanged scan must remain valid: {:?}", advanced.error);

    let forced = start_experiment(&experiment_state(10, true), json!({}));
    assert!(!forced.ok);
    assert_eq!(forced.error_code.as_deref(), Some("forced_signal"));

    let stale_state = experiment_state(10, false);
    stale_state.io_snapshot_seen_ms.store(0, std::sync::atomic::Ordering::Relaxed);
    let stale = start_experiment(&stale_state, json!({}));
    assert!(!stale.ok);
    assert_eq!(stale.error_code.as_deref(), Some("stale_scan"));

    let outside = start_experiment(&experiment_state(10, false), json!({ "value": 101 }));
    assert!(!outside.ok);
    assert_eq!(outside.error_code.as_deref(), Some("outside_limits"));

    let physical = start_experiment(&experiment_state(10, false), json!({ "source": "physical" }));
    assert!(!physical.ok);
    assert_eq!(physical.error_code.as_deref(), Some("physical_approval_required"));
}

#[test]
fn control_studio_experiment_timeout_restores_without_an_editor_request() {
    let state = experiment_state(10, false);
    let started = start_experiment(&state, json!({ "duration_ms": 20 }));
    assert!(started.ok, "start failed: {:?}", started.error);
    let _ = state.debug.drain_io_writes();
    std::thread::sleep(std::time::Duration::from_millis(80));
    assert_eq!(state.debug.drain_io_writes(), vec![(
        crate::io::IoAddress::parse("%MW0").unwrap(),
        Value::LInt(10),
    )]);
    let status = handle_request_value(json!({
        "id": 8003,
        "type": "control.experiment.status",
        "params": { "experiment_id": "experiment-1" }
    }), &state, None);
    assert!(status.ok, "status failed: {:?}", status.error);
    assert_eq!(
        status.result.as_ref().and_then(|value| value.get("status")).and_then(serde_json::Value::as_str),
        Some("completed")
    );
    assert_eq!(
        status.result.as_ref().and_then(|value| value.get("stop_reason")).and_then(serde_json::Value::as_str),
        Some("duration_complete")
    );
}

#[test]
fn control_studio_experiment_preserves_declared_lreal_signal_type() {
    let state = hmi_test_state("PROGRAM Main\nEND_PROGRAM\n");
    let address = crate::io::IoAddress::parse("%ML0").expect("experiment address");
    *state.io_snapshot.lock().expect("I/O snapshot") = Some(crate::io::IoSnapshot {
        scan: Some(11),
        memory: vec![crate::io::IoSnapshotEntry {
            name: Some(SmolStr::new("Setpoint")),
            address: address.clone(),
            value_type: Some(trust_hir::TypeId::LREAL),
            value: crate::io::IoSnapshotValue::Value(Value::LReal(0.0)),
            source: None,
        }],
        ..crate::io::IoSnapshot::default()
    });
    state.io_snapshot_seen_ms.store(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_millis() as u64,
        std::sync::atomic::Ordering::Relaxed,
    );

    let started = handle_request_value(json!({
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
            "stop_on_stale_ms": 500,
            "controller_mode": "Auto"
        }
    }), &state, None);

    assert!(started.ok, "typed LREAL start failed: {:?}", started.error);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(address.clone(), Value::LWord(5.0_f64.to_bits()))]
    );

    let aborted = handle_request_value(json!({
        "id": 8011,
        "type": "control.experiment.abort",
        "params": { "experiment_id": "lreal-step" }
    }), &state, None);
    assert!(aborted.ok, "typed LREAL abort failed: {:?}", aborted.error);
    assert_eq!(
        state.debug.drain_io_writes(),
        vec![(address, Value::LWord(0.0_f64.to_bits()))]
    );
}
