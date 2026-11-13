// A TIME parameter (protocol annex E7.1, E7.3 step 10): its number is seconds, the seconds of a whole number of
// nanoseconds, as the standard form's reset time of TwinCAT's `FB_CTRL_PI` (`tTn : TIME`) holds it.

const TIME_PARAMETER_PROGRAM: &str = "PROGRAM Main VAR Ti : TIME := T#2s; Manual : BOOL := TRUE; Observed : TIME; END_VAR Observed := Ti; END_PROGRAM";

fn time_binding(unit: &str) -> serde_json::Value {
    json!({"loop_id":"standard-loop","task_name":"$background","parameters":[
        {"symbol":"Main.Ti","iec_type":"TIME","unit":unit,"convention":"standard Ti","minimum":0.001,"maximum":3600}
    ],"mode":{"symbol":"Main.Manual","manual_value":true}})
}

fn time_status(state: &ControlState, unit: &str) -> ControlResponse {
    handle_request_value(
        json!({"id":8601,"type":"control.parameters.status","params":{"binding":time_binding(unit)}}),
        state,
        None,
    )
}

fn time_apply(state: &ControlState, status: &serde_json::Value, seconds: f64) -> ControlResponse {
    static NEXT_APPLY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let transaction_id = format!(
        "time-apply-{}",
        NEXT_APPLY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    handle_request_value(
        json!({"id":8602,"type":"control.parameters.apply","params":{
            "transaction_id":transaction_id,"binding":time_binding("s"),"expected_program_revision":status["program_revision"],
            "expected_binding_revision":status["binding_revision"],"expected_active_revision":status["active_revision"],
            "expected_values":status["values"],"values":{"Main.Ti":seconds},"source":"simulation","physical_approved":false
        }}),
        state,
        None,
    )
}

/// The reset time `T#2s` reads as 2.0 seconds; 1234.567 s, whole milliseconds as Control Studio writes them, commits
/// as `T#1234567ms` at the boundary and reads back as 1234.567. 1234.567 is not exactly 1234.567 x 10^9 nanoseconds in
/// binary, but those nanoseconds read back as the same number. Catches a TIME refused as an unsupported type, seconds
/// written as milliseconds or nanoseconds, and a value that a float product leaves a fraction of a nanosecond off.
#[test]
fn a_time_parameter_reads_and_writes_its_seconds() {
    let (state, mut harness) = generic_fixture_for(TIME_PARAMETER_PROGRAM);
    let before = time_status(&state, "s");
    assert!(before.ok, "{:?}", before.error);
    let before = before.result.unwrap();
    assert_eq!(before["values"]["Main.Ti"], 2.0);
    let apply = time_apply(&state, &before, 1234.567);
    assert!(apply.ok, "{:?}", apply.error);
    assert!(harness.cycle().errors.is_empty());
    let after = time_status(&state, "s").result.unwrap();
    assert_eq!(after["status"], "applied", "{after}");
    assert_eq!(after["values"]["Main.Ti"], 1234.567);
    assert_eq!(
        harness.get_output("Ti"),
        Some(crate::value::Value::Time(crate::value::Duration::from_millis(
            1_234_567
        )))
    );
}

/// A TIME value must be the seconds of a whole number of nanoseconds: 1.0000000001 s is 1,000,000,000.1 ns and is
/// refused. A TIME parameter's unit is seconds: a binding that names `ms` is refused. Catches a value rounded silently
/// to a nanosecond the client did not send, and a unit that would make the numbers mean something else.
#[test]
fn a_time_parameter_takes_whole_nanoseconds_in_seconds_only() {
    let (state, _harness) = generic_fixture_for(TIME_PARAMETER_PROGRAM);
    let status = time_status(&state, "s").result.unwrap();
    let refused = time_apply(&state, &status, 1.000_000_000_1);
    assert!(!refused.ok, "a fraction of a nanosecond is refused");
    let reason = format!("{:?}", refused.error);
    assert!(reason.contains("whole nanoseconds"), "{reason}");
    let milliseconds = time_status(&state, "ms");
    assert!(!milliseconds.ok, "a TIME parameter in ms is refused");
    let reason = format!("{:?}", milliseconds.error);
    assert!(
        reason.contains("invalid parameter type, units, convention or bounds"),
        "{reason}"
    );
}
