// A TIME parameter override of an isolated run (protocol annex E8.4): its value is seconds, written as the whole
// nanoseconds of the decimal the client wrote and read back in seconds, as a parameter transaction takes it (E7.3 step 10).

/// An isolated run's program with a reset time `Ti : TIME` and `Exact`, TRUE exactly while `Ti` is
/// `T#48d13h13m12s263ms` (4194792.263 s), over the search fixture's first-order plant.
const TIME_OVERRIDE_PROGRAM: &str = "PROGRAM Main VAR SP : LREAL := 1.0; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; Ti : TIME := T#2s; Exact : BOOL; END_VAR Exact := Ti = T#48d13h13m12s263ms; MV := SP - PV; END_PROGRAM";

/// A test run of ten scans that observes `Main.Exact` and overrides `Main.Ti` with `value` in `unit` (E8.4).
fn time_override_request(state: &ControlState, unit: &str, value: f64) -> serde_json::Value {
    let revision = handle_request_value(
        json!({"id":8630,"type":"control.search.status","params":{}}),
        state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = isolated_run_request(revision);
    request["step_ms"] = json!(20);
    request["duration_ms"] = json!(200);
    request["max_total_cycles"] = json!(11);
    request["observations"] = json!([{"symbol":"Main.Exact","unit":"1"}]);
    request["parameter_overrides"] = json!([{"symbol":"Main.Ti","iec_type":"TIME","unit":unit,"convention":"standard Ti","minimum":0.001,"maximum":4294967.295,"value":value}]);
    request
}

/// An isolated run writes a TIME override as the nanoseconds of the decimal the client wrote and holds it through every
/// scan (E8.4): 4194792.263 s makes `Ti = T#48d13h13m12s263ms` TRUE, the run completes, and it records the override in
/// seconds. Catches a TIME override refused as an unsupported type, one written in milliseconds or a nanosecond off, and
/// a read-back that differs from the value, which fails the run at the first task boundary.
#[test]
fn an_isolated_run_writes_a_time_override_as_its_exact_nanoseconds() {
    let (state, _root) = search_fixture_with(TIME_OVERRIDE_PROGRAM);
    let result = isolated_run_to_end(&state, time_override_request(&state, "s", 4_194_792.263));
    assert_eq!(result["status"], "completed", "{result}");
    assert_eq!(result["run"]["parameters"]["Main.Ti"], 4_194_792.263);
    assert_eq!(last_channel_value(&result["run"], "Main.Exact"), 1.0);
}

/// A TIME override is seconds of whole nanoseconds (E8.4): an override in `ms` and 1.0000000001 s are refused before the
/// run starts. Catches a unit that would make the numbers mean something else, and a value rounded silently to a
/// nanosecond the client did not send.
#[test]
fn an_isolated_run_refuses_a_time_override_in_ms_or_off_a_whole_nanosecond() {
    let (state, _root) = search_fixture_with(TIME_OVERRIDE_PROGRAM);
    for (unit, value) in [("ms", 2_000.0), ("s", 1.000_000_000_1)] {
        let start = handle_request_value(
            json!({"id":8631,"type":"control.simulation.run.start","params":time_override_request(&state, unit, value)}),
            &state,
            None,
        );
        assert!(!start.ok, "a TIME override of {value} {unit} is refused");
        let reason = format!("{:?}", start.error);
        assert!(
            reason.contains("invalid typed parameter override or bounds"),
            "{reason}"
        );
    }
}
