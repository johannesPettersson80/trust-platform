// Experiments exclude live writes. A transfer or parameter transaction pending at the start of an experiment refuses
// it, and an experiment that is running or restoring refuses transfers and parameter sets. A reload or a restart ends
// the runtime epoch an experiment was started in: the experiment stops with its reason, and no variable reference
// resolved against the program that ended is written.

/// The process image an experiment reads at %MW0, fresh for the next start.
fn boundary_io_snapshot(state: &ControlState, setpoint: i64) {
    // The fixtures' targets are simulated, as their scans report (Specification 35, A2.4).
    state.experiments.record_simulated_target(true);
    *state.io_snapshot.lock().expect("I/O snapshot") = Some(crate::io::IoSnapshot {
        scan: Some(7),
        memory: vec![crate::io::IoSnapshotEntry {
            name: Some(SmolStr::new("Setpoint")),
            address: crate::io::IoAddress::parse("%MW0").expect("experiment address"),
            value_type: Some(trust_hir::TypeId::LINT),
            value: crate::io::IoSnapshotValue::Value(Value::LInt(setpoint)),
            source: None,
        }],
        ..crate::io::IoSnapshot::default()
    });
}

/// An observation never writes, so it holds the experiment lock without moving any output.
fn boundary_observation(state: &ControlState) -> ControlResponse {
    start_experiment(
        state,
        json!({"kind": "observation", "value": 10, "restoration_value": 10,
            "trajectory": []}),
    )
}

fn boundary_status(state: &ControlState, experiment_id: &str) -> serde_json::Value {
    let response = handle_request_value(
        json!({"id": 8810, "type": "control.experiment.status", "params": {"experiment_id": experiment_id}}),
        state,
        None,
    );
    assert!(response.ok, "{:?}", response.error);
    response.result.unwrap()
}

#[test]
fn experiment_start_refuses_while_a_transfer_or_parameter_transaction_is_pending() {
    let (transfer, _transfer_harness) = transfer_fixture();
    boundary_io_snapshot(&transfer, 10);
    let before = transfer_status(&transfer);
    assert!(queue_transfer(&transfer, &before).ok);
    let refused = boundary_observation(&transfer);
    assert!(!refused.ok);
    assert_eq!(refused.error_code.as_deref(), Some("transaction_pending"));

    let (parameters, _parameter_harness) = generic_fixture();
    boundary_io_snapshot(&parameters, 10);
    let status = generic_status(&parameters);
    assert!(generic_apply(&parameters, &status).ok);
    let refused = boundary_observation(&parameters);
    assert!(!refused.ok);
    assert_eq!(refused.error_code.as_deref(), Some("transaction_pending"));
}

#[test]
fn live_transfer_and_parameter_writes_are_refused_while_an_experiment_runs() {
    let (transfer, _transfer_harness) = transfer_fixture();
    boundary_io_snapshot(&transfer, 10);
    assert!(boundary_observation(&transfer).ok);
    let before = transfer_status(&transfer);
    let refused = queue_transfer(&transfer, &before);
    assert!(!refused.ok);
    assert_eq!(refused.error_code.as_deref(), Some("experiment_active"));
    assert!(
        !transfer.debug.has_transfer_transaction(),
        "a refused transfer is not queued"
    );

    let (parameters, _parameter_harness) = generic_fixture();
    boundary_io_snapshot(&parameters, 10);
    assert!(boundary_observation(&parameters).ok);
    let status = generic_status(&parameters);
    let refused = generic_apply(&parameters, &status);
    assert!(!refused.ok);
    assert_eq!(refused.error_code.as_deref(), Some("experiment_active"));
    assert_eq!(
        generic_status(&parameters)["status"],
        "idle",
        "a refused apply is not queued"
    );
}

#[test]
fn live_writes_wait_while_an_experiment_restores_its_output_and_resume_after_it() {
    let (state, _harness) = transfer_fixture();
    state.connect_experiments();
    boundary_io_snapshot(&state, 10);
    // The default step is 15 with restoration 10: the restoration is the value the experiment starts from.
    let started = start_experiment(&state, json!({}));
    assert!(started.ok, "{:?}", started.error);
    // The first execution after the start writes the step (A3.5).
    experiment_scan(&state, 0);
    // The process has moved away from the restoration value, so the restoration is not yet confirmed.
    boundary_io_snapshot(&state, 15);
    let aborted = handle_request_value(
        json!({"id": 8811, "type": "control.experiment.abort", "params": {"experiment_id": "experiment-1"}}),
        &state,
        None,
    );
    assert!(aborted.ok, "{:?}", aborted.error);
    assert_eq!(aborted.result.as_ref().unwrap()["restoration"], "pending");
    let before = transfer_status(&state);
    let refused = queue_transfer(&state, &before);
    assert_eq!(
        refused.error_code.as_deref(),
        Some("experiment_active"),
        "a restoring experiment still holds the output"
    );

    // Once the restoration value is observed, the next scan confirms it and live writes are accepted again.
    boundary_io_snapshot(&state, 10);
    experiment_scan(&state, 20);
    assert_eq!(
        boundary_status(&state, "experiment-1")["restoration"],
        "restored",
        "the restoration is confirmed from the I/O image"
    );
    let accepted = queue_transfer(&state, &before);
    assert!(accepted.ok, "{:?}", accepted.error);
}

#[test]
fn symbolic_experiment_stops_at_a_reload_and_writes_nothing_to_the_ended_reference() {
    let state = symbol_experiment_state();
    let started = start_symbol_experiment(&state, json!({}));
    assert!(started.ok, "{:?}", started.error);
    // The first scan queues the step, which the reload then drops.
    experiment_scan(&state, 0);
    // The reload ends the program the variable's reference was resolved in.
    state.debug.invalidate_control_program();
    assert!(
        state.debug.drain_var_writes().is_empty(),
        "the queued write resolved against the ended program is dropped"
    );
    // A status request only reads: the reload is settled by the next scan (Specification 35, A6.7).
    assert_eq!(boundary_status(&state, "symbol-1")["status"], "running");
    experiment_scan(&state, 10);
    let status = boundary_status(&state, "symbol-1");
    assert_eq!(status["status"], "failed");
    assert_eq!(status["stop_reason"], "program_reloaded");
    assert_eq!(status["restoration"], "failed");
    assert_eq!(status["restoration_reason"], "program_reloaded");
    assert!(
        state.debug.drain_var_writes().is_empty(),
        "no restoration is written against the old reference"
    );
}

#[test]
fn reload_drops_queued_writes_resolved_against_the_ended_program() {
    let state = symbol_experiment_state();
    let snapshot = state.debug.snapshot().expect("a runtime snapshot");
    let reference = snapshot
        .storage
        .ref_for_global("Main")
        .expect("program instance reference");
    let Value::Instance(program) = snapshot
        .storage
        .get_global("Main")
        .expect("program instance")
    else {
        panic!("program instance");
    };
    state
        .debug
        .enqueue_reference_write(reference, Value::LReal(1.0), state.debug.runtime_epoch());
    state
        .debug
        .enqueue_instance_write(*program, "ManualOutput", Value::LReal(2.0));
    state.debug.enqueue_global_write("Gate", Value::Bool(true));
    state.debug.invalidate_control_program();
    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(
        writes[0].target,
        crate::debug::PendingVarTarget::Global("Gate".into()),
        "a name is looked up again when its write applies, so it survives the reload"
    );
}

#[test]
fn address_experiment_stops_at_a_reload_and_restores_its_output() {
    let state = experiment_state(10, false);
    let started = start_experiment(&state, json!({}));
    assert!(started.ok, "{:?}", started.error);
    experiment_scan(&state, 0);
    state.debug.invalidate_control_program();
    // A status request only reads: the next scan settles the reload (Specification 35, A6.7).
    assert_eq!(boundary_status(&state, "experiment-1")["status"], "running");
    experiment_scan(&state, 10);
    let status = boundary_status(&state, "experiment-1");
    assert_eq!(status["status"], "failed");
    assert_eq!(status["stop_reason"], "program_reloaded");
    assert!(
        state
            .debug
            .drain_io_writes()
            .contains(&(crate::io::IoAddress::parse("%MW0").unwrap(), Value::LInt(10))),
        "a process-image address is not resolved against the program, so its restoration is written"
    );
}

#[test]
fn restart_stops_a_running_experiment_with_runtime_restarted() {
    let state = experiment_state(10, false);
    let started = start_experiment(&state, json!({}));
    assert!(started.ok, "{:?}", started.error);
    // A restart is the runtime clearing its mutations at the restart boundary.
    state.debug.clear_runtime_mutations();
    // A status request only reads: the next scan settles the restart (Specification 35, A6.7).
    assert_eq!(boundary_status(&state, "experiment-1")["status"], "running");
    experiment_scan(&state, 10);
    let status = boundary_status(&state, "experiment-1");
    assert_eq!(status["status"], "failed");
    assert_eq!(status["stop_reason"], "runtime_restarted");
}

#[test]
fn symbolic_experiment_is_refused_until_a_scan_of_the_reloaded_program_completes() {
    let state = symbol_experiment_state();
    state.debug.invalidate_control_program();
    let refused = start_symbol_experiment(&state, json!({"experiment_id": "symbol-2"}));
    assert_eq!(refused.error_code.as_deref(), Some("stale_scan"));
    // A completed scan of the reloaded program refreshes the snapshot the variable is resolved in.
    let snapshot = state.debug.snapshot().expect("a runtime snapshot");
    state
        .debug
        .refresh_snapshot_from_storage(&snapshot.storage, snapshot.now);
    state
        .debug
        .finish_transfer_cycle(&snapshot.storage, snapshot.now);
    let started = start_symbol_experiment(&state, json!({"experiment_id": "symbol-2"}));
    assert!(started.ok, "{:?}", started.error);
}

#[test]
fn a_variable_write_resolved_in_an_ended_epoch_is_dropped_when_it_is_queued() {
    let state = symbol_experiment_state();
    let epoch = state.debug.runtime_epoch();
    let snapshot = state.debug.snapshot().expect("a runtime snapshot");
    let reference = snapshot
        .storage
        .ref_for_global("Main")
        .expect("program instance reference");
    state.debug.invalidate_control_program();
    state
        .debug
        .enqueue_reference_write(reference, Value::LReal(1.0), epoch);
    assert!(
        state.debug.drain_var_writes().is_empty(),
        "a write from an ended epoch never reaches the program that replaced it"
    );
}
