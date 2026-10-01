fn transfer_binding() -> serde_json::Value {
    let number = |symbol: &str, unit: &str| json!({"symbol":symbol,"iec_type":"LREAL","unit":unit,"minimum":-20,"maximum":20});
    json!({"loop_id":"ordinary-loop","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[number("Main.SP","m"),number("Main.ManualCommand","V")],
        "guards":[number("Main.Kp","V/m"),number("Main.PV","m")],
        "output":number("Main.MV","V")})
}

fn transfer_fixture() -> (ControlState, TestHarness) {
    let source = "PROGRAM Main VAR Kp : LREAL := 2.0; Manual : BOOL := TRUE; ManualCommand : LREAL := 2.0; SP AT %ML0 : LREAL := 10.0; PV : LREAL := 10.0; MV AT %QL0 : LREAL; Alias : LREAL := 10.0; END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := Kp * (SP-PV); END_IF; END_PROGRAM";
    let mut state = hmi_test_state(source);
    let mut harness = TestHarness::from_source(source).unwrap();
    state.debug = harness.runtime_mut().enable_debug();
    assert!(harness.cycle().errors.is_empty());
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    state.debug.refresh_snapshot_from_storage(
        harness.runtime().storage(),
        harness.runtime().current_time(),
    );
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
        let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
            panic!("program instance");
        };
        if forced {
            state.debug.force_instance(*id, "Kp", Value::LReal(2.0));
        } else {
            state
                .debug
                .enqueue_instance_write(*id, "Kp", Value::LReal(3.0));
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
    let mut state = hmi_test_state(source);
    let mut harness = TestHarness::from_source(source).unwrap();
    state.debug = harness.runtime_mut().enable_debug();
    assert!(harness.cycle().errors.is_empty());
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    state.debug.refresh_snapshot_from_storage(
        harness.runtime().storage(),
        harness.runtime().current_time(),
    );
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
    let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
        panic!("program instance")
    };
    state
        .debug
        .enqueue_instance_write(*id, "OuterAuto", Value::Bool(true));
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
    let mut state = hmi_test_state(source);
    let mut harness = TestHarness::from_source(source).unwrap();
    state.debug = harness.runtime_mut().enable_debug();
    assert!(harness.cycle().errors.is_empty());
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    state.debug.refresh_snapshot_from_storage(
        harness.runtime().storage(),
        harness.runtime().current_time(),
    );
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
        let (state, mut harness) = transfer_fixture();
        let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
            panic!("program instance");
        };
        let reference = harness
            .runtime()
            .storage()
            .resolved_instance_field_ref(*id, "Alias")
            .unwrap();
        harness.runtime_mut().io_mut().bind_ref_typed(
            reference,
            crate::io::IoAddress::parse(address).unwrap(),
            trust_hir::TypeId::LREAL,
        );
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
    assert_eq!(failed["values"]["Main.Manual"], false, "Auto was entered");
    assert_eq!(
        harness
            .runtime()
            .io()
            .read(&crate::io::IoAddress::parse("%QL0").unwrap())
            .unwrap(),
        Value::LWord(2.0_f64.to_bits()),
        "the rejected first output image is not published"
    );

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

    // The separately guarded recovery holds the current output in Manual, then restores the gain.
    let current = transfer_status(&state);
    let held = current["values"]["Main.MV"].clone();
    let recovery = handle_request_value(
        json!({"id":8644,"type":"control.transfer.apply","params":{
        "transaction_id":"manual-recovery","binding":transfer_binding(),
        "expected_program_revision":current["program_revision"],"expected_binding_revision":current["binding_revision"],
        "expected_values":current["values"],"values":{"Main.SP":10.0,"Main.ManualCommand":held},
        "manual":true,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(recovery.ok, "{:?}", recovery.error);
    assert!(harness.cycle().errors.is_empty());
    let recovered = transfer_status(&state);
    assert_eq!(recovered["status"], "applied", "{recovered}");
    assert_eq!(recovered["values"]["Main.Manual"], true);
    assert_eq!(recovered["values"]["Main.MV"], held);

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
    let (state, mut harness) = transfer_fixture();
    let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
        panic!("program instance");
    };
    let reference = harness
        .runtime()
        .storage()
        .resolved_instance_field_ref(*id, "Alias")
        .unwrap();
    harness.runtime_mut().io_mut().bind_ref_typed(
        reference,
        crate::io::IoAddress::parse("%QL0").unwrap(),
        trust_hir::TypeId::LREAL,
    );
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
    let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
        panic!("program instance");
    };
    state
        .debug
        .enqueue_instance_write(*id, "PV", Value::LReal(10.1));
    state
        .debug
        .enqueue_instance_write(*id, "MV", Value::LReal(4.0));
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

#[test]
fn controller_transfer_uses_declared_applied_feedback_as_output_basis() {
    let source = "PROGRAM Main VAR Kp : LREAL := 2.0; Manual : BOOL := TRUE; ManualCommand : LREAL := 2.0; SP AT %ML0 : LREAL := 10.0; PV : LREAL := 10.0; MV AT %QL0 : LREAL; Applied AT %IL0 : LREAL; END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := Kp * (SP-PV); END_IF; END_PROGRAM";
    let mut state = hmi_test_state(source);
    let mut harness = TestHarness::from_source(source).unwrap();
    state.debug = harness.runtime_mut().enable_debug();
    assert!(harness.cycle().errors.is_empty());
    harness
        .set_direct_input("%IL0", Value::LWord(2.0_f64.to_bits()))
        .unwrap();
    assert_eq!(
        harness
            .runtime()
            .io()
            .read(&crate::io::IoAddress::parse("%IL0").unwrap())
            .unwrap(),
        Value::LWord(2.0_f64.to_bits())
    );
    assert!(harness.cycle().errors.is_empty());
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    state.debug.refresh_snapshot_from_storage(
        harness.runtime().storage(),
        harness.runtime().current_time(),
    );
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
    let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
        panic!("program instance");
    };
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
        .enqueue_instance_write(*id, "Applied", Value::LReal(2.0));
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
    let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
        panic!("program instance");
    };
    state
        .debug
        .enqueue_instance_write(*id, "Integral", Value::LReal(1.0));
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
    let mut state = hmi_test_state(source);
    let mut harness = TestHarness::from_source(source).unwrap();
    state.debug = harness.runtime_mut().enable_debug();
    assert!(harness.cycle().errors.is_empty());
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    state.debug.refresh_snapshot_from_storage(
        harness.runtime().storage(),
        harness.runtime().current_time(),
    );
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
    let mut state = hmi_test_state(source);
    let mut harness = TestHarness::from_source(source).unwrap();
    state.debug = harness.runtime_mut().enable_debug();
    assert!(harness.cycle().errors.is_empty());
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    state.debug.refresh_snapshot_from_storage(
        harness.runtime().storage(),
        harness.runtime().current_time(),
    );
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
    let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
        panic!("program instance");
    };
    state
        .debug
        .enqueue_instance_write(*id, "PV", Value::LReal(10.25));
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
