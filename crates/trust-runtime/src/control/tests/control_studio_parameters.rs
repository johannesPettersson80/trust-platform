#[test]
fn generic_parameter_status_resolves_custom_symbols_without_pid_wrapper() {
    let state = hmi_test_state("PROGRAM Main VAR Kp : LREAL := 2.0; Ki : LREAL := 0.5; Manual : BOOL := TRUE; END_VAR END_PROGRAM");
    let response = handle_request_value(
        json!({"id":8500,"type":"control.parameters.status","params":{"binding":{
            "loop_id":"engineer-loop", "task_name":"$background", "parameters":[
                {"symbol":"Main.Kp","iec_type":"LREAL","unit":"points/C","convention":"parallel Kp","minimum":0,"maximum":10},
                {"symbol":"Main.Ki","iec_type":"LREAL","unit":"points/C/s","convention":"parallel Ki","minimum":0,"maximum":10}
            ],"mode":{"symbol":"Main.Manual","manual_value":true}
        }}}),
        &state,
        None,
    );
    assert!(
        response.ok,
        "custom parameters status must resolve: {:?}",
        response.error
    );
    let result = response.result.unwrap();
    assert_eq!(result["values"]["Main.Kp"], 2.0);
    assert_eq!(result["values"]["Main.Ki"], 0.5);
    assert_eq!(result["status"], "idle");
    assert!(result["program_revision"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
}

fn generic_binding() -> serde_json::Value {
    json!({"loop_id":"engineer-loop","task_name":"$background","parameters":[
        {"symbol":"Main.Kp","iec_type":"LREAL","unit":"points/C","convention":"parallel Kp","minimum":0,"maximum":10},
        {"symbol":"Main.Ki","iec_type":"LREAL","unit":"points/C/s","convention":"parallel Ki","minimum":0,"maximum":10}
    ],"mode":{"symbol":"Main.Manual","manual_value":true}})
}
fn generic_status(state: &ControlState) -> serde_json::Value {
    let result = handle_request_value(
        json!({"id":8501,"type":"control.parameters.status","params":{"binding":generic_binding()}}),
        state,
        None,
    );
    assert!(result.ok, "{:?}", result.error);
    result.result.unwrap()
}
fn generic_apply(state: &ControlState, status: &serde_json::Value) -> ControlResponse {
    handle_request_value(
        json!({"id":8502,"type":"control.parameters.apply","params":{
            "binding":generic_binding(),"expected_program_revision":status["program_revision"],
            "expected_binding_revision":status["binding_revision"],"expected_active_revision":status["active_revision"],
            "expected_values":status["values"],"values":{"Main.Kp":4.0,"Main.Ki":1.0},"source":"simulation","physical_approved":false
        }}),
        state,
        None,
    )
}
fn generic_fixture() -> (ControlState, TestHarness) {
    let source = "PROGRAM Main VAR Kp : LREAL := 2.0; Ki : LREAL := 0.5; Manual : BOOL := TRUE; Observed : LREAL; END_VAR Observed := Kp + Ki; END_PROGRAM";
    let mut state = hmi_test_state(source);
    let mut harness = TestHarness::from_source(source).unwrap();
    state.debug = harness.runtime_mut().enable_debug();
    harness.cycle();
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    state.debug.refresh_snapshot_from_storage(
        harness.runtime().storage(),
        harness.runtime().current_time(),
    );
    (state, harness)
}
#[test]
fn generic_parameter_apply_commits_whole_set_before_controller_execution_and_rolls_back() {
    let (state, mut harness) = generic_fixture();
    let before = generic_status(&state);
    let applied = generic_apply(&state, &before);
    assert!(applied.ok, "{:?}", applied.error);
    assert_eq!(generic_status(&state)["status"], "pending");
    assert!(
        state.debug.drain_var_writes().is_empty(),
        "transaction must not use separate variable writes"
    );
    harness.cycle();
    let after = generic_status(&state);
    assert_eq!(after["status"], "applied");
    assert_eq!(after["values"]["Main.Kp"], 4.0);
    assert_eq!(after["values"]["Main.Ki"], 1.0);
    assert_ne!(after["active_revision"], before["active_revision"]);
    let rollback = handle_request_value(
        json!({"id":8503,"type":"control.parameters.rollback","params":{
            "binding":generic_binding(),"expected_program_revision":after["program_revision"],
            "expected_binding_revision":after["binding_revision"],"expected_active_revision":after["active_revision"],
            "expected_values":after["values"],"previous_revision":before["active_revision"],"source":"simulation","physical_approved":false
        }}),
        &state,
        None,
    );
    assert!(rollback.ok, "{:?}", rollback.error);
    harness.cycle();
    assert_eq!(generic_status(&state)["values"], before["values"]);
}
#[test]
fn generic_parameter_mode_race_rejects_every_write_at_commit() {
    let (state, mut harness) = generic_fixture();
    let before = generic_status(&state);
    assert!(generic_apply(&state, &before).ok);
    let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
        panic!("program");
    };
    state
        .debug
        .enqueue_instance_write(*id, "Manual", Value::Bool(false));
    harness.cycle();
    let after = generic_status(&state);
    assert_eq!(after["status"], "rejected");
    assert_eq!(
        after["values"], before["values"],
        "mode race must apply no members"
    );
}

#[test]
fn generic_parameter_simulated_driver_does_not_require_physical_review() {
    let (state, mut harness) = generic_fixture();
    harness
        .runtime_mut()
        .add_io_driver("simulated", Box::new(crate::io::SimulatedIoDriver));
    let before = generic_status(&state);
    assert!(generic_apply(&state, &before).ok);
    harness.cycle();
    let after = generic_status(&state);
    assert_eq!(
        after["status"], "applied",
        "typed simulated drivers must stay in simulation: {after}"
    );
}
#[test]
fn generic_parameter_restart_invalidates_identical_program_and_values() {
    let (state, _harness) = generic_fixture();
    let before = generic_status(&state);
    state.debug.clear_runtime_mutations();
    let after = generic_status(&state);
    assert_ne!(before["program_revision"], after["program_revision"]);
    assert!(!generic_apply(&state, &before).ok);
}
#[test]
fn generic_parameter_forcing_race_rejects_without_partial_commit() {
    let (state, mut harness) = generic_fixture();
    let before = generic_status(&state);
    assert!(generic_apply(&state, &before).ok);
    let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
        panic!("program")
    };
    state.debug.force_instance(*id, "Kp", Value::LReal(2.0));
    harness.cycle();
    let after = generic_status(&state);
    assert_eq!(after["status"], "rejected");
    assert_eq!(after["values"]["Main.Ki"], 0.5);
}

#[test]
fn generic_parameter_binding_rejects_constant_members() {
    let state=hmi_test_state("PROGRAM Main VAR CONSTANT Kp : LREAL := 2.0; END_VAR VAR Ki : LREAL := 0.5; Manual : BOOL := TRUE; END_VAR END_PROGRAM");
    let response = handle_request_value(
        json!({"id":8504,"type":"control.parameters.status","params":{"binding":generic_binding()}}),
        &state,
        None,
    );
    assert!(
        !response.ok,
        "constant controller parameters must not be writable bindings"
    );
}
struct GenericPhysicalDriver;
impl crate::io::IoDriver for GenericPhysicalDriver {
    fn read_inputs(&mut self, _: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn write_outputs(&mut self, _: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
}
#[test]
fn generic_parameter_client_simulation_label_cannot_bypass_physical_review() {
    let (state, mut harness) = generic_fixture();
    harness
        .runtime_mut()
        .add_io_driver("arbitrary-name", Box::new(GenericPhysicalDriver));
    let before = generic_status(&state);
    assert!(generic_apply(&state, &before).ok);
    harness.cycle();
    let after = generic_status(&state);
    assert_eq!(after["status"], "rejected");
    assert_eq!(after["values"], before["values"]);
}

#[test]
fn generic_parameter_direct_address_binding_is_not_mutated_as_symbolic_storage() {
    let (state, mut harness) = generic_fixture();
    let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
        panic!("program")
    };
    let reference = harness
        .runtime()
        .storage()
        .resolved_instance_field_ref(*id, "Kp")
        .unwrap();
    harness.runtime_mut().io_mut().bind_ref_typed(
        reference,
        crate::io::IoAddress::parse("%QL0").unwrap(),
        trust_hir::TypeId::LREAL,
    );
    let before = generic_status(&state);
    assert!(generic_apply(&state, &before).ok);
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    let after = generic_status(&state);
    assert_eq!(
        after["status"], "rejected",
        "addressed parameter must not report symbolic application"
    );
    assert!(
        after["reason"].as_str().unwrap().contains("addressed"),
        "{after}"
    );
}

#[test]
fn generic_parameter_identical_reload_cannot_reuse_reviewed_program_revision() {
    let (state, mut harness) = generic_fixture();
    let before = generic_status(&state);
    assert!(generic_apply(&state, &before).ok);
    harness.reload_source("PROGRAM Main VAR Kp : LREAL := 2.0; Ki : LREAL := 0.5; Manual : BOOL := TRUE; Observed : LREAL; END_VAR Observed := Kp + Ki; END_PROGRAM").unwrap();
    harness.cycle();
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    let after = generic_status(&state);
    assert_ne!(
        before["program_revision"], after["program_revision"],
        "identical code reload must invalidate reviewed identity"
    );
    assert_eq!(
        after["values"], before["values"],
        "old queued transaction cannot survive reload"
    );
}

#[test]
fn generic_parameter_external_value_race_retains_rejection_and_invalidates_rollback() {
    let (state, mut harness) = generic_fixture();
    let before = generic_status(&state);
    assert!(generic_apply(&state, &before).ok);
    let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
        panic!("program")
    };
    state
        .debug
        .enqueue_instance_write(*id, "Kp", Value::LReal(3.0));
    harness.cycle();
    let after = generic_status(&state);
    assert_eq!(
        after["status"], "rejected",
        "external-write rejection must remain visible"
    );
    assert_eq!(after["values"]["Main.Kp"], 3.0);
    assert_eq!(after["values"]["Main.Ki"], 0.5);
    assert!(after["previous_revision"].is_null());
}

#[test]
fn generic_parameter_transaction_waits_for_owning_task_and_controller_observes_complete_set() {
    let (state, mut harness) = generic_fixture();
    harness
        .runtime_mut()
        .register_task(crate::task::TaskConfig {
            name: "MainTask".into(),
            interval: crate::value::Duration::from_millis(20),
            single: None,
            priority: 1,
            programs: vec!["Main".into()],
            fb_instances: vec![],
        });
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    let mut binding = generic_binding();
    binding["task_name"] = json!("MainTask");
    let status = handle_request_value(
        json!({"id":8510,"type":"control.parameters.status","params":{"binding":binding}}),
        &state,
        None,
    );
    assert!(status.ok, "{:?}", status.error);
    let before = status.result.unwrap();
    let applied = handle_request_value(
        json!({"id":8511,"type":"control.parameters.apply","params":{
            "binding":binding,"expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
            "expected_active_revision":before["active_revision"],"expected_values":before["values"],"values":{"Main.Kp":4.0,"Main.Ki":1.0},
            "source":"simulation","physical_approved":false
        }}),
        &state,
        None,
    );
    assert!(applied.ok, "{:?}", applied.error);
    assert!(harness.cycle().errors.is_empty());
    let pending = handle_request_value(
        json!({"id":8512,"type":"control.parameters.status","params":{"binding":binding}}),
        &state,
        None,
    );
    assert_eq!(
        pending.result.unwrap()["status"],
        "pending",
        "no commit before owning task becomes ready"
    );
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    let after = handle_request_value(
        json!({"id":8513,"type":"control.parameters.status","params":{"binding":binding}}),
        &state,
        None,
    );
    assert_eq!(after.result.unwrap()["status"], "applied");
    assert_eq!(harness.get_output("Main.Observed"), Some(Value::LReal(5.0)));
}

#[test]
fn generic_parameter_cancel_before_commit_fences_the_owned_identity() {
    let (state, mut harness) = generic_fixture();
    let before = generic_status(&state);
    let apply = handle_request_value(
        json!({"id":8520,"type":"control.parameters.apply","params":{
            "transaction_id":"parameter-cancel-before-commit","binding":generic_binding(),
            "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
            "expected_active_revision":before["active_revision"],"expected_values":before["values"],
            "values":{"Main.Kp":4.0,"Main.Ki":1.0},"source":"simulation","physical_approved":false
        }}),
        &state,
        None,
    );
    assert!(apply.ok, "queue parameter operation: {:?}", apply.error);
    let cancel = handle_request_value(
        json!({"id":8521,"type":"control.parameters.cancel","params":{
            "loop_id":"engineer-loop","transaction_id":"parameter-cancel-before-commit",
            "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"]
        }}),
        &state,
        None,
    );
    assert!(cancel.ok, "cancel owned parameter operation: {:?}", cancel.error);
    assert_eq!(cancel.result.unwrap()["before_dispatch"], false);
    assert!(harness.cycle().errors.is_empty());
    let after = generic_status(&state);
    assert_eq!(after["status"], "rejected");
    assert_eq!(after["values"], before["values"]);
    assert!(!state.debug.has_parameter_transaction());
    let replay = handle_request_value(
        json!({"id":8522,"type":"control.parameters.apply","params":{
            "transaction_id":"parameter-cancel-before-commit","binding":generic_binding(),
            "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
            "expected_active_revision":before["active_revision"],"expected_values":before["values"],
            "values":{"Main.Kp":4.0,"Main.Ki":1.0},"source":"simulation","physical_approved":false
        }}),
        &state,
        None,
    );
    assert!(!replay.ok, "cancelled parameter identity must not be replayable");
}

#[test]
fn generic_parameter_cancel_before_dispatch_rejects_a_late_apply() {
    let (state, _harness) = generic_fixture();
    let before = generic_status(&state);
    let cancel = handle_request_value(
        json!({"id":8523,"type":"control.parameters.cancel","params":{
            "loop_id":"engineer-loop","transaction_id":"parameter-fenced-before-dispatch",
            "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"]
        }}),
        &state,
        None,
    );
    assert!(cancel.ok, "register pre-dispatch cancellation: {:?}", cancel.error);
    assert_eq!(cancel.result.unwrap()["before_dispatch"], true);
    let late_apply = handle_request_value(
        json!({"id":8524,"type":"control.parameters.apply","params":{
            "transaction_id":"parameter-fenced-before-dispatch","binding":generic_binding(),
            "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
            "expected_active_revision":before["active_revision"],"expected_values":before["values"],
            "values":{"Main.Kp":4.0,"Main.Ki":1.0},"source":"simulation","physical_approved":false
        }}),
        &state,
        None,
    );
    assert!(!late_apply.ok, "a delayed apply must honor its pre-dispatch cancellation fence");
    assert_eq!(generic_status(&state)["values"], before["values"]);
}
