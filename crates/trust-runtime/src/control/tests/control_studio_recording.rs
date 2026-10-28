#[test]
fn control_recording_acquires_every_native_runtime_scan_without_editor_polling() {
    let source = r#"
PROGRAM Main
VAR Output AT %QD0 : DINT := 0; END_VAR
Output := Output + 1;
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile custom ST");
    let debug = studio_debug(&mut harness);
    harness.cycle();
    let mut state = control_test_state(source);
    state.debug = debug;
    let started = handle_request_value(
        json!({"id": 8400, "type": "control.recording.start", "params": {
            "recording_id": "native-custom-loop", "loop_revision": "loop-r1",
            "bindings": [{"id": "output", "address": "%QD0", "iec_type": "DINT"}],
            "maximum_samples": 100, "maximum_duration_ms": 10000
        }}),
        &state,
        None,
    );
    assert!(
        started.ok,
        "runtime scan recording must start: {:?}",
        started.error
    );
    for _ in 0..5 {
        harness.advance_time(crate::value::Duration::from_millis(10));
        harness.cycle();
    }
    let stopped = handle_request_value(
        json!({"id": 8401, "type": "control.recording.stop", "params": {
            "recording_id": "native-custom-loop", "loop_revision": "loop-r1"
        }}),
        &state,
        None,
    );
    assert!(stopped.ok, "recording must stop: {:?}", stopped.error);
    let result = stopped.result.expect("recorded samples");
    assert_eq!(result["acquisition"]["method"], "runtime_scan_buffer");
    assert_eq!(result["acquisition"]["completeness"], "complete");
    let samples = result["samples"].as_array().expect("sample array");
    assert_eq!(
        samples.len(),
        5,
        "no editor polling is needed to retain scans"
    );
    for (index, sample) in samples.iter().enumerate() {
        assert_eq!(sample["values"][0], json!((index + 2) as f64));
        assert_eq!(sample["forced"][0], false);
        assert!(
            sample["captured_at_ms"]
                .as_u64()
                .is_some_and(|time| time > 0),
            "runtime capture time must be retained"
        );
        if index > 0 {
            assert_eq!(
                sample["scan"].as_u64(),
                samples[index - 1]["scan"].as_u64().map(|scan| scan + 1)
            );
            assert!(
                sample["runtime_time_ns"].as_i64() > samples[index - 1]["runtime_time_ns"].as_i64()
            );
        }
    }
}

/// A loop's task runs every fifth 10 ms cycle: its recording holds one sample per execution of the
/// task, counted 1, 2, 3, so a long identification fits the buffer whatever the resource's cycle,
/// and an interval the task missed leaves a gap (specification 35, §17.7).
#[test]
fn control_recording_of_a_task_samples_its_executions_and_counts_them() {
    let source = r#"
CONFIGURATION Cell
RESOURCE Cpu ON PLC
TASK Loop (INTERVAL := T#50ms, PRIORITY := 1);
PROGRAM Main WITH Loop : Counter;
END_RESOURCE
END_CONFIGURATION
PROGRAM Counter
VAR Output AT %QD0 : DINT := 0; END_VAR
Output := Output + 1;
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile the task fixture");
    let debug = studio_debug(&mut harness);
    harness.cycle();
    let mut state = control_test_state(source);
    state.debug = debug;
    let start = |id: &str| {
        handle_request_value(
            json!({"id": 8410, "type": "control.recording.start", "params": {
                "recording_id": id, "loop_revision": "loop-r1",
                "bindings": [{"id": "output", "address": "%QD0", "iec_type": "DINT"}],
                "maximum_samples": 100, "maximum_duration_ms": 10000, "task": "LOOP"
            }}),
            &state,
            None,
        )
    };
    let stop = |id: &str| {
        handle_request_value(
            json!({"id": 8411, "type": "control.recording.stop", "params": {
                "recording_id": id, "loop_revision": "loop-r1"
            }}),
            &state,
            None,
        )
    };
    let unknown = handle_request_value(
        json!({"id": 8409, "type": "control.recording.start", "params": {
            "recording_id": "no-task", "loop_revision": "loop-r1",
            "bindings": [{"id": "output", "address": "%QD0", "iec_type": "DINT"}],
            "maximum_samples": 100, "maximum_duration_ms": 10000, "task": "Fast"
        }}),
        &state,
        None,
    );
    assert!(
        unknown
            .error
            .as_deref()
            .is_some_and(|error| error.contains("runs no task named Fast")),
        "a task the program does not run is refused: {:?}",
        unknown.error
    );
    let started = start("task-loop");
    assert!(started.ok, "a task recording starts: {:?}", started.error);
    for _ in 0..20 {
        harness.advance_time(crate::value::Duration::from_millis(10));
        harness.cycle();
    }
    let stopped = stop("task-loop");
    assert!(stopped.ok, "{:?}", stopped.error);
    let result = stopped.result.expect("recorded samples");
    assert_eq!(result["acquisition"]["completeness"], "complete", "{result}");
    assert!(
        result["acquisition"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("Every execution of LOOP")),
        "{result}"
    );
    let samples = result["samples"].as_array().expect("sample array");
    assert_eq!(samples.len(), 4, "one sample per execution of the 50 ms task in 200 ms: {result}");
    for (index, sample) in samples.iter().enumerate() {
        assert_eq!(sample["scan"], json!(index as u64 + 1), "{result}");
        if index > 0 {
            assert_eq!(
                sample["values"][0].as_f64(),
                samples[index - 1]["values"][0].as_f64().map(|value| value + 1.0),
                "each sample is the next execution of the task: {result}"
            );
        }
    }
    // An interval the task missed is a gap: the recording is incomplete and names it.
    handle_request_value(
        json!({"id": 8412, "type": "control.recording.release", "params": {
            "recording_id": "task-loop", "loop_revision": "loop-r1"
        }}),
        &state,
        None,
    );
    assert!(start("task-gap").ok);
    for step in [50, 120, 50] {
        harness.advance_time(crate::value::Duration::from_millis(step));
        harness.cycle();
    }
    let gap = stop("task-gap").result.expect("recorded samples");
    assert_eq!(gap["acquisition"]["completeness"], "incomplete", "{gap}");
    assert_eq!(
        gap["acquisition"]["missingScanRanges"],
        json!([{"first": 2, "last": 2}]),
        "{gap}"
    );
    let sequence: Vec<_> = gap["samples"]
        .as_array()
        .expect("samples")
        .iter()
        .map(|sample| sample["scan"].as_u64())
        .collect();
    assert_eq!(sequence, vec![Some(1), Some(3), Some(4)], "{gap}");
}

fn recording_fixture() -> (TestHarness, ControlState) {
    let source =
        "PROGRAM Main\nVAR Output AT %QD0 : DINT := 0; END_VAR\nOutput := Output + 1;\nEND_PROGRAM";
    let mut harness = TestHarness::from_source(source).expect("compile recording fixture");
    let debug = studio_debug(&mut harness);
    harness.cycle();
    let mut state = control_test_state(source);
    state.debug = debug;
    (harness, state)
}
fn start_recording(state: &ControlState, maximum_samples: usize) -> ControlResponse {
    handle_request_value(
        json!({"id": 8410, "type": "control.recording.start", "params": {
            "recording_id": "bounded", "loop_revision": "r1", "maximum_samples": maximum_samples,
            "maximum_duration_ms": 10000, "bindings": [{"id": "output", "address": "%QD0", "iec_type": "DINT"}]
        }}),
        state,
        None,
    )
}
fn stop_recording(state: &ControlState, revision: &str) -> ControlResponse {
    handle_request_value(
        json!({"id": 8411, "type": "control.recording.stop", "params": {
            "recording_id": "bounded", "loop_revision": revision
        }}),
        state,
        None,
    )
}
fn recording_cycle(harness: &mut TestHarness) {
    harness.advance_time(crate::value::Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "native cycle failed: {:?}",
        cycle.errors
    );
}

#[test]
fn control_recording_stop_is_idempotent_and_rejects_cross_revision_requests() {
    let (mut harness, state) = recording_fixture();
    assert!(start_recording(&state, 100).ok);
    assert!(
        !start_recording(&state, 100).ok,
        "a duplicate start must not replace the buffer"
    );
    recording_cycle(&mut harness);
    assert!(!stop_recording(&state, "different-loop").ok);
    recording_cycle(&mut harness);
    let first = stop_recording(&state, "r1");
    let second = stop_recording(&state, "r1");
    assert!(first.ok && second.ok);
    assert_eq!(
        first.result, second.result,
        "a lost stop reply must be recoverable"
    );
    assert_eq!(
        first.result.unwrap()["samples"].as_array().unwrap().len(),
        2
    );
    let released = handle_request_value(
        json!({"id": 8412, "type": "control.recording.release", "params": {
            "recording_id": "bounded", "loop_revision": "r1"
        }}),
        &state,
        None,
    );
    assert!(released.ok);
    assert!(start_recording(&state, 100).ok);
}

#[test]
fn control_recording_reports_capacity_and_preserves_actual_force_flags() {
    let (mut harness, state) = recording_fixture();
    assert!(
        !start_recording(&state, 1).ok,
        "invalid limits must not allocate a recording"
    );
    assert!(start_recording(&state, 2).ok);
    state
        .debug
        .try_force_io(crate::io::IoAddress::parse("%QD0").unwrap(), Value::DWord(77))
        .expect("force the output");
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
    assert!(
        state.debug.drain_var_writes().is_empty(),
        "recording must not write program variables"
    );
    assert!(
        state.debug.drain_io_writes().is_empty(),
        "recording must not excite process I/O"
    );
}

#[test]
fn control_recording_restart_preserves_partial_evidence_with_an_explicit_reason() {
    let (mut harness, state) = recording_fixture();
    assert!(start_recording(&state, 100).ok);
    recording_cycle(&mut harness);
    recording_cycle(&mut harness);
    harness
        .restart(crate::RestartMode::Cold)
        .expect("restart actual runtime");
    let response = stop_recording(&state, "r1");
    assert!(response.ok);
    let result = response.result.unwrap();
    assert_eq!(result["acquisition"]["completeness"], "incomplete");
    assert_eq!(result["stop_reason"], "runtime_boundary");
    assert_eq!(result["samples"].as_array().unwrap().len(), 2);
}

/// Starts recording `id` on DINT output `%QD0`, with the request's fields replacing the defaults.
fn start_with(state: &ControlState, id: &str, extra: serde_json::Value) -> ControlResponse {
    let mut params = json!({
        "recording_id": id, "loop_revision": "r1", "maximum_samples": 100,
        "maximum_duration_ms": 10000, "bindings": [{"id": "output", "address": "%QD0", "iec_type": "DINT"}]
    });
    for (key, value) in extra.as_object().into_iter().flatten() {
        params[key.as_str()] = value.clone();
    }
    handle_request_value(
        json!({"id": 8500, "type": "control.recording.start", "params": params}),
        state,
        None,
    )
}
fn stop_with(state: &ControlState, id: &str) -> ControlResponse {
    handle_request_value(
        json!({"id": 8501, "type": "control.recording.stop", "params": {"recording_id": id, "loop_revision": "r1"}}),
        state,
        None,
    )
}
fn status_from(state: &ControlState, id: &str, from_sample: Option<u64>) -> serde_json::Value {
    let mut params = json!({"recording_id": id, "loop_revision": "r1"});
    if let Some(from) = from_sample {
        params["from_sample"] = json!(from);
    }
    let response = handle_request_value(
        json!({"id": 8502, "type": "control.recording.status", "params": params}),
        state,
        None,
    );
    assert!(response.ok, "{:?}", response.error);
    response.result.expect("status result")
}
/// The task fixture of the task recording tests, with the harness at its first cycle.
fn task_recording_fixture() -> (TestHarness, ControlState) {
    let source = r#"
CONFIGURATION Cell
RESOURCE Cpu ON PLC
TASK Loop (INTERVAL := T#50ms, PRIORITY := 1);
PROGRAM Main WITH Loop : Counter;
END_RESOURCE
END_CONFIGURATION
PROGRAM Counter
VAR Output AT %QD0 : DINT := 0; END_VAR
Output := Output + 1;
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile the task fixture");
    let debug = studio_debug(&mut harness);
    harness.cycle();
    let mut state = control_test_state(source);
    state.debug = debug;
    (harness, state)
}
fn start_task_with(state: &ControlState, id: &str, record_every: u64) -> ControlResponse {
    start_with(
        state,
        id,
        json!({"task": "LOOP", "record_every": record_every}),
    )
}
fn advance_and_cycle(harness: &mut TestHarness, milliseconds: &[i64]) {
    for step in milliseconds {
        harness.advance_time(crate::value::Duration::from_millis(*step));
        harness.cycle();
    }
}

/// Each sample of a record period is the mean of its executions, and it is timed at the mean of
/// their times (specification 35, A4.2).
#[test]
fn record_every_averages_each_period_and_times_it_at_the_mean_of_its_executions() {
    let source = "PROGRAM Main\nVAR Output AT %QD0 : DINT := -1; END_VAR\nOutput := Output + 1;\nEND_PROGRAM";
    let mut harness = TestHarness::from_source(source).expect("compile recording fixture");
    let debug = studio_debug(&mut harness);
    harness.cycle();
    let mut state = control_test_state(source);
    state.debug = debug;
    assert!(start_with(&state, "every", json!({})).ok);
    assert!(start_with(&state, "averaged", json!({"record_every": 3})).ok);
    for _ in 0..6 {
        recording_cycle(&mut harness);
    }
    let every = stop_with(&state, "every").result.expect("every-execution samples");
    let averaged = stop_with(&state, "averaged").result.expect("averaged samples");
    let raw = every["samples"].as_array().expect("every samples");
    assert_eq!(raw.len(), 6, "{every}");
    let samples = averaged["samples"].as_array().expect("averaged samples");
    assert_eq!(samples.len(), 2, "{averaged}");
    assert_eq!(samples[0]["values"][0], json!(2.0), "the mean of 1, 2, 3");
    assert_eq!(samples[1]["values"][0], json!(5.0), "the mean of 4, 5, 6");
    assert_eq!(samples[0]["scan"], raw[2]["scan"], "a sample is stamped with its last execution");
    assert_eq!(samples[1]["scan"], raw[5]["scan"]);
    let time = |index: usize| raw[index]["runtime_time_ns"].as_i64().expect("execution timing");
    assert_eq!(
        samples[0]["runtime_time_ns"].as_i64(),
        Some((time(0) + time(1) + time(2)) / 3)
    );
    assert_eq!(
        samples[1]["runtime_time_ns"].as_i64(),
        Some((time(3) + time(4) + time(5)) / 3)
    );
    assert_eq!(samples[1]["quality"], json!(["good"]));
    assert_eq!(samples[1]["forced"], json!([false]));
    assert!(
        samples[0].get("gap").is_none(),
        "a period without a missed execution keeps the sample shape: {averaged}"
    );
}

/// A BOOL channel's sample is the value of the last execution of its period (A4.2).
#[test]
fn a_bool_channel_takes_the_last_execution_of_each_record_period() {
    let source = "PROGRAM Main\nVAR Flag AT %QX0.0 : BOOL := FALSE; END_VAR\nFlag := NOT Flag;\nEND_PROGRAM";
    let mut harness = TestHarness::from_source(source).expect("compile BOOL fixture");
    let debug = studio_debug(&mut harness);
    harness.cycle();
    let mut state = control_test_state(source);
    state.debug = debug;
    let bindings = json!([{"id": "flag", "address": "%QX0.0", "iec_type": "BOOL"}]);
    assert!(start_with(&state, "flag", json!({"bindings": bindings, "record_every": 3})).ok);
    for _ in 0..6 {
        recording_cycle(&mut harness);
    }
    let result = stop_with(&state, "flag").result.expect("BOOL samples");
    let values: Vec<_> = result["samples"]
        .as_array()
        .expect("samples")
        .iter()
        .map(|sample| sample["values"][0].clone())
        .collect();
    // The executions alternate FALSE, TRUE, FALSE, TRUE, FALSE, TRUE: the periods end FALSE, TRUE.
    assert_eq!(values, vec![json!(0.0), json!(1.0)], "{result}");
}

/// A period's quality is the worst of its executions, and any forced execution forces the sample.
#[test]
fn a_record_period_takes_the_worst_quality_of_its_executions() {
    let (mut harness, state) = recording_fixture();
    harness.cycle();
    let address = crate::io::IoAddress::parse("%QD0").unwrap();
    assert!(start_with(&state, "quality", json!({"record_every": 3})).ok);
    state
        .debug
        .try_force_io(address.clone(), Value::DWord(77))
        .expect("force the output");
    recording_cycle(&mut harness);
    recording_cycle(&mut harness);
    state.debug.release_io(&address);
    recording_cycle(&mut harness);
    for _ in 0..3 {
        recording_cycle(&mut harness);
    }
    let result = stop_with(&state, "quality").result.expect("quality samples");
    let samples = result["samples"].as_array().expect("samples");
    assert_eq!(samples.len(), 2, "{result}");
    assert_eq!(samples[0]["quality"], json!(["forced"]), "{result}");
    assert_eq!(samples[0]["forced"], json!([true]), "{result}");
    assert_eq!(samples[1]["quality"], json!(["good"]), "{result}");
    assert_eq!(samples[1]["forced"], json!([false]), "{result}");
}

/// A missed execution inside a record period marks that sample with a gap and makes the recording
/// incomplete (A4.4). A window of one execution never contains a gap, so `record_every` 1 keeps the
/// sequence and sample shape of the task recording.
#[test]
fn a_missed_task_interval_inside_a_record_period_marks_its_sample_with_a_gap() {
    let (mut harness, state) = task_recording_fixture();
    assert!(start_task_with(&state, "inside", 2).ok);
    advance_and_cycle(&mut harness, &[50, 120, 50]);
    let result = stop_with(&state, "inside").result.expect("gap samples");
    assert_eq!(result["acquisition"]["completeness"], "incomplete", "{result}");
    assert_eq!(
        result["acquisition"]["missingScanRanges"],
        json!([{"first": 2, "last": 2}]),
        "{result}"
    );
    let samples = result["samples"].as_array().expect("samples");
    assert_eq!(samples.len(), 2, "the stopped recording keeps its last, partial period (A4.2): {result}");
    assert_eq!(samples[0]["scan"], json!(3), "{result}");
    assert_eq!(samples[0]["gap"], json!(true), "{result}");
    assert_eq!(samples[1]["scan"], json!(4), "the partial period ends at its last execution: {result}");

    assert!(start_task_with(&state, "single", 1).ok);
    advance_and_cycle(&mut harness, &[50, 120, 50]);
    let single = stop_with(&state, "single").result.expect("single samples");
    let sequence: Vec<_> = single["samples"]
        .as_array()
        .expect("samples")
        .iter()
        .map(|sample| (sample["scan"].clone(), sample.get("gap").is_some()))
        .collect();
    assert_eq!(
        sequence,
        vec![(json!(1), false), (json!(3), false), (json!(4), false)],
        "{single}"
    );
}

/// A missed interval between two record periods belongs to neither: the recording is incomplete,
/// and no sample is marked.
#[test]
fn a_missed_task_interval_between_record_periods_marks_no_sample() {
    let (mut harness, state) = task_recording_fixture();
    assert!(start_task_with(&state, "between", 2).ok);
    advance_and_cycle(&mut harness, &[50, 50, 120, 50]);
    let result = stop_with(&state, "between").result.expect("samples");
    assert_eq!(result["acquisition"]["completeness"], "incomplete", "{result}");
    assert_eq!(
        result["acquisition"]["missingScanRanges"],
        json!([{"first": 3, "last": 3}]),
        "{result}"
    );
    let samples = result["samples"].as_array().expect("samples");
    assert_eq!(samples.len(), 2, "{result}");
    assert_eq!(samples[0]["scan"], json!(2), "{result}");
    assert_eq!(samples[1]["scan"], json!(5), "{result}");
    assert!(samples.iter().all(|sample| sample.get("gap").is_none()), "{result}");
}

/// The duration limit counts process time from the first captured scan (A3.2, A7): the runtime's
/// timestamps decide it, not the wall clock, so 30 quick cycles still end at process time 1 s.
#[test]
fn the_duration_limit_ends_the_recording_on_process_time_from_the_first_captured_scan() {
    let (mut harness, state) = recording_fixture();
    assert!(start_with(&state, "span", json!({"maximum_duration_ms": 1000})).ok);
    for _ in 0..30 {
        harness.advance_time(crate::value::Duration::from_millis(100));
        harness.cycle();
    }
    let result = stop_with(&state, "span").result.expect("duration samples");
    assert_eq!(result["stop_reason"], "duration_limit", "{result}");
    assert_eq!(result["samples"].as_array().unwrap().len(), 10, "{result}");
    let status = status_from(&state, "span", None);
    assert_eq!(status["state"], "stopped", "{status}");
    assert_eq!(status["elapsed_ms"], json!(900), "{status}");
}

/// Each limit of specification 35 (A7) refuses a request that exceeds it, and the message names it.
#[test]
fn recording_limits_refuse_each_bound_by_name_and_admit_the_limit_itself() {
    let (_harness, state) = recording_fixture();
    let refused = |extra: serde_json::Value| {
        let response = start_with(&state, "limit", extra);
        assert!(!response.ok, "must be refused");
        response.error.unwrap_or_default()
    };
    assert!(refused(json!({"maximum_samples": 100_001})).contains("100000"));
    let thirty_three: Vec<_> = (0..33)
        .map(|index| json!({"id": format!("c{index}"), "address": "%QD0", "iec_type": "DINT"}))
        .collect();
    assert!(refused(json!({"bindings": thirty_three})).contains("32"));
    let seventeen: Vec<_> = (0..17)
        .map(|index| json!({"id": format!("c{index}"), "address": "%QD0", "iec_type": "DINT"}))
        .collect();
    let product = refused(json!({"bindings": seventeen, "maximum_samples": 100_000}));
    assert!(product.contains("1600000"), "{product}");
    let duration = refused(json!({"maximum_duration_ms": 86_400_001}));
    assert!(duration.contains("86400000"), "{duration}");
    assert!(refused(json!({"record_every": 0})).contains("record_every"));
    assert!(refused(json!({"record_every": 100_001})).contains("100000"));

    let limit = json!({"maximum_samples": 100_000, "maximum_duration_ms": 86_400_000});
    assert!(start_with(&state, "limit-24h", limit).ok, "the limits themselves are accepted");
    assert!(start_with(&state, "second", json!({})).ok, "two recordings may be held");
    let third = start_with(&state, "third", json!({}));
    assert!(!third.ok);
    assert!(
        third.error.unwrap_or_default().contains("limit is 2"),
        "the refusal names the two-recording limit"
    );
}

/// `control.recording.status` pages samples without stopping the recording (A4.2 and the request
/// contract): the caller advances by `next_sample`, and the recording keeps capturing.
#[test]
fn recording_status_pages_samples_and_never_stops_the_recording() {
    let (mut harness, state) = recording_fixture();
    assert!(start_with(&state, "paged", json!({"maximum_samples": 5000, "maximum_duration_ms": 60_000})).ok);
    for _ in 0..5 {
        recording_cycle(&mut harness);
    }
    let early = status_from(&state, "paged", Some(0));
    assert_eq!(early["state"], "recording", "{early}");
    assert_eq!(early["sample_count"], json!(5), "{early}");
    assert_eq!(early["samples"].as_array().unwrap().len(), 5, "{early}");
    assert_eq!(early["next_sample"], json!(5), "{early}");
    assert!(early["reason"].is_null(), "{early}");
    for _ in 0..2095 {
        recording_cycle(&mut harness);
    }
    let first_page = status_from(&state, "paged", Some(0));
    assert_eq!(first_page["samples"].as_array().unwrap().len(), 2000);
    assert_eq!(first_page["next_sample"], json!(2000));
    assert_eq!(first_page["sample_count"], json!(2100));
    let rest = status_from(&state, "paged", Some(2000));
    assert_eq!(rest["samples"].as_array().unwrap().len(), 100, "{rest}");
    assert_eq!(rest["next_sample"], json!(2100), "{rest}");
    let without_start = status_from(&state, "paged", None);
    assert_eq!(without_start["samples"], json!([]), "{without_start}");
    assert_eq!(without_start["next_sample"], json!(2100), "{without_start}");
    let stopped = stop_with(&state, "paged").result.expect("all samples");
    assert_eq!(stopped["samples"].as_array().unwrap().len(), 2100);
    assert_eq!(stopped["stop_reason"], "stopped", "{stopped}");
}
