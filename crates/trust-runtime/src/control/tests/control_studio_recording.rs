#[test]
fn control_recording_acquires_every_native_runtime_scan_without_editor_polling() {
    let source = r#"
PROGRAM Main
VAR Output AT %QD0 : DINT := 0; END_VAR
Output := Output + 1;
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile custom ST");
    let debug = harness.runtime_mut().enable_debug();
    harness.cycle();
    let mut state = hmi_test_state(source);
    state.debug = debug;
    let started = handle_request_value(json!({"id": 8400, "type": "control.recording.start", "params": {
        "recording_id": "native-custom-loop", "loop_revision": "loop-r1",
        "bindings": [{"id": "output", "address": "%QD0", "iec_type": "DINT"}],
        "maximum_samples": 100, "maximum_duration_ms": 10000
    }}), &state, None);
    assert!(started.ok, "runtime scan recording must start: {:?}", started.error);
    for _ in 0..5 { harness.advance_time(crate::value::Duration::from_millis(10)); harness.cycle(); }
    let stopped = handle_request_value(json!({"id": 8401, "type": "control.recording.stop", "params": {
        "recording_id": "native-custom-loop", "loop_revision": "loop-r1"
    }}), &state, None);
    assert!(stopped.ok, "recording must stop: {:?}", stopped.error);
    let result = stopped.result.expect("recorded samples");
    assert_eq!(result["acquisition"]["method"], "runtime_scan_buffer");
    assert_eq!(result["acquisition"]["completeness"], "complete");
    let samples = result["samples"].as_array().expect("sample array");
    assert_eq!(samples.len(), 5, "no editor polling is needed to retain scans");
    for (index, sample) in samples.iter().enumerate() {
        assert_eq!(sample["values"][0], json!((index + 2) as f64));
        assert_eq!(sample["forced"][0], false);
        assert!(sample["captured_at_ms"].as_u64().is_some_and(|time| time > 0), "runtime capture time must be retained");
        if index > 0 {
            assert_eq!(sample["scan"].as_u64(), samples[index - 1]["scan"].as_u64().map(|scan| scan + 1));
            assert!(sample["runtime_time_ns"].as_i64() > samples[index - 1]["runtime_time_ns"].as_i64());
        }
    }
}

fn recording_fixture() -> (TestHarness, ControlState) {
    let source = "PROGRAM Main\nVAR Output AT %QD0 : DINT := 0; END_VAR\nOutput := Output + 1;\nEND_PROGRAM";
    let mut harness = TestHarness::from_source(source).expect("compile recording fixture");
    let debug = harness.runtime_mut().enable_debug();
    harness.cycle();
    let mut state = hmi_test_state(source);
    state.debug = debug;
    (harness, state)
}
fn start_recording(state: &ControlState, maximum_samples: usize) -> ControlResponse {
    handle_request_value(json!({"id": 8410, "type": "control.recording.start", "params": {
        "recording_id": "bounded", "loop_revision": "r1", "maximum_samples": maximum_samples,
        "maximum_duration_ms": 10000, "bindings": [{"id": "output", "address": "%QD0", "iec_type": "DINT"}]
    }}), state, None)
}
fn stop_recording(state: &ControlState, revision: &str) -> ControlResponse {
    handle_request_value(json!({"id": 8411, "type": "control.recording.stop", "params": {
        "recording_id": "bounded", "loop_revision": revision
    }}), state, None)
}
fn recording_cycle(harness: &mut TestHarness) {
    harness.advance_time(crate::value::Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "native cycle failed: {:?}", cycle.errors);
}

#[test]
fn control_recording_stop_is_idempotent_and_rejects_cross_revision_requests() {
    let (mut harness, state) = recording_fixture();
    assert!(start_recording(&state, 100).ok);
    assert!(!start_recording(&state, 100).ok, "a duplicate start must not replace the buffer");
    recording_cycle(&mut harness);
    assert!(!stop_recording(&state, "different-loop").ok);
    recording_cycle(&mut harness);
    let first = stop_recording(&state, "r1");
    let second = stop_recording(&state, "r1");
    assert!(first.ok && second.ok);
    assert_eq!(first.result, second.result, "a lost stop reply must be recoverable");
    assert_eq!(first.result.unwrap()["samples"].as_array().unwrap().len(), 2);
    let released = handle_request_value(json!({"id": 8412, "type": "control.recording.release", "params": {
        "recording_id": "bounded", "loop_revision": "r1"
    }}), &state, None);
    assert!(released.ok);
    assert!(start_recording(&state, 100).ok);
}

#[test]
fn control_recording_reports_capacity_and_preserves_actual_force_flags() {
    let (mut harness, state) = recording_fixture();
    assert!(!start_recording(&state, 1).ok, "invalid limits must not allocate a recording");
    assert!(start_recording(&state, 2).ok);
    state.debug.force_io(crate::io::IoAddress::parse("%QD0").unwrap(), Value::DWord(77));
    recording_cycle(&mut harness);
    recording_cycle(&mut harness);
    recording_cycle(&mut harness);
    let response = stop_recording(&state, "r1");
    assert!(response.ok);
    let result = response.result.unwrap();
    assert_eq!(result["stop_reason"], "sample_limit");
    assert_eq!(result["samples"].as_array().unwrap().len(), 2);
    assert_eq!(result["samples"][0]["forced"][0], true);
    assert_eq!(result["samples"][0]["values"][0], json!(77.0));
    assert!(state.debug.drain_var_writes().is_empty(), "recording must not write program variables");
    assert!(state.debug.drain_io_writes().is_empty(), "recording must not excite process I/O");
}

#[test]
fn control_recording_restart_preserves_partial_evidence_with_an_explicit_reason() {
    let (mut harness, state) = recording_fixture();
    assert!(start_recording(&state, 100).ok);
    recording_cycle(&mut harness);
    recording_cycle(&mut harness);
    harness.restart(crate::RestartMode::Cold).expect("restart actual runtime");
    let response = stop_recording(&state, "r1");
    assert!(response.ok);
    let result = response.result.unwrap();
    assert_eq!(result["acquisition"]["completeness"], "incomplete");
    assert_eq!(result["stop_reason"], "runtime_boundary");
    assert_eq!(result["samples"].as_array().unwrap().len(), 2);
}
