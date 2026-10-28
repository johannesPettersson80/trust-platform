//! Port of `control-studio-runtime-recording.test.ts`: the stop, release and recovered-owner tests. The fake runtime and the
//! fixtures here are shared with the sibling modules `runtime_recording_start_tests.rs` (the start rules of `runtimeRecording.ts`:
//! journaling, refusals, limits, identifiers and the release swept by the next start) and `runtime_recording_evidence_tests.rs`
//! (the workflow sample, evidence, the record-period sequence and the acquisition claims). The fake runtime answers as annex E4
//! names each answer.

use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{json, Value};
use trust_control::experiment::{
    ControlEvidenceSource, RecordingRecoveryBinding, RecordingRecoveryOwner,
};
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::Completeness;

use crate::runtime_client::fake_runtime::{answer, tank_loop, FakeRuntime};

use super::*;

#[cfg(test)]
#[path = "runtime_recording_start_tests.rs"]
mod start;

#[cfg(test)]
#[path = "runtime_recording_evidence_tests.rs"]
mod evidence;

/// The loop of the recording tests: the tank, in the scope of the owners below (`project`, `simulator`, `loop`).
fn scope_loop() -> LoopDefinition {
    let mut value = json!(tank_loop());
    value["id"] = json!("loop");
    value["projectId"] = json!("project");
    value["runtimeId"] = json!("simulator");
    value["revision"] = json!("sha256:loop");
    serde_json::from_value(value).expect("the loop decodes")
}

fn address(id: &str, address: &str, iec_type: &str) -> RecordingRecoveryBinding {
    RecordingRecoveryBinding {
        id: id.into(),
        address: Some(address.into()),
        symbol: None,
        iec_type: iec_type.into(),
    }
}

fn symbol(id: &str, symbol: &str, iec_type: &str) -> RecordingRecoveryBinding {
    RecordingRecoveryBinding {
        id: id.into(),
        address: None,
        symbol: Some(symbol.into()),
        iec_type: iec_type.into(),
    }
}

/// The three signals `loop_inspection_bindings` reads for the scope loop.
fn loop_bindings() -> Vec<RecordingRecoveryBinding> {
    vec![
        address("setpoint", "%ML0", "LREAL"),
        address("processVariable", "%IL0", "LREAL"),
        address("manipulatedVariable", "%QL0", "LREAL"),
    ]
}

/// The four signals of the `TypeScript` fixture: the three loop signals and the quality flag `valid`.
fn fixture_bindings() -> Vec<RecordingRecoveryBinding> {
    let mut bindings = loop_bindings();
    bindings.push(address("valid", "%MX16.0", "BOOL"));
    bindings
}

fn owner_with(
    program: Option<&str>,
    bindings: Vec<RecordingRecoveryBinding>,
) -> RecordingRecoveryOwner {
    RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: "record".into(),
        project_id: "project".into(),
        runtime_id: "simulator".into(),
        loop_id: "loop".into(),
        loop_revision: "sha256:loop".into(),
        source: ControlEvidenceSource::Simulation,
        program_revision: program.map(str::to_string),
        start_acknowledgement_unconfirmed: None,
        bindings,
    }
}

/// The bindings as the runtime's stop answer echoes them (E4.7).
fn bindings_json(bindings: &[RecordingRecoveryBinding]) -> Value {
    Value::Array(
        bindings
            .iter()
            .map(|binding| {
                let mut value = json!({ "id": binding.id, "iec_type": binding.iec_type });
                if let Some(address) = &binding.address {
                    value["address"] = json!(address);
                }
                if let Some(symbol) = &binding.symbol {
                    value["symbol"] = json!(symbol);
                }
                value
            })
            .collect(),
    )
}

/// One sample of a recording (E4.4): its values, none forced, all good.
fn sample(scan: u64, runtime_time_ns: i64, values: Vec<Value>) -> Value {
    let count = values.len();
    let mut sample = json!({
        "scan": scan, "runtime_time_ns": runtime_time_ns, "captured_at_ms": 1000,
        "forced": vec![false; count], "quality": vec!["good"; count]
    });
    sample["values"] = Value::Array(values);
    sample
}

/// The acquisition of a complete recording of two scans (E4.5).
fn complete_acquisition() -> Value {
    json!({
        "method": "runtime_scan_buffer", "completeness": "complete", "observedSampleCount": 2,
        "scanRange": { "first": 1, "last": 2 }, "missingScanRanges": [], "droppedObservedSamples": 0,
        "actualCycleTimingAvailable": true, "freshnessAvailable": false, "reason": "Captured every scan."
    })
}

/// The stop answer of the recording `owner` names (E4.7).
fn stopped(owner: &RecordingRecoveryOwner, samples: Vec<Value>) -> Value {
    let mut stopped = json!({
        "recording_id": owner.recording_id, "loop_revision": owner.loop_revision,
        "program_revision": owner.program_revision, "bindings": bindings_json(&owner.bindings),
        "stop_reason": "stopped", "acquisition": complete_acquisition()
    });
    stopped["samples"] = Value::Array(samples);
    stopped
}

/// The two scans of `owner`: the values of the `TypeScript` fixture, with `valid` as the given flag.
fn two_scans(owner: &RecordingRecoveryOwner, valid: f64) -> Vec<Value> {
    let values = |a: f64, b: f64, c: f64| -> Vec<Value> {
        let mut values = vec![json!(a), json!(b), json!(c)];
        if owner.bindings.len() > 3 {
            values.push(json!(valid));
        }
        if owner.bindings.len() > 4 {
            values.push(json!(1.0));
        }
        values
    };
    vec![
        sample(1, 0, values(1.0, 0.5, 2.0)),
        sample(2, 10_000_000, values(1.0, 0.5, 2.0)),
    ]
}

/// A runtime that stops with `stopped`, and releases a recording as E4.7 names the answer.
fn serving(stopped: Value) -> FakeRuntime {
    FakeRuntime::start(
        move |request| match request["type"].as_str().unwrap_or_default() {
            "control.recording.stop" => Some(answer(request, stopped.clone())),
            "control.recording.release" => Some(answer(
                request,
                json!({ "recording_id": "record", "status": "released" }),
            )),
            _ => None,
        },
    )
}

/// The session of `owner` on `fake`, rebuilt from the owner as Stop reconstructs it.
fn session_on(
    fake: &FakeRuntime,
    loop_definition: &LoopDefinition,
    owner: &RecordingRecoveryOwner,
    pending: &Arc<PendingReleases>,
) -> RuntimeRecordingSession {
    RuntimeRecordingSession::from_recovery_owner(
        &fake.connection(None),
        loop_definition,
        owner,
        pending,
    )
    .expect("the owner belongs to the loop")
}

#[test]
fn retains_the_original_native_recording_identity_after_stop_and_release() {
    let owner = owner_with(None, fixture_bindings());
    let fake = serving(stopped(&owner, two_scans(&owner, 1.0)));
    let pending = Arc::new(PendingReleases::new());
    let mut session = session_on(&fake, &scope_loop(), &owner, &pending);
    let run = session.stop().expect("stopped");
    assert_eq!(
        run.recording_id, "record",
        "the run names the native recording"
    );
    assert_eq!(
        session.stop().expect("stopped again"),
        run,
        "a repeated stop returns the same run"
    );
    assert_eq!(
        fake.requests_of("control.recording.stop").len(),
        1,
        "a repeated stop is answered from the first one"
    );
    session.release().expect("released");
    assert_eq!(
        pending.waiting_on(&fake.connection(None).endpoint),
        [] as [PendingRelease; 0]
    );
}

#[test]
fn a_lost_release_reply_cannot_discard_an_already_received_recording() {
    let owner = owner_with(None, fixture_bindings());
    let releases = Arc::new(AtomicUsize::new(0));
    let stopped_result = stopped(&owner, two_scans(&owner, 1.0));
    let counter = Arc::clone(&releases);
    let fake =
        FakeRuntime::start(
            move |request| match request["type"].as_str().unwrap_or_default() {
                "control.recording.stop" => Some(answer(request, stopped_result.clone())),
                "control.recording.release" if counter.fetch_add(1, Ordering::SeqCst) == 0 => None,
                "control.recording.release" => Some(answer(
                    request,
                    json!({ "recording_id": "record", "status": "released" }),
                )),
                _ => None,
            },
        );
    let pending = Arc::new(PendingReleases::new());
    let mut session = session_on(&fake, &scope_loop(), &owner, &pending);
    assert_eq!(session.stop().expect("stopped").samples.len(), 2);
    session
        .release()
        .expect("the lost reply is not an error of the release");
    assert_eq!(
        pending.waiting_on(&fake.connection(None).endpoint).len(),
        1,
        "the stopped buffer stays pending"
    );
    assert_eq!(session.stop().expect("stopped again").samples.len(), 2);
    session.release().expect("the retry is acknowledged");
    assert_eq!(
        pending.waiting_on(&fake.connection(None).endpoint),
        [] as [PendingRelease; 0]
    );
    assert_eq!(fake.requests_of("control.recording.release").len(), 2);
}

#[test]
fn a_stop_alone_sends_no_release_and_the_release_call_sends_one() {
    let owner = owner_with(None, fixture_bindings());
    let fake = serving(stopped(&owner, two_scans(&owner, 1.0)));
    let pending = Arc::new(PendingReleases::new());
    let mut session = session_on(&fake, &scope_loop(), &owner, &pending);
    assert_eq!(session.stop().expect("stopped").samples.len(), 2);
    assert!(
        fake.requests_of("control.recording.release").is_empty(),
        "stop alone never frees the buffer"
    );
    assert_eq!(
        pending.waiting_on(&fake.connection(None).endpoint).len(),
        1,
        "the stopped buffer waits for its release"
    );
    session.release().expect("released");
    assert_eq!(
        fake.requests_of("control.recording.release").len(),
        1,
        "the release is sent only when the caller releases the recording"
    );
}

#[test]
fn a_release_is_refused_before_the_recording_is_stopped() {
    let owner = owner_with(None, fixture_bindings());
    let fake = serving(stopped(&owner, two_scans(&owner, 1.0)));
    let mut session = session_on(
        &fake,
        &scope_loop(),
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    let Err(rejected) = session.release() else {
        panic!("a release before the stop is refused");
    };
    assert_eq!(rejected.message, "Stop the recording before releasing it.");
    assert!(
        fake.requests_of("control.recording.release").is_empty(),
        "no release request is sent for a recording that was not stopped"
    );
}

#[test]
fn exposes_an_immutable_credential_free_owner_for_extension_lifetime_recovery() {
    let owner = owner_with(Some("program"), {
        let mut bindings = fixture_bindings();
        bindings.push(symbol("controllerMode", "P1.Manual", "BOOL"));
        bindings
    });
    let fake = serving(stopped(&owner, two_scans(&owner, 1.0)));
    let session = session_on(
        &fake,
        &scope_loop(),
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    assert_eq!(
        session.recovery_owner(),
        owner,
        "the owner is the recording's identity"
    );

    let mut copy = session.recovery_owner();
    copy.bindings[0].id = "mutated".into();
    assert_eq!(
        session.recovery_owner().bindings[0].id,
        "setpoint",
        "the persisted copy does not change the live session"
    );
    let text = serde_json::to_string(&owner).expect("the owner encodes");
    assert!(
        !text.contains("endpoint") && !text.contains("token") && !text.contains("authorization")
    );
}

#[test]
fn reconstructs_stop_only_for_the_persisted_recording_owner() {
    let owner = owner_with(None, fixture_bindings());
    let fake = serving(stopped(&owner, two_scans(&owner, 1.0)));
    let pending = Arc::new(PendingReleases::new());
    let mut session = session_on(&fake, &scope_loop(), &owner, &pending);
    assert_eq!(session.stop().expect("stopped").samples.len(), 2);
    let stops = fake.requests_of("control.recording.stop");
    assert_eq!(
        stops[0]["params"],
        json!({ "recording_id": "record", "loop_revision": "sha256:loop" })
    );

    // An owner of another loop revision is refused before any request is sent ("does not match the active loop scope").
    let mut revised = scope_loop();
    revised.revision = "sha256:changed".into();
    let refusal = RuntimeRecordingSession::from_recovery_owner(
        &fake.connection(None),
        &revised,
        &owner,
        &pending,
    )
    .err()
    .expect("an owner outside the scope of the loop is refused");
    assert!(
        refusal
            .message
            .contains("does not match the active loop scope"),
        "{refusal:?}"
    );
    assert_eq!(
        fake.requests_of("control.recording.stop").len(),
        1,
        "the refused owner sent no request of its own"
    );
}

#[test]
fn a_lost_start_acknowledgement_stays_on_the_recovered_owner_and_the_samples_are_retained() {
    let mut owner = owner_with(None, fixture_bindings());
    owner.start_acknowledgement_unconfirmed = Some(true);
    let fake = serving(stopped(&owner, two_scans(&owner, 1.0)));
    let mut session = session_on(
        &fake,
        &scope_loop(),
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    assert_eq!(
        session.recovery_owner().start_acknowledgement_unconfirmed,
        Some(true)
    );
    let run = session.stop().expect("stopped");
    assert_eq!(
        run.samples.len(),
        2,
        "the same owner still retains the runtime's actual samples"
    );
    let evidence = session.evidence(&run);
    assert_eq!(
        evidence.completeness,
        Completeness::Incomplete,
        "a recovered owner cannot upgrade an unacknowledged start to complete evidence (E11.4)"
    );
    assert!(
        evidence
            .reason
            .contains("Start acknowledgement unconfirmed"),
        "{}",
        evidence.reason
    );
    assert!(
        evidence.reason.contains("Extension Host reload"),
        "{}",
        evidence.reason
    );
}

#[test]
fn a_lost_start_acknowledgement_stops_the_recording_its_owner_names() {
    let mut owner = owner_with(None, fixture_bindings());
    owner.start_acknowledgement_unconfirmed = Some(true);
    let fake = serving(stopped(&owner, two_scans(&owner, 1.0)));
    let mut session = session_on(
        &fake,
        &scope_loop(),
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    assert_eq!(session.stop().expect("stopped").samples.len(), 2);
    let stops = fake.requests_of("control.recording.stop");
    assert_eq!(stops.len(), 1, "the recovered owner sends one stop");
    assert_eq!(
        stops[0]["params"],
        json!({ "recording_id": "record", "loop_revision": "sha256:loop" }),
        "an unconfirmed start is stopped on its own identifier, never replaced (E11.4)"
    );
}

#[test]
fn an_acknowledged_recovered_owner_stops_its_recording() {
    let owner = owner_with(None, fixture_bindings());
    let fake = serving(stopped(&owner, two_scans(&owner, 1.0)));
    let mut session = session_on(
        &fake,
        &scope_loop(),
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    let run = session.stop().expect("stopped");
    assert_eq!(run.samples.len(), 2);
    let evidence = session.evidence(&run);
    assert_eq!(
        evidence.completeness,
        Completeness::Complete,
        "an acknowledged recovered owner keeps its complete evidence"
    );
    assert!(
        !evidence
            .reason
            .contains("Start acknowledgement unconfirmed"),
        "{}",
        evidence.reason
    );
    assert_eq!(
        session.recovery_owner().start_acknowledgement_unconfirmed,
        None
    );
}
