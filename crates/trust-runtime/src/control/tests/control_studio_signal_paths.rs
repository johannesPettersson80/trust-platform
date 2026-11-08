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
    state.debug = studio_debug(&mut harness);
    harness.cycle();
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
    state.debug = studio_debug(&mut harness);
    harness.cycle();
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

const GVL_SIGNALS_SOURCE: &str = r#"
NAMESPACE GVL_IO
VAR_GLOBAL
    lrLevel : LREAL;
END_VAR
END_NAMESPACE

VAR_GLOBAL
    gSetpoint : LREAL := 2.5;
END_VAR

PROGRAM Main
VAR
    Ticks : LREAL;
END_VAR
    Ticks := Ticks + 1.0;
    GVL_IO.lrLevel := Ticks * 0.5;
END_PROGRAM
"#;

/// Protocol annex E2.1 (the CODESYS toolkit's S9, its reads): a symbol's root is a global variable
/// of the loaded program, so a global of a namespace, a CODESYS GVL member, is read by its qualified
/// name (`GVL_IO.lrLevel`), and a global scalar by its own name (`gSetpoint`). Worked by hand: after
/// two scans `Ticks` is 2, so `GVL_IO.lrLevel` is 1.0, and `gSetpoint` keeps its 2.5. Catches the
/// check that took only a global or a program instance as a path's first name, which refused
/// `GVL_IO` as "program instance not found", and the one that wanted a member after it.
#[test]
fn control_signals_read_a_global_of_a_namespace_and_a_global_scalar() {
    let mut state = control_test_state(GVL_SIGNALS_SOURCE);
    let mut harness = TestHarness::from_source(GVL_SIGNALS_SOURCE).unwrap();
    state.debug = studio_debug(&mut harness);
    harness.cycle();
    let bindings = json!([
        {"id": "gvl", "symbol": "GVL_IO.lrLevel", "iec_type": "LREAL"},
        {"id": "global", "symbol": "gSetpoint", "iec_type": "LREAL"}
    ]);
    let first = handle_request_value(
        json!({"id": 8723, "type": "control.signals.inspect", "params": {"loop_revision": "gvl", "bindings": bindings}}),
        &state,
        None,
    );
    assert!(first.ok, "a GVL member and a global are selectable: {:?}", first.error);
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    let second = handle_request_value(
        json!({"id": 8724, "type": "control.signals.inspect", "params": {"loop_revision": "gvl", "bindings": bindings}}),
        &state,
        None,
    );
    let second = second.result.unwrap();
    assert_eq!(second["available"], true, "{second}");
    assert_eq!(second["values"], json!([1.0, 2.5]));
}
