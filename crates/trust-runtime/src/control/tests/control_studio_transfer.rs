fn transfer_binding() -> serde_json::Value {
    let number = |symbol: &str, unit: &str| json!({"symbol":symbol,"iec_type":"LREAL","unit":unit,"minimum":-20,"maximum":20});
    json!({"loop_id":"ordinary-loop","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[number("Main.SP","m"),number("Main.ManualCommand","V")],
        "guards":[number("Main.Kp","V/m"),number("Main.PV","m")],
        "output":number("Main.MV","V")})
}

fn transfer_fixture() -> (ControlState, TestHarness) {
    transfer_fixture_with_alias("Alias : LREAL := 10.0;")
}

/// The transfer fixture whose `Alias` variable is declared as `alias`: unbound, or bound to an address that the image's
/// bindings then name (specification 44 §5.6).
fn transfer_fixture_with_alias(alias: &str) -> (ControlState, TestHarness) {
    let source = format!("PROGRAM Main VAR Kp : LREAL := 2.0; Manual : BOOL := TRUE; ManualCommand : LREAL := 2.0; SP AT %ML0 : LREAL := 10.0; PV : LREAL := 10.0; MV AT %QL0 : LREAL; {alias} END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := Kp * (SP-PV); END_IF; END_PROGRAM");
    let mut state = simulated_control_state(&source);
    let mut harness = simulated_harness(&source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    (state, harness)
}

fn transfer_status(state: &ControlState) -> serde_json::Value {
    let response = handle_request_value(
        json!({"id":8600,"type":"control.transfer.status","params":{"binding":transfer_binding()}}),
        state,
        None,
    );
    assert!(
        response.ok,
        "guarded transfer status must resolve the memory reference and symbolic mode: {:?}",
        response.error
    );
    response.result.unwrap()
}

fn queue_transfer(state: &ControlState, before: &serde_json::Value) -> ControlResponse {
    handle_request_value(
        json!({"id":8601,"type":"control.transfer.apply","params":{
        "transaction_id":"transfer-1","binding":transfer_binding(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
        state,
        None,
    )
}

#[test]
fn controller_transfer_commits_reference_and_mode_together_before_actual_st() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    assert_eq!(before["values"]["Main.Manual"], true);
    assert_eq!(before["values"]["Main.MV"], 2.0);
    let request = json!({"id":8601,"type":"control.transfer.apply","params":{
        "transaction_id":"transfer-1","binding":transfer_binding(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}});
    let response = handle_request_value(request, &state, None);
    assert!(response.ok, "{:?}", response.error);
    assert_eq!(response.result.unwrap()["status"], "pending");
    assert_eq!(
        transfer_status(&state)["values"],
        before["values"],
        "queued is not adopted"
    );
    assert!(state.debug.drain_var_writes().is_empty());
    assert!(state.debug.drain_io_writes().is_empty());
    assert!(harness.cycle().errors.is_empty());
    let after = transfer_status(&state);
    assert_eq!(after["status"], "applied");
    assert_eq!(after["values"]["Main.Manual"], false);
    assert_eq!(after["values"]["Main.SP"], 11.0);
    assert_eq!(
        after["values"]["Main.Kp"], 2.0,
        "transfer cannot retune the verified gain"
    );
    assert_eq!(
        after["values"]["Main.MV"], 2.0,
        "first actual Auto output is continuous"
    );
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        transfer_status(&state)["values"]["Main.SP"],
        11.0,
        "memory image retains the transferred reference next scan"
    );
}

#[test]
fn controller_transfer_status_reports_the_runtime_clock_of_its_scan() {
    let (state, mut harness) = transfer_fixture();
    let first = transfer_status(&state)["runtime_time_ns"]
        .as_i64()
        .expect("the status names the runtime time of the scan it reports");
    assert_eq!(first, harness.runtime().current_time().as_nanos());
    harness.advance_time(crate::value::Duration::from_millis(100));
    assert!(harness.cycle().errors.is_empty());
    let second = transfer_status(&state)["runtime_time_ns"].as_i64().unwrap();
    assert!(
        second > first,
        "the runtime clock advances with the scans it reports: {first} then {second}"
    );
}

#[test]
fn controller_transfer_pending_binding_cannot_be_replaced_by_status() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    assert!(queue_transfer(&state, &before).ok);
    let mut replacement = transfer_binding();
    replacement["states"][0]["maximum"] = json!(15.0);
    let response = handle_request_value(
        json!({"id":8602,"type":"control.transfer.status","params":{"binding":replacement}}),
        &state,
        None,
    );
    assert!(
        !response.ok,
        "status must reject binding replacement while a reviewed transfer is pending"
    );
    assert_eq!(
        transfer_status(&state)["binding_revision"],
        before["binding_revision"]
    );
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(
        transfer_status(&state)["status"],
        "applied",
        "rejected replacement preserves the original transaction"
    );
}

#[test]
fn controller_transfer_competing_value_or_force_rejects_whole_set() {
    for forced in [false, true] {
        let (state, mut harness) = transfer_fixture();
        let before = transfer_status(&state);
        assert!(queue_transfer(&state, &before).ok);
        if forced {
            state
                .debug
                .try_force_global("Main.Kp", Value::LReal(2.0))
                .expect("force the gain");
        } else {
            state
                .debug
                .try_enqueue_global_write("Main.Kp", Value::LReal(3.0))
                .expect("queue the gain write");
        }
        assert!(harness.cycle().errors.is_empty());
        let after = transfer_status(&state);
        assert_eq!(after["status"], "rejected");
        assert_eq!(after["values"]["Main.Manual"], true);
        assert_eq!(after["values"]["Main.SP"], before["values"]["Main.SP"]);
        assert_eq!(after["values"]["Main.ManualCommand"], 2.0);
    }
}

#[test]
fn controller_transfer_guards_a_bool_member_and_rejects_the_transfer_when_it_changes() {
    // A cascade's inner loop guards its outer controller's mode (Specification 35 §3).
    let source = "PROGRAM Main VAR Kp : LREAL := 2.0; Manual : BOOL := TRUE; OuterAuto : BOOL := FALSE; ManualCommand : LREAL := 2.0; SP AT %ML0 : LREAL := 10.0; PV : LREAL := 10.0; MV AT %QL0 : LREAL; END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := Kp * (SP-PV); END_IF; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    let mut binding = transfer_binding();
    binding["guards"].as_array_mut().unwrap().push(
        json!({"symbol":"Main.OuterAuto","iec_type":"BOOL","unit":"1","minimum":0,"maximum":1}),
    );
    let status = |state: &ControlState| {
        let response = handle_request_value(
            json!({"id":8610,"type":"control.transfer.status","params":{"binding":binding.clone()}}),
            state,
            None,
        );
        assert!(
            response.ok,
            "a BOOL guard is a valid transfer member: {:?}",
            response.error
        );
        response.result.unwrap()
    };
    let before = status(&state);
    assert_eq!(
        before["values"]["Main.OuterAuto"], 0.0,
        "a BOOL member is observed as 0 or 1"
    );
    let queued = handle_request_value(
        json!({"id":8611,"type":"control.transfer.apply","params":{
        "transaction_id":"transfer-guarded","binding":binding.clone(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(queued.ok, "{:?}", queued.error);
    state
        .debug
        .try_enqueue_global_write("Main.OuterAuto", Value::Bool(true))
        .expect("queue the outer mode write");
    assert!(harness.cycle().errors.is_empty());
    let after = status(&state);
    assert_eq!(
        after["status"], "rejected",
        "the outer controller switched to Auto after review"
    );
    assert_eq!(after["values"]["Main.Manual"], true);
    assert_eq!(after["values"]["Main.SP"], before["values"]["Main.SP"]);
}

#[test]
fn controller_transfer_switches_additional_modes_with_the_mode_and_returns_them_in_manual() {
    // A cascade enters Auto with both controllers at one task boundary, and Restore returns both to Manual (Specification 35 §17.11).
    let source = "PROGRAM Main VAR Kp : LREAL := 2.0; Manual : BOOL := TRUE; OuterManual : BOOL := TRUE; ManualCommand : LREAL := 2.0; SP AT %ML0 : LREAL := 10.0; PV : LREAL := 10.0; MV AT %QL0 : LREAL; END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := Kp * (SP-PV); END_IF; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    let mut binding = transfer_binding();
    binding["additional_modes"] = json!([{"symbol":"Main.OuterManual","manual_value":true}]);
    let status = |state: &ControlState| {
        let response = handle_request_value(
            json!({"id":8620,"type":"control.transfer.status","params":{"binding":binding.clone()}}),
            state,
            None,
        );
        assert!(
            response.ok,
            "an additional mode is a valid transfer member: {:?}",
            response.error
        );
        response.result.unwrap()
    };
    let transfer = |state: &ControlState, before: &serde_json::Value, id: &str, manual: bool| {
        let response = handle_request_value(
            json!({"id":8621,"type":"control.transfer.apply","params":{
            "transaction_id":id,"binding":binding.clone(),
            "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
            "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
            "manual":manual,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
            state,
            None,
        );
        assert!(response.ok, "{:?}", response.error);
    };
    let before = status(&state);
    assert_eq!(
        before["values"]["Main.OuterManual"], true,
        "the additional mode is observed like the mode"
    );
    transfer(&state, &before, "cascade-auto", false);
    assert!(harness.cycle().errors.is_empty());
    let auto = status(&state);
    assert_eq!(auto["status"], "applied", "{auto}");
    assert_eq!(
        [
            auto["values"]["Main.Manual"].clone(),
            auto["values"]["Main.OuterManual"].clone()
        ],
        [json!(false), json!(false)],
        "both controllers leave Manual at the same task boundary"
    );
    transfer(&state, &auto, "cascade-manual", true);
    assert!(harness.cycle().errors.is_empty());
    let manual = status(&state);
    assert_eq!(manual["status"], "applied", "{manual}");
    assert_eq!(
        [
            manual["values"]["Main.Manual"].clone(),
            manual["values"]["Main.OuterManual"].clone()
        ],
        [json!(true), json!(true)],
        "a Manual transfer returns both controllers to Manual"
    );
}

#[test]
fn controller_transfer_rejects_other_targets_overlapping_its_memory_state() {
    for address in ["%ML0", "%ML4"] {
        // The alias's binding is the program image's (specification 44 §5.6).
        let (state, mut harness) =
            transfer_fixture_with_alias(&format!("Alias AT {address} : LREAL := 10.0;"));
        let before = transfer_status(&state);
        assert!(queue_transfer(&state, &before).ok);
        assert!(harness.cycle().errors.is_empty());
        let after = transfer_status(&state);
        assert_eq!(
            after["status"], "rejected",
            "a different target overlaps the transferred state at {address}"
        );
        assert_eq!(after["values"]["Main.Manual"], true);
        assert_eq!(after["values"]["Main.SP"], before["values"]["Main.SP"]);
    }
}

struct TransferOutputFailureDriver;
impl crate::io::IoDriver for TransferOutputFailureDriver {
    fn is_simulated(&self) -> bool {
        true
    }
    fn read_inputs(&mut self, _: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn write_outputs(&mut self, _: &[u8]) -> Result<(), RuntimeError> {
        Err(RuntimeError::ControlError("transfer output failure".into()))
    }
}

#[test]
fn controller_transfer_retains_output_failure_under_original_program_identity() {
    let (state, mut harness) = transfer_fixture();
    harness.runtime_mut().add_io_driver(
        "failing-simulated-output",
        Box::new(TransferOutputFailureDriver),
    );
    let before = transfer_status(&state);
    assert!(queue_transfer(&state, &before).ok);
    assert!(!harness.cycle().errors.is_empty());
    let after = transfer_status(&state);
    assert_eq!(
        after["status"], "failed",
        "a failed output commit must remain visible after fault invalidation"
    );
    assert_eq!(
        after["transaction"]["program_revision"],
        before["program_revision"]
    );
    assert_eq!(after["transaction"]["confirmation"], "post-st");
    assert_eq!(
        after["transaction"]["observed_values"]["Main.Manual"],
        false
    );
    assert!(after["transaction"]["reason"]
        .as_str()
        .unwrap()
        .contains("transfer output failure"));
    assert_ne!(
        after["program_revision"], before["program_revision"],
        "the runtime fault invalidates prior write authority"
    );
}

fn gain_binding() -> serde_json::Value {
    json!({"loop_id":"ordinary-loop","task_name":"$background","parameters":[
        {"symbol":"Main.Kp","iec_type":"LREAL","unit":"V/m","convention":"proportional gain","minimum":0,"maximum":10}],
        "mode":{"symbol":"Main.Manual","manual_value":true}})
}

fn gain_status(state: &ControlState) -> serde_json::Value {
    let response = handle_request_value(
        json!({"id":8640,"type":"control.parameters.status","params":{"binding":gain_binding()}}),
        state,
        None,
    );
    assert!(response.ok, "{:?}", response.error);
    response.result.unwrap()
}

#[test]
fn controller_transfer_rejected_first_output_fails_the_transfer_without_halting_the_runtime() {
    let (state, mut harness) = transfer_fixture();
    let gains = gain_status(&state);
    let staged = handle_request_value(
        json!({"id":8641,"type":"control.parameters.apply","params":{
            "transaction_id":"gain-1","binding":gain_binding(),"expected_program_revision":gains["program_revision"],
            "expected_binding_revision":gains["binding_revision"],"expected_active_revision":gains["active_revision"],
            "expected_values":gains["values"],"values":{"Main.Kp":3.0},"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(staged.ok, "{:?}", staged.error);
    assert!(harness.cycle().errors.is_empty());
    let staged_gain = gain_status(&state);
    assert_eq!(staged_gain["status"], "applied");

    // Kp 3 and SP 11 make the first Auto output 3 V against the held 2 V.
    let before = transfer_status(&state);
    let entry = handle_request_value(
        json!({"id":8642,"type":"control.transfer.apply","params":{
        "transaction_id":"discontinuous-entry","binding":transfer_binding(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(entry.ok, "{:?}", entry.error);
    assert!(
        harness.cycle().errors.is_empty(),
        "a first output outside the reviewed tolerance fails the transfer; it is not a runtime fault"
    );
    let failed = transfer_status(&state);
    assert_eq!(failed["status"], "failed");
    assert!(failed["transaction"]["reason"]
        .as_str()
        .unwrap()
        .contains("violates reviewed transfer tolerance"));
    assert_eq!(
        failed["program_revision"], before["program_revision"],
        "no reload or restart happened, so write authority keeps its identity"
    );
    assert_eq!(
        failed["transaction"]["restored"], true,
        "the rejected commit was reverted before the output commit"
    );
    assert_eq!(
        failed["values"]["Main.Manual"], true,
        "a rejected Auto entry leaves the loop in Manual"
    );
    assert_eq!(
        failed["values"]["Main.SP"], before["values"]["Main.SP"],
        "the transferred reference returns to its pre-transfer value"
    );
    assert_eq!(
        failed["values"]["Main.ManualCommand"],
        before["values"]["Main.ManualCommand"]
    );
    assert_eq!(
        failed["values"]["Main.MV"], 2.0,
        "the output holds its task-boundary value"
    );
    assert_eq!(
        harness
            .runtime()
            .read_io(&crate::io::IoAddress::parse("%QL0").unwrap())
            .unwrap(),
        Value::LWord(2.0_f64.to_bits()),
        "the rejected first output image is not published"
    );
    assert!(harness.cycle().errors.is_empty());
    let next = transfer_status(&state);
    assert_eq!(
        next["values"]["Main.Manual"], true,
        "the next scan still runs in Manual"
    );
    assert_eq!(next["values"]["Main.MV"], 2.0);

    let cancelled = handle_request_value(
        json!({"id":8643,"type":"control.parameters.cancel","params":{"loop_id":"ordinary-loop",
            "transaction_id":"gain-1","expected_program_revision":staged_gain["program_revision"],
            "expected_binding_revision":staged_gain["binding_revision"]}}),
        &state,
        None,
    );
    assert!(
        cancelled.ok,
        "the applied gain remains resolvable: {:?}",
        cancelled.error
    );
    let cancelled = cancelled.result.unwrap();
    assert_eq!(cancelled["confirmed"], true);
    assert_eq!(cancelled["before_dispatch"], false);

    // The gain is rolled back, so the same Auto entry can be continuous: Kp 2 and SP 11 make the first output the held 2 V.
    let adopted = gain_status(&state);
    let rollback = handle_request_value(
        json!({"id":8645,"type":"control.parameters.rollback","params":{
            "transaction_id":"gain-rollback","binding":gain_binding(),"expected_program_revision":adopted["program_revision"],
            "expected_binding_revision":adopted["binding_revision"],"expected_active_revision":adopted["active_revision"],
            "expected_values":adopted["values"],"previous_revision":adopted["previous_revision"],"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(rollback.ok, "{:?}", rollback.error);
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(gain_status(&state)["values"], gains["values"]);

    // A new transaction retries the Auto entry on the reverted loop, and the continuous entry is accepted.
    let current = transfer_status(&state);
    let retry = handle_request_value(
        json!({"id":8646,"type":"control.transfer.apply","params":{
        "transaction_id":"discontinuous-entry-retry","binding":transfer_binding(),
        "expected_program_revision":current["program_revision"],"expected_binding_revision":current["binding_revision"],
        "expected_values":current["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(retry.ok, "{:?}", retry.error);
    assert!(harness.cycle().errors.is_empty());
    let recovered = transfer_status(&state);
    assert_eq!(recovered["status"], "applied", "{recovered}");
    assert_eq!(recovered["values"]["Main.Manual"], false);
    assert_eq!(recovered["values"]["Main.MV"], 2.0);
}

#[test]
fn controller_transfer_failed_manual_to_manual_transfer_reverts_the_whole_set() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    // A Manual-to-Manual step of the command beyond the reviewed tolerance: the held output would jump from 2 V to 5 V.
    let step = handle_request_value(
        json!({"id":8647,"type":"control.transfer.apply","params":{
        "transaction_id":"manual-step","binding":transfer_binding(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":10.0,"Main.ManualCommand":5.0},
        "manual":true,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(step.ok, "{:?}", step.error);
    assert!(
        harness.cycle().errors.is_empty(),
        "a rejected transfer is not a runtime fault"
    );
    let failed = transfer_status(&state);
    assert_eq!(failed["status"], "failed");
    assert!(failed["transaction"]["reason"]
        .as_str()
        .unwrap()
        .contains("violates reviewed transfer tolerance"));
    assert_eq!(failed["transaction"]["restored"], true);
    assert_eq!(failed["values"]["Main.Manual"], true);
    assert_eq!(
        failed["values"]["Main.ManualCommand"], 2.0,
        "the commanded step returns to its pre-transfer value"
    );
    assert_eq!(failed["values"]["Main.SP"], 10.0);
    assert_eq!(
        failed["values"]["Main.MV"], 2.0,
        "the output holds its task-boundary value"
    );
    assert_eq!(
        harness
            .runtime()
            .read_io(&crate::io::IoAddress::parse("%QL0").unwrap())
            .unwrap(),
        Value::LWord(2.0_f64.to_bits()),
        "the rejected step is not published"
    );
}

#[test]
fn controller_transfer_stop_before_commit_retains_terminal_outcome_without_writes() {
    let (state, harness) = transfer_fixture();
    let before = transfer_status(&state);
    assert!(queue_transfer(&state, &before).ok);
    state.debug.clear_runtime_mutations();
    let after = transfer_status(&state);
    assert_eq!(
        after["status"], "failed",
        "runtime interruption cannot leave a transaction pending forever"
    );
    assert_eq!(after["values"], before["values"]);
    assert_eq!(
        after["transaction"]["program_revision"],
        before["program_revision"]
    );
    assert!(after["transaction"]["observed_values"].is_null());
    assert!(!state.debug.has_transfer_transaction());
    drop(harness);
}

#[test]
fn controller_transfer_rejects_an_aliased_output_owner_before_switching_mode() {
    // The alias's binding is the program image's (specification 44 §5.6).
    let (state, mut harness) = transfer_fixture_with_alias("Alias AT %QL0 : LREAL := 10.0;");
    let before = transfer_status(&state);
    assert!(queue_transfer(&state, &before).ok);
    assert!(harness.cycle().errors.is_empty());
    let after = transfer_status(&state);
    assert_eq!(
        after["status"], "rejected",
        "an alias can overwrite the output image after the controller's storage check"
    );
    assert_eq!(after["values"]["Main.Manual"], true);
    assert_eq!(after["values"]["Main.SP"], before["values"]["Main.SP"]);
}

#[test]
fn controller_transfer_cancel_before_commit_is_retryable_and_makes_no_writes() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    assert!(queue_transfer(&state, &before).ok);
    let request = json!({"id":8603,"type":"control.transfer.cancel","params":{
        "loop_id":"ordinary-loop","transaction_id":"transfer-1","expected_program_revision":before["program_revision"]}});
    for _ in 0..2 {
        let response = handle_request_value(request.clone(), &state, None);
        assert!(
            response.ok,
            "cancel the owned pending transfer: {:?}",
            response.error
        );
        assert_eq!(response.result.unwrap()["status"], "cancelled");
    }
    assert!(harness.cycle().errors.is_empty());
    let after = transfer_status(&state);
    assert_eq!(after["status"], "cancelled");
    assert_eq!(after["values"], before["values"]);
    assert!(after["transaction"]["observed_values"].is_null());
    assert!(!state.debug.has_transfer_transaction());
    assert!(
        !queue_transfer(&state, &before).ok,
        "an apply retry cannot restart the cancelled transaction"
    );
}

#[test]
fn controller_transfer_cannot_replay_an_older_transaction_after_recovery() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    assert!(queue_transfer(&state, &before).ok);
    assert!(harness.cycle().errors.is_empty());
    let automatic = transfer_status(&state);
    let recovery = handle_request_value(
        json!({"id":8604,"type":"control.transfer.apply","params":{
        "transaction_id":"recovery-2","binding":transfer_binding(),
        "expected_program_revision":automatic["program_revision"],"expected_binding_revision":automatic["binding_revision"],
        "expected_values":automatic["values"],"values":{"Main.SP":before["values"]["Main.SP"],"Main.ManualCommand":2.0},
        "manual":true,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(recovery.ok, "{:?}", recovery.error);
    assert!(harness.cycle().errors.is_empty());
    let recovered = transfer_status(&state);
    assert_eq!(recovered["status"], "applied");
    assert_eq!(recovered["values"], before["values"]);
    assert!(
        !queue_transfer(&state, &before).ok,
        "an old delayed apply request must not re-enter Auto after recovery"
    );
    assert!(!state.debug.has_transfer_transaction());
}

#[test]
fn controller_transfer_bounds_loop_records_and_rejects_oversized_ids() {
    let (state, _harness) = transfer_fixture();
    for index in 0..33 {
        let mut binding = transfer_binding();
        binding["loop_id"] = json!(format!("loop-{index}"));
        let response = handle_request_value(
            json!({"id":8605,"type":"control.transfer.status","params":{"binding":binding}}),
            &state,
            None,
        );
        assert_eq!(
            response.ok,
            index < 32,
            "loop-record capacity must not grow without a bound"
        );
    }
}

#[test]
fn controller_transfer_rejects_oversized_loop_identity() {
    let (state, _harness) = transfer_fixture();
    let mut binding = transfer_binding();
    binding["loop_id"] = json!("x".repeat(129));
    let response = handle_request_value(
        json!({"id":8605,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    );
    assert!(!response.ok, "bound loop identity size");
}

#[test]
fn controller_transfer_bounds_replay_memory_without_evicting_used_ids() {
    let (state, _harness) = transfer_fixture();
    let before = transfer_status(&state);
    for index in 0..=1024 {
        let id = format!("bounded-{index}");
        let response = handle_request_value(
            json!({"id":8606,"type":"control.transfer.apply","params":{
            "transaction_id":id,"binding":transfer_binding(),
            "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
            "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
            "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
            &state,
            None,
        );
        assert_eq!(
            response.ok,
            index < 1024,
            "bound replay memory without dropping an earlier identity"
        );
        if index < 1024 {
            let cancelled = handle_request_value(
                json!({"id":8607,"type":"control.transfer.cancel","params":{
                "loop_id":"ordinary-loop","transaction_id":id,"expected_program_revision":before["program_revision"]}}),
                &state,
                None,
            );
            assert!(cancelled.ok);
        }
    }
    assert!(!state.debug.has_transfer_transaction());
}

struct TransferCancelDuringOutput {
    debug: crate::debug::DebugControl,
    request: serde_json::Value,
    response: std::sync::Arc<std::sync::Mutex<Option<serde_json::Value>>>,
}
impl crate::io::IoDriver for TransferCancelDuringOutput {
    fn is_simulated(&self) -> bool {
        true
    }
    fn read_inputs(&mut self, _: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn write_outputs(&mut self, _: &[u8]) -> Result<(), RuntimeError> {
        let reply = self
            .debug
            .transfer_cancel(self.request.clone())
            .map_err(|reason| RuntimeError::ControlError(reason.into()))?;
        *self.response.lock().unwrap() = Some(reply);
        Ok(())
    }
}

#[test]
fn controller_transfer_cancel_after_st_waits_for_actual_output_commit() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    let response = std::sync::Arc::new(std::sync::Mutex::new(None));
    harness.runtime_mut().add_io_driver("cancel-during-output", Box::new(TransferCancelDuringOutput {
        debug: state.debug.clone(), response: response.clone(), request: json!({
            "loop_id":"ordinary-loop","transaction_id":"transfer-1","expected_program_revision":before["program_revision"]}) }));
    assert!(queue_transfer(&state, &before).ok);
    assert!(harness.cycle().errors.is_empty());
    let during_commit = response.lock().unwrap().clone().unwrap();
    assert_eq!(during_commit["status"], "pending");
    assert_eq!(during_commit["confirmed"], false);
    let after = transfer_status(&state);
    assert_eq!(after["status"], "applied");
    assert_eq!(after["transaction"]["confirmation"], "output-image");
    assert_eq!(
        after["transaction"]["observed_values"]["Main.Manual"],
        false
    );
}

#[test]
fn controller_transfer_simulation_label_cannot_bypass_actual_driver_authority() {
    let (state, mut harness) = transfer_fixture();
    harness
        .runtime_mut()
        .add_io_driver("physical-driver", Box::new(GenericPhysicalDriver));
    let before = transfer_status(&state);
    assert!(queue_transfer(&state, &before).ok);
    assert!(harness.cycle().errors.is_empty());
    let after = transfer_status(&state);
    assert_eq!(after["status"], "rejected");
    assert!(after["reason"]
        .as_str()
        .unwrap()
        .contains("physical review"));
    assert_eq!(after["values"], before["values"]);
}

#[test]
fn controller_transfer_accepts_javascript_integer_number_roundtrip() {
    let (state, mut harness) = transfer_fixture();
    let mut before = transfer_status(&state);
    for value in before["values"].as_object_mut().unwrap().values_mut() {
        if let Some(number) = value.as_f64() {
            assert_eq!(
                number.fract(),
                0.0,
                "this fixture has integral-valued LREAL observations"
            );
            *value = json!(number as i64);
        }
    }
    let response = queue_transfer(&state, &before);
    assert!(
        response.ok,
        "JSON integer representations retain their IEC numeric meaning: {:?}",
        response.error
    );
    assert!(harness.cycle().errors.is_empty());
    assert_eq!(transfer_status(&state)["status"], "applied");
}

#[test]
fn controller_transfer_tracks_fresh_pv_and_committed_output_at_task_boundary() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    let response = handle_request_value(
        json!({"id":8608,"type":"control.transfer.apply","params":{
        "transaction_id":"boundary-tracking","binding":transfer_binding(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false,
        "tracking":{"kind":"proportional","reference_symbol":"Main.SP","manual_command_symbol":"Main.ManualCommand",
            "process_variable":"Main.PV","gain_symbol":"Main.Kp","align_reference":true}}}),
        &state,
        None,
    );
    assert!(
        response.ok,
        "a reviewed tracking policy must use the owning boundary: {:?}",
        response.error
    );
    state
        .debug
        .try_enqueue_global_write("Main.PV", Value::LReal(10.1))
        .expect("queue the measurement write");
    state
        .debug
        .try_enqueue_global_write("Main.MV", Value::LReal(4.0))
        .expect("queue the output variable write");
    assert!(harness.cycle().errors.is_empty());
    let after = transfer_status(&state);
    assert_eq!(after["status"], "applied");
    assert_eq!(after["values"]["Main.SP"], 11.1);
    assert_eq!(
        after["values"]["Main.MV"], 2.0,
        "hold the last committed output, not an uncommitted variable write"
    );
    assert_eq!(after["transaction"]["initial_values"]["Main.PV"], 10.1);
    assert_eq!(after["transaction"]["initial_values"]["Main.MV"], 2.0);
    assert_eq!(after["transaction"]["applied_values"]["Main.SP"], 11.1);
}

/// An Auto entry with `entry_error_gain` at reference 11.0 and Manual command 2.0 V.
fn entry_with_error_gain(
    state: &ControlState,
    before: &serde_json::Value,
    gain: f64,
    manual: bool,
    tracked: bool,
) -> ControlResponse {
    let mut params = json!({
        "transaction_id":format!("entry-error-{gain}-{manual}-{tracked}"),"binding":transfer_binding(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":manual,"maximum_output_change":0.000001,"entry_error_gain":gain,
        "source":"simulation","physical_approved":false});
    if tracked {
        params["tracking"] = json!({"kind":"proportional","reference_symbol":"Main.SP",
            "manual_command_symbol":"Main.ManualCommand","process_variable":"Main.PV","gain_symbol":"Main.Kp",
            "align_reference":false});
    }
    handle_request_value(
        json!({"id":8650,"type":"control.transfer.apply","params":params}),
        state,
        None,
    )
}

/// Annex E6.9 and specification 35 A6.4: the first output after an Auto entry may move by `maximum_output_change` plus
/// `entry_error_gain` times the error the controller enters with, taken in the transfer's own scan. The measurement falls
/// from the reviewed 10.0 to 9.9 before the boundary, so the controller enters with e = 11.0 − 9.9 = 1.1 and its first
/// output Kp·e = 2.2 V is 0.2 V above the held 2.0 V. A gain of 0.19 allows 1e-6 + 0.19·1.1 = 0.209 V and the entry is
/// applied; a gain of 0.18 allows 0.198 V and the entry is refused. Catches a runtime that leaves the gain out (1e-6 refuses
/// both), and one that takes the reviewed error 1.0 (0.19 V refuses the first).
#[test]
fn controller_transfer_allows_the_integral_step_on_the_error_it_enters_with() {
    for (gain, applied) in [(0.19, true), (0.18, false)] {
        let (state, mut harness) = transfer_fixture();
        let before = transfer_status(&state);
        let response = entry_with_error_gain(&state, &before, gain, false, true);
        assert!(response.ok, "gain {gain}: {:?}", response.error);
        state
            .debug
            .try_enqueue_global_write("Main.PV", Value::LReal(9.9))
            .expect("queue the measurement write");
        assert!(harness.cycle().errors.is_empty());
        let after = transfer_status(&state);
        assert_eq!(after["transaction"]["initial_values"]["Main.PV"], 9.9);
        if applied {
            assert_eq!(after["status"], "applied", "gain {gain}: {after}");
            assert_eq!(after["values"]["Main.Manual"], false);
            let output = after["values"]["Main.MV"].as_f64().unwrap();
            assert!((output - 2.2).abs() < 1e-9, "first output {output}");
        } else {
            assert_eq!(after["status"], "failed", "gain {gain}: {after}");
            assert!(after["transaction"]["reason"]
                .as_str()
                .unwrap()
                .contains("violates reviewed transfer tolerance"));
            assert_eq!(after["values"]["Main.Manual"], true);
        }
    }
}

/// Annex E6.5 check 6: `entry_error_gain` is a finite number of zero or more, sent with an Auto entry that carries the
/// tracking naming the reference and the process variable. Catches a runtime that accepts a negative gain, a gain on a
/// Manual transfer, or a gain without the tracking it reads the error through.
#[test]
fn controller_transfer_refuses_an_entry_error_gain_it_cannot_apply() {
    let (state, _harness) = transfer_fixture();
    let before = transfer_status(&state);
    for (gain, manual, tracked) in [(-0.1, false, true), (0.1, true, true), (0.1, false, false)] {
        let response = entry_with_error_gain(&state, &before, gain, manual, tracked);
        assert!(
            !response.ok,
            "gain {gain}, manual {manual}, tracked {tracked}"
        );
        assert_eq!(refusal_text(&response), "invalid entry error gain");
    }
    assert_eq!(
        transfer_status(&state)["status"],
        before["status"],
        "a refused apply writes no record"
    );
}

/// A velocity-form PI controller with Kp 2 and Ki·Δt 0.05, as `Control.PID` executes one: in Manual it holds its output
/// at the Manual command, and in every execution it keeps the measurement it received.
const CARRIED_PI: &str = "PROGRAM Main VAR Manual : BOOL := TRUE; SP : LREAL := 10.0; PV : LREAL := 10.0; ManualCommand : LREAL := 2.0; MV AT %QL0 : LREAL; Nominal : LREAL := 2.0; LastPV : LREAL := 10.0; END_VAR IF Manual THEN Nominal := ManualCommand; ELSE Nominal := Nominal - LREAL#2.0 * (PV - LastPV) + LREAL#0.05 * (SP - PV); END_IF; MV := Nominal; LastPV := PV; END_PROGRAM";

/// The controller of [`CARRIED_PI`] with a stale memory: it keeps the measurement only in Auto, so in Manual it still holds
/// the 9.0 of an earlier Auto period.
const STALE_PI: &str = "PROGRAM Main VAR Manual : BOOL := TRUE; SP : LREAL := 10.0; PV : LREAL := 10.0; ManualCommand : LREAL := 2.0; MV AT %QL0 : LREAL; Nominal : LREAL := 2.0; LastPV : LREAL := 9.0; END_VAR IF Manual THEN Nominal := ManualCommand; ELSE Nominal := Nominal - LREAL#2.0 * (PV - LastPV) + LREAL#0.05 * (SP - PV); LastPV := PV; END_IF; MV := Nominal; END_PROGRAM";

fn carried_pi_fixture(source: &str) -> (ControlState, TestHarness) {
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    (state, harness)
}

fn carried_pi_binding() -> serde_json::Value {
    let number = |symbol: &str, unit: &str| json!({"symbol":symbol,"iec_type":"LREAL","unit":unit,"minimum":-20,"maximum":20});
    json!({"loop_id":"carried-pi-loop","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[number("Main.SP","m"),number("Main.ManualCommand","V"),number("Main.Nominal","V"),number("Main.LastPV","m")],
        "guards":[number("Main.PV","m")],
        "output":number("Main.MV","V")})
}

/// A transfer of the carried PI controller, to Manual when `manual` and to Auto otherwise, that carries every state as it
/// stands, as Identify's mode transfers do (specification 35, B3.4 step 1), with `entry_error_gain` 0.05 on an Auto entry
/// and `measurement_gain` as its `entry_measurement_gain`; without the tracking when not `tracked`.
fn carried_pi_transfer(
    state: &ControlState,
    before: &serde_json::Value,
    id: &str,
    manual: bool,
    tracked: bool,
    measurement_gain: Option<f64>,
) -> ControlResponse {
    let mut params = json!({
        "transaction_id":id,"binding":carried_pi_binding(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],
        "values":{"Main.SP":10.0,"Main.ManualCommand":2.0,"Main.Nominal":2.0,"Main.LastPV":before["values"]["Main.LastPV"]},
        "manual":manual,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false});
    if tracked {
        params["tracking"] = json!({"kind":"declared-state-v1","reference_symbol":"Main.SP",
            "manual_command_symbol":"Main.ManualCommand","process_variable":"Main.PV","align_output":false,
            "initialization_values":{"Main.Nominal":2.0,"Main.LastPV":before["values"]["Main.LastPV"]},
            "bounded_state_symbols":["Main.Nominal","Main.LastPV"]});
    }
    if !manual && tracked {
        params["entry_error_gain"] = json!(0.05);
    }
    if let Some(gain) = measurement_gain {
        params["entry_measurement_gain"] = json!(gain);
    }
    handle_request_value(
        json!({"id":8660,"type":"control.transfer.apply","params":params}),
        state,
        None,
    )
}

/// Specification 35 A6.4 and annex E6.9: an Auto entry that carries the controller's states responds to the measurement's
/// change since the scan before the transfer, so its first output may also move by `entry_measurement_gain` times that
/// change, which the runtime reads itself: the process variable of the scan the apply read, and of the transfer's own scan.
/// The controller last executed in Manual at 10.0, and the measurement rises to 10.2 before the boundary, so its first Auto
/// output is 2.0 − 2·0.2 − 0.05·0.2 = 1.59 V, 0.41 V below the held 2.0 V. A gain of 2 allows 1e-6 + 0.05·0.2 + 2·0.2 =
/// 0.410001 V and the entry is applied; a gain of 1.99 allows 0.408001 V and the entry is refused. Catches a runtime that
/// leaves the measurement's change out (0.010001 V refuses both), and one that reads both measurements in the same scan (no
/// change).
#[test]
fn controller_transfer_allows_the_response_to_the_measurement_change_before_the_entry() {
    for (gain, applied) in [(2.0, true), (1.99, false)] {
        let (state, mut harness) = carried_pi_fixture(CARRIED_PI);
        let before = transfer_status_for(&state, carried_pi_binding());
        let response = carried_pi_transfer(
            &state,
            &before,
            &format!("carried-{gain}"),
            false,
            true,
            Some(gain),
        );
        assert!(response.ok, "gain {gain}: {:?}", response.error);
        state
            .debug
            .try_enqueue_global_write("Main.PV", Value::LReal(10.2))
            .expect("queue the measurement write");
        assert!(harness.cycle().errors.is_empty());
        let after = transfer_status_for(&state, carried_pi_binding());
        assert_eq!(after["transaction"]["initial_values"]["Main.PV"], 10.2);
        if applied {
            assert_eq!(after["status"], "applied", "gain {gain}: {after}");
            assert_eq!(after["values"]["Main.Manual"], false);
            let output = after["values"]["Main.MV"].as_f64().unwrap();
            assert!((output - 1.59).abs() < 1e-9, "first output {output}");
        } else {
            assert_eq!(after["status"], "failed", "gain {gain}: {after}");
            assert!(after["transaction"]["reason"]
                .as_str()
                .unwrap()
                .contains("violates reviewed transfer tolerance"));
            assert_eq!(after["values"]["Main.Manual"], true);
        }
    }
}

/// Specification 35 A6.4: the measurement's change is the one the runtime sees, not the one the controller's memory claims.
/// The controller kept 9.0 from an earlier Auto period while the measurement stood at 10.0, so its first Auto output is
/// 2.0 − 2·(10.0 − 9.0) = 0.0 V, a bump of 2.0 V that its stale memory causes. The measurement did not change before the
/// entry, so the allowance stays 1e-6 V and the entry is refused even with a gain of 2. Catches a runtime that takes the
/// change from the controller's own state, which would allow exactly the bump that state causes.
#[test]
fn controller_transfer_refuses_the_bump_of_a_controller_memory_that_did_not_track() {
    let (state, mut harness) = carried_pi_fixture(STALE_PI);
    let before = transfer_status_for(&state, carried_pi_binding());
    assert_eq!(before["values"]["Main.LastPV"], 9.0);
    let response = carried_pi_transfer(&state, &before, "stale", false, true, Some(2.0));
    assert!(response.ok, "{:?}", response.error);
    assert!(harness.cycle().errors.is_empty());
    let after = transfer_status_for(&state, carried_pi_binding());
    assert_eq!(after["status"], "failed", "{after}");
    assert!(after["transaction"]["reason"]
        .as_str()
        .unwrap()
        .contains("violates reviewed transfer tolerance"));
    assert_eq!(after["values"]["Main.Manual"], true);
}

/// Annex E6.5 check 6: `entry_measurement_gain` is a finite number of zero or more, sent with an Auto entry that carries
/// the tracking naming the process variable. Catches a runtime that accepts a negative gain, a gain on a Manual transfer,
/// or a gain without the tracking it reads the measurement through.
#[test]
fn controller_transfer_refuses_an_entry_measurement_gain_it_cannot_apply() {
    let (state, _harness) = carried_pi_fixture(CARRIED_PI);
    let before = transfer_status_for(&state, carried_pi_binding());
    for (case, gain, manual, tracked) in [
        ("negative", -0.1, false, true),
        ("manual", 2.0, true, true),
        ("untracked", 2.0, false, false),
    ] {
        let response = carried_pi_transfer(&state, &before, case, manual, tracked, Some(gain));
        assert!(!response.ok, "{case}");
        assert_eq!(
            refusal_text(&response),
            "invalid entry measurement gain",
            "{case}"
        );
    }
    assert_eq!(
        transfer_status_for(&state, carried_pi_binding())["status"],
        before["status"],
        "a refused apply writes no record"
    );
}

#[test]
fn controller_transfer_uses_declared_applied_feedback_as_output_basis() {
    let source = "PROGRAM Main VAR Kp : LREAL := 2.0; Manual : BOOL := TRUE; ManualCommand : LREAL := 2.0; SP AT %ML0 : LREAL := 10.0; PV : LREAL := 10.0; MV AT %QL0 : LREAL; Applied AT %IL0 : LREAL; END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := Kp * (SP-PV); END_IF; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    harness
        .set_direct_input("%IL0", Value::LWord(2.0_f64.to_bits()))
        .unwrap();
    assert_eq!(
        harness
            .runtime()
            .read_io(&crate::io::IoAddress::parse("%IL0").unwrap())
            .unwrap(),
        Value::LWord(2.0_f64.to_bits())
    );
    assert!(harness.cycle().errors.is_empty());
    let number = |symbol: &str, unit: &str| json!({"symbol":symbol,"iec_type":"LREAL","unit":unit,"minimum":-20,"maximum":20});
    let binding = json!({"loop_id":"applied-feedback-loop","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[number("Main.SP","m"),number("Main.ManualCommand","V")],
        "guards":[number("Main.Kp","V/m"),number("Main.PV","m"),number("Main.Applied","V")],
        "output":number("Main.MV","V"),"tracking_outputs":{"Main.MV":"Main.Applied"}});
    let status = handle_request_value(
        json!({"id":8630,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    );
    assert!(status.ok, "applied feedback status: {:?}", status.error);
    let before = status.result.unwrap();
    assert_eq!(before["values"]["Main.Applied"], 2.0);
    let apply = handle_request_value(
        json!({"id":8631,"type":"control.transfer.apply","params":{
        "transaction_id":"applied-feedback-basis","binding":binding,
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false,
        "tracking":{"kind":"proportional","reference_symbol":"Main.SP","manual_command_symbol":"Main.ManualCommand",
            "process_variable":"Main.PV","gain_symbol":"Main.Kp","align_reference":true}}}),
        &state,
        None,
    );
    assert!(
        apply.ok,
        "queue applied-feedback transfer: {:?}",
        apply.error
    );
    state
        .debug
        .try_enqueue_global_write("Main.Applied", Value::LReal(2.0))
        .expect("queue the feedback write");
    assert!(harness.cycle().errors.is_empty());
    let after = handle_request_value(
        json!({"id":8632,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    )
    .result
    .unwrap();
    assert_eq!(after["status"], "applied");
    assert_eq!(after["transaction"]["initial_values"]["Main.Applied"], 2.0);
    assert_eq!(
        after["transaction"]["applied_values"]["Main.ManualCommand"],
        2.0
    );
}

#[test]
fn controller_transfer_aligns_declared_parallel_pid_states_at_task_boundary() {
    let (state, mut harness) = pid_transfer_fixture();
    let binding = pid_transfer_binding();
    let before = transfer_status_for(&state, binding.clone());
    let response = handle_request_value(
        json!({"id":8609,"type":"control.transfer.apply","params":{
        "transaction_id":"pid-boundary-tracking","binding":binding,
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],
        "values":{"Main.SP":11.0,"Main.ManualCommand":2.0,"Main.Integral":0.0,"Main.PreviousError":0.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false,
        "tracking":{"kind":"parallel-pid-v1","reference_symbol":"Main.SP","manual_command_symbol":"Main.ManualCommand",
            "process_variable":"Main.PV","proportional_gain_symbol":"Main.Kp","integral_gain_symbol":"Main.Ki","derivative_gain_symbol":"Main.Kd",
            "integral_state_symbol":"Main.Integral","derivative_state_symbol":"Main.PreviousError",
            "proportional_setpoint_weight":1.0,"derivative_setpoint_weight":1.0,"cycle_seconds":0.02,"align_output":true}}}),
        &state,
        None,
    );
    assert!(
        response.ok,
        "the runtime must accept the declared PID state-alignment policy: {:?}",
        response.error
    );
    assert!(harness.cycle().errors.is_empty());
    let after = transfer_status_for(&state, pid_transfer_binding());
    assert!(after["supported_tracking"]
        .as_array()
        .is_some_and(|values| values.iter().any(|value| value == "parallel-pid-v1")));
    assert_eq!(after["status"], "applied");
    assert_eq!(after["values"]["Main.Manual"], false);
    assert_eq!(after["values"]["Main.SP"], 11.0);
    assert_eq!(after["values"]["Main.Integral"], 0.0);
    assert_eq!(after["values"]["Main.PreviousError"], 1.0);
    assert_eq!(
        after["values"]["Main.MV"], 2.0,
        "the first actual PID output is continuous"
    );
    assert_eq!(after["transaction"]["initial_values"]["Main.MV"], 2.0);
    assert_eq!(
        after["transaction"]["applied_values"]["Main.Integral"],
        -0.02
    );
}

#[test]
fn controller_transfer_recovers_parallel_pid_to_manual_after_a_live_state_update() {
    let (state, mut harness) = pid_transfer_fixture();
    let binding = pid_transfer_binding();
    let before = transfer_status_for(&state, binding.clone());
    let response = handle_request_value(
        json!({"id":8611,"type":"control.transfer.apply","params":{
        "transaction_id":"pid-manual-recovery","binding":binding,
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],
        "values":{"Main.SP":10.0,"Main.ManualCommand":0.0,"Main.Integral":0.0,"Main.PreviousError":0.0},
        "manual":true,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false,
        "tracking":{"kind":"parallel-pid-v1","reference_symbol":"Main.SP","manual_command_symbol":"Main.ManualCommand",
            "process_variable":"Main.PV","proportional_gain_symbol":"Main.Kp","integral_gain_symbol":"Main.Ki","derivative_gain_symbol":"Main.Kd",
            "integral_state_symbol":"Main.Integral","derivative_state_symbol":"Main.PreviousError",
            "proportional_setpoint_weight":1.0,"derivative_setpoint_weight":1.0,"cycle_seconds":0.02,"align_output":false}}}),
        &state,
        None,
    );
    assert!(
        response.ok,
        "a reviewed PID recovery must remain task-boundary-safe: {:?}",
        response.error
    );
    state
        .debug
        .try_enqueue_global_write("Main.Integral", Value::LReal(1.0))
        .expect("queue the live state write");
    assert!(harness.cycle().errors.is_empty());
    let after = transfer_status_for(&state, pid_transfer_binding());
    assert_eq!(after["status"], "applied");
    assert_eq!(after["values"]["Main.Manual"], true);
    assert_eq!(
        after["values"]["Main.ManualCommand"], 2.0,
        "recovery holds the committed applied output"
    );
    assert_eq!(after["values"]["Main.Integral"], 0.0);
}

fn pid_transfer_binding() -> serde_json::Value {
    let number = |symbol: &str, unit: &str| json!({"symbol":symbol,"iec_type":"LREAL","unit":unit,"minimum":-20,"maximum":20});
    json!({"loop_id":"parallel-pid-loop","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[number("Main.SP","m"),number("Main.ManualCommand","V"),number("Main.Integral","V"),number("Main.PreviousError","m")],
        "guards":[number("Main.Kp","V/m"),number("Main.Ki","V/(m*s)"),number("Main.Kd","V*s/m"),number("Main.PV","m")],
        "output":number("Main.MV","V")})
}

fn pid_transfer_fixture() -> (ControlState, TestHarness) {
    let source = "PROGRAM Main VAR Kp : LREAL := 2.0; Ki : LREAL := 1.0; Kd : LREAL := 3.0; Integral : LREAL := 0.0; PreviousError : LREAL := 0.0; Manual : BOOL := TRUE; ManualCommand : LREAL := 2.0; SP AT %ML0 : LREAL := 10.0; PV : LREAL := 10.0; MV AT %QL0 : LREAL; END_VAR IF Manual THEN MV := ManualCommand; ELSE Integral := Integral + Ki * (SP-PV) * LREAL#0.02; MV := Kp * (SP-PV) + Integral + Kd * ((SP-PV) - PreviousError) / LREAL#0.02; PreviousError := SP-PV; END_IF; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    (state, harness)
}

fn transfer_status_for(state: &ControlState, binding: serde_json::Value) -> serde_json::Value {
    let response = handle_request_value(
        json!({"id":8610,"type":"control.transfer.status","params":{"binding":binding}}),
        state,
        None,
    );
    assert!(
        response.ok,
        "PID transfer status must resolve the declared state binding: {:?}",
        response.error
    );
    response.result.unwrap()
}

#[test]
fn controller_transfer_declared_model_output_formula_uses_fresh_boundary_measurement() {
    let source = "PROGRAM Main VAR Manual : BOOL := TRUE; SP : LREAL := 10.0; PV : LREAL := 10.0; ManualCommand : LREAL := 2.0; MV AT %QL0 : LREAL; Filter : LREAL := 0.0; END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := ManualCommand; END_IF; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    let binding = json!({"loop_id":"declared-formula-loop","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[
            {"symbol":"Main.SP","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20},
            {"symbol":"Main.ManualCommand","iec_type":"LREAL","unit":"V","minimum":-20,"maximum":20},
            {"symbol":"Main.Filter","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20}],
        "guards":[{"symbol":"Main.PV","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20}],
        "output":{"symbol":"Main.MV","iec_type":"LREAL","unit":"V","minimum":-20,"maximum":20}});
    let status = handle_request_value(
        json!({"id":8620,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    );
    assert!(status.ok, "declared formula status: {:?}", status.error);
    let before = status.result.unwrap();
    let apply = handle_request_value(
        json!({"id":8621,"type":"control.transfer.apply","params":{
        "transaction_id":"declared-model-output-formula","binding":binding,
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0,"Main.Filter":0.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false,
        "tracking":{"kind":"declared-state-v1","reference_symbol":"Main.SP","manual_command_symbol":"Main.ManualCommand",
            "process_variable":"Main.PV","align_output":true,"initialization_values":{"Main.Filter":0.0},
            "initialization_formulas":{"Main.Filter":{"kind":"model-output","process_variable":"Main.PV"}},"bounded_state_symbols":[]}}}),
        &state,
        None,
    );
    assert!(
        apply.ok,
        "queue declared formula transfer: {:?}",
        apply.error
    );
    state
        .debug
        .try_enqueue_global_write("Main.PV", Value::LReal(10.25))
        .expect("queue the measurement write");
    assert!(harness.cycle().errors.is_empty());
    let after = handle_request_value(
        json!({"id":8622,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    )
    .result
    .unwrap();
    assert_eq!(after["status"], "applied");
    assert_eq!(after["transaction"]["applied_values"]["Main.Filter"], 10.25);
}

/// Specification 35, §17.1: a loop whose program sets the manual output from the output in Auto
/// (Create PLC code's loops, so Manual takes over where the loop is) declares that manual output a
/// bounded post-scan state; the transfer to Auto is applied although the program changes it.
#[test]
fn controller_transfer_accepts_a_manual_output_that_follows_the_output_in_auto() {
    let source = "PROGRAM Main VAR Manual : BOOL := TRUE; SP : LREAL := 10.0; PV : LREAL := 10.0; ManualCommand : LREAL := 2.0; MV AT %QL0 : LREAL; Filter : LREAL := 0.0; END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := ManualCommand + 0.0001; ManualCommand := MV; END_IF; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    let binding = json!({"loop_id":"tracking-manual-loop","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[
            {"symbol":"Main.SP","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20},
            {"symbol":"Main.ManualCommand","iec_type":"LREAL","unit":"V","minimum":-20,"maximum":20},
            {"symbol":"Main.Filter","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20}],
        "guards":[{"symbol":"Main.PV","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20}],
        "output":{"symbol":"Main.MV","iec_type":"LREAL","unit":"V","minimum":-20,"maximum":20}});
    let before = transfer_status_for(&state, binding.clone());
    let apply = handle_request_value(
        json!({"id":8631,"type":"control.transfer.apply","params":{
        "transaction_id":"tracking-manual-output","binding":binding.clone(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":10.0,"Main.ManualCommand":2.0,"Main.Filter":0.0},
        "manual":false,"maximum_output_change":0.01,"source":"simulation","physical_approved":false,
        "tracking":{"kind":"declared-state-v1","reference_symbol":"Main.SP","manual_command_symbol":"Main.ManualCommand",
            "process_variable":"Main.PV","align_output":true,"initialization_values":{"Main.Filter":0.0},
            "bounded_state_symbols":["Main.ManualCommand"]}}}),
        &state,
        None,
    );
    assert!(
        apply.ok,
        "a manual output that follows the output is a bounded state: {:?}",
        apply.error
    );
    assert!(harness.cycle().errors.is_empty());
    let after = transfer_status_for(&state, binding);
    assert_eq!(after["status"], "applied", "{after}");
}

/// Leaving Auto: the program copies its moving output into the manual output every scan, so the
/// manual output changes between the reviewed snapshot and the task boundary; declared bounded,
/// it is accepted there and Manual starts from the held output (specification 35, §17.1).
#[test]
fn controller_transfer_to_manual_accepts_a_manual_output_that_follows_the_output() {
    let source = "PROGRAM Main VAR Manual : BOOL := FALSE; SP : LREAL := 10.0; PV : LREAL := 10.0; ManualCommand : LREAL := 2.0; MV AT %QL0 : LREAL := 2.0; Filter : LREAL := 0.0; END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := MV + 0.001; ManualCommand := MV; END_IF; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    let binding = json!({"loop_id":"tracking-leave-auto","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[
            {"symbol":"Main.SP","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20},
            {"symbol":"Main.ManualCommand","iec_type":"LREAL","unit":"V","minimum":-20,"maximum":20},
            {"symbol":"Main.Filter","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20}],
        "guards":[{"symbol":"Main.PV","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20}],
        "output":{"symbol":"Main.MV","iec_type":"LREAL","unit":"V","minimum":-20,"maximum":20}});
    let before = transfer_status_for(&state, binding.clone());
    let reviewed = before["values"]["Main.ManualCommand"].as_f64().unwrap();
    // One more Auto scan: the output and the manual output that follows it move on.
    assert!(harness.cycle().errors.is_empty());
    let apply = handle_request_value(
        json!({"id":8632,"type":"control.transfer.apply","params":{
        "transaction_id":"tracking-leave-auto","binding":binding.clone(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":10.0,"Main.ManualCommand":reviewed,"Main.Filter":0.0},
        "manual":true,"maximum_output_change":0.01,"source":"simulation","physical_approved":false,
        "tracking":{"kind":"declared-state-v1","reference_symbol":"Main.SP","manual_command_symbol":"Main.ManualCommand",
            "process_variable":"Main.PV","align_output":false,"initialization_values":{"Main.Filter":0.0},
            "bounded_state_symbols":["Main.ManualCommand"]}}}),
        &state,
        None,
    );
    assert!(apply.ok, "{:?}", apply.error);
    assert!(harness.cycle().errors.is_empty());
    let after = transfer_status_for(&state, binding);
    assert_eq!(
        after["status"], "applied",
        "the manual output the program sets is accepted at the boundary: {after}"
    );
    assert_eq!(after["values"]["Main.Manual"], json!(true), "{after}");
    let held = after["values"]["Main.ManualCommand"].as_f64().unwrap();
    assert!(
        held > reviewed,
        "Manual starts from the output held at the boundary, not the reviewed value: {held} after {reviewed}"
    );
}

/// A loop whose output has no address: the program hands it on to the block that owns the I/O, as
/// Create PLC code writes a PID loop (`EH101.AutoPower := TIC101_Output`). The transfer holds the
/// output's value at the task boundary, the value the last scan drove the plant with
/// (specification 35, §17.1).
#[test]
fn controller_transfer_holds_an_output_the_program_hands_on_without_an_address() {
    let source = "PROGRAM Main VAR Manual : BOOL := FALSE; SP : LREAL := 10.0; PV : LREAL := 10.0; ManualCommand : LREAL := 2.0; MV : LREAL := 2.0; Heater AT %QL0 : LREAL; Filter : LREAL := 0.0; END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := MV + 0.001; ManualCommand := MV; END_IF; Heater := MV; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    let binding = json!({"loop_id":"tracking-unaddressed-output","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[
            {"symbol":"Main.SP","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20},
            {"symbol":"Main.ManualCommand","iec_type":"LREAL","unit":"V","minimum":-20,"maximum":20},
            {"symbol":"Main.Filter","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20}],
        "guards":[{"symbol":"Main.PV","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20}],
        "output":{"symbol":"Main.MV","iec_type":"LREAL","unit":"V","minimum":-20,"maximum":20}});
    let before = transfer_status_for(&state, binding.clone());
    let apply = handle_request_value(
        json!({"id":8633,"type":"control.transfer.apply","params":{
        "transaction_id":"tracking-unaddressed-output","binding":binding.clone(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":10.0,"Main.ManualCommand":2.0,"Main.Filter":0.0},
        "manual":true,"maximum_output_change":0.01,"source":"simulation","physical_approved":false,
        "tracking":{"kind":"declared-state-v1","reference_symbol":"Main.SP","manual_command_symbol":"Main.ManualCommand",
            "process_variable":"Main.PV","align_output":false,"initialization_values":{"Main.Filter":0.0},
            "bounded_state_symbols":["Main.ManualCommand"]}}}),
        &state,
        None,
    );
    assert!(apply.ok, "{:?}", apply.error);
    assert!(harness.cycle().errors.is_empty());
    let after = transfer_status_for(&state, binding);
    assert_eq!(
        after["status"], "applied",
        "an unaddressed output holds its boundary value: {after}"
    );
    assert_eq!(after["values"]["Main.Manual"], json!(true), "{after}");
    let held = after["values"]["Main.ManualCommand"].as_f64().unwrap();
    let output = after["values"]["Main.MV"].as_f64().unwrap();
    assert_eq!(held, output, "Manual holds the output it took over: {after}");
}

// Control Studio's tests read a compiled program through the program access seam (specification 44 §13.3).
use crate::runtime::ProgramRead as _;

/// A debug control attached to `harness`'s runtime, as trust-runtime attaches its control: each scan publishes the
/// program's state into it, and the scan calls Control Studio's services at its access points (specification 44 §8.8,
/// §13.3).
fn studio_debug(harness: &mut TestHarness) -> crate::debug::DebugControl {
    let debug = crate::debug::DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    debug
}

/// The value at `path` in the program of `harness`, as its last scan left it.
fn studio_value(harness: &TestHarness, path: &str) -> Value {
    harness
        .runtime()
        .with_program_view(|program| program.value(path))
        .unwrap_or_else(|| panic!("{path} names a value"))
}

/// Writes `value` at `path` in the program of `harness` between scans, as the program itself would.
fn studio_set(harness: &mut TestHarness, path: &str, value: Value) {
    harness.runtime_mut().with_program(|program| {
        let at = program
            .locate(path)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        program
            .write_at(&at, &value)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
    });
}

/// A simulated target: simulation is enabled, and the scans that follow publish that all I/O is simulated (A2.4).
fn simulated_control_state(source: &str) -> ControlState {
    let state = control_test_state(source);
    state.experiments.record_simulated_target(true);
    state
}

/// A harness whose runtime is simulated: simulation is enabled, so the runtime's own target fact is simulated while all
/// of its I/O is simulation-only (A2.4).
fn simulated_harness(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("build harness");
    harness.runtime_mut().set_simulation_enabled(true);
    harness
}

/// The text of a refusal.
fn refusal_text(response: &ControlResponse) -> &str {
    response.error.as_deref().unwrap_or_default()
}

fn transfer_apply_request(
    state: &ControlState,
    before: &serde_json::Value,
    transaction_id: &str,
    source: &str,
    approved: bool,
) -> ControlResponse {
    handle_request_value(
        json!({"id":8701,"type":"control.transfer.apply","params":{
        "transaction_id":transaction_id,"binding":transfer_binding(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":source,"physical_approved":approved}}),
        state,
        None,
    )
}

/// The Auto entry of the fixture, tracked by the proportional law on its measured PV (A6.4 bands apply to the PV).
fn tracked_transfer_apply(
    state: &ControlState,
    before: &serde_json::Value,
    transaction_id: &str,
) -> ControlResponse {
    handle_request_value(
        json!({"id":8720,"type":"control.transfer.apply","params":{
        "transaction_id":transaction_id,"binding":transfer_binding(),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false,
        "tracking":{"kind":"proportional","reference_symbol":"Main.SP","manual_command_symbol":"Main.ManualCommand",
            "process_variable":"Main.PV","gain_symbol":"Main.Kp","align_reference":true}}}),
        state,
        None,
    )
}

/// Writes a field of the fixture's program instance into the program between scans, so that a task boundary can be
/// checked without running the program.
fn write_main_field(program: &mut dyn crate::runtime::ProgramWrite, field: &str, value: Value) {
    let at = program
        .locate(&format!("Main.{field}"))
        .unwrap_or_else(|error| panic!("Main.{field}: {error}"));
    program
        .write_at(&at, &value)
        .unwrap_or_else(|error| panic!("Main.{field}: {error}"));
}

#[test]
fn controller_transfer_label_must_agree_with_the_runtime_target_fact() {
    let (state, _harness) = transfer_fixture();
    let before = transfer_status(&state);
    let physical = transfer_apply_request(&state, &before, "label-physical", "physical", true);
    assert!(!physical.ok);
    assert_eq!(
        refusal_text(&physical),
        "The request says physical, but the runtime's target is simulated."
    );
    state.experiments.record_simulated_target(false);
    let simulated = transfer_apply_request(&state, &before, "label-simulation", "simulation", false);
    assert!(!simulated.ok);
    assert_eq!(
        refusal_text(&simulated),
        "The request says simulation, but the runtime's target is physical."
    );
}

#[test]
fn controller_transfer_physical_target_needs_approval_at_apply() {
    let (state, _harness) = transfer_fixture();
    state.experiments.record_simulated_target(false);
    let before = transfer_status(&state);
    let unapproved = transfer_apply_request(&state, &before, "physical-unapproved", "physical", false);
    assert!(!unapproved.ok);
    assert_eq!(refusal_text(&unapproved), "physical review required");
    let approved = transfer_apply_request(&state, &before, "physical-approved", "physical", true);
    assert!(approved.ok, "{:?}", approved.error);
    assert_eq!(transfer_status(&state)["status"], "pending");
}

#[test]
fn controller_transfer_physical_target_at_the_boundary_needs_approval() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    let applied = transfer_apply_request(&state, &before, "boundary-approval", "simulation", false);
    assert!(applied.ok, "{:?}", applied.error);
    // The target turns physical after the review and before the boundary, for example when a fieldbus client appears:
    // the boundary of the programs outside every task, at which the scan commits the transfer.
    harness
        .runtime_mut()
        .with_program(|program| state.debug.commit_transfer("$background", program, true));
    let after = transfer_status(&state);
    assert_eq!(after["status"], "rejected");
    assert_eq!(
        after["reason"],
        "attached I/O requires explicit physical review"
    );
    assert_eq!(after["values"], before["values"], "a refused boundary writes nothing");
}

#[test]
fn controller_transfer_status_of_another_binding_leaves_the_record_unchanged() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    assert!(queue_transfer(&state, &before).ok);
    assert!(harness.cycle().errors.is_empty());
    let applied = transfer_status(&state);
    assert_eq!(applied["status"], "applied");
    let mut replacement = transfer_binding();
    replacement["states"][0]["maximum"] = json!(15.0);
    let other = handle_request_value(
        json!({"id":8710,"type":"control.transfer.status","params":{"binding":replacement}}),
        &state,
        None,
    );
    assert!(other.ok, "{:?}", other.error);
    let other = other.result.unwrap();
    assert_eq!(other["status"], "idle", "a binding reports only the transaction it owns");
    assert!(other["transaction"].is_null());
    let again = transfer_status(&state);
    assert_eq!(again["status"], "applied");
    assert_eq!(
        again["transaction"], applied["transaction"],
        "a status read does not overwrite the record"
    );
}

#[test]
fn controller_transfer_measurement_outside_its_range_is_rejected_at_the_boundary() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    let response = tracked_transfer_apply(&state, &before, "measured-out-of-range");
    assert!(response.ok, "{:?}", response.error);
    harness.runtime_mut().with_program(|program| {
        write_main_field(program, "PV", Value::LReal(25.0));
        state.debug.commit_transfer("$background", program, false);
    });
    let after = transfer_status(&state);
    assert_eq!(after["status"], "rejected");
    assert!(
        after["reason"]
            .as_str()
            .unwrap()
            .contains("Main.PV = 25 is outside its declared range [-20, 20]"),
        "{after}"
    );
}

#[test]
fn controller_transfer_bad_quality_measurement_is_rejected_at_the_boundary() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    assert!(tracked_transfer_apply(&state, &before, "measured-not-finite").ok);
    harness.runtime_mut().with_program(|program| {
        write_main_field(program, "PV", Value::LReal(f64::NAN));
        state.debug.commit_transfer("$background", program, false);
    });
    let after = transfer_status(&state);
    assert_eq!(after["status"], "rejected");
    assert_eq!(after["reason"], "Main.PV has bad quality: not a finite number");
}

#[test]
fn controller_transfer_refuses_a_live_write_while_the_snapshot_is_stale() {
    let (state, _harness) = transfer_fixture();
    state.debug.invalidate_control_program();
    let before = transfer_status(&state);
    let response = transfer_apply_request(&state, &before, "stale-snapshot", "simulation", false);
    assert!(!response.ok);
    assert!(
        refusal_text(&response).contains("runtime snapshot is stale"),
        "{:?}",
        response.error
    );
}

#[test]
fn controller_transfer_fault_reverts_the_committed_members_in_the_same_scan() {
    let (state, mut harness) = transfer_fixture();
    let before = transfer_status(&state);
    assert!(queue_transfer(&state, &before).ok);
    // The boundary of the programs outside every task, through the program access seam (specification 44 §13.3).
    harness
        .runtime_mut()
        .with_program(|program| state.debug.commit_transfer("$background", program, false));
    assert_eq!(
        harness.get_output("Main.Manual"),
        Some(Value::Bool(false)),
        "the commit wrote the mode"
    );
    harness.runtime_mut().with_program(|program| {
        state
            .debug
            .fail_transfer("runtime fault in the owning task", program);
    });
    assert_eq!(
        harness.get_output("Main.Manual"),
        Some(Value::Bool(true)),
        "the mode is reverted in the same scan"
    );
    assert_eq!(
        harness.get_output("Main.SP"),
        Some(Value::LReal(10.0)),
        "the reference is reverted in the same scan"
    );
    let failed = transfer_status(&state);
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["transaction"]["restored"], true);
}

#[test]
fn controller_transfer_restores_a_bool_output_as_bool_after_a_rejected_first_output() {
    let source = "PROGRAM Main VAR Manual : BOOL := TRUE; SP : LREAL := 10.0; ManualCommand : LREAL := 2.0; PV : LREAL := 10.0; Valve : BOOL := FALSE; END_VAR IF Manual THEN Valve := FALSE; ELSE Valve := TRUE; END_IF; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    let binding = json!({"loop_id":"valve-loop","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[{"symbol":"Main.SP","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20},
            {"symbol":"Main.ManualCommand","iec_type":"LREAL","unit":"V","minimum":-20,"maximum":20}],
        "guards":[{"symbol":"Main.PV","iec_type":"LREAL","unit":"m","minimum":-20,"maximum":20}],
        "output":{"symbol":"Main.Valve","iec_type":"BOOL","unit":"1","minimum":0,"maximum":1}});
    let before = handle_request_value(
        json!({"id":8730,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    )
    .result
    .unwrap();
    let entry = handle_request_value(
        json!({"id":8731,"type":"control.transfer.apply","params":{
        "transaction_id":"valve-entry","binding":binding,
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(entry.ok, "{:?}", entry.error);
    assert!(harness.cycle().errors.is_empty());
    let failed = handle_request_value(
        json!({"id":8732,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    )
    .result
    .unwrap();
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["transaction"]["restored"], true);
    assert_eq!(
        harness.get_output("Main.Valve"),
        Some(Value::Bool(false)),
        "a BOOL output is restored as BOOL"
    );
}

#[test]
fn controller_transfer_refusal_names_the_running_transfer_and_parameter() {
    let (state, _harness) = transfer_fixture();
    let before = transfer_status(&state);
    assert!(queue_transfer(&state, &before).ok);
    let second = transfer_apply_request(&state, &before, "transfer-2", "simulation", false);
    assert!(!second.ok);
    assert_eq!(
        refusal_text(&second),
        "a controller transfer transfer-1 is pending"
    );
    let gains = gain_status(&state);
    let parameter = handle_request_value(
        json!({"id":8661,"type":"control.parameters.apply","params":{
        "transaction_id":"parameter-blocked","binding":gain_binding(),
        "expected_program_revision":gains["program_revision"],"expected_binding_revision":gains["binding_revision"],
        "expected_active_revision":gains["active_revision"],"expected_values":gains["values"],
        "values":{"Main.Kp":3.0},"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(!parameter.ok);
    assert_eq!(
        refusal_text(&parameter),
        "a controller transfer transfer-1 is pending"
    );
}

#[test]
fn controller_transfer_loop_identity_message_names_the_limit() {
    let (state, _harness) = transfer_fixture();
    let mut binding = transfer_binding();
    binding["loop_id"] = json!("x".repeat(129));
    let response = handle_request_value(
        json!({"id":8740,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    );
    assert!(!response.ok);
    assert_eq!(
        refusal_text(&response),
        "transfer loop_id must be 1 to 128 characters"
    );
}

#[test]
fn controller_transfer_identifiers_are_counted_in_characters() {
    let (state, _harness) = transfer_fixture();
    let before = transfer_status(&state);
    // 128 characters are 256 bytes: a byte count would refuse both requests below.
    let id = "é".repeat(128);
    let applied = transfer_apply_request(&state, &before, &id, "simulation", false);
    assert!(applied.ok, "{:?}", applied.error);
    let cancelled = handle_request_value(
        json!({"id":8750,"type":"control.transfer.cancel","params":{"loop_id":"ordinary-loop",
            "transaction_id":id,"expected_program_revision":before["program_revision"]}}),
        &state,
        None,
    );
    assert!(cancelled.ok, "{:?}", cancelled.error);
    assert_eq!(cancelled.result.unwrap()["status"], "cancelled");
}

#[test]
fn controller_transfer_pending_label_names_the_live_write() {
    let (state, _harness) = transfer_fixture();
    assert_eq!(state.debug.pending_transaction_label(), None);
    let before = transfer_status(&state);
    assert!(queue_transfer(&state, &before).ok);
    assert_eq!(
        state.debug.pending_transaction_label().as_deref(),
        Some("a controller transfer transfer-1")
    );
    let cancelled = handle_request_value(
        json!({"id":8760,"type":"control.transfer.cancel","params":{"loop_id":"ordinary-loop",
            "transaction_id":"transfer-1","expected_program_revision":before["program_revision"]}}),
        &state,
        None,
    );
    assert!(cancelled.ok, "{:?}", cancelled.error);
    assert_eq!(state.debug.pending_transaction_label(), None);
}
