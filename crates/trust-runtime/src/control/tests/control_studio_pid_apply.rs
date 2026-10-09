fn pid_state() -> ControlState {
    let state = control_test_state(
        r#"
TYPE PIDAntiWindup : (BackCalculation := 0, Combined := 1); END_TYPE
TYPE PIDParameters : STRUCT
    ProportionalGain : LREAL;
    IntegralGain : LREAL;
    DerivativeGain : LREAL;
    ProportionalSetpointWeight : LREAL;
    DerivativeSetpointWeight : LREAL;
    Bias : LREAL;
    OutputMinimum : LREAL;
    OutputMaximum : LREAL;
    RateMinimum : LREAL;
    RateMaximum : LREAL;
    TrackingTime : LREAL;
    AntiWindup : PIDAntiWindup;
END_STRUCT END_TYPE
FUNCTION_BLOCK PID
VAR ParametersValue : PIDParameters; END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR Controller : PID; END_VAR
END_PROGRAM
"#,
    );
    // The target is simulated until a test says otherwise: a scan has published a simulated target (A2.4).
    state.experiments.record_simulated_target(true);
    state
}

#[test]
fn pid_apply_is_revision_guarded_atomic_and_rollback_restores_exact_record() {
    let state = pid_state();
    let status = handle_request_value(
        json!({"id": 8100, "type": "control.pid.status", "params": { "instance_path": "Main.Controller" }}),
        &state,
        None,
    );
    assert!(status.ok, "status failed: {:?}", status.error);
    assert_eq!(
        status.result.as_ref().unwrap()["parameters"]["AntiWindup"],
        "BackCalculation"
    );
    let revision = status.result.as_ref().unwrap()["active_revision"]
        .as_str()
        .unwrap()
        .to_string();
    let applied = handle_request_value(
        json!({"id": 8101, "type": "control.pid.apply", "params": {
        "instance_path": "Main.Controller", "source": "simulation", "expected_revision": revision,
        "parameters": { "ProportionalGain": 2.0, "IntegralGain": 0.5, "DerivativeGain": 0.0,
            "OutputMinimum": 0.0, "OutputMaximum": 100.0, "AntiWindup": "Combined" }, "physical_approved": false }}),
        &state,
        None,
    );
    assert!(applied.ok, "apply failed: {:?}", applied.error);
    let result = applied.result.as_ref().unwrap();
    assert_eq!(result["previous_revision"], revision);
    let active = result["pending_revision"].as_str().unwrap().to_string();
    assert_ne!(active, revision);
    assert!(
        result.get("active_revision").is_none(),
        "a queued write is not active evidence"
    );
    let writes = state.debug.drain_var_writes();
    assert_eq!(
        writes.len(),
        1,
        "complete parameter record must be one queued write"
    );
    let applied_status = handle_request_value(
        json!({"id": 8104, "type": "control.pid.status", "params": {
        "instance_path": "Main.Controller" }}),
        &state,
        None,
    );
    assert_eq!(
        applied_status.result.as_ref().unwrap()["active_revision"],
        revision
    );
    assert_eq!(
        applied_status.result.as_ref().unwrap()["pending_revision"],
        active
    );
    assert_eq!(applied_status.result.as_ref().unwrap()["confirmed"], false);

    for write in writes {
        if let crate::debug::PendingVarTarget::Instance(instance_id, name) = write.target {
            state.debug.with_snapshot(|snapshot| {
                snapshot
                    .storage
                    .set_instance_var(instance_id, name, write.value)
            });
        }
    }
    let confirmed_status = handle_request_value(
        json!({"id": 8105, "type": "control.pid.status", "params": {
        "instance_path": "Main.Controller" }}),
        &state,
        None,
    );
    assert_eq!(
        confirmed_status.result.as_ref().unwrap()["active_revision"],
        active
    );
    assert_eq!(confirmed_status.result.as_ref().unwrap()["confirmed"], true);
    assert_eq!(
        confirmed_status.result.as_ref().unwrap()["parameters"]["AntiWindup"],
        "Combined"
    );
    let stale = handle_request_value(
        json!({"id": 8102, "type": "control.pid.apply", "params": {
        "instance_path": "Main.Controller", "source": "simulation", "expected_revision": revision,
        "parameters": { "ProportionalGain": 3.0 }, "physical_approved": false }}),
        &state,
        None,
    );
    assert!(!stale.ok);
    assert_eq!(
        stale.error_code.as_deref(),
        Some("stale_controller_revision")
    );
    let rollback = handle_request_value(
        json!({"id": 8103, "type": "control.pid.rollback", "params": {
        "instance_path": "Main.Controller", "expected_active_revision": active, "previous_revision": revision }}),
        &state,
        None,
    );
    assert!(rollback.ok, "rollback failed: {:?}", rollback.error);
    assert_eq!(
        rollback.result.as_ref().unwrap()["pending_revision"],
        revision
    );
    assert!(rollback
        .result
        .as_ref()
        .unwrap()
        .get("active_revision")
        .is_none());
    assert_eq!(state.debug.drain_var_writes().len(), 1);
}

#[test]
fn physical_pid_apply_requires_separate_immediate_approval() {
    let state = pid_state();
    state.experiments.record_simulated_target(false);
    let status = handle_request_value(
        json!({"id": 8110, "type": "control.pid.status", "params": { "instance_path": "Main.Controller" }}),
        &state,
        None,
    );
    let revision = status.result.as_ref().unwrap()["active_revision"]
        .as_str()
        .unwrap();
    let response = handle_request_value(
        json!({"id": 8111, "type": "control.pid.apply", "params": {
        "instance_path": "Main.Controller", "source": "physical", "expected_revision": revision,
        "parameters": { "ProportionalGain": 2.0 }, "physical_approved": false }}),
        &state,
        None,
    );
    assert!(!response.ok);
    assert_eq!(
        response.error_code.as_deref(),
        Some("physical_approval_required")
    );
    assert!(state.debug.drain_var_writes().is_empty());
}

#[test]
fn pid_label_that_contradicts_the_target_is_refused_and_writes_nothing() {
    let state = pid_state();
    state.experiments.record_simulated_target(false);
    let status = handle_request_value(
        json!({"id": 8130, "type": "control.pid.status", "params": { "instance_path": "Main.Controller" }}),
        &state,
        None,
    );
    let revision = status.result.as_ref().unwrap()["active_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let simulation_label = handle_request_value(
        json!({"id": 8131, "type": "control.pid.apply", "params": {
        "instance_path": "Main.Controller", "source": "simulation", "expected_revision": revision,
        "parameters": { "ProportionalGain": 2.0 }, "physical_approved": true }}),
        &state,
        None,
    );
    assert!(!simulation_label.ok);
    assert_eq!(
        simulation_label.error_code.as_deref(),
        Some("invalid_target")
    );
    assert!(format!("{:?}", simulation_label.error).contains(
        "The request says simulation, but the runtime's target is physical."
    ));
    assert!(state.debug.drain_var_writes().is_empty());
}

#[test]
fn pid_status_reads_a_pending_write_and_clears_nothing() {
    let state = pid_state();
    let status = handle_request_value(
        json!({"id": 8140, "type": "control.pid.status", "params": { "instance_path": "Main.Controller" }}),
        &state,
        None,
    );
    let revision = status.result.as_ref().unwrap()["active_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let applied = handle_request_value(
        json!({"id": 8141, "type": "control.pid.apply", "params": {
        "instance_path": "Main.Controller", "source": "simulation", "expected_revision": revision,
        "parameters": { "ProportionalGain": 2.0 }, "physical_approved": false }}),
        &state,
        None,
    );
    assert!(applied.ok, "apply failed: {:?}", applied.error);
    let pending = applied.result.as_ref().unwrap()["pending_revision"].clone();
    for id in [8142, 8143] {
        let read = handle_request_value(
            json!({"id": id, "type": "control.pid.status", "params": { "instance_path": "Main.Controller" }}),
            &state,
            None,
        );
        let result = read.result.expect("status result");
        assert_eq!(result["pending_revision"], pending);
        assert_eq!(result["confirmed"], json!(false));
    }
    let second = handle_request_value(
        json!({"id": 8144, "type": "control.pid.apply", "params": {
        "instance_path": "Main.Controller", "source": "simulation", "expected_revision": revision,
        "parameters": { "ProportionalGain": 3.0 }, "physical_approved": false }}),
        &state,
        None,
    );
    assert_eq!(second.error_code.as_deref(), Some("controller_write_pending"));
    assert!(format!("{:?}", second.error).contains("is pending"));
}

#[test]
fn physical_pid_rollback_needs_the_engineers_approval() {
    let state = pid_state();
    state.experiments.record_simulated_target(false);
    let rollback = handle_request_value(
        json!({"id": 8150, "type": "control.pid.rollback", "params": {
        "instance_path": "Main.Controller", "expected_active_revision": "sha256:0", "previous_revision": "sha256:0" }}),
        &state,
        None,
    );
    assert!(!rollback.ok);
    assert_eq!(
        rollback.error_code.as_deref(),
        Some("physical_approval_required")
    );
    assert!(state.debug.drain_var_writes().is_empty());
}

#[test]
fn invalid_pid_parameter_record_is_rejected_without_a_partial_write() {
    let state = pid_state();
    let before = handle_request_value(
        json!({"id": 8120, "type": "control.pid.status", "params": {
            "instance_path": "Main.Controller"
        }}),
        &state,
        None,
    );
    let before_result = before.result.as_ref().expect("initial status");
    let revision = before_result["active_revision"]
        .as_str()
        .expect("active revision")
        .to_string();
    let proportional_gain = before_result["parameters"]["ProportionalGain"].clone();

    let rejected = handle_request_value(
        json!({"id": 8121, "type": "control.pid.apply", "params": {
            "instance_path": "Main.Controller",
            "source": "simulation",
            "expected_revision": revision,
            "parameters": {
                "ProportionalGain": 2.0,
                "UnknownParameter": 3.0
            },
            "physical_approved": false
        }}),
        &state,
        None,
    );
    assert!(!rejected.ok);
    assert_eq!(
        rejected.error_code.as_deref(),
        Some("invalid_controller_request")
    );
    assert!(
        state.debug.drain_var_writes().is_empty(),
        "an invalid record must not queue even its valid fields"
    );

    let after = handle_request_value(
        json!({"id": 8122, "type": "control.pid.status", "params": {
            "instance_path": "Main.Controller"
        }}),
        &state,
        None,
    );
    let after_result = after.result.as_ref().expect("status after rejection");
    assert_eq!(after_result["active_revision"], revision);
    assert!(after_result["pending_revision"].is_null());
    assert_eq!(
        after_result["parameters"]["ProportionalGain"],
        proportional_gain
    );
}
