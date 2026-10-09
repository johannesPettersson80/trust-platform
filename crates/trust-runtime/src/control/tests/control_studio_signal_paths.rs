// Control signal reads through structure fields, inherited members and enumerations
// (Specification 35, control signal reads): what a loop's controller sees is read where the
// program keeps it, inside blocks and records, not only in plain addressed variables.

const SIGNAL_PATHS_SOURCE: &str = r#"
TYPE LoopMode : (Off, Manual, Automatic); END_TYPE
TYPE Sample :
STRUCT
    ProcessValue : LREAL;
    Mode : LoopMode;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Sensor
VAR_INPUT
    Value : LREAL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK TemperatureSensor EXTENDS Sensor
END_FUNCTION_BLOCK

FUNCTION_BLOCK Holder
VAR
    State : Sample;
END_VAR
    State.ProcessValue := State.ProcessValue + 2.0;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Probe : TemperatureSensor;
    Keeper : Holder;
    Input : Sample := (Mode := LoopMode#Automatic);
END_VAR
    Probe(Value := 7.5);
    Keeper();
    Input.ProcessValue := Probe.Value;
END_PROGRAM
"#;

#[test]
fn control_signals_read_inherited_members_structure_fields_and_enumerations() {
    let mut state = control_test_state(SIGNAL_PATHS_SOURCE);
    let mut harness = TestHarness::from_source(SIGNAL_PATHS_SOURCE).unwrap();
    state.debug = harness.runtime_mut().enable_debug();
    harness.cycle();
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    state.debug.refresh_snapshot_from_storage(
        harness.runtime().storage(),
        harness.runtime().current_time(),
    );
    let bindings = json!([
        {"id": "inherited", "symbol": "Main.Probe.Value", "iec_type": "LREAL"},
        {"id": "block-field", "symbol": "Main.Keeper.State.ProcessValue", "iec_type": "LREAL"},
        {"id": "record-field", "symbol": "Main.Input.ProcessValue", "iec_type": "LREAL"},
        {"id": "enumeration", "symbol": "Main.Input.Mode", "iec_type": "LoopMode"}
    ]);
    let first = handle_request_value(
        json!({"id": 8720, "type": "control.signals.inspect", "params": {"loop_revision": "paths", "bindings": bindings}}),
        &state,
        None,
    );
    assert!(
        first.ok,
        "fields, inherited members and enumerations must be selectable: {:?}",
        first.error
    );
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    let second = handle_request_value(
        json!({"id": 8721, "type": "control.signals.inspect", "params": {"loop_revision": "paths", "bindings": bindings}}),
        &state,
        None,
    );
    let second = second.result.unwrap();
    assert_eq!(second["available"], true, "{second}");
    assert_eq!(second["values"], json!([7.5, 4.0, 7.5, 2.0]));
    assert_eq!(
        second["quality"],
        json!(["good", "good", "good", "good"])
    );
}

#[test]
fn an_enumeration_needs_its_own_type_name_and_a_missing_member_is_refused() {
    let mut state = control_test_state(SIGNAL_PATHS_SOURCE);
    let mut harness = TestHarness::from_source(SIGNAL_PATHS_SOURCE).unwrap();
    state.debug = harness.runtime_mut().enable_debug();
    harness.cycle();
    *state.metadata.lock().unwrap() = harness.runtime().metadata_snapshot();
    state.debug.refresh_snapshot_from_storage(
        harness.runtime().storage(),
        harness.runtime().current_time(),
    );
    for (symbol, iec_type) in [
        ("Main.Input.Mode", "DINT"),
        ("Main.Input.Missing", "LREAL"),
        ("Main.Probe.Missing", "LREAL"),
    ] {
        let response = handle_request_value(
            json!({"id": 8722, "type": "control.signals.inspect", "params": {"loop_revision": "paths-refused", "bindings": [{"id": "channel", "symbol": symbol, "iec_type": iec_type}]}}),
            &state,
            None,
        );
        assert!(!response.ok, "{symbol} as {iec_type} must be refused");
    }
}
