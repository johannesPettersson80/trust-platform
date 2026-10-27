// Experiments advance on the runtime's own scans (Specification 35, A3.5 and A6.5). Each test publishes scans the way the
// runtime's cycle publishes them, so no test waits on a thread or on the wall clock. Experiments here follow the
// background programs, which execute in every scan of the 10 ms test cycle.

const EXECUTOR_SOURCE: &str = r#"
PROGRAM Main
VAR
    Setpoint : LREAL := 10.0;
    Manual : BOOL := FALSE;
END_VAR
END_PROGRAM
"#;

/// The executor's fixture: a simulated runtime of `EXECUTOR_SOURCE` whose scans publish its state to the control, after
/// its first scan, with the I/O the tests publish.
fn executor_state() -> (ControlState, TestHarness) {
    let mut state = control_test_state(EXECUTOR_SOURCE);
    let mut harness = simulated_harness(EXECUTOR_SOURCE);
    state.debug = studio_debug(&mut harness);
    state.connect_experiments();
    // The target is simulated until a test publishes a physical scan (Specification 35, A2.4).
    state.experiments.record_simulated_target(true);
    // The first scan publishes the program's state, which a start request reads.
    assert!(harness.cycle().errors.is_empty());
    *state.io_snapshot.lock().expect("I/O snapshot") = Some(executor_memory(0, 10, 50));
    (state, harness)
}

fn executor_entry(address: &str, value: i64) -> crate::io::IoSnapshotEntry {
    crate::io::IoSnapshotEntry {
        name: None,
        address: crate::io::IoAddress::parse(address).expect("address"),
        value_type: Some(trust_hir::TypeId::LINT),
        value_type_name: None,
        value: crate::io::IoSnapshotValue::Value(Value::LInt(value)),
        source: None,
    }
}

/// One scan's I/O: the experiment's output `%MW0` and the feedback `%MW2`.
fn executor_memory(scan: u64, output: i64, feedback: i64) -> crate::io::IoSnapshot {
    crate::io::IoSnapshot {
        scan: Some(scan),
        memory: vec![
            executor_entry("%MW0", output),
            executor_entry("%MW2", feedback),
        ],
        ..crate::io::IoSnapshot::default()
    }
}

/// The program's variables in the scans that follow, as the program would leave them.
fn executor_values(harness: &mut TestHarness, setpoint: f64, manual: bool) {
    studio_set(harness, "Main.Setpoint", Value::LReal(setpoint));
    studio_set(harness, "Main.Manual", Value::Bool(manual));
}

/// Publishes one scan at `time_ms` of process time, with the program of `harness` as the scan left it, as the runtime's
/// cycle publishes it.
fn executor_publish(
    state: &ControlState,
    harness: &TestHarness,
    time_ms: i64,
    io: crate::io::IoSnapshot,
    simulation_only_io: bool,
) {
    // A test scan's target is simulated exactly when its I/O is simulation-only: the simulation is enabled in these tests.
    harness.runtime().with_program_view(|program| {
        state.debug.push_control_snapshot_at(
            io,
            time_ms * 1_000_000,
            program,
            &[],
            simulation_only_io,
            simulation_only_io,
        );
    });
}

/// A setpoint step on `Main.Setpoint` in Auto unless the overrides say otherwise.
fn executor_start(state: &ControlState, overrides: serde_json::Value) -> ControlResponse {
    let mut params = json!({
        "experiment_id": "executor-1",
        "source": "simulation",
        "kind": "setpoint_step",
        "task": "$background",
        "symbol": "Main.Setpoint",
        "value": 15.0,
        "restoration_value": 10.0,
        "minimum": 0,
        "maximum": 100,
        "duration_ms": 1_000,
        "expected_scan": 0,
        "mode_binding": { "symbol": "Main.Manual", "manual_value": true }
    });
    if let (Some(base), Some(changes)) = (params.as_object_mut(), overrides.as_object()) {
        base.extend(changes.clone());
    }
    handle_request_value(
        json!({ "id": 9400, "type": "control.experiment.start", "params": params }),
        state,
        None,
    )
}

fn executor_status(state: &ControlState) -> serde_json::Value {
    let response = handle_request_value(
        json!({ "id": 9401, "type": "control.experiment.status", "params": { "experiment_id": "executor-1" } }),
        state,
        None,
    );
    assert!(response.ok, "status failed: {:?}", response.error);
    response.result.expect("status result")
}

fn executor_abort(state: &ControlState) -> ControlResponse {
    handle_request_value(
        json!({ "id": 9402, "type": "control.experiment.abort", "params": { "experiment_id": "executor-1" } }),
        state,
        None,
    )
}

/// The values queued for the program's variables since the last drain, in queue order.
fn executor_drained_values(state: &ControlState) -> Vec<Value> {
    state
        .debug
        .drain_control_writes()
        .into_iter()
        .map(|write| write.value)
        .collect()
}

/// The values queued for the addressed output since the last drain, in queue order.
fn executor_drained_outputs(state: &ControlState) -> Vec<Value> {
    state
        .debug
        .drain_io_writes()
        .into_iter()
        .map(|(_, value)| value)
        .collect()
}

#[test]
fn executor_queues_each_point_for_the_execution_at_its_time() {
    let (state, mut harness) = executor_state();
    executor_values(&mut harness, 10.0, false);
    let started = executor_start(
        &state,
        json!({
            "value": 20.0,
            "trajectory": [{ "at_ms": 0, "value": 20.0 }, { "at_ms": 20, "value": 30.0 }]
        }),
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    assert!(
        executor_drained_values(&state).is_empty(),
        "the start request writes nothing"
    );
    // The first scan queues the point due at the next execution (10 ms) and nothing later.
    executor_publish(&state, &harness, 0, executor_memory(1, 10, 50), true);
    assert_eq!(executor_drained_values(&state), vec![Value::LReal(20.0)]);
    // The point at 20 ms takes effect at the execution at 20 ms, so the write is queued at the scan at 10 ms.
    executor_values(&mut harness, 20.0, false);
    executor_publish(&state, &harness, 10, executor_memory(2, 10, 50), true);
    assert_eq!(executor_drained_values(&state), vec![Value::LReal(30.0)]);
    let status = executor_status(&state);
    assert_eq!(status["last_applied"]["index"], json!(0));
    assert_eq!(status["last_applied"]["time_ms"], json!(0));
    assert_eq!(status["last_applied"]["process_time_ms"], json!(10));
}

#[test]
fn executor_applies_points_that_an_old_poll_would_have_merged_at_their_own_execution() {
    // Scans every 10 ms of process time: a runtime at ten times real time produces several of them in one old 20 ms poll.
    let (state, mut harness) = executor_state();
    let started = executor_start(
        &state,
        json!({
            "value": 1.0,
            "trajectory": [
                { "at_ms": 0, "value": 1.0 }, { "at_ms": 10, "value": 2.0 },
                { "at_ms": 20, "value": 3.0 }, { "at_ms": 30, "value": 4.0 },
                { "at_ms": 40, "value": 5.0 }
            ]
        }),
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    executor_values(&mut harness, 10.0, false);
    for (index, expected) in [2.0, 3.0, 4.0, 5.0].into_iter().enumerate() {
        let time = 10 * index as i64;
        executor_publish(&state, &harness, time, executor_memory(index as u64 + 1, 10, 50), true);
        assert_eq!(
            executor_drained_values(&state),
            vec![Value::LReal(expected)],
            "the scan at {time} ms"
        );
        executor_values(&mut harness, expected, false);
    }
    executor_publish(&state, &harness, 40, executor_memory(5, 10, 50), true);
    assert!(
        executor_drained_values(&state).is_empty(),
        "no point is due after the last one"
    );
}

#[test]
fn executor_switches_a_relay_at_the_execution_after_the_crossing() {
    let (state, mut harness) = executor_state();
    executor_values(&mut harness, 10.0, true);
    // A scan publishes the program's values, which the start reads.
    assert!(harness.cycle().errors.is_empty());
    let started = executor_start(
        &state,
        json!({
            "kind": "relay", "symbol": null, "address": "%MW0", "value": 100, "restoration_value": 10,
            "minimum": 0, "maximum": 100,
            "mode_binding": { "symbol": "Main.Manual", "manual_value": true },
            "relay_feedback": {
                "address": "%MW2", "value_type": "LINT", "center": 50.0, "hysteresis": 5.0,
                "low_value": 0, "high_value": 100
            }
        }),
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    executor_publish(&state, &harness, 0, executor_memory(1, 10, 50), true);
    assert_eq!(executor_drained_outputs(&state), vec![Value::LInt(100)]);
    executor_publish(&state, &harness, 10, executor_memory(2, 100, 60), true);
    assert_eq!(executor_drained_outputs(&state), vec![Value::LInt(0)]);
    executor_publish(&state, &harness, 20, executor_memory(3, 0, 60), true);
    assert!(
        executor_drained_outputs(&state).is_empty(),
        "the level holds until the measurement crosses back"
    );
}

#[test]
fn executor_stops_on_a_mode_change_and_confirms_the_restoration() {
    let (state, mut harness) = executor_state();
    executor_values(&mut harness, 10.0, true);
    // A scan publishes the program's values, which the start reads.
    assert!(harness.cycle().errors.is_empty());
    let started = executor_start(
        &state,
        json!({
            "kind": "mv_step", "symbol": null, "address": "%MW0", "value": 60, "restoration_value": 10,
            "mode_binding": { "symbol": "Main.Manual", "manual_value": true }
        }),
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    executor_publish(&state, &harness, 0, executor_memory(1, 10, 50), true);
    assert_eq!(executor_drained_outputs(&state), vec![Value::LInt(60)]);
    executor_values(&mut harness, 10.0, false);
    executor_publish(&state, &harness, 10, executor_memory(2, 60, 50), true);
    assert_eq!(
        executor_drained_outputs(&state),
        vec![Value::LInt(10)],
        "leaving Manual queues the restoration"
    );
    let status = executor_status(&state);
    assert_eq!(status["status"], json!("failed"));
    assert_eq!(status["stop_reason"], json!("mode_changed"));
    executor_publish(&state, &harness, 20, executor_memory(3, 10, 50), true);
    assert_eq!(executor_status(&state)["restoration"], json!("restored"));
}

/// One scan's I/O with an `LREAL` bound to `%ML8`, as a program's `AT %ML8 : LREAL` declaration decodes it.
fn real_memory(scan: u64, output: f64) -> crate::io::IoSnapshot {
    crate::io::IoSnapshot {
        scan: Some(scan),
        memory: vec![crate::io::IoSnapshotEntry {
            name: None,
            address: crate::io::IoAddress::parse("%ML8").expect("address"),
            value_type: Some(trust_hir::TypeId::LREAL),
            value_type_name: None,
            value: crate::io::IoSnapshotValue::Value(Value::LReal(output)),
            source: None,
        }],
        ..crate::io::IoSnapshot::default()
    }
}

/// Annex E5.6: an addressed step is written in the process image's form and reads back as its binding decodes it; it is
/// confirmed after the execution that follows the write, and the status reports the value read back.
#[test]
fn an_addressed_point_is_confirmed_in_its_binding_type_and_reported_as_read_back() {
    let (state, mut harness) = executor_state();
    executor_values(&mut harness, 10.0, true);
    // A scan publishes the program's values, which the start reads.
    assert!(harness.cycle().errors.is_empty());
    *state.io_snapshot.lock().expect("I/O snapshot") = Some(real_memory(0, 10.0));
    let started = executor_start(
        &state,
        json!({
            "kind": "mv_step", "symbol": null, "address": "%ML8", "value_type": "LREAL", "value": 60.0,
            "restoration_value": 10.0, "mode_binding": { "symbol": "Main.Manual", "manual_value": true }
        }),
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    executor_publish(&state, &harness, 0, real_memory(1, 10.0), true);
    assert_eq!(executor_drained_outputs(&state).len(), 1, "the step is written once");
    assert!(executor_status(&state)["last_applied"].is_null(), "nothing is applied before it is read back");
    executor_publish(&state, &harness, 10, real_memory(2, 60.0), true);
    let status = executor_status(&state);
    let applied = &status["last_applied"];
    assert_eq!(applied["index"], json!(0));
    assert_eq!(applied["value"], json!(60.0), "the value read back, as the binding decodes it");
    assert_eq!(applied["scan"], json!(2));
}

#[test]
fn executor_completes_at_its_duration_and_restores_the_setpoint() {
    let (state, mut harness) = executor_state();
    assert!(executor_start(&state, json!({ "value": 15.0 })).ok);
    executor_values(&mut harness, 15.0, false);
    for step in 0i64..99 {
        executor_publish(&state, &harness, step * 10, executor_memory(step as u64 + 1, 10, 50), true);
    }
    assert_eq!(executor_status(&state)["status"], json!("running"));
    executor_publish(&state, &harness, 990, executor_memory(100, 10, 50), true);
    let status = executor_status(&state);
    assert_eq!(status["status"], json!("completed"));
    assert_eq!(status["stop_reason"], json!("duration_complete"));
    // A later write to a variable before the boundary replaces the earlier one (A3.5): the restoration replaces the
    // excitation that no boundary applied, so the next boundary applies the restoration alone.
    assert_eq!(executor_drained_values(&state), vec![Value::LReal(10.0)]);
    executor_values(&mut harness, 10.0, false);
    executor_publish(&state, &harness, 1000, executor_memory(101, 10, 50), true);
    assert_eq!(executor_status(&state)["restoration"], json!("restored"));
}

#[test]
fn executor_abort_queues_the_restoration_after_the_excitation_already_queued() {
    let (state, mut harness) = executor_state();
    assert!(executor_start(&state, json!({ "value": 15.0 })).ok);
    executor_values(&mut harness, 10.0, false);
    executor_publish(&state, &harness, 0, executor_memory(1, 10, 50), true);
    assert!(executor_abort(&state).ok);
    // The restoration replaces the excitation that no boundary applied (A3.5), so it is the value applied at the next scan.
    assert_eq!(
        executor_drained_values(&state),
        vec![Value::LReal(10.0)],
        "the restoration is the value applied at the next scan"
    );
    assert_eq!(executor_status(&state)["status"], json!("aborted"));
}

#[test]
fn executor_fails_a_restoration_not_read_back_within_three_executions() {
    let (state, mut harness) = executor_state();
    assert!(executor_start(&state, json!({ "value": 15.0 })).ok);
    executor_values(&mut harness, 15.0, false);
    executor_publish(&state, &harness, 0, executor_memory(1, 10, 50), true);
    assert!(executor_abort(&state).ok);
    for (index, time) in [10_i64, 20].into_iter().enumerate() {
        executor_publish(&state, &harness, time, executor_memory(index as u64 + 2, 10, 50), true);
        assert_eq!(executor_status(&state)["restoration"], json!("pending"));
    }
    executor_publish(&state, &harness, 30, executor_memory(4, 10, 50), true);
    let status = executor_status(&state);
    assert_eq!(status["status"], json!("failed_to_restore"));
    assert_eq!(status["restoration"], json!("failed"));
    assert_eq!(status["restoration_reason"], json!("confirmation_timeout"));
}

#[test]
fn executor_confirms_a_restoration_read_back_at_the_next_execution() {
    let (state, mut harness) = executor_state();
    assert!(executor_start(&state, json!({ "value": 15.0 })).ok);
    executor_values(&mut harness, 15.0, false);
    executor_publish(&state, &harness, 0, executor_memory(1, 10, 50), true);
    assert!(executor_abort(&state).ok);
    executor_values(&mut harness, 10.0, false);
    executor_publish(&state, &harness, 10, executor_memory(2, 10, 50), true);
    let status = executor_status(&state);
    assert_eq!(status["status"], json!("aborted"));
    assert_eq!(status["restoration"], json!("restored"));
}

#[test]
fn executor_writes_nothing_to_a_physical_target_without_approval() {
    let (state, mut harness) = executor_state();
    assert!(executor_start(&state, json!({ "value": 15.0 })).ok);
    executor_values(&mut harness, 10.0, false);
    executor_publish(&state, &harness, 0, executor_memory(1, 10, 50), false);
    let status = executor_status(&state);
    assert_eq!(status["status"], json!("failed"));
    assert_eq!(status["stop_reason"], json!("physical_approval_required"));
    assert_eq!(status["restoration"], json!("not_required"));
    assert!(executor_drained_values(&state).is_empty());
    assert!(executor_drained_outputs(&state).is_empty());
}

#[test]
fn executor_refuses_an_unknown_task_and_points_closer_than_the_task_interval() {
    let (state, _harness) = executor_state();
    let unknown = executor_start(&state, json!({ "task": "NoSuchTask" }));
    assert!(!unknown.ok);
    assert_eq!(unknown.error_code.as_deref(), Some("invalid_experiment"));
    let close = executor_start(
        &state,
        json!({ "trajectory": [{ "at_ms": 0, "value": 15.0 }, { "at_ms": 5, "value": 16.0 }] }),
    );
    assert!(!close.ok);
    assert_eq!(close.error_code.as_deref(), Some("invalid_experiment"));
    assert!(format!("{:?}", close.error).contains("closer than the task interval"));
}

#[test]
fn executor_accepts_a_day_of_process_time_and_refuses_a_millisecond_more() {
    let (accepted, _accepted_harness) = executor_state();
    assert!(executor_start(&accepted, json!({ "duration_ms": 86_400_000 })).ok);
    let (refused, _refused_harness) = executor_state();
    let longer = executor_start(&refused, json!({ "duration_ms": 86_400_001 }));
    assert!(!longer.ok);
    assert_eq!(longer.error_code.as_deref(), Some("invalid_experiment"));
}

#[test]
fn executor_finish_ends_an_experiment_as_a_completion_without_asserting_an_abort() {
    let (state, mut harness) = executor_state();
    assert!(executor_start(&state, json!({ "value": 15.0 })).ok);
    executor_values(&mut harness, 15.0, false);
    executor_publish(&state, &harness, 0, executor_memory(1, 10, 50), true);
    let finished = super::experiment_handlers::handle_finish(
        9403,
        Some(json!({ "experiment_id": "executor-1" })),
        &state,
    );
    assert!(finished.ok, "finish failed: {:?}", finished.error);
    let status = executor_status(&state);
    assert_eq!(status["status"], json!("completed"));
    assert_eq!(status["stop_reason"], json!("finished_early"));
    assert_eq!(status["restoration"], json!("pending"));
    // The restoration replaces the excitation that no boundary applied (A3.5).
    assert_eq!(executor_drained_values(&state), vec![Value::LReal(10.0)]);
    executor_values(&mut harness, 10.0, false);
    executor_publish(&state, &harness, 10, executor_memory(2, 10, 50), true);
    assert_eq!(executor_status(&state)["restoration"], json!("restored"));
    let again = super::experiment_handlers::handle_finish(
        9404,
        Some(json!({ "experiment_id": "executor-1" })),
        &state,
    );
    assert!(again.ok);
    assert_eq!(executor_status(&state)["status"], json!("completed"));
    assert!(executor_drained_values(&state).is_empty());
}

#[test]
fn control_status_reports_simulation_only_io_once_a_cycle_has_run() {
    let mut state = control_test_state(EXECUTOR_SOURCE);
    let mut harness = TestHarness::from_source(EXECUTOR_SOURCE).expect("build harness");
    state.debug = studio_debug(&mut harness);
    let simulation_only_io = |state: &ControlState| {
        let response = handle_request_value(json!({ "id": 9405, "type": "status" }), state, None);
        assert!(response.ok, "status failed: {:?}", response.error);
        response.result.expect("status result")["simulation_only_io"].clone()
    };
    assert_eq!(simulation_only_io(&state), json!(false));
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(simulation_only_io(&state), json!(true));
}

#[test]
fn a_six_hour_experiment_runs_from_the_scan_cycle_without_a_thread() {
    // The task runs every 60 s, so six hours of process time take 360 executions, and two more confirm the restoration.
    let source = r#"
CONFIGURATION Cell
RESOURCE Cpu ON PLC
TASK MainTask (INTERVAL := T#60s, PRIORITY := 1);
PROGRAM Cell WITH MainTask : Plant;
END_RESOURCE
END_CONFIGURATION
PROGRAM Plant
VAR
    Setpoint : LREAL := 10.0;
    Manual : BOOL := FALSE;
END_VAR
END_PROGRAM
"#;
    let mut state = control_test_state(source);
    let mut harness = TestHarness::from_source(source).expect("build harness");
    state.debug = studio_debug(&mut harness);
    // A simulated resource: its target is simulated, so a "simulation" request is not contradicted (A2.4).
    harness.runtime_mut().set_simulation_enabled(true);
    state.connect_experiments();
    assert!(harness.cycle().errors.is_empty());
    *state.io_snapshot.lock().expect("I/O snapshot") = state.debug.latest_io_snapshot();
    let started = handle_request_value(
        json!({
            "id": 9410,
            "type": "control.experiment.start",
            "params": {
                "experiment_id": "long-1", "source": "simulation", "kind": "setpoint_step",
                "task": "MainTask", "symbol": "Cell.Setpoint", "value": 15.0, "restoration_value": 10.0,
                "minimum": 0, "maximum": 100, "duration_ms": 21_600_000, "expected_scan": 1,
                "mode_binding": { "symbol": "Cell.Manual", "manual_value": true }
            }
        }),
        &state,
        None,
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    for _ in 0..362 {
        harness.advance_time(crate::value::Duration::from_millis(60_000));
        assert!(harness.cycle().errors.is_empty());
    }
    let response = handle_request_value(
        json!({ "id": 9411, "type": "control.experiment.status", "params": { "experiment_id": "long-1" } }),
        &state,
        None,
    );
    let status = response.result.expect("status result");
    assert_eq!(status["status"], json!("completed"));
    assert_eq!(status["stop_reason"], json!("duration_complete"));
    assert_eq!(status["restoration"], json!("restored"));
    assert_eq!(status["duration_ms"], json!(21_600_000));
}

/// A driver that is not simulated: it stands for physical I/O in the target-fact tests.
struct UnsimulatedDriverFixture;

impl crate::io::IoDriver for UnsimulatedDriverFixture {
    fn read_inputs(&mut self, _inputs: &mut [u8]) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, _outputs: &[u8]) -> Result<(), crate::error::RuntimeError> {
        Ok(())
    }
}

#[test]
fn the_runtime_target_is_simulated_only_with_simulation_enabled_and_simulated_io() {
    // Simulation off: the target is physical whatever its I/O (Specification 35, A2.4).
    let mut harness = TestHarness::from_source(EXECUTOR_SOURCE).expect("build harness");
    assert!(!harness.runtime().simulated_target());
    harness.runtime_mut().set_simulation_enabled(true);
    assert!(harness.runtime().simulated_target());
    // One physical driver makes a simulated resource's target physical.
    harness
        .runtime_mut()
        .add_io_driver("physical", Box::new(UnsimulatedDriverFixture));
    assert!(!harness.runtime().simulated_target());
}

#[test]
fn a_label_that_contradicts_the_targets_class_is_refused() {
    let (state, _harness) = executor_state();
    let physical_label = executor_start(
        &state,
        json!({ "source": "physical", "physical_approval": { "user_approved": true, "topology_reviewed": true } }),
    );
    assert!(!physical_label.ok);
    assert_eq!(physical_label.error_code.as_deref(), Some("invalid_target"));
    assert!(format!("{:?}", physical_label.error)
        .contains("The request says physical, but the runtime's target is simulated."));
    state.experiments.record_simulated_target(false);
    let simulation_label = executor_start(&state, json!({}));
    assert!(!simulation_label.ok);
    assert_eq!(simulation_label.error_code.as_deref(), Some("invalid_target"));
    assert!(format!("{:?}", simulation_label.error)
        .contains("The request says simulation, but the runtime's target is physical."));
}

#[test]
fn a_physical_target_needs_the_engineers_approval_at_the_start() {
    let (state, _harness) = executor_state();
    state.experiments.record_simulated_target(false);
    let unapproved = executor_start(&state, json!({ "source": "physical" }));
    assert!(!unapproved.ok);
    assert_eq!(
        unapproved.error_code.as_deref(),
        Some("physical_approval_required")
    );
    let approved = executor_start(
        &state,
        json!({ "source": "physical", "physical_approval": { "user_approved": true, "topology_reviewed": true } }),
    );
    assert!(approved.ok, "approved start failed: {:?}", approved.error);
    assert_eq!(executor_status(&state)["source"], json!("physical"));
}

#[test]
fn an_experiment_cannot_start_before_the_runtime_has_scanned_its_target() {
    let state = control_test_state(EXECUTOR_SOURCE);
    state.connect_experiments();
    let response = executor_start(&state, json!({}));
    assert!(!response.ok);
    assert_eq!(response.error_code.as_deref(), Some("io_unavailable"));
}

#[test]
fn a_second_experiment_is_refused_while_the_first_runs_or_restores_and_names_it() {
    let (state, mut harness) = executor_state();
    assert!(executor_start(&state, json!({})).ok);
    let second = executor_start(&state, json!({ "experiment_id": "executor-2" }));
    assert!(!second.ok);
    assert_eq!(second.error_code.as_deref(), Some("experiment_active"));
    assert!(format!("{:?}", second.error).contains("experiment executor-1 is running"));
    // The first experiment wrote, then was aborted: its restoration waits for a scan (A6.6).
    executor_values(&mut harness, 10.0, false);
    executor_publish(&state, &harness, 0, executor_memory(1, 10, 50), true);
    assert!(executor_abort(&state).ok);
    let restoring = executor_start(&state, json!({ "experiment_id": "executor-2" }));
    assert!(!restoring.ok);
    assert_eq!(restoring.error_code.as_deref(), Some("experiment_active"));
    assert!(format!("{:?}", restoring.error)
        .contains("experiment executor-1 is restoring its output"));
    // Once the restoration is confirmed, a new experiment may start.
    executor_values(&mut harness, 10.0, false);
    executor_publish(&state, &harness, 10, executor_memory(2, 10, 50), true);
    assert_eq!(executor_status(&state)["restoration"], json!("restored"));
    assert!(executor_start(&state, json!({ "experiment_id": "executor-2" })).ok);
}

#[test]
fn a_point_between_two_executions_takes_effect_at_the_next_one_and_a_point_on_one_at_its_own() {
    // Executions every 10 ms. The 15 ms point lies between the executions at 10 and 20 ms, so it takes effect at 20 ms.
    // The 30 ms point lies on the execution at 30 ms, so it takes effect there (A3.5).
    let (state, mut harness) = executor_state();
    let started = executor_start(
        &state,
        json!({
            "value": 20.0,
            "trajectory": [
                { "at_ms": 0, "value": 20.0 }, { "at_ms": 15, "value": 30.0 }, { "at_ms": 30, "value": 40.0 }
            ]
        }),
    );
    assert!(started.ok, "start failed: {:?}", started.error);
    executor_values(&mut harness, 10.0, false);
    executor_publish(&state, &harness, 0, executor_memory(1, 10, 50), true);
    assert_eq!(executor_drained_values(&state), vec![Value::LReal(20.0)]);
    executor_values(&mut harness, 20.0, false);
    executor_publish(&state, &harness, 10, executor_memory(2, 20, 50), true);
    assert_eq!(
        executor_drained_values(&state),
        vec![Value::LReal(30.0)],
        "the 15 ms point is due at the 20 ms execution, so the 10 ms scan queues it"
    );
    executor_values(&mut harness, 30.0, false);
    executor_publish(&state, &harness, 20, executor_memory(3, 30, 50), true);
    let between = executor_status(&state);
    assert_eq!(between["last_applied"]["index"], json!(1));
    assert_eq!(between["last_applied"]["time_ms"], json!(15));
    assert_eq!(between["last_applied"]["process_time_ms"], json!(20));
    assert_eq!(executor_drained_values(&state), vec![Value::LReal(40.0)]);
    executor_values(&mut harness, 40.0, false);
    executor_publish(&state, &harness, 30, executor_memory(4, 40, 50), true);
    let on = executor_status(&state);
    assert_eq!(on["last_applied"]["index"], json!(2));
    assert_eq!(on["last_applied"]["time_ms"], json!(30));
    assert_eq!(on["last_applied"]["process_time_ms"], json!(30));
}
