// Contract: docs/specs/control-studio-experiment-abort.md.
// The protocol owns ordered writes and observed confirmation; the ST program
// owns what its declared abort input does to its actuator.

fn abort_policy_state() -> ControlState {
    let state = experiment_state(10, false);
    let mut snapshot = state.io_snapshot.lock().unwrap();
    for (name, address) in [("Skid.Gate", "%MX8.0"), ("Skid.Trip", "%MX8.1")] {
        snapshot.as_mut().unwrap().memory.push(crate::io::IoSnapshotEntry {
            name: Some(SmolStr::new(name)),
            address: crate::io::IoAddress::parse(address).unwrap(),
            value_type: Some(trust_hir::TypeId::BOOL),
            value: crate::io::IoSnapshotValue::Value(Value::Bool(false)),
            source: None,
        });
    }
    drop(snapshot);
    state
}

fn abort_policy_start(state: &ControlState, overrides: serde_json::Value) -> ControlResponse {
    let mut params = json!({"kind":"project_sequence","address":"%MX8.0", "value":true,
        "restoration_value":false,"value_type":"BOOL","minimum":0,"maximum":1,
        "abort_binding":{"address":"%MX8.1","value_type":"BOOL","asserted_value":true}});
    params.as_object_mut().unwrap().extend(overrides.as_object().unwrap().clone());
    start_experiment(state, params)
}

fn abort_policy_publish(state: &ControlState, scan: u64, active: bool, trip: bool) {
    let mut snapshot = state.io_snapshot.lock().unwrap();
    let snapshot = snapshot.as_mut().unwrap();
    snapshot.scan = Some(scan);
    for (address, value) in [("%MX8.0", active), ("%MX8.1", trip)] {
        snapshot.memory.iter_mut().find(|entry| entry.address == crate::io::IoAddress::parse(address).unwrap()).unwrap().value = crate::io::IoSnapshotValue::Value(Value::Bool(value));
    }
    state.io_snapshot_seen_ms.store(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64, std::sync::atomic::Ordering::Relaxed);
}

fn abort_policy_status(state: &ControlState) -> serde_json::Value {
    let response = handle_request_value(json!({"id":8402,"type":"control.experiment.status","params":{"experiment_id":"experiment-1"}}), state, None);
    assert!(response.ok, "{:?}", response.error);
    response.result.unwrap()
}

fn abort_policy_wait(state: &ControlState, key: &str, expected: &str) -> serde_json::Value {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let status = abort_policy_status(state);
        if status[key] == expected { return status; }
        assert!(std::time::Instant::now() < until, "waiting for {key}={expected}: {status}");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn project_abort_binding_asserts_before_deactivation_and_requires_later_observed_scan() {
    let state = abort_policy_state();
    let started = abort_policy_start(&state, json!({}));
    assert!(started.ok, "reviewed project abort binding must start: {:?}", started.error);
    assert_eq!(state.debug.drain_io_writes(), vec![(crate::io::IoAddress::parse("%MX8.0").unwrap(), Value::Bool(true))]);
    abort_policy_publish(&state, 8, true, false);
    let aborted = handle_request_value(json!({"id":8401,"type":"control.experiment.abort","params":{"experiment_id":"experiment-1"}}), &state, None);
    assert!(aborted.ok);
    assert_eq!(state.debug.drain_io_writes(), vec![
        (crate::io::IoAddress::parse("%MX8.1").unwrap(), Value::Bool(true)),
        (crate::io::IoAddress::parse("%MX8.0").unwrap(), Value::Bool(false)),
    ], "assert the program-owned abort before withdrawing activation");
    abort_policy_publish(&state, 8, false, true);
    std::thread::sleep(std::time::Duration::from_millis(40));
    assert_eq!(abort_policy_status(&state)["restoration"], "pending", "same-scan values cannot acknowledge ST execution");
    abort_policy_publish(&state, 9, false, true);
    let observed = abort_policy_wait(&state, "restoration", "restored");
    assert_eq!(observed["abort_binding"]["state"], "observed");
    assert_eq!(observed["abort_binding"]["observed_scan"], 9);
}

#[test]
fn project_abort_binding_normal_timeout_only_restores_activation() {
    let state = abort_policy_state();
    let started = abort_policy_start(&state, json!({"duration_ms":20}));
    assert!(started.ok, "{:?}", started.error);
    let _ = state.debug.drain_io_writes();
    abort_policy_publish(&state, 8, true, false);
    abort_policy_wait(&state, "status", "completed");
    assert_eq!(state.debug.drain_io_writes(), vec![(crate::io::IoAddress::parse("%MX8.0").unwrap(), Value::Bool(false))]);
    abort_policy_publish(&state, 9, false, false);
    let observed = abort_policy_wait(&state, "restoration", "restored");
    assert_eq!(observed["abort_binding"]["state"], "not_requested");
}

#[test]
fn project_abort_binding_stale_failure_asserts_abort_and_reports_unobserved_restoration() {
    let state = abort_policy_state();
    let started = abort_policy_start(&state, json!({"stop_on_stale_ms":40}));
    assert!(started.ok, "{:?}", started.error);
    let _ = state.debug.drain_io_writes();
    state.io_snapshot_seen_ms.store(0, std::sync::atomic::Ordering::Relaxed);
    abort_policy_wait(&state, "status", "failed");
    assert_eq!(state.debug.drain_io_writes(), vec![
        (crate::io::IoAddress::parse("%MX8.1").unwrap(), Value::Bool(true)),
        (crate::io::IoAddress::parse("%MX8.0").unwrap(), Value::Bool(false)),
    ]);
    let failed = abort_policy_wait(&state, "status", "failed_to_restore");
    assert_eq!(failed["abort_binding"]["state"], "failed");
    assert_eq!(failed["stop_reason"], "stale_runtime_data");
}

#[test]
fn project_abort_binding_rejects_latched_forced_and_alias_bindings_without_writes() {
    let state = abort_policy_state();
    abort_policy_publish(&state, 7, false, true);
    let latched = abort_policy_start(&state, json!({}));
    assert!(!latched.ok);
    assert_eq!(latched.error_code.as_deref(), Some("abort_already_asserted"));
    assert!(state.debug.drain_io_writes().is_empty());
    abort_policy_publish(&state, 8, false, false);
    state.io_snapshot.lock().unwrap().as_mut().unwrap().forced.push(crate::io::IoAddress::parse("%MX8.1").unwrap());
    let forced = abort_policy_start(&state, json!({}));
    assert_eq!(forced.error_code.as_deref(), Some("forced_signal"));
    assert!(state.debug.drain_io_writes().is_empty());
    state.io_snapshot.lock().unwrap().as_mut().unwrap().forced.clear();
    let alias = abort_policy_start(&state, json!({"abort_binding":{"address":"%MX8.0","value_type":"BOOL","asserted_value":true}}));
    assert!(!alias.ok);
    assert!(state.debug.drain_io_writes().is_empty());
}

fn abort_runtime_fixture(source: &str, simulation_path: Option<&std::path::Path>) -> (ControlState, crate::Runtime, Option<crate::simulation::SimulationController>, u64) {
    let mut state = hmi_test_state("PROGRAM UnrelatedHarness END_PROGRAM");
    let session = crate::harness::CompileSession::from_sources(vec![crate::harness::SourceFile::with_path("src/process.st", source)]);
    let mut runtime = session.build_runtime().expect("compile actual ST program");
    runtime.apply_bytecode_bytes(&session.build_bytecode_bytes().unwrap(), None).unwrap();
    state.debug = runtime.enable_debug();
    *state.metadata.lock().unwrap() = runtime.metadata_snapshot();
    let simulation = simulation_path.map(|path| crate::simulation::SimulationController::new(crate::simulation::SimulationConfig::load(path).unwrap()));
    (state, runtime, simulation, 0)
}

fn abort_runtime_scan(fixture: &mut (ControlState, crate::Runtime, Option<crate::simulation::SimulationController>, u64)) {
    let (state, runtime, simulation, scan) = fixture;
    if let Some(simulation) = simulation.as_mut() {
        simulation.apply_pre_cycle(crate::value::Duration::from_millis(*scan as i64 * 100), runtime).unwrap();
    }
    runtime.advance_time(crate::value::Duration::from_millis(100));
    runtime.execute_cycle().expect("execute actual owning task");
    *scan += 1;
    if let Some(simulation) = simulation.as_mut() {
        simulation.apply_post_cycle(crate::value::Duration::from_millis(*scan as i64 * 100), runtime).unwrap();
    }
    let mut snapshot = runtime.io().snapshot();
    snapshot.scan = Some(*scan);
    *state.io_snapshot.lock().unwrap() = Some(snapshot);
    state.io_snapshot_seen_ms.store(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64, std::sync::atomic::Ordering::Relaxed);
}

fn abort_runtime_start(state: &ControlState, activation: &str, abort: &str, duration: u64) {
    let scan = state.io_snapshot.lock().unwrap().as_ref().unwrap().scan.unwrap();
    let response = start_experiment(state, json!({"kind":"project_sequence","address":activation,
        "value":true,"restoration_value":false,"value_type":"BOOL","minimum":0,"maximum":1,
        "duration_ms":duration,"expected_scan":scan,"stop_on_stale_ms":1000,
        "abort_binding":{"address":abort,"value_type":"BOOL","asserted_value":true}}));
    assert!(response.ok, "{:?}", response.error);
}

fn abort_runtime_output(runtime: &crate::Runtime) -> f64 {
    match runtime.io().read(&crate::io::IoAddress::parse("%QL0").unwrap()).unwrap() {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected actual LREAL actuator output, got {other:?}"),
    }
}

#[test]
fn project_abort_binding_executes_arbitrary_st_policy_and_keeps_normal_hold_separate() {
    let source = r#"
PROGRAM Mixer
VAR Permit : BOOL; HaltRequest : BOOL; Drive : LREAL; END_VAR
IF HaltRequest THEN Drive := LREAL#0.0;
ELSIF Permit THEN Drive := LREAL#0.9;
ELSE Drive := LREAL#0.7;
END_IF;
END_PROGRAM
CONFIGURATION Config
RESOURCE Res ON PLC
TASK Cycle(INTERVAL := T#100ms, PRIORITY := 1);
PROGRAM Unit WITH Cycle : Mixer;
END_RESOURCE
VAR_CONFIG
Unit.Permit AT %MX8.0 : BOOL;
Unit.HaltRequest AT %MX8.1 : BOOL;
Unit.Drive AT %QL0 : LREAL;
END_VAR
END_CONFIGURATION
"#;
    for aborted in [false, true] {
        let mut fixture = abort_runtime_fixture(source, None);
        abort_runtime_scan(&mut fixture);
        assert_eq!(abort_runtime_output(&fixture.1), 0.7);
        abort_runtime_start(&fixture.0, "%MX8.0", "%MX8.1", if aborted { 5000 } else { 50 });
        abort_runtime_scan(&mut fixture);
        assert_eq!(abort_runtime_output(&fixture.1), 0.9);
        if aborted {
            let response = handle_request_value(json!({"id":8410,"type":"control.experiment.abort","params":{"experiment_id":"experiment-1"}}), &fixture.0, None);
            assert!(response.ok);
        } else { abort_policy_wait(&fixture.0, "status", "completed"); }
        assert_eq!(abort_policy_status(&fixture.0)["restoration"], "pending");
        abort_runtime_scan(&mut fixture);
        let observed = abort_policy_wait(&fixture.0, "restoration", "restored");
        assert_eq!(abort_runtime_output(&fixture.1), if aborted { 0.0 } else { 0.7 });
        assert_eq!(observed["abort_binding"]["state"], if aborted { "observed" } else { "not_requested" });
    }
}

#[test]
fn project_abort_binding_drives_actual_lesson14_abort_to_zero_and_normal_completion_to75() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment");
    let source = std::fs::read_to_string(root.join("src/Main.st")).unwrap();
    for aborted in [false, true] {
        let mut fixture = abort_runtime_fixture(&source, Some(&root.join("simulation.toml")));
        for _ in 0..60 { abort_runtime_scan(&mut fixture); }
        assert_eq!(abort_runtime_output(&fixture.1), 75.0);
        abort_runtime_start(&fixture.0, "%MX64.0", "%MX64.1", if aborted { 5000 } else { 50 });
        abort_runtime_scan(&mut fixture);
        assert_eq!(abort_runtime_output(&fixture.1), 85.0);
        if aborted {
            let response = handle_request_value(json!({"id":8411,"type":"control.experiment.abort","params":{"experiment_id":"experiment-1"}}), &fixture.0, None);
            assert!(response.ok);
        } else { abort_policy_wait(&fixture.0, "status", "completed"); }
        abort_runtime_scan(&mut fixture);
        abort_policy_wait(&fixture.0, "restoration", "restored");
        assert_eq!(abort_runtime_output(&fixture.1), if aborted { 0.0 } else { 75.0 });
    }
}
