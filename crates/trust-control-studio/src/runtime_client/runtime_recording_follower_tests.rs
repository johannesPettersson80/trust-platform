//! The production `RecordingFollower`: the session's status pages as an identification campaign reads them. The rule that
//! turns a page into samples is `trust_control::recording::channels::follow_samples`, which is tested there; these tests prove
//! the wiring: the pages are read from the runtime in order, with the session's loop and bindings, and every page counts from
//! the origin of the first. The runtime is a fake that answers as annex E4.6 names each answer.

use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::experiment::{ControlEvidenceSource, RecordingRecoveryOwner};
use trust_control::identification::campaign::FollowSample;
use trust_control::signals::loop_inspection_bindings;

use super::{recovery_binding, PendingReleases, RuntimeRecordingSession};
use crate::runtime_client::fake_runtime::{answer, refusal, tank_loop, FakeRuntime};
use crate::workflow::identify_campaign::RecordingFollower;

const RECORDING_ID: &str = "recording-1";

/// Scan `index` of the recording that the fake serves: 10 ms apart from 5 s of runtime time, with a setpoint of 1, a
/// measurement of 0.5 and a request of 2, all of good quality. The fourth scan has lost its measurement: the value is null
/// and its quality is bad (E4.4).
fn scan(index: u64) -> Value {
    let lost = index == 3;
    let measurement = if lost { Value::Null } else { json!(0.5) };
    let quality = if lost { "bad" } else { "good" };
    json!({
        "scan": index + 1,
        "runtime_time_ns": 5_000_000_000_u64 + index * 10_000_000,
        "captured_at_ms": 1000,
        "values": [1.0, measurement, 2.0],
        "forced": [false, false, false],
        "quality": ["good", quality, "good"]
    })
}

/// A fake runtime that serves a recording of `total` scans two at a time, from the index that a status request names; with
/// no index it serves the end of the recording (E4.6).
fn serving(total: u64) -> FakeRuntime {
    FakeRuntime::start(move |request| {
        let from = request["params"]["from_sample"].as_u64().unwrap_or(total);
        let next = (from + 2).min(total);
        let samples: Vec<Value> = (from..next).map(scan).collect();
        Some(answer(
            request,
            json!({
                "recording_id": RECORDING_ID, "state": "recording", "elapsed_ms": 30,
                "sample_count": total, "complete": false, "reason": null,
                "samples": samples, "next_sample": next
            }),
        ))
    })
}

/// The session of the tank loop's recording on `fake`, rebuilt from its owner as a reattached follow rebuilds it. Its
/// bindings are the loop's inspection bindings, as a started session has them.
fn session_on(fake: &FakeRuntime) -> RuntimeRecordingSession {
    let loop_definition = tank_loop();
    let owner = RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: RECORDING_ID.to_string(),
        project_id: loop_definition.project_id.clone(),
        runtime_id: loop_definition.runtime_id.clone(),
        loop_id: loop_definition.id.clone(),
        loop_revision: loop_definition.revision.clone(),
        source: ControlEvidenceSource::Simulation,
        program_revision: None,
        start_acknowledgement_unconfirmed: None,
        bindings: loop_inspection_bindings(&loop_definition)
            .iter()
            .map(recovery_binding)
            .collect(),
    };
    RuntimeRecordingSession::from_recovery_owner(
        &fake.connection(None),
        &loop_definition,
        &owner,
        &Arc::new(PendingReleases::new()),
    )
    .expect("the owner belongs to the loop")
}

fn assert_followed(followed: &FollowSample, seconds: f64, good: bool, output: Option<f64>) {
    assert!(
        (followed.elapsed_seconds - seconds).abs() <= 1e-9,
        "{followed:?} is not at {seconds} s"
    );
    assert_eq!(followed.good, good, "{followed:?}");
    assert_eq!(followed.output.is_some(), output.is_some(), "{followed:?}");
    if let (Some(actual), Some(expected)) = (followed.output, output) {
        assert!((actual - expected).abs() <= 1e-9, "{followed:?}");
    }
}

#[test]
fn the_session_is_followed_page_by_page_and_every_page_counts_from_the_first_sample() {
    let fake = serving(4);
    let mut session = session_on(&fake);

    let first = RecordingFollower::follow(&mut session).expect("the first page");
    assert_eq!(first.len(), 2);
    assert_followed(&first[0], 0.0, true, Some(0.5));
    assert_followed(&first[1], 0.01, true, Some(0.5));

    // The second page does not start again from zero: its scans are 20 and 30 ms after the first sample.
    let second = RecordingFollower::follow(&mut session).expect("the second page");
    assert_eq!(second.len(), 2);
    assert_followed(&second[0], 0.02, true, Some(0.5));
    assert_followed(&second[1], 0.03, false, None);

    let third = RecordingFollower::follow(&mut session).expect("the third page");
    assert_eq!(
        third,
        Vec::new(),
        "the recording has no sample after the last one read"
    );

    let requests: Vec<(Value, Value)> = fake
        .requests_of("control.recording.status")
        .iter()
        .map(|request| {
            (
                request["params"]["loop_revision"].clone(),
                request["params"]["from_sample"].clone(),
            )
        })
        .collect();
    assert_eq!(
        requests,
        vec![
            (json!("sha256:loop-1"), json!(0)),
            (json!("sha256:loop-1"), json!(2)),
            (json!("sha256:loop-1"), json!(4)),
        ],
        "each request names the loop revision and continues after the samples already read"
    );
}

#[test]
fn a_refused_status_request_is_the_failure_of_the_follow() {
    let fake = FakeRuntime::start(|request| {
        Some(refusal(
            request,
            "recording_unavailable",
            "No recording holds the identifier recording-1.",
        ))
    });
    let mut session = session_on(&fake);
    let failure = RecordingFollower::follow(&mut session).expect_err("the runtime refused");
    assert_eq!(
        failure.message,
        "No recording holds the identifier recording-1."
    );
}
