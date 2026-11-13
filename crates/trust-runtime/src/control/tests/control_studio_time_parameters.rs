// A TIME parameter (protocol annex E7.1, E7.3 step 10): its number is seconds, the seconds of a whole number of
// nanoseconds, as the standard form's reset time of TwinCAT's `FB_CTRL_PI` (`tTn : TIME`) holds it. A transfer reads
// it as a guard in seconds (E6.2, E6.3).

const TIME_PARAMETER_PROGRAM: &str = "PROGRAM Main VAR Ti : TIME := T#2s; Manual : BOOL := TRUE; Observed : TIME; END_VAR Observed := Ti; END_PROGRAM";

/// A reset time of 1 ms up to `T#49d17h2m47s295ms`, the largest TIME of CODESYS.
fn time_binding(unit: &str) -> serde_json::Value {
    json!({"loop_id":"standard-loop","task_name":"$background","parameters":[
        {"symbol":"Main.Ti","iec_type":"TIME","unit":unit,"convention":"standard Ti","minimum":0.001,"maximum":4294967.295}
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

/// 4194792.263 s, whole milliseconds (about 48.5 days), commits as `T#4194792263ms` and reads back as 4194792.263: the
/// runtime takes the decimal the client wrote. Its product with 10^9 in binary is 4194792263000000.5, whose nearest
/// nanoseconds read back as 4194792.263000001. Catches a TIME taken from that product, which refuses some of the
/// whole-millisecond times from 2^22 s on.
#[test]
fn a_time_parameter_takes_the_decimal_the_client_wrote() {
    let (state, mut harness) = generic_fixture_for(TIME_PARAMETER_PROGRAM);
    let before = time_status(&state, "s");
    assert!(before.ok, "{:?}", before.error);
    let before = before.result.unwrap();
    let apply = time_apply(&state, &before, 4_194_792.263);
    assert!(apply.ok, "{:?}", apply.error);
    assert!(harness.cycle().errors.is_empty());
    let after = time_status(&state, "s").result.unwrap();
    assert_eq!(after["status"], "applied", "{after}");
    assert_eq!(after["values"]["Main.Ti"], 4_194_792.263);
    assert_eq!(
        harness.get_output("Ti"),
        Some(crate::value::Value::Time(crate::value::Duration::from_millis(
            4_194_792_263
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

/// The transfer fixture of `control_studio_transfer.rs` with a reset time `Ti : TIME := T#2s`.
fn time_guard_fixture() -> (ControlState, TestHarness) {
    let source = "PROGRAM Main VAR Kp : LREAL := 2.0; Ti : TIME := T#2s; Manual : BOOL := TRUE; ManualCommand : LREAL := 2.0; SP AT %ML0 : LREAL := 10.0; PV : LREAL := 10.0; MV AT %QL0 : LREAL; END_VAR IF Manual THEN MV := ManualCommand; ELSE MV := Kp * (SP-PV); END_IF; END_PROGRAM";
    let mut state = simulated_control_state(source);
    let mut harness = simulated_harness(source);
    state.debug = studio_debug(&mut harness);
    assert!(harness.cycle().errors.is_empty());
    (state, harness)
}

/// The ordinary transfer binding with `Main.Ti` as a TIME member of `role` in `unit`.
fn time_member_binding(role: &str, unit: &str) -> serde_json::Value {
    let mut binding = transfer_binding();
    binding[role].as_array_mut().unwrap().push(
        json!({"symbol":"Main.Ti","iec_type":"TIME","unit":unit,"minimum":0.001,"maximum":3600}),
    );
    binding
}

fn time_guard_status(state: &ControlState, binding: &serde_json::Value) -> ControlResponse {
    handle_request_value(
        json!({"id":8620,"type":"control.transfer.status","params":{"binding":binding}}),
        state,
        None,
    )
}

/// The Auto entry of the ordinary transfer, which expects the values `before` read, `Main.Ti` among them.
fn time_guard_apply(state: &ControlState, before: &serde_json::Value, transaction_id: &str) -> ControlResponse {
    handle_request_value(
        json!({"id":8621,"type":"control.transfer.apply","params":{
        "transaction_id":transaction_id,"binding":time_member_binding("guards","s"),
        "expected_program_revision":before["program_revision"],"expected_binding_revision":before["binding_revision"],
        "expected_values":before["values"],"values":{"Main.SP":11.0,"Main.ManualCommand":2.0},
        "manual":false,"maximum_output_change":0.000001,"source":"simulation","physical_approved":false}}),
        state,
        None,
    )
}

/// A transfer guards a reset time that the law keeps as `TIME` (E6.2): `T#2s` reads as 2.0 seconds, and the Auto entry
/// that expects those seconds commits and leaves the guard as it was. Catches a TIME guard refused as an unsupported
/// type, a TIME read in milliseconds or nanoseconds, and seconds that fail the exact comparison with the status's.
#[test]
fn a_transfer_guards_a_time_in_seconds() {
    let (state, mut harness) = time_guard_fixture();
    let binding = time_member_binding("guards", "s");
    let before = time_guard_status(&state, &binding);
    assert!(before.ok, "{:?}", before.error);
    let before = before.result.unwrap();
    assert_eq!(before["values"]["Main.Ti"], 2.0);
    let queued = time_guard_apply(&state, &before, "time-guard-1");
    assert!(queued.ok, "{:?}", queued.error);
    assert!(harness.cycle().errors.is_empty());
    let after = time_guard_status(&state, &binding).result.unwrap();
    assert_eq!(after["status"], "applied", "{after}");
    assert_eq!(after["values"]["Main.Manual"], false);
    assert_eq!(after["values"]["Main.Ti"], 2.0);
}

/// A reset time changed by one millisecond after the review, `T#2s1ms` (2.001 s), rejects the transfer, and the loop
/// stays in Manual. A transfer takes a TIME only as a guard in seconds: a TIME state, which the transfer would write, and
/// a TIME guard in `ms` are refused. Catches a guard compared more coarsely than TIME holds it, a TIME that a transfer
/// writes, and a unit that would make the numbers mean something else.
#[test]
fn a_transfer_rejects_a_changed_time_guard_and_takes_a_time_only_as_a_guard_in_seconds() {
    let (state, mut harness) = time_guard_fixture();
    let binding = time_member_binding("guards", "s");
    let before = time_guard_status(&state, &binding);
    assert!(before.ok, "{:?}", before.error);
    let before = before.result.unwrap();
    let queued = time_guard_apply(&state, &before, "time-guard-2");
    assert!(queued.ok, "{:?}", queued.error);
    state
        .debug
        .try_enqueue_global_write(
            "Main.Ti",
            Value::Time(crate::value::Duration::from_millis(2_001)),
        )
        .expect("queue the reset time write");
    assert!(harness.cycle().errors.is_empty());
    let after = time_guard_status(&state, &binding).result.unwrap();
    assert_eq!(after["status"], "rejected", "{after}");
    assert_eq!(after["values"]["Main.Manual"], true);
    assert_eq!(after["values"]["Main.Ti"], 2.001);
    for (role, unit) in [("states", "s"), ("guards", "ms")] {
        let refused = time_guard_status(&state, &time_member_binding(role, unit));
        assert!(!refused.ok, "a TIME among the {role} in {unit} is refused");
        let reason = format!("{:?}", refused.error);
        assert!(
            reason.contains("invalid transfer member type, unit, or bounds"),
            "{reason}"
        );
    }
}
