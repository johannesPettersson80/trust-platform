// Protocol annex E6.2, E6.3 and E7.1 (specification 35 A6.1): a transfer and a parameter transaction bind an enumeration mode by
// the members that mean Manual and Auto, and status reads it as TRUE in Manual and FALSE in Auto.

/// A loop whose mode is an enumeration with a third member, as CODESYS and TwinCAT code often has: in `Manual` the output is the
/// manual command, in `Auto` the law, and in `Off` zero. The mode starts at `mode`.
fn enum_mode_fixture(mode: &str) -> (ControlState, TestHarness) {
    let source = format!("TYPE E_Mode : (Off, Manual, Auto); END_TYPE PROGRAM Main VAR Kp : LREAL := 2.0; Ki : LREAL := 0.5; Mode : E_Mode := E_Mode#{mode}; ManualCommand : LREAL := 2.0; SP AT %ML0 : LREAL := 10.0; PV : LREAL := 10.0; MV AT %QL0 : LREAL; END_VAR IF Mode = E_Mode#Manual THEN MV := ManualCommand; ELSIF Mode = E_Mode#Auto THEN MV := Kp * (SP - PV); ELSE MV := 0.0; END_IF; END_PROGRAM");
    let mut state = simulated_control_state(&source);
    let mut harness = simulated_harness(&source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    (state, harness)
}

fn enum_transfer_binding(mode: serde_json::Value) -> serde_json::Value {
    let number = |symbol: &str, unit: &str| json!({"symbol":symbol,"iec_type":"LREAL","unit":unit,"minimum":-20,"maximum":20});
    json!({"loop_id":"enum-loop","task_name":"$background","mode":mode,
        "states":[number("Main.SP","m"),number("Main.ManualCommand","V")],
        "guards":[number("Main.Kp","V/m"),number("Main.PV","m")],
        "output":number("Main.MV","V")})
}

fn enum_transfer_status(state: &ControlState, binding: &serde_json::Value) -> ControlResponse {
    handle_request_value(
        json!({"id":8700,"type":"control.transfer.status","params":{"binding":binding}}),
        state,
        None,
    )
}

/// Catches an enumeration mode refused as "transfer mode must be BOOL", the Auto member not written (the law would not run),
/// another member written, and a status that reads the mode otherwise than TRUE in Manual and FALSE in Auto.
#[test]
fn controller_transfer_switches_an_enumeration_mode_by_its_manual_and_auto_members() {
    let (state, mut harness) = enum_mode_fixture("Manual");
    let binding = enum_transfer_binding(
        json!({"symbol":"Main.Mode","manual_value":"Manual","auto_value":"Auto"}),
    );
    let before = enum_transfer_status(&state, &binding);
    assert!(before.ok, "{:?}", before.error);
    let before = before.result.unwrap();
    assert_eq!(before["values"]["Main.Mode"], true, "Manual reads TRUE");
    assert_eq!(before["values"]["Main.MV"], 2.0);
    let apply = handle_request_value(
        json!({"id":8701,"type":"control.transfer.apply","params":{
        "transaction_id":"enum-transfer-1","binding":binding,
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
        &state,
        None,
    );
    assert!(apply.ok, "{:?}", apply.error);
    assert!(harness.cycle().errors.is_empty());
    let after = enum_transfer_status(&state, &binding).result.unwrap();
    assert_eq!(after["status"], "applied", "{after}");
    assert_eq!(after["values"]["Main.Mode"], false, "Auto reads FALSE");
    // Worked by hand: in Auto the law gives Kp·(SP − PV) = 2·(11 − 10) = 2, the manual command the loop held.
    assert_eq!(after["values"]["Main.MV"], 2.0);
}

/// Catches a loop in a third member, here `Off`, read as Manual or Auto: its status is refused with the member's name, so no
/// transfer starts from it (specification 35 A6.1).
#[test]
fn controller_transfer_status_refuses_a_mode_in_neither_member() {
    let (state, _harness) = enum_mode_fixture("Off");
    let binding = enum_transfer_binding(
        json!({"symbol":"Main.Mode","manual_value":"Manual","auto_value":"Auto"}),
    );
    let response = enum_transfer_status(&state, &binding);
    assert!(!response.ok, "a loop in Off is in neither mode");
    let error = response.error.unwrap_or_default();
    // The member's name as the image stores it; its case is the compiler's.
    assert!(
        error
            .to_ascii_lowercase()
            .contains("mode main.mode is off, neither manual nor auto"),
        "{error}"
    );
}

/// Catches an enumeration mode accepted without its Auto member (a transfer to Auto would have nothing to write), with a name its
/// type lacks, with one member for both modes, or with a boolean; and a BOOL mode accepted with an Auto member.
#[test]
fn controller_transfer_refuses_a_mode_without_its_two_values() {
    let (state, _harness) = enum_mode_fixture("Manual");
    for (mode, reason) in [
        (
            json!({"symbol":"Main.Mode","manual_value":"Manual"}),
            "an enumeration mode takes the names of two values of",
        ),
        (
            json!({"symbol":"Main.Mode","manual_value":"Manual","auto_value":"Automatic"}),
            "Automatic is not a value of",
        ),
        (
            json!({"symbol":"Main.Mode","manual_value":"Manual","auto_value":"manual"}),
            "manual_value and auto_value name the same value",
        ),
        (
            json!({"symbol":"Main.Mode","manual_value":true}),
            "an enumeration mode takes the names of two values of",
        ),
    ] {
        let response = enum_transfer_status(&state, &enum_transfer_binding(mode.clone()));
        assert!(!response.ok, "{mode} is refused");
        let error = response.error.unwrap_or_default();
        assert!(error.contains(reason), "{mode}: {error}");
    }
    let (state, _harness) = transfer_fixture();
    let mut binding = transfer_binding();
    binding["mode"]["auto_value"] = json!("Auto");
    let response = enum_transfer_status(&state, &binding);
    assert!(!response.ok, "a BOOL mode with an Auto member is refused");
    assert!(
        response
            .error
            .unwrap_or_default()
            .contains("a BOOL mode takes a boolean manual_value and no auto_value"),
    );
}

/// Catches a parameter transaction that refuses an enumeration mode ("mode must be BOOL"), and one that takes a member other than
/// the Manual one (here Off) for Manual.
#[test]
fn parameter_transactions_confirm_manual_by_the_enumeration_member() {
    for (mode, is_manual) in [("Manual", true), ("Off", false)] {
        let (state, mut harness) = enum_mode_fixture(mode);
        let binding = json!({"loop_id":"enum-loop","task_name":"$background","parameters":[
            {"symbol":"Main.Kp","iec_type":"LREAL","unit":"V/m","convention":"parallel Kp","minimum":0,"maximum":10},
            {"symbol":"Main.Ki","iec_type":"LREAL","unit":"V/m/s","convention":"parallel Ki","minimum":0,"maximum":10}
        ],"mode":{"symbol":"Main.Mode","manual_value":"Manual","auto_value":"Auto"}});
        let status = handle_request_value(
            json!({"id":8710,"type":"control.parameters.status","params":{"binding":binding}}),
            &state,
            None,
        );
        assert!(status.ok, "{mode}: {:?}", status.error);
        let status = status.result.unwrap();
        let apply = handle_request_value(
            json!({"id":8711,"type":"control.parameters.apply","params":{
                "transaction_id":format!("enum-apply-{mode}"),"binding":binding,"expected_program_revision":status["program_revision"],
                "expected_binding_revision":status["binding_revision"],"expected_active_revision":status["active_revision"],
                "expected_values":status["values"],"values":{"Main.Kp":4.0,"Main.Ki":1.0},"source":"simulation","physical_approved":false
            }}),
            &state,
            None,
        );
        if is_manual {
            assert!(apply.ok, "{mode}: {:?}", apply.error);
            assert!(harness.cycle().errors.is_empty());
        } else {
            assert!(!apply.ok, "{mode} is not Manual");
            assert!(
                apply
                    .error
                    .unwrap_or_default()
                    .contains("confirmed Manual mode required"),
                "{mode}"
            );
        }
    }
}

/// An experiment on a loop whose mode, an enumeration, starts at `mode`, bound by the names of its Manual and Auto members
/// (protocol annex E5.3).
fn start_enum_mode_experiment(mode: &str, kind: &str, symbol: &str, restoration: f64) -> ControlResponse {
    let source = format!("TYPE E_Mode : (Off, Manual, Auto); END_TYPE PROGRAM Main VAR ManualOutput : LREAL := 12.5; Setpoint : LREAL := 20.0; Mode : E_Mode := E_Mode#{mode}; END_VAR END_PROGRAM");
    let state = control_test_state(&source);
    state.connect_experiments();
    state.experiments.record_simulated_target(true);
    with_fresh_scan(&state);
    handle_request_value(
        json!({"id": 8720, "type": "control.experiment.start", "params": {
            "experiment_id": format!("enum-{mode}-{kind}"), "source": "simulation", "kind": kind, "task": "$background",
            "symbol": symbol, "value": 40.0, "restoration_value": restoration, "minimum": 0, "maximum": 100,
            "duration_ms": 5_000, "expected_scan": 7,
            "mode_binding": {"symbol": "Main.Mode", "manual_value": "Manual", "auto_value": "Auto"}}}),
        &state,
        None,
    )
}

/// Catches an enumeration mode refused when its binding names members, Manual taken for any member but `Manual`, and Auto taken
/// for any member but `Auto` once the binding names it: a loop in `Off` would have its setpoint stepped.
#[test]
fn experiments_bind_an_enumeration_mode_by_its_manual_and_auto_members() {
    // A plant-input step runs in Manual only, and a setpoint step in Auto only.
    for (mode, kind, symbol, restoration, starts) in [
        ("Manual", "output_step", "Main.ManualOutput", 12.5, true),
        ("Off", "output_step", "Main.ManualOutput", 12.5, false),
        ("Auto", "setpoint_step", "Main.Setpoint", 20.0, true),
        ("Off", "setpoint_step", "Main.Setpoint", 20.0, false),
    ] {
        let response = start_enum_mode_experiment(mode, kind, symbol, restoration);
        if starts {
            assert!(response.ok, "{kind} in {mode}: {:?}", response.error);
        } else {
            assert!(!response.ok, "{kind} in {mode} does not start");
            assert_eq!(
                response.error_code.as_deref(),
                Some("controller_mode_blocked"),
                "{kind} in {mode}: {:?}",
                response.error
            );
        }
    }
}
