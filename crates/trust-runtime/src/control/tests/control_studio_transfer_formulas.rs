#[test]
fn controller_transfer_scaled_measurement_formula_multiplies_the_fresh_measurement_by_the_gain() {
    // A feedforward state starts at K_ff times the measured disturbance (Specification 35 §17.12).
    let source = "PROGRAM Main VAR Manual : BOOL := TRUE; SP : LREAL := 10.0; PV : LREAL := 10.0; ManualCommand : LREAL := 2.0; MV AT %QL0 : LREAL; Load : LREAL := 30.0; Kff : LREAL := 0.5; FeedState : LREAL := 0.0; END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := ManualCommand; END_IF; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = harness.runtime_mut().enable_debug();
    assert!(harness.cycle().errors.is_empty());
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    state.debug.refresh_snapshot_from_storage(
        harness.runtime().storage(),
        harness.runtime().current_time(),
    );
    let binding = json!({"loop_id":"scaled-measurement-loop","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[
            {"symbol":"Main.SP","iec_type":"LREAL","unit":"degC","minimum":-20,"maximum":20},
            {"symbol":"Main.ManualCommand","iec_type":"LREAL","unit":"%","minimum":-100,"maximum":100},
            {"symbol":"Main.FeedState","iec_type":"LREAL","unit":"%","minimum":-100,"maximum":100}],
        "guards":[
            {"symbol":"Main.Kff","iec_type":"LREAL","unit":"%/kW","minimum":-10,"maximum":10},
            {"symbol":"Main.PV","iec_type":"LREAL","unit":"degC","minimum":-20,"maximum":20},
            {"symbol":"Main.Load","iec_type":"LREAL","unit":"kW","minimum":0,"maximum":100}],
        "output":{"symbol":"Main.MV","iec_type":"LREAL","unit":"%","minimum":-100,"maximum":100}});
    let status = handle_request_value(
        json!({"id":8720,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    );
    assert!(status.ok, "scaled-measurement status: {:?}", status.error);
    let before = status.result.unwrap();
    let apply = handle_request_value(
        json!({"id":8721,"type":"control.transfer.apply","params":{
        "transaction_id":"scaled-measurement-formula","binding":binding,
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":10.0,"Main.ManualCommand":2.0,"Main.FeedState":0.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false,
        "tracking":{"kind":"declared-state-v1","reference_symbol":"Main.SP","manual_command_symbol":"Main.ManualCommand",
            "process_variable":"Main.PV","align_output":true,"additional_measurements":["Main.Load"],
            "initialization_values":{"Main.FeedState":0.0},
            "initialization_formulas":{"Main.FeedState":{"kind":"scaled-measurement","process_variable":"Main.Load","gain_symbol":"Main.Kff"}},
            "bounded_state_symbols":[]}}}),
        &state,
        None,
    );
    assert!(
        apply.ok,
        "queue scaled-measurement transfer: {:?}",
        apply.error
    );
    let Value::Instance(id) = harness.runtime().storage().get_global("Main").unwrap() else {
        panic!("program instance");
    };
    // The disturbance moves before the boundary: the state follows the fresh measurement, not the reviewed one.
    state
        .debug
        .enqueue_instance_write(*id, "Load", Value::LReal(36.0));
    assert!(harness.cycle().errors.is_empty());
    let after = handle_request_value(
        json!({"id":8722,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    )
    .result
    .unwrap();
    assert_eq!(after["status"], "applied", "{after}");
    assert_eq!(
        after["transaction"]["applied_values"]["Main.FeedState"],
        18.0
    );
}

#[test]
fn controller_transfer_decoupled_output_formula_starts_each_member_at_its_share_of_the_plant_inputs(
) {
    // A decoupled group member starts at row j of D⁻¹·u (Specification 35 §17.15).
    let source = "PROGRAM Main VAR Manual : BOOL := TRUE; SP : LREAL := 35.0; PV : LREAL := 35.0; ManualOne : LREAL := 50.0; ManualTwo : LREAL := 50.0; HeaterOne AT %QL0 : LREAL; HeaterTwo AT %QL8 : LREAL; D11 : LREAL := 1.0; D12 : LREAL := -0.5; D21 : LREAL := -0.5; D22 : LREAL := 1.0; RequestOne : LREAL := 0.0; RequestTwo : LREAL := 0.0; END_VAR HeaterOne := ManualOne; HeaterTwo := ManualTwo; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = harness.runtime_mut().enable_debug();
    assert!(harness.cycle().errors.is_empty());
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    state.debug.refresh_snapshot_from_storage(
        harness.runtime().storage(),
        harness.runtime().current_time(),
    );
    let member = |symbol: &str, unit: &str, minimum: f64, maximum: f64| json!({"symbol":symbol,"iec_type":"LREAL","unit":unit,"minimum":minimum,"maximum":maximum});
    let binding = json!({"loop_id":"decoupled-output-loop","task_name":"$background",
        "mode":{"symbol":"Main.Manual","manual_value":true},
        "states":[member("Main.SP","degC",0.0,100.0), member("Main.ManualOne","%",0.0,100.0), member("Main.ManualTwo","%",0.0,100.0),
            member("Main.RequestOne","%",-500.0,500.0), member("Main.RequestTwo","%",-500.0,500.0)],
        "guards":[member("Main.D11","1",-10.0,10.0), member("Main.D12","1",-10.0,10.0), member("Main.D21","1",-10.0,10.0),
            member("Main.D22","1",-10.0,10.0), member("Main.PV","degC",0.0,100.0)],
        "output":member("Main.HeaterOne","%",0.0,100.0),
        "additional_outputs":[member("Main.HeaterTwo","%",0.0,100.0)]});
    let status = handle_request_value(
        json!({"id":8730,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    );
    assert!(status.ok, "decoupled-output status: {:?}", status.error);
    let before = status.result.unwrap();
    let formula = |row: usize| {
        json!({"kind":"decoupled-output","output_symbols":["Main.HeaterOne","Main.HeaterTwo"],
            "matrix_symbols":[["Main.D11","Main.D12"],["Main.D21","Main.D22"]],"row":row})
    };
    let apply = handle_request_value(
        json!({"id":8731,"type":"control.transfer.apply","params":{
        "transaction_id":"decoupled-output-formula","binding":binding,
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],
        "values":{"Main.SP":35.0,"Main.ManualOne":50.0,"Main.ManualTwo":50.0,"Main.RequestOne":0.0,"Main.RequestTwo":0.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false,
        "tracking":{"kind":"declared-state-v1","reference_symbol":"Main.SP","manual_command_symbol":"Main.ManualOne",
            "process_variable":"Main.PV","align_output":true,
            "additional_commands":[{"manual_command_symbol":"Main.ManualTwo","output_symbol":"Main.HeaterTwo"}],
            "initialization_values":{"Main.RequestOne":0.0,"Main.RequestTwo":0.0},
            "initialization_formulas":{"Main.RequestOne":formula(0),"Main.RequestTwo":formula(1)},
            "bounded_state_symbols":[]}}}),
        &state,
        None,
    );
    assert!(
        apply.ok,
        "queue decoupled-output transfer: {:?}",
        apply.error
    );
    assert!(harness.cycle().errors.is_empty());
    let after = handle_request_value(
        json!({"id":8732,"type":"control.transfer.status","params":{"binding":binding}}),
        &state,
        None,
    )
    .result
    .unwrap();
    assert_eq!(after["status"], "applied", "{after}");
    // D = [[1, -0.5], [-0.5, 1]], u = [50, 50]: D⁻¹·u = [100, 100].
    let applied = &after["transaction"]["applied_values"];
    assert!(
        (applied["Main.RequestOne"].as_f64().unwrap() - 100.0).abs() < 1e-9,
        "{applied}"
    );
    assert!(
        (applied["Main.RequestTwo"].as_f64().unwrap() - 100.0).abs() < 1e-9,
        "{applied}"
    );
}
