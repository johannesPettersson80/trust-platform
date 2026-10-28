//! The start of a runtime recording (E11.2, E11.4 and E11.6): the owner is journaled before the dispatch, a refusal clears
//! it, a lost acknowledgement keeps it, and the record period, the task and the identifier of a start are checked. Split
//! from `runtime_recording_tests.rs` by concept.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::experiment::{ControlExperimentPlan, RecordingRecoveryOwner};
use trust_control::failure::Failure;
use trust_control::identification::connected_record::connected_identification_record;
use trust_control::limits::{record_period_executions, RECORDING_SAMPLE_LIMIT};
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::Completeness;

use crate::runtime_client::fake_runtime::{answer, refusal, FakeRuntime};

use super::super::{
    new_recording_id, PendingReleases, RecordingOwnerPersistence, RecordingStartError,
    RecordingStartPlan, RuntimeRecordingSession,
};
use super::{
    fixture_bindings, loop_bindings, owner_with, sample, scope_loop, session_on, stopped, two_scans,
};

/// The scope loop bound to the parameter record of `task`, as the `TypeScript` test's `bound(task)` declares it.
fn scope_loop_bound_to(task: &str) -> LoopDefinition {
    let mut value = json!(scope_loop());
    value["parameterBinding"] = json!({
        "loop_id": "loop", "task_name": task, "parameters": [],
        "mode": { "symbol": "P1.AutoMode", "manual_value": false }
    });
    serde_json::from_value(value).expect("the loop decodes")
}

/// The scope loop with its three loop signals named by symbol alone, so that its start is symbolic (E4.2).
fn symbolic_scope_loop() -> LoopDefinition {
    let mut value = json!(scope_loop());
    for signal in ["setpoint", "processVariable", "manipulatedVariable"] {
        value["signals"][signal]
            .as_object_mut()
            .expect("a signal object")
            .remove("address");
    }
    serde_json::from_value(value).expect("the loop decodes")
}

/// The scope loop as a project-scripted experiment of 2 s, as lesson 1 declares its project test (C12): the project owns the
/// sequence, and the Boolean authority it is enabled by is the activation.
fn project_scripted_scope_loop() -> LoopDefinition {
    let mut value = json!(scope_loop());
    value["experiment"] = json!({
        "execution": "project-scripted", "kind": "observation", "amplitude": 0.0,
        "durationSeconds": 2.0, "seed": 1,
        "activation": {
            "symbol": "P1.ExperimentEnabled", "address": "%MX24.0",
            "iecType": "BOOL", "direction": "write"
        }
    });
    serde_json::from_value(value).expect("the loop decodes")
}

/// The status of a simulated runtime with one task and a cycle of `cycle_ms` (E1.3).
fn status(cycle_ms: u64) -> Value {
    json!({
        "session_epoch": 7, "simulation_enabled": true, "simulation_only_io": true,
        "simulation_time_scale": 1, "simulation_achieved_time_scale": null,
        "program_revision": "program", "tasks": [{ "name": "MainTask", "interval_ms": 100 }],
        "cycle_interval_ms": cycle_ms
    })
}

/// Records the owner's journal writes and the order of the start's calls.
struct Recorder {
    events: Arc<Mutex<Vec<String>>>,
}

impl RecordingOwnerPersistence for Recorder {
    fn persist(&self, owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        self.events
            .lock()
            .push(format!("persist:{}", owner.recording_id));
        Ok(())
    }

    fn clear(&self, _owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        self.events.lock().push("clear".to_string());
        Ok(())
    }
}

fn started(
    result: Result<RuntimeRecordingSession, RecordingStartError<RuntimeRecordingSession>>,
) -> RuntimeRecordingSession {
    match result {
        Ok(session) => session,
        Err(RecordingStartError::Unconfirmed(unconfirmed)) => {
            panic!("the start is unconfirmed: {}", unconfirmed.message())
        }
        Err(RecordingStartError::Failed(failure)) => panic!("the start failed: {failure:?}"),
    }
}

fn start_error(
    result: Result<RuntimeRecordingSession, RecordingStartError<RuntimeRecordingSession>>,
) -> RecordingStartError<RuntimeRecordingSession> {
    match result {
        Ok(_) => panic!("the start succeeded"),
        Err(error) => error,
    }
}

fn plan(duration_ms: u64, settling_ms: Option<u64>) -> RecordingStartPlan {
    RecordingStartPlan {
        recording_id: "recording-1".into(),
        planned_duration_ms: duration_ms,
        expected_settling_ms: settling_ms,
    }
}

#[test]
fn records_about_200_samples_per_expected_settling_time_never_more_than_one_per_execution() {
    // The TypeScript numbers, with the settling times in ms (the caller converts the study's seconds).
    // A 45 min validation of a process that settles in 25 min, at a 100 ms task: one sample per 7.5 s, 75 executions each.
    assert_eq!(
        record_period_executions(2_700_000, 100, RECORDING_SAMPLE_LIMIT, Some(1_500_000)),
        75
    );
    // A fast process keeps every execution.
    assert_eq!(
        record_period_executions(30_000, 100, RECORDING_SAMPLE_LIMIT, Some(10_000)),
        1
    );
    // A recording keeps at least 200 samples, even when it is shorter than the settling time.
    assert_eq!(
        record_period_executions(600_000, 100, RECORDING_SAMPLE_LIMIT, Some(7_200_000)),
        30
    );
    // Without an expected settling time only the sample limit sets the period.
    assert_eq!(
        record_period_executions(28_800_000, 100, RECORDING_SAMPLE_LIMIT, None),
        3
    );
}

#[test]
fn a_start_journals_its_owner_before_dispatch_and_sends_the_record_period_of_a_settling_process() {
    let events: Arc<Mutex<Vec<String>>> = Arc::default();
    let dispatched = Arc::clone(&events);
    let fake =
        FakeRuntime::start(
            move |request| match request["type"].as_str().unwrap_or_default() {
                "status" => Some(answer(request, status(100))),
                "control.recording.start" => {
                    dispatched.lock().push("dispatch".to_string());
                    Some(answer(
                        request,
                        json!({ "recording_id": "recording-1", "status": "recording" }),
                    ))
                }
                _ => None,
            },
        );
    let recorder = Recorder {
        events: Arc::clone(&events),
    };
    let pending = Arc::new(PendingReleases::new());
    let session = started(RuntimeRecordingSession::start(
        &fake.connection(None),
        &scope_loop(),
        &plan(2_700_000, Some(1_500_000)),
        Some(&recorder),
        &pending,
    ));
    assert_eq!(
        *events.lock(),
        ["persist:recording-1", "dispatch"],
        "the owner is written before the dispatch (E11.2)"
    );
    assert_eq!(session.recording_id(), "recording-1");
    let params = fake.requests_of("control.recording.start")[0]["params"].clone();
    assert_eq!(
        params["record_every"], 75,
        "one sample per 7.5 s of a 100 ms cycle"
    );
    assert_eq!(params["maximum_samples"], RECORDING_SAMPLE_LIMIT);
    assert_eq!(params["maximum_duration_ms"], 2_700_000);
    assert!(
        params.get("task").is_none(),
        "a background recording samples every resource cycle"
    );
    assert_eq!(
        params["program_revision"],
        json!("program"),
        "a recording names the program it runs on, which a connected record needs (A4.4)"
    );
}

/// The stop answer of the 2 s project-scripted campaign: one sample per 100 ms scan over 21 scans, every scan captured in order,
/// as the runtime answers a complete recording (E4.5, E4.7). It names the identity, the bindings and the program revision the
/// start sent.
fn two_second_stop(start: &Value) -> Value {
    let samples: Vec<Value> = (1..=21_u64)
        .map(|scan| {
            let time = i64::try_from(scan - 1).expect("a scan index") * 100_000_000;
            sample(scan, time, vec![json!(50.0), json!(40.0), json!(0.0)])
        })
        .collect();
    json!({
        "recording_id": start["recording_id"], "loop_revision": start["loop_revision"],
        "program_revision": start["program_revision"], "bindings": start["bindings"],
        "stop_reason": "stopped",
        "acquisition": {
            "method": "runtime_scan_buffer", "completeness": "complete", "observedSampleCount": 21,
            "scanRange": { "first": 1, "last": 21 }, "missingScanRanges": [],
            "droppedObservedSamples": 0, "actualCycleTimingAvailable": true,
            "freshnessAvailable": false,
            "reason": "Every runtime scan in the recording interval was captured."
        },
        "samples": samples
    })
}

/// The plan of a project's own sequence, which the campaign of a project-scripted loop records (C12).
fn project_sequence_plan() -> ControlExperimentPlan {
    serde_json::from_value(json!({
        "id": "experiment:loop:11:project_sequence", "source": "simulation",
        "kind": "project_sequence", "affectedSignal": "loop", "valueType": "LREAL",
        "from": 0.0, "to": 0.0, "unit": "%", "minimum": 0.0, "maximum": 100.0,
        "durationMs": 2000.0, "expectedScan": 11, "stopConditions": [],
        "restorationValue": 0.0, "requiresPhysicalApproval": false, "trajectory": []
    }))
    .expect("the fixture plan decodes")
}

#[test]
fn a_two_second_project_scripted_recording_of_addressed_signals_is_a_complete_connected_record() {
    // Lesson 1's signals are all addressed, so no symbol asks for a program revision. The record still needs one (A4.4), and
    // the runtime checks the revision a start names against its own, so the start names the status's revision.
    let captured = Arc::new(Mutex::new(Value::Null));
    let fake =
        FakeRuntime::start(
            move |request| match request["type"].as_str().unwrap_or_default() {
                "status" => Some(answer(request, status(100))),
                "control.recording.start"
                    if request["params"]["program_revision"] == json!("program") =>
                {
                    *captured.lock() = request["params"].clone();
                    Some(answer(
                        request,
                        json!({ "recording_id": "recording-1", "status": "recording" }),
                    ))
                }
                "control.recording.start" => Some(refusal(
                    request,
                    "recording_unavailable",
                    "recording program revision changed or is missing",
                )),
                "control.recording.stop" => {
                    Some(answer(request, two_second_stop(&captured.lock())))
                }
                _ => None,
            },
        );
    let mut session = started(RuntimeRecordingSession::start(
        &fake.connection(None),
        &project_scripted_scope_loop(),
        &plan(2_000, None),
        None,
        &Arc::new(PendingReleases::new()),
    ));
    let run = session
        .stop()
        .expect("the recording stops and answers its samples");
    assert_eq!(
        run.program_revision.as_deref(),
        Some("program"),
        "the recording names the program it ran on"
    );
    assert_eq!(
        session.evidence(&run).completeness,
        Completeness::Complete,
        "the runtime captured every scan of the 2 s run"
    );
    let experiment = project_sequence_plan();
    let record = connected_identification_record(
        &project_scripted_scope_loop(),
        &run,
        None,
        &experiment,
        0.0,
        None,
        |_, _| Ok::<(), Failure>(()),
    )
    .expect("the complete recording is a connected record");
    assert_eq!(record.request.acquisition.program_revision, "program");
    assert_eq!(
        record.run.samples.len(),
        21,
        "one sample per scan of the 2 s run"
    );
}

#[test]
fn a_loop_recording_samples_its_task_and_a_background_program_samples_every_cycle() {
    // The loop's mode is named by symbol, so each start first takes one coherent inspection (E4.2, E11.4).
    let fake = FakeRuntime::start(
        |request| match request["type"].as_str().unwrap_or_default() {
            "status" => Some(answer(request, status(100))),
            "control.signals.inspect" => {
                let params = &request["params"];
                let count = params["bindings"].as_array().map_or(0, Vec::len);
                Some(answer(
                    request,
                    json!({
                        "available": true, "program_revision": "program",
                        "loop_revision": params["loop_revision"], "bindings": params["bindings"],
                        "scan": 1, "runtime_time_ns": 100_000_000, "captured_at_ms": 1000,
                        "values": vec![1.0; count], "forced": vec![false; count],
                        "quality": vec!["good"; count]
                    }),
                ))
            }
            "control.recording.start" => Some(answer(
                request,
                json!({ "recording_id": "recording-1", "status": "recording" }),
            )),
            _ => None,
        },
    );
    let pending = Arc::new(PendingReleases::new());
    for task in ["MainTask", "$background"] {
        let session = started(RuntimeRecordingSession::start(
            &fake.connection(None),
            &scope_loop_bound_to(task),
            &plan(60_000, None),
            None,
            &pending,
        ));
        assert_eq!(session.recording_id(), "recording-1");
    }
    let recorded_tasks: Vec<Option<Value>> = fake
        .requests_of("control.recording.start")
        .iter()
        .map(|request| request["params"].get("task").cloned())
        .collect();
    assert_eq!(
        recorded_tasks,
        vec![Some(json!("MainTask")), None],
        "a loop's recording names its task (E4.2, A4.1); a background program names none, so it samples every cycle"
    );
}

#[test]
fn a_symbolic_start_waits_for_a_coherent_scan_and_sends_one_start_with_its_program_revision() {
    let served_inspections = AtomicUsize::new(0);
    let fake = FakeRuntime::start(move |request| {
        match request["type"].as_str().unwrap_or_default() {
            "status" => Some(answer(request, status(100))),
            "control.signals.inspect" => {
                let params = &request["params"];
                let reply = if served_inspections.fetch_add(1, Ordering::SeqCst) == 0 {
                    json!({
                        "available": false, "program_revision": "program",
                        "loop_revision": params["loop_revision"], "bindings": params["bindings"],
                        "reason": "Waiting for the next coherent runtime scan."
                    })
                } else {
                    json!({
                        "available": true, "program_revision": "program",
                        "loop_revision": params["loop_revision"], "bindings": params["bindings"],
                        "scan": 1, "runtime_time_ns": 100_000_000, "captured_at_ms": 1000,
                        "values": [1.0, 0.5, 2.0], "forced": [false, false, false],
                        "quality": ["good", "good", "good"]
                    })
                };
                Some(answer(request, reply))
            }
            "control.recording.start" => Some(answer(
                request,
                json!({ "recording_id": "recording-1", "status": "recording" }),
            )),
            _ => None,
        }
    });
    let session = started(RuntimeRecordingSession::start(
        &fake.connection(None),
        &symbolic_scope_loop(),
        &plan(60_000, None),
        None,
        &Arc::new(PendingReleases::new()),
    ));
    let inspect_requests = fake.requests_of("control.signals.inspect");
    let start_requests = fake.requests_of("control.recording.start");
    assert_eq!(
        inspect_requests.len(),
        2,
        "the pending answer is followed by one fresh inspection (E11.4)"
    );
    assert_eq!(
        inspect_requests[1]["params"]["expected_program_revision"],
        json!("program"),
        "the fresh inspection is pinned to the program revision the first one fixed (E11.4)"
    );
    assert_eq!(
        start_requests.len(),
        1,
        "the start is sent once, after the coherent scan"
    );
    assert_eq!(
        start_requests[0]["params"]["program_revision"],
        json!("program")
    );
    assert_eq!(
        start_requests[0]["params"]["bindings"], inspect_requests[1]["params"]["bindings"],
        "the start names the bindings of the coherent scan (E4.2)"
    );
    assert_eq!(
        session.recovery_owner().program_revision.as_deref(),
        Some("program"),
        "the owner keeps the program revision of the start"
    );
}

#[test]
fn a_refused_start_clears_the_owner_it_journaled() {
    let fake = FakeRuntime::start(
        |request| match request["type"].as_str().unwrap_or_default() {
            "status" => Some(answer(request, status(100))),
            "control.recording.start" => Some(refusal(
                request,
                "recording_unavailable",
                "Recording channels are limited to 32.",
            )),
            _ => None,
        },
    );
    let events: Arc<Mutex<Vec<String>>> = Arc::default();
    let recorder = Recorder {
        events: Arc::clone(&events),
    };
    let error = start_error(RuntimeRecordingSession::start(
        &fake.connection(None),
        &scope_loop(),
        &plan(60_000, None),
        Some(&recorder),
        &Arc::new(PendingReleases::new()),
    ));
    match error {
        RecordingStartError::Failed(failure) => {
            assert_eq!(failure.message, "Recording channels are limited to 32.");
        }
        RecordingStartError::Unconfirmed(_) => panic!("a refusal is not an unconfirmed start"),
    }
    assert_eq!(
        *events.lock(),
        ["persist:recording-1", "clear"],
        "no buffer exists, so nothing is kept (E11.6)"
    );
}

#[test]
fn a_lost_start_acknowledgement_keeps_the_owner_and_stops_the_recording_it_may_have_begun() {
    // The stop answers for the recording this start began: the plan's identifier and the program revision the status read names,
    // which the start sends for a non-symbolic recording (E4.2) and the stop answers with (E4.7).
    let owner = RecordingRecoveryOwner {
        recording_id: "recording-1".into(),
        ..owner_with(Some("program"), loop_bindings())
    };
    let stop_result = stopped(
        &owner,
        vec![
            sample(1, 0, vec![json!(1.0), json!(0.5), json!(2.0)]),
            sample(2, 10_000_000, vec![json!(1.0), json!(0.5), json!(2.0)]),
        ],
    );
    let events: Arc<Mutex<Vec<String>>> = Arc::default();
    let dispatched = Arc::clone(&events);
    let fake =
        FakeRuntime::start(
            move |request| match request["type"].as_str().unwrap_or_default() {
                "status" => Some(answer(request, status(100))),
                "control.recording.start" => {
                    dispatched.lock().push("dispatch".to_string());
                    None
                }
                "control.recording.stop" => Some(answer(request, stop_result.clone())),
                _ => None,
            },
        );
    let recorder = Recorder {
        events: Arc::clone(&events),
    };
    let error = start_error(RuntimeRecordingSession::start(
        &fake.connection(None),
        &scope_loop(),
        &plan(60_000, None),
        Some(&recorder),
        &Arc::new(PendingReleases::new()),
    ));
    let RecordingStartError::Unconfirmed(unconfirmed) = error else {
        panic!("a lost acknowledgement is unconfirmed");
    };
    let message = unconfirmed.message();
    assert!(
        message.contains("Recording Start acknowledgement was not confirmed"),
        "{message}"
    );
    assert!(
        message.contains("The existing recording was stopped and its actual samples retained."),
        "{message}"
    );
    assert_eq!(
        unconfirmed.recording.as_ref().map(|run| run.samples.len()),
        Some(2)
    );
    assert_eq!(
        *events.lock(),
        ["persist:recording-1", "dispatch", "persist:recording-1"],
        "the owner is written again with its unconfirmed acknowledgement"
    );
    assert_eq!(
        unconfirmed
            .session
            .recovery_owner()
            .start_acknowledgement_unconfirmed,
        Some(true)
    );
}

#[test]
fn a_request_that_breaks_a_limit_is_refused_before_it_is_journaled_or_sent() {
    let fake = FakeRuntime::start(
        |request| match request["type"].as_str().unwrap_or_default() {
            "status" => Some(answer(request, status(100))),
            _ => None,
        },
    );
    let events: Arc<Mutex<Vec<String>>> = Arc::default();
    let recorder = Recorder {
        events: Arc::clone(&events),
    };
    let error = start_error(RuntimeRecordingSession::start(
        &fake.connection(None),
        &scope_loop(),
        &plan(5, None),
        Some(&recorder),
        &Arc::new(PendingReleases::new()),
    ));
    assert!(matches!(error, RecordingStartError::Failed(_)));
    assert!(
        events.lock().is_empty(),
        "nothing is journaled for a request that is not sent"
    );
    assert_eq!(
        fake.requests_of("control.recording.start"),
        [] as [Value; 0]
    );
}

#[test]
fn a_release_whose_reply_was_lost_is_retried_by_the_next_start_on_the_same_endpoint() {
    let owner = owner_with(None, fixture_bindings());
    let stop_result = stopped(&owner, two_scans(&owner, 1.0));
    let releases = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&releases);
    let fake =
        FakeRuntime::start(
            move |request| match request["type"].as_str().unwrap_or_default() {
                "status" => Some(answer(request, status(100))),
                "control.recording.stop" => Some(answer(request, stop_result.clone())),
                "control.recording.release" if counter.fetch_add(1, Ordering::SeqCst) == 0 => None,
                "control.recording.release" => Some(answer(
                    request,
                    json!({ "recording_id": "record", "status": "released" }),
                )),
                "control.recording.start" => Some(answer(
                    request,
                    json!({ "recording_id": "recording-1", "status": "recording" }),
                )),
                _ => None,
            },
        );
    let pending = Arc::new(PendingReleases::new());
    let mut session = session_on(&fake, &scope_loop(), &owner, &pending);
    session.stop().expect("stopped");
    session.release().expect("the lost reply is kept pending");
    assert_eq!(pending.waiting_on(&fake.connection(None).endpoint).len(), 1);

    started(RuntimeRecordingSession::start(
        &fake.connection(None),
        &scope_loop(),
        &plan(60_000, None),
        None,
        &pending,
    ));
    assert!(
        pending
            .waiting_on(&fake.connection(None).endpoint)
            .is_empty(),
        "the next start released it"
    );
    assert_eq!(fake.requests_of("control.recording.release").len(), 2);
}

#[test]
fn a_new_recording_identifier_is_recording_dash_and_32_hex_digits_and_is_not_repeated() {
    let first = new_recording_id();
    let second = new_recording_id();
    assert!(first.starts_with("recording-"));
    assert_eq!(first.len(), "recording-".len() + 32);
    assert!(first["recording-".len()..]
        .chars()
        .all(|character| character.is_ascii_hexdigit()));
    assert_ne!(first, second);
}
