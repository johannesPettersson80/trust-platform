//! Specification 35, B2, A3.4 and B9.2: the monitor's answer from the runtime's status and one coherent scan, the session and
//! endpoint rules, and the target's label, against the fake runtime. A call answers at once and its read runs in the background,
//! so a poll below waits for the read it starts: the next poll answers that read's sample. The observation time is passed in,
//! so the tests drive it.

// The values are the numbers the fake runtime answers with, passed on unchanged, so they are compared exactly.
#![allow(clippy::float_cmp)]

use std::sync::{mpsc, Arc};
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::json;
use trust_control::loop_monitor::{LoopMonitorSample, MonitorSource, AWAITING_SCAN};
use trust_control::messages::MonitorRequest;
use trust_control::runtime::RuntimeConnection;

use super::*;
use crate::runtime_client::fake_runtime::{answer, refusal, tank_loop, FakeRuntime};

/// What the fake runtime is in: its status report, and the scan it answers with.
#[derive(Debug, Clone)]
struct Plant {
    status: serde_json::Value,
    scan: u64,
    program_revision: String,
    pending: bool,
    process_variable: Option<f64>,
}

impl Plant {
    /// A simulated target whose status carries the speed words the runtime gives for its speeds (specification 35, B9.2).
    fn simulated(scale: u32, achieved: Option<f64>, cycle_ms: u64, speed_text: &str) -> Self {
        Self {
            status: json!({
                "state": "running", "session_epoch": 3, "simulation_enabled": true, "simulation_only_io": true,
                "simulation_time_scale": scale, "simulation_achieved_time_scale": achieved,
                "simulated": true, "simulation_speed_text": speed_text,
                "program_revision": "sha256:program", "tasks": [{ "name": "MainTask", "interval_ms": 10 }],
                "cycle_interval_ms": cycle_ms
            }),
            scan: 41,
            program_revision: "rev-1".into(),
            pending: false,
            process_variable: Some(21.5),
        }
    }
}

fn serve(request: &serde_json::Value, plant: &Plant) -> Option<serde_json::Value> {
    match request["type"].as_str()? {
        "status" => Some(answer(request, plant.status.clone())),
        "control.signals.inspect" => Some(answer(request, inspection(request, plant))),
        _ => None,
    }
}

/// The inspection answer for the request's bindings: the setpoint 50, the output 36, and the measurement as the plant gives it.
fn inspection(request: &serde_json::Value, plant: &Plant) -> serde_json::Value {
    let params = &request["params"];
    let bindings = params["bindings"].clone();
    let bindings_list = bindings.as_array().cloned().unwrap_or_default();
    if plant.pending {
        return json!({
            "available": false, "loop_revision": params["loop_revision"], "program_revision": plant.program_revision,
            "bindings": bindings, "reason": "Waiting for the next coherent runtime scan."
        });
    }
    let mut values = Vec::new();
    let mut quality = Vec::new();
    for binding in &bindings_list {
        let (value, good) = match binding["id"].as_str() {
            Some("setpoint") => (json!(50.0), true),
            Some("manipulatedVariable") => (json!(36.0), true),
            Some("processVariable") => match plant.process_variable {
                Some(value) => (json!(value), true),
                None => (serde_json::Value::Null, false),
            },
            _ => (json!(0.0), true),
        };
        values.push(value);
        quality.push(if good { "good" } else { "missing" });
    }
    let count = bindings_list.len();
    json!({
        "available": true, "loop_revision": params["loop_revision"], "program_revision": plant.program_revision,
        "bindings": bindings, "scan": plant.scan, "runtime_time_ns": plant.scan * 100_000_000,
        "captured_at_ms": 1_000, "values": values, "forced": vec![false; count], "quality": quality
    })
}

fn runtime(plant: &Arc<Mutex<Plant>>) -> FakeRuntime {
    let plant = Arc::clone(plant);
    FakeRuntime::start(move |request| serve(request, &plant.lock()))
}

/// A fake runtime whose status answers wait for the test: each token sent on `release` lets one status answer go, and `reached`
/// receives a message when a status request arrives, before it waits.
struct Gate {
    reached: mpsc::Receiver<()>,
    release: mpsc::Sender<()>,
}

fn gated_runtime(plant: &Arc<Mutex<Plant>>) -> (FakeRuntime, Gate) {
    let (reached_tx, reached) = mpsc::channel::<()>();
    let (release, release_rx) = mpsc::channel::<()>();
    let reached_tx = Mutex::new(reached_tx);
    let release_rx = Mutex::new(release_rx);
    let plant = Arc::clone(plant);
    let fake = FakeRuntime::start(move |request| {
        if request["type"].as_str() == Some("status") {
            let _ = reached_tx.lock().send(());
            let _ = release_rx.lock().recv_timeout(Duration::from_secs(10));
        }
        serve(request, &plant.lock())
    });
    (fake, Gate { reached, release })
}

fn request(connection: Option<RuntimeConnection>, session: Option<&str>) -> MonitorRequest {
    MonitorRequest {
        project_root: "/project".into(),
        runtime_id: "runtime:local".into(),
        loop_id: "loop:tank".into(),
        editor_session_id: "editor-1".into(),
        connection,
        runtime_session_id: session.map(str::to_string),
    }
}

fn fixed_clock(now: u64) -> Clock {
    Arc::new(move || now)
}

/// One call at `now`: it answers at once, and starts a read when none is in flight.
fn observe(
    session: &mut MonitorSession,
    fake: &FakeRuntime,
    session_id: Option<&str>,
    now: u64,
) -> MonitorResult {
    session.observe(
        &tank_loop(),
        &request(Some(fake.connection(None)), session_id),
        &fixed_clock(now),
    )
}

/// One poll of the page: the call answers, and then the read it started has ended, so the next poll answers that read.
fn poll(
    session: &mut MonitorSession,
    fake: &FakeRuntime,
    session_id: Option<&str>,
    now: u64,
) -> MonitorResult {
    poll_connection(session, Some(fake.connection(None)), session_id, now)
}

fn poll_connection(
    session: &mut MonitorSession,
    connection: Option<RuntimeConnection>,
    session_id: Option<&str>,
    now: u64,
) -> MonitorResult {
    let result = session.observe(
        &tank_loop(),
        &request(connection, session_id),
        &fixed_clock(now),
    );
    join_read(session);
    result
}

/// Waits for the read in flight to end, so that the test sees what it wrote.
fn join_read(session: &mut MonitorSession) {
    if let Some(read) = session.read.take() {
        read.join().expect("the read ends");
    }
}

fn available(result: &MonitorResult) -> &trust_control::loop_monitor::AvailableSample {
    match &result.sample {
        LoopMonitorSample::Available(sample) => sample,
        LoopMonitorSample::Unavailable(sample) => {
            panic!("the sample is unavailable: {}", sample.reason)
        }
    }
}

fn reason(result: &MonitorResult) -> &str {
    match &result.sample {
        LoopMonitorSample::Unavailable(sample) => &sample.reason,
        LoopMonitorSample::Available(_) => panic!("the sample is available"),
    }
}

#[test]
fn the_engine_answers_the_monitor_method() {
    assert!(
        crate::commands::METHODS.contains(&crate::commands::MONITOR),
        "the engine answers controlStudio/monitor"
    );
}

#[test]
fn a_running_loop_is_answered_from_one_status_read_and_one_scan() {
    let plant = Arc::new(Mutex::new(Plant::simulated(
        10,
        Some(10.0),
        10,
        "\u{00D7}10",
    )));
    let fake = runtime(&plant);
    let mut session = MonitorSession::default();
    let first = poll(&mut session, &fake, Some("debug-1"), 1_000);
    assert_eq!(
        reason(&first),
        AWAITING_SCAN,
        "nothing is held before the first read"
    );
    let result = poll(&mut session, &fake, Some("debug-1"), 1_000);
    let sample = available(&result);
    assert_eq!(sample.source, MonitorSource::Simulation);
    assert_eq!(sample.scan, 41);
    assert_eq!(sample.process_variable.value, 21.5);
    assert_eq!(sample.runtime_session_id.as_deref(), Some("debug-1"));
    assert_eq!(
        result.target_label.as_deref(),
        Some("Simulation \u{00D7}10")
    );
    let types: Vec<String> = fake
        .requests()
        .iter()
        .map(|request| request["type"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        types,
        [
            "status",
            "control.signals.inspect",
            "status",
            "control.signals.inspect"
        ],
        "each poll reads the status and one scan, and a monitor call writes nothing"
    );
}

#[test]
fn a_monitor_call_returns_at_once_while_the_runtime_read_blocks() {
    let plant = Arc::new(Mutex::new(Plant::simulated(
        10,
        Some(10.0),
        10,
        "\u{00D7}10",
    )));
    let (fake, gate) = gated_runtime(&plant);
    let mut session = MonitorSession::default();
    let result = observe(&mut session, &fake, Some("debug-1"), 1_000);
    assert_eq!(
        reason(&result),
        AWAITING_SCAN,
        "the call answers while its read waits at the runtime"
    );
    gate.reached
        .recv_timeout(Duration::from_secs(5))
        .expect("the read reaches the runtime");
    assert!(session.reading(), "the read is still in flight");
    gate.release
        .send(())
        .expect("the runtime is waiting for its answer");
    join_read(&mut session);
}

#[test]
fn a_second_call_while_a_read_is_in_flight_starts_no_second_read() {
    let plant = Arc::new(Mutex::new(Plant::simulated(
        10,
        Some(10.0),
        10,
        "\u{00D7}10",
    )));
    let (fake, gate) = gated_runtime(&plant);
    let mut session = MonitorSession::default();
    observe(&mut session, &fake, Some("debug-1"), 1_000);
    gate.reached
        .recv_timeout(Duration::from_secs(5))
        .expect("the read reaches the runtime");
    let started = session.read.as_ref().map(|read| read.thread().id());
    let second = observe(&mut session, &fake, Some("debug-1"), 1_010);
    assert_eq!(
        reason(&second),
        AWAITING_SCAN,
        "the read in flight has not answered yet"
    );
    assert_eq!(
        session.read.as_ref().map(|read| read.thread().id()),
        started,
        "a call while a read is in flight starts no second read"
    );
    gate.release
        .send(())
        .expect("the runtime is waiting for its answer");
    join_read(&mut session);
    assert_eq!(
        fake.requests_of("status").len(),
        1,
        "one read asked the runtime for its status"
    );
}

#[test]
fn the_answer_after_the_read_carries_its_sample() {
    let plant = Arc::new(Mutex::new(Plant::simulated(
        10,
        Some(10.0),
        10,
        "\u{00D7}10",
    )));
    let (fake, gate) = gated_runtime(&plant);
    let mut session = MonitorSession::default();
    observe(&mut session, &fake, Some("debug-1"), 1_000);
    gate.reached
        .recv_timeout(Duration::from_secs(5))
        .expect("the read reaches the runtime");
    gate.release
        .send(())
        .expect("the runtime is waiting for its answer");
    join_read(&mut session);
    // This call answers the read that has ended, and starts a read of its own, which waits at the gate for a second token.
    let result = observe(&mut session, &fake, Some("debug-1"), 1_010);
    let sample = available(&result);
    assert_eq!(sample.scan, 41);
    assert_eq!(sample.process_variable.value, 21.5);
    assert_eq!(sample.runtime_session_id.as_deref(), Some("debug-1"));
    assert_eq!(
        result.target_label.as_deref(),
        Some("Simulation \u{00D7}10")
    );
    gate.release
        .send(())
        .expect("the second read is waiting for its answer");
    join_read(&mut session);
}

#[test]
fn the_target_label_is_answered_only_when_it_changes() {
    let plant = Arc::new(Mutex::new(Plant::simulated(
        10,
        Some(10.0),
        10,
        "\u{00D7}10",
    )));
    let fake = runtime(&plant);
    let mut session = MonitorSession::default();
    assert_eq!(
        poll(&mut session, &fake, Some("debug-1"), 1_000).target_label,
        None,
        "nothing is known before the first read"
    );
    assert_eq!(
        poll(&mut session, &fake, Some("debug-1"), 1_000)
            .target_label
            .as_deref(),
        Some("Simulation \u{00D7}10")
    );
    plant.lock().scan = 42;
    assert_eq!(
        poll(&mut session, &fake, Some("debug-1"), 1_010).target_label,
        None
    );
    plant.lock().status = Plant::simulated(10, Some(7.0), 10, "\u{00D7}7 of \u{00D7}10").status;
    plant.lock().scan = 43;
    assert_eq!(
        poll(&mut session, &fake, Some("debug-1"), 1_020).target_label,
        None,
        "the read this poll starts sees the new speed; the next poll answers it"
    );
    assert_eq!(
        poll(&mut session, &fake, Some("debug-1"), 1_030)
            .target_label
            .as_deref(),
        Some("Simulation \u{00D7}7 of \u{00D7}10"),
        "an achieved speed more than 5 % below the configured one is shown"
    );
    assert_eq!(
        poll(&mut session, &fake, Some("debug-1"), 1_040).target_label,
        None
    );
}

#[test]
fn a_pending_scan_is_answered_at_once_and_its_program_revision_is_carried() {
    let plant = Arc::new(Mutex::new(Plant::simulated(1, None, 10, "")));
    plant.lock().pending = true;
    plant.lock().program_revision = "rev-2".into();
    let fake = runtime(&plant);
    let mut session = MonitorSession::default();
    let first = poll(&mut session, &fake, Some("debug-1"), 1_000);
    assert_eq!(
        reason(&first),
        AWAITING_SCAN,
        "nothing is held before the first read"
    );
    assert_eq!(
        fake.requests_of("control.signals.inspect").len(),
        1,
        "the call asks once, and does not wait for the scan"
    );
    plant.lock().pending = false;
    assert_eq!(
        reason(&poll(&mut session, &fake, Some("debug-1"), 1_010)),
        "Waiting for the next coherent runtime scan.",
        "the runtime's pending answer is the sample of the first read"
    );
    available(&poll(&mut session, &fake, Some("debug-1"), 1_020));
    let inspections = fake.requests_of("control.signals.inspect");
    assert_eq!(
        inspections[1]["params"]["expected_program_revision"], "rev-2",
        "the next scan must have the program revision the pending answer gave"
    );
}

#[test]
fn a_new_debug_session_restarts_the_observation_and_the_same_session_keeps_the_order_of_scans() {
    let plant = Arc::new(Mutex::new(Plant::simulated(1, None, 10, "")));
    let fake = runtime(&plant);
    let mut session = MonitorSession::default();
    poll(&mut session, &fake, Some("debug-1"), 1_000);
    available(&poll(&mut session, &fake, Some("debug-1"), 1_000));
    plant.lock().scan = 1;
    assert_eq!(
        reason(&poll(&mut session, &fake, Some("debug-2"), 1_010)),
        AWAITING_SCAN,
        "a new session holds none of the old session's samples"
    );
    assert_eq!(
        available(&poll(&mut session, &fake, Some("debug-2"), 1_020)).scan,
        1,
        "a new session may start its scans again"
    );
    plant.lock().scan = 0;
    available(&poll(&mut session, &fake, Some("debug-2"), 1_030));
    assert_eq!(
        reason(&poll(&mut session, &fake, Some("debug-2"), 1_040)),
        "Runtime scan moved backwards from 1 to 0"
    );
}

#[test]
fn a_scan_that_has_not_advanced_for_longer_than_the_connection_loss_timeout_is_stale() {
    let plant = Arc::new(Mutex::new(Plant::simulated(1, None, 10, "")));
    let fake = runtime(&plant);
    let mut session = MonitorSession::default();
    poll(&mut session, &fake, Some("debug-1"), 1_000);
    available(&poll(&mut session, &fake, Some("debug-1"), 6_000));
    available(&poll(&mut session, &fake, Some("debug-1"), 6_001));
    assert_eq!(
        reason(&poll(&mut session, &fake, Some("debug-1"), 6_010)),
        "Runtime scan is stale",
        "5 s is the threshold for a 10 ms resource cycle: stale only after it"
    );
    plant.lock().scan = 42;
    assert_eq!(
        reason(&poll(&mut session, &fake, Some("debug-1"), 6_020)),
        "Runtime scan is stale",
        "the read that sees the new scan is answered by the next poll"
    );
    available(&poll(&mut session, &fake, Some("debug-1"), 6_030));
}

#[test]
fn a_long_resource_cycle_lengthens_the_stale_threshold_to_three_cycles() {
    let plant = Arc::new(Mutex::new(Plant::simulated(1, None, 3_000, "")));
    let fake = runtime(&plant);
    let mut session = MonitorSession::default();
    poll(&mut session, &fake, Some("debug-1"), 1_000);
    available(&poll(&mut session, &fake, Some("debug-1"), 10_000));
    available(&poll(&mut session, &fake, Some("debug-1"), 10_001));
    assert_eq!(
        reason(&poll(&mut session, &fake, Some("debug-1"), 10_002)),
        "Runtime scan is stale",
        "three 3 s cycles are 9 s"
    );
}

#[test]
fn a_runtime_that_is_not_running_is_not_producing_scans_and_reads_nothing() {
    let plant = Arc::new(Mutex::new(Plant::simulated(1, None, 10, "")));
    let fake = runtime(&plant);
    let mut session = MonitorSession::default();
    let result = session.observe(
        &tank_loop(),
        &request(None, Some("debug-1")),
        &fixed_clock(1_000),
    );
    assert_eq!(reason(&result), "Runtime is not producing scans");
    assert_eq!(result.target_label, None);
    assert_eq!(
        fake.requests(),
        [] as [serde_json::Value; 0],
        "no connection, no runtime request"
    );
}

#[test]
fn an_unreachable_endpoint_is_an_unavailable_sample_with_the_reason() {
    let mut session = MonitorSession::default();
    let connection = RuntimeConnection {
        endpoint: "http://localhost:9".into(),
        token: None,
        runtime_id: None,
    };
    assert_eq!(
        reason(&poll_connection(
            &mut session,
            Some(connection.clone()),
            None,
            1_000
        )),
        AWAITING_SCAN
    );
    let result = poll_connection(&mut session, Some(connection), None, 1_000);
    assert_eq!(
        reason(&result),
        "Control Studio cannot reach the runtime. http://localhost:9 is not a runtime endpoint."
    );
    assert!(
        matches!(&result.sample, LoopMonitorSample::Unavailable(sample) if sample.source == MonitorSource::Physical),
        "an unknown target is physical (A2.4)"
    );
}

#[test]
fn a_status_that_cannot_be_read_leaves_the_target_physical_and_unlabelled_but_still_reads_the_scan()
{
    let plant = Arc::new(Mutex::new(Plant::simulated(
        10,
        Some(10.0),
        10,
        "\u{00D7}10",
    )));
    plant.lock().status = serde_json::Value::Null;
    let fake = runtime(&plant);
    let mut session = MonitorSession::default();
    poll(&mut session, &fake, Some("debug-1"), 1_000);
    let result = poll(&mut session, &fake, Some("debug-1"), 1_000);
    let sample = available(&result);
    assert_eq!(sample.source, MonitorSource::Physical);
    assert_eq!(result.target_label, None);
}

#[test]
fn a_missing_process_variable_is_named_by_its_address_and_scan() {
    let plant = Arc::new(Mutex::new(Plant::simulated(1, None, 10, "")));
    plant.lock().process_variable = None;
    let fake = runtime(&plant);
    let mut session = MonitorSession::default();
    poll(&mut session, &fake, Some("debug-1"), 1_000);
    assert_eq!(
        reason(&poll(&mut session, &fake, Some("debug-1"), 1_000)),
        "Process variable %IL0 is missing from runtime scan 41"
    );
}

#[test]
fn a_refused_status_is_not_a_scan_and_the_observation_says_so() {
    let fake = FakeRuntime::start(|request| match request["type"].as_str() {
        Some("status") => Some(refusal(
            request,
            "invalid_auth_token",
            "The control token is wrong.",
        )),
        _ => None,
    });
    let mut session = MonitorSession::default();
    poll(&mut session, &fake, Some("debug-1"), 1_000);
    let result = poll(&mut session, &fake, Some("debug-1"), 1_000);
    assert!(
        matches!(&result.sample, LoopMonitorSample::Unavailable(_)),
        "a runtime that refuses the status gives no scan"
    );
}

#[test]
fn a_request_for_another_runtime_is_answered_unavailable_before_the_loop_is_read() {
    let slot = Mutex::new(None);
    let arguments = [json!({
        "projectRoot": "/project", "runtimeId": "runtime:local", "loopId": "loop:tank", "editorSessionId": "editor-1",
        "connection": { "endpoint": "tcp://127.0.0.1:9", "runtimeId": "runtime:other" }
    })];
    let reply =
        answer_command_with(&slot, &arguments, &fixed_clock(1_000)).expect("the monitor answers");
    assert_eq!(reply["sample"]["availability"], "unavailable", "{reply}");
    assert_eq!(reply["sample"]["loopId"], "loop:tank");
    assert_eq!(
        reply["sample"]["source"], "Physical",
        "an unknown target is physical (A2.4)"
    );
    assert!(
        reply["sample"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("runtime:other")),
        "the reason names the selected runtime: {reply}"
    );
}

#[test]
fn an_argument_that_is_not_a_monitor_request_is_refused() {
    let slot = Mutex::new(None);
    let failure = answer_command_with(
        &slot,
        &[json!({ "loopId": "loop:tank" })],
        &fixed_clock(1_000),
    )
    .expect_err("a monitor request names its project, runtime, loop and editor session");
    assert_eq!(failure.cause, trust_control::failure::Cause::Implementation);
}
