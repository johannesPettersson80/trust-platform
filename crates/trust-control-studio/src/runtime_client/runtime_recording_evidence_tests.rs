//! The answers and samples of a runtime recording: the applied and requested inputs, the channels, the controller mode, the
//! evidence and its completeness, and the status pages that a running recording is followed through. Split from
//! `runtime_recording_tests.rs` by concept.

use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::loop_definition::{AuxiliaryRole, LoopDefinition, ModeWhenTrue};
use trust_control::recording::channels::{
    applied_mv, binding_forced, binding_value, capture_additional_channels, requested_mv,
    sample_controller_mode, sample_quality, ChannelQuality, RecordedChannel,
};
use trust_control::recording::evidence::RecordingEvidence;
use trust_control::recording::sequence::steady_record_period;
use trust_control::recording::{Completeness, RecordedRun, RecordingState};

use crate::runtime_client::fake_runtime::{answer, FakeRuntime};

use super::super::PendingReleases;
use super::{
    address, fixture_bindings, loop_bindings, owner_with, sample, scope_loop, serving, session_on,
    stopped, symbol, two_scans,
};

/// The scope loop with the quality flag `valid` of channel `primary`, as the `TypeScript` fixture declares it.
fn recording_loop() -> LoopDefinition {
    let mut value = json!(scope_loop());
    value["signals"]["additional"] = json!([{
        "id": "valid", "channel": "primary", "role": "quality", "symbol": "P1.Valid",
        "address": "%MX16.0", "iecType": "BOOL", "direction": "read", "unit": "%",
        "range": { "minimum": 0.0, "maximum": 100.0 }
    }]);
    serde_json::from_value(value).expect("the loop decodes")
}

/// The scope loop with a declared actuator, whose measured position is `%IL8`.
fn actuator_loop() -> LoopDefinition {
    let mut value = json!(scope_loop());
    value["signals"]["actuator"] = json!({
        "symbol": "P1.Stem", "address": "%IL8", "iecType": "LREAL", "direction": "read",
        "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
    });
    serde_json::from_value(value).expect("the loop decodes")
}

/// The recording loop with its controller mode read through the channel `mode`, where TRUE means `when_true`.
fn mode_loop(mode: &str, when_true: &str) -> LoopDefinition {
    let mut value = json!(recording_loop());
    value["controller"]["modeSignal"] = json!(mode);
    value["controller"]["modeWhenTrue"] = json!(when_true);
    serde_json::from_value(value).expect("the loop decodes")
}

#[test]
fn an_unaddressed_symbolic_mode_run_keeps_its_program_revision() {
    let mut bindings = fixture_bindings();
    bindings.push(symbol("controllerMode", "P1.Manual", "BOOL"));
    let owner = owner_with(Some("program"), bindings);
    let fake = serving(stopped(&owner, two_scans(&owner, 1.0)));
    let mut session = session_on(
        &fake,
        &recording_loop(),
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    let run = session.stop().expect("stopped");
    assert_eq!(run.program_revision.as_deref(), Some("program"));
}

#[test]
fn rejects_recordings_whose_identity_or_quality_vector_does_not_match_the_request() {
    let owner = owner_with(Some("program"), {
        let mut bindings = fixture_bindings();
        bindings.push(symbol("controllerMode", "P1.Manual", "BOOL"));
        bindings
    });
    let mutations: [fn(&mut Value); 3] = [
        |run| run["program_revision"] = json!("other"),
        |run| {
            run["bindings"][4] =
                json!({ "id": "controllerMode", "symbol": "Other.Manual", "iec_type": "BOOL" });
        },
        |run| run["samples"][0]["quality"] = json!(["good"]),
    ];
    for mutate in mutations {
        let mut result = stopped(&owner, two_scans(&owner, 1.0));
        mutate(&mut result);
        let fake = serving(result);
        let mut session = session_on(
            &fake,
            &recording_loop(),
            &owner,
            &Arc::new(PendingReleases::new()),
        );
        let failure = session
            .stop()
            .expect_err("the answer does not match the request");
        assert!(
            failure.message.contains("does not match the protocol"),
            "{failure:?}"
        );
    }
}

#[test]
fn a_declared_actuator_is_the_applied_input_and_without_one_the_command_is() {
    let owner = owner_with(None, {
        let mut bindings = loop_bindings();
        bindings.push(address("actuator", "%IL8", "LREAL"));
        bindings.push(address("valid", "%MX16.0", "BOOL"));
        bindings
    });
    let scans = vec![
        sample(
            1,
            0,
            vec![json!(1.0), json!(0.5), json!(2.0), json!(1.5), json!(1.0)],
        ),
        sample(
            2,
            10_000_000,
            vec![json!(1.0), json!(0.5), json!(2.0), json!(1.5), json!(1.0)],
        ),
    ];
    let fake = serving(stopped(&owner, scans));
    let loop_definition = actuator_loop();
    let mut session = session_on(
        &fake,
        &loop_definition,
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    let run = session.stop().expect("stopped");
    assert_eq!(
        applied_mv(&run, &loop_definition, &run.samples[0]),
        Some(1.5),
        "the applied input is what the actuator reached"
    );

    let plain_owner = owner_with(None, fixture_bindings());
    let plain_fake = serving(stopped(&plain_owner, two_scans(&plain_owner, 1.0)));
    let plain_loop = recording_loop();
    let mut plain = session_on(
        &plain_fake,
        &plain_loop,
        &plain_owner,
        &Arc::new(PendingReleases::new()),
    );
    let plain_run = plain.stop().expect("stopped");
    assert_eq!(
        applied_mv(&plain_run, &plain_loop, &plain_run.samples[0]),
        Some(2.0),
        "without actuator feedback the command is the only applied input"
    );
    assert_eq!(
        requested_mv(&run, &loop_definition, &run.samples[0]),
        Some(2.0),
        "the manipulated variable is the requested output"
    );
    assert_eq!(
        requested_mv(&plain_run, &plain_loop, &plain_run.samples[0]),
        None,
        "without a declared actuator there is no separate request"
    );
}

#[test]
fn retains_auxiliary_channel_values_and_their_declared_meaning() {
    let owner = owner_with(None, fixture_bindings());
    let fake = serving(stopped(&owner, two_scans(&owner, 0.0)));
    let loop_definition = recording_loop();
    let mut session = session_on(
        &fake,
        &loop_definition,
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    let run: RecordedRun = session.stop().expect("stopped");
    let scan: &trust_control::recording::RecordedSample = &run.samples[0];
    let channels = capture_additional_channels(
        &loop_definition,
        |id| binding_value(&run, scan, id),
        |id| binding_forced(&run, scan, id),
    );
    assert_eq!(
        channels,
        vec![RecordedChannel {
            id: "valid".into(),
            channel: "primary".into(),
            role: AuxiliaryRole::Quality,
            unit: "%".into(),
            value: Some(0.0),
            quality: ChannelQuality::Bad,
        }]
    );
}

#[test]
fn preserves_a_false_sensor_quality_signal_as_bad_identification_data() {
    let owner = owner_with(None, fixture_bindings());
    let fake = serving(stopped(&owner, two_scans(&owner, 0.0)));
    let loop_definition = recording_loop();
    let mut session = session_on(
        &fake,
        &loop_definition,
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    let run = session.stop().expect("stopped");
    assert_eq!(
        sample_quality(&run, &loop_definition, &run.samples[0]),
        ChannelQuality::Bad,
        "a complete acquisition can still contain bad-quality values"
    );
    assert_eq!(
        session.evidence(&run).completeness,
        Completeness::Complete,
        "bad-quality values do not make a complete acquisition incomplete"
    );
}

#[test]
fn accepts_samples_at_one_steady_record_period_and_refuses_a_missed_execution() {
    assert!(steady_record_period(&[10, 11, 12], false));
    assert!(
        steady_record_period(&[10, 14, 18, 20], true),
        "a stopped recording's final sample may cover fewer executions"
    );
    assert!(
        !steady_record_period(&[10, 14, 18, 20], false),
        "only a stopped recording ends with a partial period"
    );
    assert!(
        !steady_record_period(&[10, 14, 19], true),
        "a final step longer than the period is a missed execution"
    );
    assert!(
        !steady_record_period(&[10, 14, 17, 21], false),
        "a period that changes is a missed execution"
    );
    assert!(
        !steady_record_period(&[10, 14], true),
        "two samples cannot establish a period, so they must be consecutive"
    );
    assert!(steady_record_period(&[10, 11], true));
}

/// The evidence of a stopped recording whose samples carry the sequence values of `sequence`: one execution is 10 ms, and
/// every sample reads the three loop signals (the `stopWith` of `control-studio-lifecycle.test.ts`).
fn evidence_of_sequence(sequence: &[u64]) -> RecordingEvidence {
    let owner = owner_with(None, loop_bindings());
    let samples: Vec<Value> = sequence
        .iter()
        .map(|&scan| {
            let runtime_time_ns = i64::try_from(scan * 10_000_000).expect("a runtime time");
            json!({
                "scan": scan, "runtime_time_ns": runtime_time_ns, "captured_at_ms": 1000 + scan,
                "values": [1.0, 0.5, 2.0], "forced": [false, false, false],
                "quality": ["good", "good", "good"]
            })
        })
        .collect();
    let mut answer = stopped(&owner, samples);
    answer["acquisition"]["observedSampleCount"] = json!(sequence.len());
    answer["acquisition"]["scanRange"] = json!({
        "first": sequence[0], "last": sequence[sequence.len() - 1]
    });
    let fake = serving(answer);
    let mut session = session_on(
        &fake,
        &scope_loop(),
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    let run = session.stop().expect("stopped");
    session.evidence(&run)
}

#[test]
fn a_recording_at_record_period_ten_is_complete_and_a_missed_execution_is_a_gap() {
    // The TypeScript numbers of `control-studio-lifecycle.test.ts`: sequence values at record period 10.
    let steady = evidence_of_sequence(&[10, 20, 30, 40]);
    assert_eq!(
        steady.completeness,
        Completeness::Complete,
        "every sample covers ten executions"
    );
    let partial = evidence_of_sequence(&[10, 20, 30, 35]);
    assert_eq!(
        partial.completeness,
        Completeness::Complete,
        "the final sample may cover fewer executions"
    );
    let missed = evidence_of_sequence(&[10, 20, 35, 40]);
    assert_eq!(
        missed.completeness,
        Completeness::Incomplete,
        "a missed execution inside the record period is a gap"
    );
    assert!(missed.reason.contains("record period"), "{missed:?}");
}

#[test]
fn follows_a_running_recording_through_the_status_pages_which_carry_no_loop_revision() {
    let owner = owner_with(None, fixture_bindings());
    let scans = two_scans(&owner, 1.0);
    let fake = FakeRuntime::start(move |request| {
        match request["type"].as_str().unwrap_or_default() {
            "control.recording.status" => {
                let from = usize::try_from(request["params"]["from_sample"].as_u64().unwrap_or(0))
                    .unwrap_or(usize::MAX);
                let page: Vec<Value> = scans.iter().skip(from).cloned().collect();
                Some(answer(
                    request,
                    json!({
                        "recording_id": "record", "state": "recording", "elapsed_ms": 10, "sample_count": 2,
                        "complete": false, "reason": null, "samples": page, "next_sample": 2
                    }),
                ))
            }
            _ => None,
        }
    });
    let mut session = session_on(
        &fake,
        &scope_loop(),
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    let first = session.follow().expect("first page");
    assert_eq!(first.state, RecordingState::Recording);
    assert_eq!(
        (first.sample_count, first.samples.len(), first.next_sample),
        (2, 2, 2)
    );
    assert_eq!(
        first
            .samples
            .iter()
            .map(|sample| sample.scan)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let next = session.follow().expect("next page");
    assert!(
        next.samples.is_empty(),
        "the next page continues after the samples already read"
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
            (json!("sha256:loop"), json!(0)),
            (json!("sha256:loop"), json!(2)),
        ],
        "each request names the loop revision, which the runtime checks"
    );
}

#[test]
fn resolves_a_symbolic_controller_mode_through_its_declared_bool_channel() {
    // The fixture of the TypeScript test: the mode is the quality flag `valid` at %MX16.0, and TRUE means Auto.
    let loop_definition = mode_loop("P1.Valid", "Auto");
    let owner = owner_with(None, fixture_bindings());
    let fake = serving(stopped(&owner, two_scans(&owner, 1.0)));
    let mut session = session_on(
        &fake,
        &loop_definition,
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    let run = session.stop().expect("stopped");
    assert_eq!(
        sample_controller_mode(&run, &loop_definition, &run.samples[0]),
        Some(ModeWhenTrue::Auto)
    );
}

#[test]
fn decodes_an_unaddressed_symbolic_bool_mode_and_reads_a_bad_quality_mode_as_unavailable() {
    let mut bindings = fixture_bindings();
    bindings.push(symbol("controllerMode", "P1.Manual", "BOOL"));
    let owner = owner_with(Some("program"), bindings);
    let loop_definition = mode_loop("P1.Manual", "Manual");
    let mut result = stopped(&owner, two_scans(&owner, 1.0));
    // The TypeScript mutation: the mode channel of the second sample is bad.
    result["samples"][1]["quality"][4] = json!("bad");
    let fake = serving(result);
    let mut session = session_on(
        &fake,
        &loop_definition,
        &owner,
        &Arc::new(PendingReleases::new()),
    );
    let run = session.stop().expect("stopped");
    assert_eq!(run.program_revision.as_deref(), Some("program"));
    assert_eq!(
        sample_controller_mode(&run, &loop_definition, &run.samples[0]),
        Some(ModeWhenTrue::Manual)
    );
    assert_eq!(
        sample_controller_mode(&run, &loop_definition, &run.samples[1]),
        None,
        "a numeric mode value with bad quality cannot establish the current controller mode"
    );
    assert_eq!(
        sample_quality(&run, &loop_definition, &run.samples[1]),
        ChannelQuality::Bad
    );
}

#[test]
fn rejects_malformed_symbolic_identities_quality_vectors_and_bool_values() {
    let mut bindings = fixture_bindings();
    bindings.push(symbol("controllerMode", "P1.Manual", "BOOL"));
    let owner = owner_with(Some("program"), bindings);
    let mutations: [fn(&mut Value); 4] = [
        |run| run["program_revision"] = json!("other"),
        |run| {
            run["bindings"][4] =
                json!({ "id": "controllerMode", "symbol": "Other.Manual", "iec_type": "BOOL" });
        },
        |run| run["samples"][0]["quality"] = json!(["good"]),
        |run| run["samples"][0]["values"][4] = json!(2.0),
    ];
    for mutate in mutations {
        let mut result = stopped(&owner, two_scans(&owner, 1.0));
        mutate(&mut result);
        let fake = serving(result);
        let mut session = session_on(
            &fake,
            &recording_loop(),
            &owner,
            &Arc::new(PendingReleases::new()),
        );
        let failure = session.stop().expect_err("the answer is refused");
        let words = failure.message.to_lowercase();
        assert!(
            words.contains("recording") || words.contains("samples") || words.contains("bool"),
            "{failure:?}"
        );
    }
}

#[test]
fn rejects_contradictory_complete_acquisition_claims() {
    let owner = owner_with(None, fixture_bindings());
    let claims: [fn(&mut Value); 2] = [
        |run| run["acquisition"]["missingScanRanges"] = json!([{ "first": 1, "last": 1 }]),
        |run| run["acquisition"]["actualCycleTimingAvailable"] = json!(false),
    ];
    for mutate in claims {
        let mut result = stopped(&owner, two_scans(&owner, 1.0));
        mutate(&mut result);
        let fake = serving(result);
        let mut session = session_on(
            &fake,
            &recording_loop(),
            &owner,
            &Arc::new(PendingReleases::new()),
        );
        let failure = session
            .stop()
            .expect_err("a complete claim that the answer contradicts is refused");
        assert!(failure.message.contains("acquisition"), "{failure:?}");
    }
}
