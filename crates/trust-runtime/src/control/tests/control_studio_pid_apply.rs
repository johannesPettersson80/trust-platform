fn pid_state() -> ControlState {
    hmi_test_state(r#"
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
"#)
}

#[test]
fn pid_apply_is_revision_guarded_atomic_and_rollback_restores_exact_record() {
    let state = pid_state();
    let status = handle_request_value(json!({"id": 8100, "type": "control.pid.status", "params": { "instance_path": "Main.Controller" }}), &state, None);
    assert!(status.ok, "status failed: {:?}", status.error);
    assert_eq!(status.result.as_ref().unwrap()["parameters"]["AntiWindup"], "BackCalculation");
    let revision = status.result.as_ref().unwrap()["active_revision"].as_str().unwrap().to_string();
    let applied = handle_request_value(json!({"id": 8101, "type": "control.pid.apply", "params": {
        "instance_path": "Main.Controller", "source": "simulation", "expected_revision": revision,
        "parameters": { "ProportionalGain": 2.0, "IntegralGain": 0.5, "DerivativeGain": 0.0,
            "OutputMinimum": 0.0, "OutputMaximum": 100.0, "AntiWindup": "Combined" }, "physical_approved": false }}), &state, None);
    assert!(applied.ok, "apply failed: {:?}", applied.error);
    let result = applied.result.as_ref().unwrap();
    assert_eq!(result["previous_revision"], revision);
    let active = result["pending_revision"].as_str().unwrap().to_string();
    assert_ne!(active, revision);
    assert!(result.get("active_revision").is_none(), "a queued write is not active evidence");
    let writes = state.debug.drain_var_writes();
    assert_eq!(writes.len(), 1, "complete parameter record must be one queued write");
    let applied_status = handle_request_value(json!({"id": 8104, "type": "control.pid.status", "params": {
        "instance_path": "Main.Controller" }}), &state, None);
    assert_eq!(applied_status.result.as_ref().unwrap()["active_revision"], revision);
    assert_eq!(applied_status.result.as_ref().unwrap()["pending_revision"], active);
    assert_eq!(applied_status.result.as_ref().unwrap()["confirmed"], false);

    for write in writes {
        if let crate::debug::PendingVarTarget::Instance(instance_id, name) = write.target {
            state.debug.with_snapshot(|snapshot| snapshot.storage.set_instance_var(instance_id, name, write.value));
        }
    }
    let confirmed_status = handle_request_value(json!({"id": 8105, "type": "control.pid.status", "params": {
        "instance_path": "Main.Controller" }}), &state, None);
    assert_eq!(confirmed_status.result.as_ref().unwrap()["active_revision"], active);
    assert_eq!(confirmed_status.result.as_ref().unwrap()["confirmed"], true);
    assert_eq!(confirmed_status.result.as_ref().unwrap()["parameters"]["AntiWindup"], "Combined");
    let stale = handle_request_value(json!({"id": 8102, "type": "control.pid.apply", "params": {
        "instance_path": "Main.Controller", "source": "simulation", "expected_revision": revision,
        "parameters": { "ProportionalGain": 3.0 }, "physical_approved": false }}), &state, None);
    assert!(!stale.ok);
    assert_eq!(stale.error_code.as_deref(), Some("stale_controller_revision"));
    let rollback = handle_request_value(json!({"id": 8103, "type": "control.pid.rollback", "params": {
        "instance_path": "Main.Controller", "expected_active_revision": active, "previous_revision": revision }}), &state, None);
    assert!(rollback.ok, "rollback failed: {:?}", rollback.error);
    assert_eq!(rollback.result.as_ref().unwrap()["pending_revision"], revision);
    assert!(rollback.result.as_ref().unwrap().get("active_revision").is_none());
    assert_eq!(state.debug.drain_var_writes().len(), 1);
}

#[test]
fn physical_pid_apply_requires_separate_immediate_approval() {
    let state = pid_state();
    let status = handle_request_value(json!({"id": 8110, "type": "control.pid.status", "params": { "instance_path": "Main.Controller" }}), &state, None);
    let revision = status.result.as_ref().unwrap()["active_revision"].as_str().unwrap();
    let response = handle_request_value(json!({"id": 8111, "type": "control.pid.apply", "params": {
        "instance_path": "Main.Controller", "source": "physical", "expected_revision": revision,
        "parameters": { "ProportionalGain": 2.0 }, "physical_approved": false }}), &state, None);
    assert!(!response.ok);
    assert_eq!(response.error_code.as_deref(), Some("physical_approval_required"));
    assert!(state.debug.drain_var_writes().is_empty());
}
