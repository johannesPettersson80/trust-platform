// Protocol annex E3 and E4 (specification 35 A6.1): a mode channel reads an enumeration mode as Control Studio reads a mode, 1 in
// its Manual member and 0 in its Auto member, with no usable value in another member.

/// A program whose mode, an enumeration with a third member, starts at `mode`.
fn enum_mode_signals(mode: &str) -> (ControlState, TestHarness) {
    let source = format!(
        "TYPE E_Mode : (Off, Manual, Auto); END_TYPE PROGRAM Main VAR Mode : E_Mode := E_Mode#{mode}; END_VAR END_PROGRAM"
    );
    let mut state = control_test_state(&source);
    let mut harness = TestHarness::from_source(&source).unwrap();
    state.debug = studio_debug(&mut harness);
    harness.cycle();
    (state, harness)
}

/// The inspection of the program's mode as a mode channel, after the scan that follows its selection.
fn inspect_mode(state: &ControlState, harness: &mut TestHarness) -> serde_json::Value {
    let request = json!({"id": 8730, "type": "control.signals.inspect", "params": {"loop_revision": "enum-mode", "bindings": [
        {"id": "mode", "symbol": "Main.Mode", "iec_type": "BOOL", "mode": {"manual_value": "Manual", "auto_value": "Auto"}}
    ]}});
    let first = handle_request_value(request.clone(), state, None);
    assert!(first.ok, "{:?}", first.error);
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    handle_request_value(request, state, None).result.unwrap()
}

/// Catches an enumeration mode read as its member's number (`Off` is 0, `Manual` 1 and `Auto` 2), which would read Auto as 2 and
/// Off as Auto, and a member that is neither mode read as one.
#[test]
fn a_mode_channel_reads_an_enumeration_as_manual_one_and_auto_zero() {
    for (mode, value, quality) in [
        ("Manual", json!(1.0), "good"),
        ("Auto", json!(0.0), "good"),
        ("Off", json!(null), "bad"),
    ] {
        let (state, mut harness) = enum_mode_signals(mode);
        let read = inspect_mode(&state, &mut harness);
        assert_eq!(read["values"], json!([value]), "{mode}: {read}");
        assert_eq!(read["quality"], json!([quality]), "{mode}: {read}");
    }
}

/// Catches a mode channel accepted with another type than the BOOL reading it gives, with a member its type lacks, or with one
/// member for both modes.
#[test]
fn a_mode_channel_is_a_bool_reading_of_an_enumeration_with_two_of_its_members() {
    let (state, _harness) = enum_mode_signals("Manual");
    for binding in [
        json!({"id": "mode", "symbol": "Main.Mode", "iec_type": "E_Mode", "mode": {"manual_value": "Manual", "auto_value": "Auto"}}),
        json!({"id": "mode", "symbol": "Main.Mode", "iec_type": "BOOL", "mode": {"manual_value": "Manual", "auto_value": "Automatic"}}),
        json!({"id": "mode", "symbol": "Main.Mode", "iec_type": "BOOL", "mode": {"manual_value": "Manual", "auto_value": "manual"}}),
    ] {
        let response = handle_request_value(
            json!({"id": 8731, "type": "control.signals.inspect", "params": {"loop_revision": "enum-mode-refused", "bindings": [binding]}}),
            &state,
            None,
        );
        assert!(!response.ok, "{binding} must be refused");
    }
}
