#[test]
fn symbolic_inspection_is_pending_until_a_coherent_actual_scan() {
    let source="PROGRAM Main VAR PV : LREAL; MV : LREAL; Gate : BOOL := TRUE; END_VAR PV := PV + 1.0; MV := 2.0 * PV; END_PROGRAM";
    let mut state = control_test_state(source);
    let mut harness = TestHarness::from_source(source).unwrap();
    state.debug = studio_debug(&mut harness);
    harness.cycle();
    let bindings = json!([{"id":"pv","symbol":"Main.PV","iec_type":"LREAL"},{"id":"mv","symbol":"Main.MV","iec_type":"LREAL"},{"id":"gate","symbol":"Main.Gate","iec_type":"BOOL"}]);
    let first = handle_request_value(
        json!({"id":8700,"type":"control.signals.inspect","params":{"loop_revision":"loop-a","bindings":bindings}}),
        &state,
        None,
    );
    assert!(
        first.ok,
        "symbolic inspection must be supported: {:?}",
        first.error
    );
    assert_eq!(first.result.as_ref().unwrap()["available"], false);
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    let second = handle_request_value(
        json!({"id":8701,"type":"control.signals.inspect","params":{"loop_revision":"loop-a","bindings":bindings}}),
        &state,
        None,
    );
    let second = second.result.unwrap();
    assert_eq!(second["available"], true);
    assert_eq!(second["bindings"], bindings);
    assert_eq!(second["values"], json!([2.0, 4.0, 1.0]));
    assert_eq!(second["scan"], 1);
    assert_eq!(second["quality"], json!(["good", "good", "good"]));
}

#[test]
fn control_recording_empty_result_does_not_claim_actual_cycle_timing() {
    let (_harness, state) = recording_fixture();
    let start = handle_request_value(
        json!({"id":8705,"type":"control.recording.start","params":{"recording_id":"empty-timing","loop_revision":"empty-r1","bindings":[{"id":"output","address":"%QD0","iec_type":"DINT"}],"maximum_samples":2,"maximum_duration_ms":10000}}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let stop = handle_request_value(
        json!({"id":8706,"type":"control.recording.stop","params":{"recording_id":"empty-timing","loop_revision":"empty-r1"}}),
        &state,
        None,
    );
    let result = stop.result.unwrap();
    assert_eq!(result["samples"], json!([]));
    assert_eq!(
        result["acquisition"]["actualCycleTimingAvailable"], false,
        "no timing measurements exist before a scan is captured"
    );
}

const SYMBOLIC_SOURCE:&str="PROGRAM Main VAR PV : LREAL; MV : LREAL; Gate : BOOL := TRUE; END_VAR PV := PV + 1.0; MV := 2.0 * PV; END_PROGRAM";
fn symbolic_fixture() -> (TestHarness, ControlState, serde_json::Value, String) {
    let mut state = control_test_state(SYMBOLIC_SOURCE);
    let mut harness = TestHarness::from_source(SYMBOLIC_SOURCE).unwrap();
    state.debug = studio_debug(&mut harness);
    harness.cycle();
    let bindings = json!([{"id":"pv","symbol":"Main.PV","iec_type":"LREAL"},{"id":"mv","symbol":"Main.MV","iec_type":"LREAL"},{"id":"gate","symbol":"Main.Gate","iec_type":"BOOL"}]);
    let first = handle_request_value(
        json!({"id":8710,"type":"control.signals.inspect","params":{"loop_revision":"symbol-loop","bindings":bindings}}),
        &state,
        None,
    );
    assert!(first.ok, "{:?}", first.error);
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    let second = handle_request_value(
        json!({"id":8711,"type":"control.signals.inspect","params":{"loop_revision":"symbol-loop","bindings":bindings}}),
        &state,
        None,
    );
    let result = second.result.unwrap();
    assert_eq!(result["available"], true);
    (
        harness,
        state,
        bindings,
        result["program_revision"].as_str().unwrap().into(),
    )
}
fn start_symbolic_recording(
    state: &ControlState,
    bindings: serde_json::Value,
    revision: String,
) -> ControlResponse {
    handle_request_value(
        json!({"id":8712,"type":"control.recording.start","params":{"recording_id":"symbol-record","loop_revision":"symbol-loop","program_revision":revision,"bindings":bindings,"maximum_samples":10,"maximum_duration_ms":10000}}),
        state,
        None,
    )
}
#[test]
fn symbolic_recording_retains_scan_values_and_channel_quality_separately() {
    let (mut harness, state, bindings, revision) = symbolic_fixture();
    let start = start_symbolic_recording(&state, bindings.clone(), revision.clone());
    assert!(start.ok, "symbolic recording must start: {:?}", start.error);
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    state
        .debug
        .try_force_global("Main.MV", Value::LReal(123.0))
        .expect("force the output variable");
    harness.advance_time(crate::value::Duration::from_millis(20));
    assert!(harness.cycle().errors.is_empty());
    let stop = handle_request_value(
        json!({"id":8713,"type":"control.recording.stop","params":{"recording_id":"symbol-record","loop_revision":"symbol-loop"}}),
        &state,
        None,
    );
    let result = stop.result.unwrap();
    assert_eq!(result["bindings"], bindings);
    assert_eq!(result["program_revision"], revision);
    assert_eq!(result["samples"][0]["values"], json!([3.0, 6.0, 1.0]));
    assert_eq!(
        result["samples"][0]["quality"],
        json!(["good", "good", "good"])
    );
    assert_eq!(result["samples"][1]["quality"][1], "forced");
    assert_eq!(
        result["acquisition"]["completeness"], "complete",
        "scan coverage and channel usability are different claims"
    );
}
#[test]
fn symbolic_recording_bad_type_is_not_reused_or_marked_good() {
    let (mut harness, state, bindings, revision) = symbolic_fixture();
    assert!(start_symbolic_recording(&state, bindings, revision).ok);
    // A compiled program's memory holds every variable in its declared type (specification 44 §5.8), so a value that is
    // no number of its channel reaches the recorder only as a value its type holds but no measurement is, such as a
    // non-finite LREAL in a corrupt scan: the recorder marks it bad, never good or reused.
    studio_set(&mut harness, "Main.PV", Value::LReal(f64::NAN));
    // The cycle stamps each published scan with its number; this one follows the last real scan.
    let last_scan = state
        .debug
        .latest_io_snapshot()
        .and_then(|snapshot| snapshot.scan)
        .expect("the fixture published scans");
    let io = crate::io::IoSnapshot {
        scan: Some(last_scan + 1),
        ..crate::io::IoSnapshot::default()
    };
    harness.runtime().with_program_view(|program| {
        state
            .debug
            .push_control_snapshot_at(io, 20_000_000, program, &[], true, true);
    });
    let stop = handle_request_value(
        json!({"id":8714,"type":"control.recording.stop","params":{"recording_id":"symbol-record","loop_revision":"symbol-loop"}}),
        &state,
        None,
    );
    let result = stop.result.unwrap();
    assert_eq!(result["samples"][0]["quality"][0], "bad");
    assert!(result["samples"][0]["values"][0].is_null());
}
#[test]
fn symbolic_recording_and_inspection_invalidate_on_identical_reload() {
    let (mut harness, state, bindings, revision) = symbolic_fixture();
    assert!(start_symbolic_recording(&state, bindings.clone(), revision.clone()).ok);
    harness.advance_time(crate::value::Duration::from_millis(20));
    harness.cycle();
    harness.reload_source(SYMBOLIC_SOURCE).unwrap();
    let stale = handle_request_value(
        json!({"id":8715,"type":"control.signals.inspect","params":{"loop_revision":"symbol-loop","bindings":bindings,"expected_program_revision":revision}}),
        &state,
        None,
    );
    assert!(!stale.ok);
    let stop = handle_request_value(
        json!({"id":8716,"type":"control.recording.stop","params":{"recording_id":"symbol-record","loop_revision":"symbol-loop"}}),
        &state,
        None,
    );
    let result = stop.result.unwrap();
    assert_eq!(result["stop_reason"], "runtime_boundary");
    assert_eq!(result["acquisition"]["completeness"], "incomplete");
}

#[test]
fn symbolic_inspection_replaces_binding_identity_without_reusing_old_values() {
    let (mut harness, state, _bindings, revision) = symbolic_fixture();
    let bindings = json!([{"id":"pv","symbol":"Main.MV","iec_type":"LREAL"}]);
    let pending = handle_request_value(
        json!({"id":8720,"type":"control.signals.inspect","params":{"loop_revision":"symbol-loop","bindings":bindings,"expected_program_revision":revision}}),
        &state,
        None,
    );
    assert_eq!(pending.result.unwrap()["available"], false);
    harness.advance_time(crate::value::Duration::from_millis(20));
    harness.cycle();
    let ready = handle_request_value(
        json!({"id":8721,"type":"control.signals.inspect","params":{"loop_revision":"symbol-loop","bindings":bindings}}),
        &state,
        None,
    );
    let result = ready.result.unwrap();
    assert_eq!(result["bindings"], bindings);
    assert_eq!(result["values"], json!([6.0]));
    let bad = handle_request_value(
        json!({"id":8722,"type":"control.signals.inspect","params":{"loop_revision":"symbol-loop","bindings":[{"id":"bad","address":"%QL0","symbol":"Main.MV","iec_type":"LREAL"}]}}),
        &state,
        None,
    );
    assert!(!bad.ok);
}
#[test]
fn symbolic_recording_bytecode_reload_invalidates_program_guard() {
    let (mut harness, state, bindings, revision) = symbolic_fixture();
    assert!(start_symbolic_recording(&state, bindings.clone(), revision.clone()).ok);
    // The image a client sends with `bytecode.reload` changes the running program online (specification 44 §12).
    let bytes = program_image(SYMBOLIC_SOURCE);
    harness
        .runtime_mut()
        .apply_online_change_bytes(&bytes)
        .unwrap();
    let stale = handle_request_value(
        json!({"id":8723,"type":"control.signals.inspect","params":{"loop_revision":"symbol-loop","bindings":bindings,"expected_program_revision":revision}}),
        &state,
        None,
    );
    assert!(!stale.ok);
    let result=handle_request_value(json!({"id":8724,"type":"control.recording.stop","params":{"recording_id":"symbol-record","loop_revision":"symbol-loop"}}),&state,None).result.unwrap();
    assert_eq!(result["stop_reason"], "runtime_boundary");
}

#[test]
fn symbolic_integer_process_signals_are_inspected_and_recorded_without_wrappers() {
    let source="PROGRAM Main VAR Count : DINT := 41; UnsignedCount : UDINT := 10; END_VAR Count := Count + 1; UnsignedCount := UnsignedCount + 1; END_PROGRAM";
    let mut state = control_test_state(source);
    let mut harness = TestHarness::from_source(source).unwrap();
    state.debug = studio_debug(&mut harness);
    harness.cycle();
    let bindings = json!([{"id":"count","symbol":"Main.Count","iec_type":"DINT"},{"id":"unsigned","symbol":"Main.UnsignedCount","iec_type":"UDINT"}]);
    let first = handle_request_value(
        json!({"id":8730,"type":"control.signals.inspect","params":{"loop_revision":"integer-loop","bindings":bindings}}),
        &state,
        None,
    );
    assert!(
        first.ok,
        "numeric reading must not inherit floating-point gain-write restriction: {:?}",
        first.error
    );
    harness.advance_time(crate::value::Duration::from_millis(20));
    harness.cycle();
    let inspected=handle_request_value(json!({"id":8731,"type":"control.signals.inspect","params":{"loop_revision":"integer-loop","bindings":bindings}}),&state,None).result.unwrap();
    assert_eq!(inspected["values"], json!([43.0, 12.0]));
    let start = handle_request_value(
        json!({"id":8732,"type":"control.recording.start","params":{"recording_id":"integer-record","loop_revision":"integer-loop","program_revision":inspected["program_revision"],"bindings":bindings,"maximum_samples":2,"maximum_duration_ms":10000}}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    for _ in 0..2 {
        harness.advance_time(crate::value::Duration::from_millis(20));
        harness.cycle();
    }
    let recorded=handle_request_value(json!({"id":8733,"type":"control.recording.stop","params":{"recording_id":"integer-record","loop_revision":"integer-loop"}}),&state,None).result.unwrap();
    assert_eq!(recorded["samples"][0]["values"], json!([44.0, 13.0]));
    assert_eq!(recorded["samples"][1]["values"], json!([45.0, 14.0]));
    assert_eq!(recorded["acquisition"]["completeness"], "complete");
}
