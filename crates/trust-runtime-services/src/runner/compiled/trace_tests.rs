//! A compiled program's cycle traces through a device's loop (specification 48 RU-59, "Every
//! runtime"; specification 34 STATE-08: a board works exactly like trust-runtime): a scan that the
//! debugger held ends a recording `paused` with the cycles before it, and a scan that faults is the
//! recording's last cycle, with the kernel's fault record. The programs are the stand-in debug
//! bodies of the cycle's tests, whose statements 0 and 1, of POU 0, are on lines 1 and 2 of
//! `main.st`.

use alloc::string::String;
use alloc::vec::Vec;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value as Json};
use trust_runtime_core::deployment::ControlMode;
use trust_runtime_core::native::KERNEL_FUNCTIONS;
use trust_runtime_core::trace::{decode, Change, ChangeKind, ChangeSource, CycleFault, TraceEnd};

use super::tests::{cycle_with, image_of, Cycle};
use crate::runner::Runner;
use crate::tests::runner_platform::TestPlatform;
use crate::tests::runner_ports::{ms, TestClock, TestLink, TestWait};

type Loop<'i> = Runner<TestLink, TestWait, Cycle<'i>, TestPlatform, TestClock>;

/// The loop of the image `bytes` on `platform`, its link and its wait, after its first pass, which
/// runs the scan due at power-on, and a recording of 5 cycles that starts after that scan.
fn recording(bytes: &[u8], platform: TestPlatform) -> (Loop<'_>, TestLink, TestWait) {
    let clock = TestClock::default();
    let link = TestLink::new(4_096, clock.clone());
    let wait = TestWait::new(clock.clone());
    let (cycle, _, _) = cycle_with(bytes, Vec::new(), &KERNEL_FUNCTIONS);
    let mut runner = Runner::new(link.clone(), wait.clone(), cycle, platform, clock, 64);
    runner.run_pass().expect("the test wait does not fail");
    link.send(&json!({"id": 1, "type": "trace.record", "params": {"cycles": 5}}));
    runner.run_pass().expect("the test wait does not fail");
    let started = link.take_replies();
    assert_eq!(started[0]["result"]["trace"], json!(1), "{started:?}");
    wait.advance_time();
    (runner, link, wait)
}

/// Sends each request at its time, in milliseconds, and runs passes until the last one is
/// answered; returns the replies by their request's number. The loop runs no more passes than it
/// needs, so that no scan after the last request runs.
fn exchange(
    runner: &mut Loop<'_>,
    link: &TestLink,
    wait: &TestWait,
    requests: Vec<(u64, Json)>,
) -> Vec<Json> {
    let last = requests.last().map(|(_, request)| request["id"].clone());
    for (at, request) in requests {
        let link = link.clone();
        wait.at(ms(at), move || link.send(&request));
    }
    let mut replies = Vec::new();
    for _ in 0..12 {
        runner.run_pass().expect("the test wait does not fail");
        replies.extend(link.take_replies());
        if replies
            .iter()
            .any(|reply| Some(&reply["id"]) == last.as_ref())
        {
            return replies;
        }
    }
    panic!("no reply to the last request: {replies:?}");
}

/// Sends `request` now and runs passes until it is answered; returns the reply. A faulted loop
/// takes it at its next poll.
fn ask(runner: &mut Loop<'_>, link: &TestLink, request: &Json) -> Json {
    link.send(request);
    for _ in 0..12 {
        runner.run_pass().expect("the test wait does not fail");
        if let Some(answer) = link
            .take_replies()
            .into_iter()
            .find(|reply| reply["id"] == request["id"])
        {
            return answer;
        }
    }
    panic!("no reply to {request}");
}

/// The reply to request `id`.
fn reply(replies: &[Json], id: u64) -> &Json {
    replies
        .iter()
        .find(|reply| reply["id"] == id)
        .unwrap_or_else(|| panic!("a reply to request {id}: {replies:?}"))
}

#[test]
fn a_scan_the_debugger_holds_ends_a_recording_paused_with_the_cycles_before() {
    // RU-59: a scan that the debugger held is no cycle a replay could run, so it ends a recording
    // `paused` with the cycles before it, on a chip as on a computer. Worked by hand on the debug
    // body that counts: the recording records the scan at 10 ms; the breakpoint on line 2, set at
    // 12 ms, holds the scan at 20 ms until `resume` at 26 ms; `trace.read` at 27 ms finds the trace
    // ended `paused` with that 1 cycle. Catches a held scan recorded as a cycle (2 cycles, still
    // recording) and a hold that does not end the recording. The breakpoint stays armed, so no
    // pass may reach the scan at 30 ms, which would hold again.
    let bytes = image_of(48, true);
    let platform = TestPlatform {
        debug_enabled: true,
        control_mode: ControlMode::Debug,
        ..TestPlatform::default()
    };
    let (mut runner, link, wait) = recording(&bytes, platform);
    let replies = exchange(
        &mut runner,
        &link,
        &wait,
        alloc::vec![
            (
                12,
                json!({"id": 2, "type": "breakpoints.set", "params": {"source": "main.st", "lines": [1]}}),
            ),
            (26, json!({"id": 3, "type": "resume"})),
            (
                27,
                json!({"id": 4, "type": "trace.read", "params": {"trace": 1}}),
            ),
        ],
    );
    assert_eq!(reply(&replies, 2)["ok"], json!(true), "{replies:?}");
    assert_eq!(reply(&replies, 3)["ok"], json!(true), "{replies:?}");
    let state = &reply(&replies, 4)["result"];
    assert_eq!(state["state"], json!("ended"), "{replies:?}");
    assert_eq!(state["reason"], json!("paused"), "{replies:?}");
    assert_eq!(state["cycles"], json!(1), "{replies:?}");
}

#[test]
fn a_faulted_scan_is_a_recordings_last_cycle_with_its_fault() {
    // RU-59: a scan that faults is the recording's last cycle, with the kernel's record of the
    // fault (specification 44 §8.6), and the recording ends `faulted`, on a chip as on a computer.
    // Worked by hand on the debug body that divides by zero in its second statement once its count
    // is above 100: the recording records the scan at 10 ms; `set` of `MAIN.COUNT` to 150 at
    // 12 ms reaches the scan at 20 ms, which counts to 151 and faults in statement 1. The trace ends
    // `faulted` with 2 cycles: the first without a fault; the second with the write from the
    // control link, the byte 150 under the name the image gives its one type (string 0 of its
    // pool, the empty string), and the fault of code 1 (a division by zero) in POU 0, statement 1.
    // Catches a faulted scan not recorded (1 cycle), recorded without its fault or at another
    // statement, and a recording that the fault does not end.
    let bytes = image_of(40, true);
    let (mut runner, link, wait) = recording(&bytes, TestPlatform::default());
    let replies = exchange(
        &mut runner,
        &link,
        &wait,
        alloc::vec![
            (
                12,
                json!({"id": 2, "type": "set", "params": {"target": "global:MAIN.COUNT", "value": "150"}}),
            ),
            (
                25,
                json!({"id": 3, "type": "trace.read", "params": {"trace": 1}}),
            ),
        ],
    );
    assert_eq!(reply(&replies, 2)["ok"], json!(true), "{replies:?}");
    let state = &reply(&replies, 3)["result"];
    assert_eq!(state["state"], json!("ended"), "{replies:?}");
    assert_eq!(state["reason"], json!("faulted"), "{replies:?}");
    assert_eq!(state["cycles"], json!(2), "{replies:?}");

    let bytes = usize::try_from(state["bytes"].as_u64().expect("an ended trace's bytes"))
        .expect("a trace of a few kilobytes");
    let mut file = Vec::new();
    while file.len() < bytes {
        let part = ask(
            &mut runner,
            &link,
            &json!({"id": 10, "type": "trace.read", "params": {"trace": 1, "offset": file.len()}}),
        );
        let data = part["result"]["data"].as_str().map_or_else(
            || panic!("a part: {part}"),
            |data| STANDARD.decode(data).expect("a part's data is base64"),
        );
        assert!(!data.is_empty(), "a part short of the file's end: {part}");
        file.extend_from_slice(&data);
    }
    let trace = decode(&file).expect("the served file decodes").trace;
    assert_eq!(trace.meta.end, TraceEnd::Faulted);
    assert_eq!(trace.cycles.len(), 2);
    assert_eq!(trace.cycles[0].fault, None);
    assert_eq!(
        trace.cycles[1].changes,
        [Change {
            source: ChangeSource::ControlLink,
            kind: ChangeKind::VariableWrite {
                path: "MAIN.COUNT".into(),
                type_name: String::new(),
                value: alloc::vec![150],
            },
        }]
    );
    assert_eq!(
        trace.cycles[1].fault,
        Some(CycleFault {
            code: 1,
            pou: 0,
            statement: 1
        })
    );
}
