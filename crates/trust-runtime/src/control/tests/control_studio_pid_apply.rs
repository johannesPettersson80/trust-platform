/// A stand-in for the library's `Control.PID` (specification 45): its parameter record, and the mode observation its
/// `Update` keeps, here declared as a controller whose last `Update` ran in Manual (protocol annex E7.7).
const PID_SOURCE: &str = r#"
TYPE PIDMode : (Auto := 0, Manual := 1, Track := 2); END_TYPE
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
VAR ModeValue : PIDMode := PIDMode#Manual; ModeObserved : BOOL := TRUE; END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR Controller : PID; END_VAR
END_PROGRAM
"#;

fn pid_state() -> ControlState {
    let state = control_test_state(PID_SOURCE);
    // The target is simulated until a test says otherwise: a scan has published a simulated target (A2.4).
    state.experiments.record_simulated_target(true);
    state
}

#[test]
fn pid_apply_is_revision_guarded_atomic_and_rollback_restores_exact_record() {
    let mut state = pid_state();
    // The controller runs in a simulated runtime whose scans publish its values to the control (specification 44 §8.8).
    let mut harness = simulated_harness(PID_SOURCE);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
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
            "OutputMinimum": 0.0, "OutputMaximum": 100.0, "TrackingTime": 1.0, "AntiWindup": "Combined" },
        "physical_approved": false }}),
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
    // E7.7: the complete record is one parameter set, committed whole at the boundary of the task that runs Main (the
    // programs outside every task here), never a queued variable write.
    assert!(
        state.debug.drain_var_writes().is_empty(),
        "a controller write is no queued variable write"
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

    // The next scan commits the record at Main's boundary, confirms it after Main ran, and publishes it.
    assert!(harness.cycle().errors.is_empty());
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
    // E7.5, E7.7: the rollback is the same parameter set, writing the retained record back at the next boundary.
    assert!(state.debug.drain_var_writes().is_empty());
    assert!(harness.cycle().errors.is_empty());
    let restored = handle_request_value(
        json!({"id": 8106, "type": "control.pid.status", "params": {
        "instance_path": "Main.Controller" }}),
        &state,
        None,
    );
    let restored = restored.result.expect("status after the rollback");
    assert_eq!(restored["active_revision"], json!(revision));
    assert_eq!(restored["confirmed"], json!(true));
    assert!(restored["pending_revision"].is_null(), "{restored}");
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

/// The record `Main.Controller` reports now.
fn pid_status(state: &ControlState, id: u64) -> serde_json::Value {
    let status = handle_request_value(
        json!({"id": id, "type": "control.pid.status", "params": { "instance_path": "Main.Controller" }}),
        state,
        None,
    );
    assert!(status.ok, "status failed: {:?}", status.error);
    status.result.expect("controller status")
}

/// Applies `parameters` to `Main.Controller` against the revision its status reports now.
fn pid_apply(state: &ControlState, id: u64, parameters: serde_json::Value) -> ControlResponse {
    let revision = pid_status(state, id)["active_revision"].clone();
    handle_request_value(
        json!({"id": id + 1, "type": "control.pid.apply", "params": {
        "instance_path": "Main.Controller", "source": "simulation", "expected_revision": revision,
        "parameters": parameters, "physical_approved": false }}),
        state,
        None,
    )
}

/// Protocol annex E7.7, with E7.3 step 7: a controller write needs the controller to have run in Manual, which the
/// controller records in `ModeObserved` and `ModeValue`. A controller whose last `Update` ran in Auto, and one that has
/// not run an `Update` since the start, are refused with that reason, and nothing is pending. Catches a write queued
/// without the mode check, and a controller that never ran read as running in Manual.
#[test]
fn a_pid_write_needs_a_controller_that_ran_in_manual() {
    for (source, ran) in [
        (
            PID_SOURCE.replace("PIDMode#Manual", "PIDMode#Auto"),
            "its last Update ran in Auto",
        ),
        (
            PID_SOURCE.replace(
                "ModeObserved : BOOL := TRUE",
                "ModeObserved : BOOL := FALSE",
            ),
            "it has not run an Update since the start",
        ),
    ] {
        let state = control_test_state(&source);
        state.experiments.record_simulated_target(true);
        let refused = pid_apply(&state, 8160, json!({ "ProportionalGain": 2.0 }));
        assert!(!refused.ok);
        assert_eq!(
            refused.error_code.as_deref(),
            Some("invalid_controller_request")
        );
        let reason = refusal_text(&refused);
        assert!(
            reason.contains("confirmed Manual mode required"),
            "{reason}"
        );
        assert!(reason.contains(ran), "{reason}");
        let after = pid_status(&state, 8162);
        assert!(after["pending_revision"].is_null(), "{after}");
        assert_eq!(after["confirmed"], json!(true));
    }
}

/// Protocol annex E7.7 ("a bounds check of each value") with the operating domain of specification 45 §2.4: output limits
/// in order, a falling rate limit not above 0 and a rising one not below it, and a positive tracking time with integral
/// action. The fixture's record is zero, so `IntegralGain` 0.5 alone leaves `TrackingTime` at 0. Each record outside the
/// domain is refused, and nothing is pending. Catches a write that only checks that each number is finite.
#[test]
fn a_pid_record_outside_the_controllers_operating_domain_is_refused() {
    let state = pid_state();
    for (parameters, rule) in [
        (
            json!({ "OutputMinimum": 50.0, "OutputMaximum": 10.0 }),
            "OutputMinimum 50 exceeds OutputMaximum 10",
        ),
        (
            json!({ "RateMinimum": 1.0 }),
            "RateMinimum <= 0 <= RateMaximum",
        ),
        (
            json!({ "IntegralGain": 0.5 }),
            "TrackingTime > 0 when IntegralGain <> 0",
        ),
    ] {
        let refused = pid_apply(&state, 8170, parameters);
        assert!(!refused.ok);
        assert_eq!(
            refused.error_code.as_deref(),
            Some("invalid_controller_request")
        );
        assert!(refusal_text(&refused).contains(rule), "{:?}", refused.error);
        assert!(pid_status(&state, 8172)["pending_revision"].is_null());
    }
}

/// The controller of [`PID_SOURCE`] with a method its program calls in each execution of `Loop`, every 20 ms from one
/// interval after the start: it sets the record's `Bias` to 5.
const PID_TASK_SOURCE: &str = r#"
TYPE PIDMode : (Auto := 0, Manual := 1, Track := 2); END_TYPE
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
VAR ModeValue : PIDMode := PIDMode#Manual; ModeObserved : BOOL := TRUE; END_VAR
METHOD PUBLIC SetBias
VAR_INPUT Bias : LREAL; END_VAR
ParametersValue.Bias := Bias;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM MainProgram
VAR Controller : PID; END_VAR
Controller.SetBias(Bias := 5.0);
END_PROGRAM
CONFIGURATION Plant
RESOURCE Cell ON PLC
TASK Loop (INTERVAL := T#20ms, PRIORITY := 1);
PROGRAM Main WITH Loop : MainProgram;
END_RESOURCE
END_CONFIGURATION
"#;

/// Runs the scan at the start, in which `Loop` is not due, and the scan 20 ms later, in which `Loop` runs `Main` for the
/// first time: the program sets `Bias` to 5.
fn pid_task_harness(state: &mut ControlState) -> TestHarness {
    state.experiments.record_simulated_target(true);
    let mut harness = simulated_harness(PID_TASK_SOURCE);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    harness
}

/// Protocol annex E7.7 with E7.4 and A6.8: a controller write is committed at the start of the task that runs the
/// controller and confirmed after it. After `Loop` has run once (`Bias` 5), a write of `ProportionalGain` 2 and `Bias` 3
/// writes nothing in a scan in which `Loop` is not due; when `Loop` runs 20 ms later, the record is committed, the program
/// sets `Bias` to 5 over the written 3, and the confirmation rejects the write naming `Bias`: the field the program did not
/// change, `ProportionalGain`, returns to its previous 0, and `Bias` keeps the program's 5. Catches a write applied outside
/// its task's boundary, and a rejection that keeps Studio's other values or overwrites the program's.
#[test]
fn a_pid_write_is_committed_at_its_task_boundary_and_rejected_when_the_program_changes_it() {
    let mut state = control_test_state(PID_TASK_SOURCE);
    let mut harness = pid_task_harness(&mut state);
    assert_eq!(pid_status(&state, 8180)["parameters"]["Bias"], 5.0);
    let applied = pid_apply(
        &state,
        8181,
        json!({ "ProportionalGain": 2.0, "Bias": 3.0 }),
    );
    assert!(applied.ok, "apply failed: {:?}", applied.error);
    assert!(harness.cycle().errors.is_empty());
    let waiting = pid_status(&state, 8183);
    assert_eq!(waiting["confirmed"], json!(false), "{waiting}");
    assert_eq!(
        waiting["parameters"]["ProportionalGain"], 0.0,
        "nothing is written before the task that runs the controller"
    );
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    let rejected = pid_status(&state, 8184);
    assert_eq!(rejected["confirmed"], json!(true), "{rejected}");
    assert!(rejected["pending_revision"].is_null(), "{rejected}");
    let reason = rejected["reason"].as_str().expect("the rejection's reason");
    assert!(
        reason.contains("controller changed parameter values before confirmation"),
        "{reason}"
    );
    assert!(
        reason.contains("Main.Controller.ParametersValue.Bias"),
        "{reason}"
    );
    assert!(!reason.contains("ProportionalGain"), "{reason}");
    assert_eq!(rejected["parameters"]["ProportionalGain"], 0.0);
    assert_eq!(rejected["parameters"]["Bias"], 5.0);
}

/// Protocol annex E7.7 and A6.7: a controller's record of its mode counts only once the controller's task has run in the
/// current program. An online change keeps every member (specification 44 §12.2) and continues the task schedule, so
/// right after it `ModeObserved` and `ModeValue` still hold the old program's last mode, and `Loop`, which ran at 20 ms, is
/// next due at 40 ms: a write in the scan at 20 ms is refused, and the same write after `Loop` ran at 40 ms is accepted.
/// Catches a mode taken from the program before the change.
#[test]
fn a_pid_write_after_an_online_change_waits_for_the_controllers_task_to_run() {
    let mut state = control_test_state(PID_TASK_SOURCE);
    let mut harness = pid_task_harness(&mut state);
    harness
        .reload_source(PID_TASK_SOURCE)
        .expect("change the program online");
    assert!(harness.cycle().errors.is_empty());
    let refused = pid_apply(&state, 8200, json!({ "ProportionalGain": 2.0 }));
    assert!(!refused.ok);
    assert_eq!(
        refused.error_code.as_deref(),
        Some("invalid_controller_request")
    );
    assert!(
        refusal_text(&refused)
            .contains("its task Loop has not run since the program was loaded or restarted"),
        "{:?}",
        refused.error
    );
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    let accepted = pid_apply(&state, 8202, json!({ "ProportionalGain": 2.0 }));
    assert!(accepted.ok, "apply failed: {:?}", accepted.error);
}

/// Protocol annex E7.7 with E7.4: the mode is checked again at the task's boundary. A controller that ran in Manual when
/// the write was accepted, and whose last `Update` ran in Auto by the boundary, takes nothing: the write is rejected with
/// the mode's reason and the record keeps its values. Catches a mode check made only when the write is accepted.
#[test]
fn a_pid_write_whose_controller_left_manual_before_its_boundary_is_rejected() {
    let mut state = pid_state();
    let mut harness = simulated_harness(PID_SOURCE);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    let applied = pid_apply(&state, 8190, json!({ "ProportionalGain": 2.0 }));
    assert!(applied.ok, "apply failed: {:?}", applied.error);
    // Before the next scan the controller's last `Update` runs in Auto, as the controller records it.
    let auto = Value::Enum(Box::new(crate::value::EnumValue::from_canonical_parts(
        "PIDMode".into(),
        "Auto".into(),
        0,
    )));
    studio_set(&mut harness, "Main.Controller.ModeValue", auto);
    assert!(harness.cycle().errors.is_empty());
    let rejected = pid_status(&state, 8192);
    assert_eq!(rejected["confirmed"], json!(true), "{rejected}");
    let reason = rejected["reason"].as_str().expect("the rejection's reason");
    assert!(reason.contains("its last Update ran in Auto"), "{reason}");
    assert_eq!(rejected["parameters"]["ProportionalGain"], 0.0);
}
