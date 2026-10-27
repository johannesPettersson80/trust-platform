#[test]
fn generic_parameter_status_resolves_custom_symbols_without_pid_wrapper() {
    let state = simulated_control_state("PROGRAM Main VAR Kp : LREAL := 2.0; Ki : LREAL := 0.5; Manual : BOOL := TRUE; END_VAR END_PROGRAM");
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
    static NEXT_APPLY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let transaction_id = format!(
        "generic-apply-{}",
        NEXT_APPLY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    handle_request_value(
        json!({"id":8502,"type":"control.parameters.apply","params":{
            "transaction_id":transaction_id,"binding":generic_binding(),"expected_program_revision":status["program_revision"],
            "expected_binding_revision":status["binding_revision"],"expected_active_revision":status["active_revision"],
            "expected_values":status["values"],"values":{"Main.Kp":4.0,"Main.Ki":1.0},"source":"simulation","physical_approved":false
        }}),
        state,
        None,
    )
}
fn generic_fixture() -> (ControlState, TestHarness) {
    generic_fixture_for("PROGRAM Main VAR Kp : LREAL := 2.0; Ki : LREAL := 0.5; Manual : BOOL := TRUE; Observed : LREAL; END_VAR Observed := Kp + Ki; END_PROGRAM")
}
fn generic_fixture_for(source: &str) -> (ControlState, TestHarness) {
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    // The first scan publishes the program's state, which the requests read (specification 44 §8.8).
    harness.cycle();
    (state, harness)
}
#[test]
fn generic_parameter_confirmation_reverts_studio_values_and_keeps_the_program_change() {
    // Manual, and the program sets Ki itself after the Studio set is committed and before it is confirmed.
    let (state, mut harness) = generic_fixture_for("PROGRAM Main VAR Kp : LREAL := 2.0; Ki : LREAL := 0.5; Manual : BOOL := TRUE; Observed : LREAL; END_VAR IF Manual THEN Ki := 2.0; END_IF; Observed := Kp + Ki; END_PROGRAM");
    let before = generic_status(&state);
    // Studio writes Kp 4 and Ki 1; the program then changes Ki and leaves Kp alone.
    let apply = generic_apply(&state, &before);
    assert!(apply.ok, "{:?}", apply.error);
    assert!(harness.cycle().errors.is_empty());
    let after = generic_status(&state);
    assert_eq!(after["status"], "rejected");
    let reason = after["reason"].as_str().unwrap();
    assert!(
        reason.contains("Main.Ki"),
        "the rejection names the member the program changed: {reason}"
    );
    assert!(!reason.contains("Main.Kp"), "{reason}");
    assert_eq!(
        after["values"]["Main.Kp"], 2.0,
        "Studio's Kp, which the program did not change, returns to its pre-apply value"
    );
    assert_eq!(
        after["values"]["Main.Ki"], 2.0,
        "the program's own Ki stays live"
    );
    assert!(after["previous_revision"].is_null());
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
        json!({"id":8503,"type":"control.parameters.rollback","params":{"transaction_id":"generic-rollback",
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
    // A Live Values write of the mode, applied at the cycle start before the task's boundary.
    state
        .debug
        .try_enqueue_global_write("Main.Manual", Value::Bool(false))
        .expect("queue the mode write");
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
    state
        .debug
        .try_force_global("Main.Kp", Value::LReal(2.0))
        .expect("force the parameter");
    harness.cycle();
    let after = generic_status(&state);
    assert_eq!(after["status"], "rejected");
    assert_eq!(after["values"]["Main.Ki"], 0.5);
}

#[test]
fn generic_parameter_binding_rejects_constant_members() {
    let state=simulated_control_state("PROGRAM Main VAR CONSTANT Kp : LREAL := 2.0; END_VAR VAR Ki : LREAL := 0.5; Manual : BOOL := TRUE; END_VAR END_PROGRAM");
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
    // A compiled program's bindings are its image's (specification 44 §5.6): a parameter bound to an
    // address is refused when the binding is reviewed, before any write is accepted, and the
    // refusal names the address binding.
    let (state, mut harness) = generic_fixture_for("PROGRAM Main VAR Kp AT %QL0 : LREAL := 2.0; Ki : LREAL := 0.5; Manual : BOOL := TRUE; Observed : LREAL; END_VAR Observed := Kp + Ki; END_PROGRAM");
    let status = handle_request_value(
        json!({"id":8501,"type":"control.parameters.status","params":{"binding":generic_binding()}}),
        &state,
        None,
    );
    assert!(
        !status.ok,
        "addressed parameter must not report symbolic application"
    );
    assert!(refusal_text(&status).contains("addressed"), "{:?}", status.error);
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(studio_value(&harness, "Main.Kp"), Value::LReal(2.0));
}

#[test]
fn generic_parameter_identical_reload_cannot_reuse_reviewed_program_revision() {
    let (state, mut harness) = generic_fixture();
    let before = generic_status(&state);
    assert!(generic_apply(&state, &before).ok);
    harness.reload_source("PROGRAM Main VAR Kp : LREAL := 2.0; Ki : LREAL := 0.5; Manual : BOOL := TRUE; Observed : LREAL; END_VAR Observed := Kp + Ki; END_PROGRAM").unwrap();
    harness.cycle();
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
    state
        .debug
        .try_enqueue_global_write("Main.Kp", Value::LReal(3.0))
        .expect("queue the external write");
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
    // Main runs only in MainTask, whose first activation is one interval after the start.
    let (state, mut harness) = generic_fixture_for(
        "PROGRAM MainProgram VAR Kp : LREAL := 2.0; Ki : LREAL := 0.5; Manual : BOOL := TRUE; \
         Observed : LREAL; END_VAR Observed := Kp + Ki; END_PROGRAM \
         CONFIGURATION Plant RESOURCE Controller ON PLC \
         TASK MainTask (INTERVAL := T#20ms, PRIORITY := 1); \
         PROGRAM Main WITH MainTask : MainProgram; END_RESOURCE END_CONFIGURATION",
    );
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
        json!({"id":8511,"type":"control.parameters.apply","params":{"transaction_id":"pending-apply",
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

/// Protocol annex E7.4, E9.3 and E9.4: a set its task's end confirmed is applied, and a fault later in the same scan, in a
/// program outside every task, which runs after the tasks, does not undo that outcome: it ends the session, which drops
/// the retained previous values, so `previous_values` is null. Studio's 4.0 and 1.0 stay in the program, since a fault
/// reverts a transfer's members (A6.3) but no confirmed parameter. Catches a confirmation that waits for the scan's end
/// and is then lost to the fault, reported as `runtime interrupted before parameter confirmation`.
#[test]
fn generic_parameter_set_confirmed_before_a_later_fault_in_its_scan_stays_applied() {
    // Main runs in MainTask, first one interval after the start; Late runs in every scan after the tasks and divides by
    // its divisor.
    let (state, mut harness) = generic_fixture_for(
        "PROGRAM MainProgram VAR Kp : LREAL := 2.0; Ki : LREAL := 0.5; Manual : BOOL := TRUE; \
         Observed : LREAL; END_VAR Observed := Kp + Ki; END_PROGRAM \
         PROGRAM Divider VAR Divisor : DINT := 1; Quotient : DINT; END_VAR \
         Quotient := DINT#100 / Divisor; END_PROGRAM \
         CONFIGURATION Plant RESOURCE Controller ON PLC \
         TASK MainTask (INTERVAL := T#20ms, PRIORITY := 1); \
         PROGRAM Main WITH MainTask : MainProgram; PROGRAM Late : Divider; \
         END_RESOURCE END_CONFIGURATION",
    );
    let mut binding = generic_binding();
    binding["task_name"] = json!("MainTask");
    let status = |state: &ControlState| {
        let response = handle_request_value(
            json!({"id":8514,"type":"control.parameters.status","params":{"binding":binding}}),
            state,
            None,
        );
        assert!(response.ok, "{:?}", response.error);
        response.result.unwrap()
    };
    let before = status(&state);
    let applied = handle_request_value(
        json!({"id":8515,"type":"control.parameters.apply","params":{"transaction_id":"fault-apply",
            "binding":binding,"expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
            "expected_active_revision":before["active_revision"],"expected_values":before["values"],"values":{"Main.Kp":4.0,"Main.Ki":1.0},
            "source":"simulation","physical_approved":false
        }}),
        &state,
        None,
    );
    assert!(applied.ok, "{:?}", applied.error);
    // The scan in which MainTask first runs ends in a division by zero after the task.
    studio_set(&mut harness, "Late.Divisor", Value::DInt(0));
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(
        !harness.cycle().errors.is_empty(),
        "the scan faults after MainTask"
    );
    let after = status(&state);
    assert_eq!(after["status"], "applied", "{after}");
    assert_eq!(after["transaction_id"], "fault-apply");
    assert!(after["reason"].is_null(), "{after}");
    assert!(after["previous_values"].is_null(), "{after}");
    assert_eq!(after["values"]["Main.Kp"], 4.0);
    assert_eq!(after["values"]["Main.Ki"], 1.0);
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
    assert!(
        cancel.ok,
        "cancel owned parameter operation: {:?}",
        cancel.error
    );
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
    assert!(
        !replay.ok,
        "cancelled parameter identity must not be replayable"
    );
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
    assert!(
        cancel.ok,
        "register pre-dispatch cancellation: {:?}",
        cancel.error
    );
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
    assert!(
        !late_apply.ok,
        "a delayed apply must honor its pre-dispatch cancellation fence"
    );
    assert_eq!(generic_status(&state)["values"], before["values"]);
}

/// A parameter apply of the generic binding that names its transaction, source label and approval.
fn generic_apply_as(
    state: &ControlState,
    status: &serde_json::Value,
    transaction_id: &str,
    source: &str,
    approved: bool,
) -> ControlResponse {
    handle_request_value(
        json!({"id":8590,"type":"control.parameters.apply","params":{
            "transaction_id":transaction_id,"binding":generic_binding(),
            "expected_program_revision":status["program_revision"],"expected_binding_revision":status["binding_revision"],
            "expected_active_revision":status["active_revision"],"expected_values":status["values"],
            "values":{"Main.Kp":4.0,"Main.Ki":1.0},"source":source,"physical_approved":approved
        }}),
        state,
        None,
    )
}

#[test]
fn generic_parameter_label_must_agree_with_the_runtime_target_fact() {
    let (state, _harness) = generic_fixture();
    let before = generic_status(&state);
    let physical = generic_apply_as(&state, &before, "label-physical", "physical", true);
    assert!(!physical.ok);
    assert_eq!(
        refusal_text(&physical),
        "The request says physical, but the runtime's target is simulated."
    );
    state.experiments.record_simulated_target(false);
    let simulated = generic_apply_as(&state, &before, "label-simulation", "simulation", false);
    assert!(!simulated.ok);
    assert_eq!(
        refusal_text(&simulated),
        "The request says simulation, but the runtime's target is physical."
    );
}

#[test]
fn generic_parameter_physical_target_needs_approval_at_apply() {
    let (state, _harness) = generic_fixture();
    state.experiments.record_simulated_target(false);
    let before = generic_status(&state);
    let unapproved = generic_apply_as(&state, &before, "physical-unapproved", "physical", false);
    assert!(!unapproved.ok);
    assert_eq!(refusal_text(&unapproved), "physical review required");
    let approved = generic_apply_as(&state, &before, "physical-approved", "physical", true);
    assert!(approved.ok, "{:?}", approved.error);
    assert_eq!(generic_status(&state)["status"], "pending");
}

#[test]
fn generic_parameter_physical_target_at_the_boundary_needs_approval() {
    let (state, mut harness) = generic_fixture();
    let before = generic_status(&state);
    assert!(generic_apply_as(&state, &before, "boundary-approval", "simulation", false).ok);
    // The target turns physical after the review and before the boundary, for example when a fieldbus client appears:
    // the boundary of the programs outside every task, at which the scan commits the set.
    harness
        .runtime_mut()
        .with_program(|program| state.debug.commit_parameters("$background", program, true));
    let after = generic_status(&state);
    assert_eq!(after["status"], "rejected");
    assert_eq!(
        after["reason"],
        "attached I/O requires explicit physical review"
    );
    assert_eq!(after["values"], before["values"]);
}

#[test]
fn generic_parameter_status_keeps_the_retained_values_and_the_rollback() {
    let (state, mut harness) = generic_fixture();
    let before = generic_status(&state);
    assert!(generic_apply_as(&state, &before, "keep-previous", "simulation", false).ok);
    assert!(harness.cycle().errors.is_empty());
    let applied = generic_status(&state);
    assert_eq!(applied["status"], "applied");
    state
        .debug
        .try_enqueue_global_write("Main.Kp", Value::LReal(3.0))
        .expect("queue the external write");
    assert!(harness.cycle().errors.is_empty());
    let after = generic_status(&state);
    assert_eq!(
        after["status"], "applied",
        "a status read does not reset the outcome: {after}"
    );
    assert_eq!(
        after["previous_values"], before["values"],
        "the retained previous values survive a status read"
    );
    let rollback = handle_request_value(
        json!({"id":8560,"type":"control.parameters.rollback","params":{
            "transaction_id":"keep-rollback","binding":generic_binding(),
            "expected_program_revision":after["program_revision"],"expected_binding_revision":after["binding_revision"],
            "expected_active_revision":after["active_revision"],"expected_values":after["values"],
            "previous_revision":after["previous_revision"],"source":"simulation","physical_approved":false
        }}),
        &state,
        None,
    );
    assert!(!rollback.ok);
    assert!(
        refusal_text(&rollback).contains("stale rollback revision"),
        "{:?}",
        rollback.error
    );
}

#[test]
fn generic_parameter_interrupt_keeps_the_reason_of_a_stopped_transaction() {
    let (state, _harness) = generic_fixture();
    let before = generic_status(&state);
    assert!(generic_apply_as(&state, &before, "stopped-apply", "simulation", false).ok);
    state.debug.clear_runtime_mutations();
    let after = generic_status(&state);
    assert_eq!(after["status"], "rejected");
    assert_eq!(
        after["transaction_id"], "stopped-apply",
        "the stopped transaction keeps its identity"
    );
    assert!(
        after["reason"]
            .as_str()
            .unwrap()
            .contains("runtime interrupted"),
        "{after}"
    );
}

#[test]
fn generic_parameter_apply_requires_a_transaction_id() {
    let (state, _harness) = generic_fixture();
    let before = generic_status(&state);
    let response = handle_request_value(
        json!({"id":8570,"type":"control.parameters.apply","params":{
            "binding":generic_binding(),"expected_program_revision":before["program_revision"],
            "expected_binding_revision":before["binding_revision"],"expected_active_revision":before["active_revision"],
            "expected_values":before["values"],"values":{"Main.Kp":4.0,"Main.Ki":1.0},
            "source":"simulation","physical_approved":false
        }}),
        &state,
        None,
    );
    assert!(!response.ok);
    assert!(
        refusal_text(&response).contains("transaction_id"),
        "{:?}",
        response.error
    );
    assert_eq!(generic_status(&state)["status"], "idle", "nothing is registered");
}

#[test]
fn generic_parameter_refusal_names_the_running_parameter_transaction() {
    let (state, _harness) = generic_fixture();
    let before = generic_status(&state);
    assert!(generic_apply_as(&state, &before, "p-7", "simulation", false).ok);
    let second = generic_apply_as(&state, &before, "p-8", "simulation", false);
    assert!(!second.ok);
    assert_eq!(
        refusal_text(&second),
        "a parameter transaction p-7 is pending"
    );
}

#[test]
fn generic_parameter_stale_value_names_the_stale_member_with_expected_and_actual() {
    let (state, _harness) = generic_fixture();
    let before = generic_status(&state);
    let response = handle_request_value(
        json!({"id":8580,"type":"control.parameters.apply","params":{
            "transaction_id":"stale-value","binding":generic_binding(),
            "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
            "expected_active_revision":before["active_revision"],"expected_values":{"Main.Kp":99.0,"Main.Ki":0.5},
            "values":{"Main.Kp":4.0,"Main.Ki":1.0},"source":"simulation","physical_approved":false
        }}),
        &state,
        None,
    );
    assert!(!response.ok);
    assert_eq!(
        refusal_text(&response),
        "stale program, binding, or expected values: Main.Kp expected 99, observed 2"
    );
}

#[test]
fn generic_parameter_identifiers_are_counted_in_characters() {
    let (state, _harness) = generic_fixture();
    let before = generic_status(&state);
    // 128 characters are 256 bytes: a byte count would refuse this identifier.
    let id = "é".repeat(128);
    assert!(generic_apply_as(&state, &before, &id, "simulation", false).ok);
    let oversized = "é".repeat(129);
    let refused = handle_request_value(
        json!({"id":8591,"type":"control.parameters.cancel","params":{"loop_id":"engineer-loop",
            "transaction_id":oversized,"expected_program_revision":before["program_revision"],
            "expected_binding_revision":before["binding_revision"]}}),
        &state,
        None,
    );
    assert!(!refused.ok, "129 characters exceed the limit");
    let cancelled = handle_request_value(
        json!({"id":8592,"type":"control.parameters.cancel","params":{"loop_id":"engineer-loop",
            "transaction_id":id,"expected_program_revision":before["program_revision"],
            "expected_binding_revision":before["binding_revision"]}}),
        &state,
        None,
    );
    assert!(cancelled.ok, "{:?}", cancelled.error);
    assert_eq!(cancelled.result.unwrap()["confirmed"], true);
}

#[test]
fn runtime_epoch_is_unique_across_runtime_instances_and_takes_a_new_value_on_reload() {
    let source = "PROGRAM Main VAR Kp : LREAL := 2.0; END_VAR END_PROGRAM";
    let mut first = TestHarness::from_source(source).unwrap();
    let mut second = TestHarness::from_source(source).unwrap();
    let first_debug = studio_debug(&mut first);
    let second_debug = studio_debug(&mut second);
    let first_epoch = first_debug.runtime_epoch();
    assert_ne!(
        first_epoch,
        second_debug.runtime_epoch(),
        "two runtimes in one process share no epoch"
    );
    assert!(
        first_epoch >= 1_000_000_000_000,
        "the epoch starts at the process start time: {first_epoch}"
    );
    assert!(
        first_epoch < 1_u64 << 53,
        "the epoch stays exact in a JSON number: {first_epoch}"
    );
    first_debug.invalidate_control_program();
    assert!(
        first_debug.runtime_epoch() > first_epoch,
        "a reload takes a new epoch"
    );
}
