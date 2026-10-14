//! Annex E4 and specification 35, A7 and A8.3: recording requests, recorded runs and their files.

use serde_json::json;

use super::evidence::{recording_evidence, RecordingEvidence, UNCONFIRMED_START_RECOVERED};
use super::*;
use crate::limits::STUDY_STORAGE_LIMIT_BYTES;

fn request() -> RecordingRequest {
    RecordingRequest {
        recording_id: "recording-1".into(),
        loop_revision: "sha256:loop".into(),
        program_revision: Some("sha256:program".into()),
        bindings: vec![
            SignalBinding {
                id: "processVariable".into(),
                address: Some("%IL0".into()),
                symbol: None,
                iec_type: "LREAL".into(),
                unit: None,
            },
            SignalBinding {
                id: "controllerMode".into(),
                address: None,
                symbol: Some("P1.AutoMode".into()),
                iec_type: "BOOL".into(),
                unit: None,
            },
        ],
        maximum_samples: 1_000,
        maximum_duration_ms: 60_000,
        task: Some("MainTask".into()),
        record_every: 1,
    }
}

/// A stop answer in the runtime's own words (E4.7); the runtime writes every value as a float.
fn stopped() -> serde_json::Value {
    json!({
        "recording_id": "recording-1", "loop_revision": "sha256:loop", "program_revision": "sha256:program",
        "bindings": [{ "id": "processVariable", "address": "%IL0", "iec_type": "LREAL" },
            { "id": "controllerMode", "symbol": "P1.AutoMode", "iec_type": "BOOL" }],
        "stop_reason": "stopped",
        "samples": [
            { "scan": 1, "runtime_time_ns": 100_000_000, "captured_at_ms": 1_700_000_000_000_u64, "values": [20.5, 1.0], "forced": [false, false], "quality": ["good", "good"] },
            { "scan": 3, "runtime_time_ns": 300_000_000, "captured_at_ms": 1_700_000_000_200_u64, "values": [null, 1.0], "forced": [false, false], "quality": ["bad", "good"], "gap": true }
        ],
        "acquisition": { "method": "runtime_scan_buffer", "completeness": "incomplete", "observedSampleCount": 2,
            "scanRange": { "first": 1, "last": 3 }, "missingScanRanges": [{ "first": 2, "last": 2 }], "droppedObservedSamples": 0,
            "actualCycleTimingAvailable": true, "freshnessAvailable": false, "reason": "execution 2 is missing" }
    })
}

#[test]
fn a_request_within_the_limits_of_a7_is_sent_as_annex_e4_2_states_it() {
    request().check().expect("within the limits");
    let value = serde_json::to_value(request()).expect("json");
    assert!(
        value.get("record_every").is_none(),
        "the default of 1 is not sent"
    );
    assert_eq!(
        value["bindings"][1],
        json!({ "id": "controllerMode", "symbol": "P1.AutoMode", "iec_type": "BOOL" })
    );
}

#[test]
fn a_request_over_a_limit_is_refused_with_the_limit_named() {
    type Edit = fn(&mut RecordingRequest);
    let cases: [(Edit, &str); 6] = [
        (
            |request| request.maximum_samples = 100_001,
            "2 to 100000 samples",
        ),
        (|request| request.maximum_samples = 1, "2 to 100000 samples"),
        (|request| request.maximum_duration_ms = 86_400_001, "24 h"),
        (|request| request.record_every = 0, "1 to 100000 executions"),
        (
            |request| request.program_revision = None,
            "needs the program revision",
        ),
        (
            |request| {
                request.maximum_samples = 100_000;
                request.bindings = (0..17)
                    .map(|index| SignalBinding {
                        id: format!("s{index}"),
                        address: Some(format!("%IL{}", index * 8)),
                        symbol: None,
                        iec_type: "LREAL".into(),
                        unit: None,
                    })
                    .collect();
            },
            "1600000 values",
        ),
    ];
    for (change, expected) in cases {
        let mut changed = request();
        change(&mut changed);
        let failure = changed.check().expect_err("refused");
        assert!(
            failure.message.contains(expected),
            "{} should name {expected}",
            failure.message
        );
    }
}

#[test]
fn a_recorded_run_decodes_from_the_runtimes_stop_answer_and_checks_it_answers_the_request() {
    let run: RecordedRun =
        serde_json::from_value(stopped()).expect("the runtime's stop answer decodes");
    run.check_answers(&request())
        .expect("it answers the request");
    assert_eq!(run.acquisition.completeness, Completeness::Incomplete);
    assert!(run.samples[1].gap);
    assert_eq!(
        serde_json::to_value(&run).expect("json"),
        stopped(),
        "it writes back exactly what the runtime returned"
    );
    let mut misaligned = run.clone();
    misaligned.samples[0].quality.pop();
    assert!(misaligned.check_answers(&request()).is_err());
    let mut other = request();
    other.recording_id = "recording-2".into();
    assert!(
        run.check_answers(&other).is_err(),
        "a run of another recording is not this one's evidence"
    );
}

/// A complete run of two consecutive samples of one process value.
fn complete_run() -> RecordedRun {
    fixtures::run(
        vec![fixtures::binding("processVariable", "%IL0", "LREAL")],
        vec![
            fixtures::sample(1, Some(0), &[Some(2.5)], &[Quality::Good]),
            fixtures::sample(2, Some(10_000_000), &[Some(2.5)], &[Quality::Good]),
        ],
    )
}

#[test]
fn a_recording_file_is_named_by_its_content_and_read_back_only_under_that_name() {
    let run: RecordedRun = serde_json::from_value(stopped()).expect("decoded");
    let evidence = recording_evidence(&run, None);
    let file = RecordingFile::new("loop:tank", EvidenceSource::SimulatedPlant, run, evidence);
    let id = file.id().expect("identified");
    assert!(id.starts_with("rec-") && id.len() == 36, "{id}");
    let text = file.encode().expect("encoded");
    assert_eq!(
        RecordingFile::decode(&text, "loop:tank", &id).expect("decoded"),
        file
    );
    assert!(
        RecordingFile::decode(&text, "loop:other", &id).is_err(),
        "another loop's recording is refused"
    );
    assert!(
        RecordingFile::decode(&text, "loop:tank", "rec-00000000000000000000000000000000").is_err(),
        "a renamed file is refused"
    );
    let mut physical = file.clone();
    physical.source = EvidenceSource::PhysicalPlant;
    assert_ne!(
        physical.id().expect("identified"),
        id,
        "the source is part of the evidence"
    );
    let mut relabelled = file.clone();
    relabelled.evidence.reason = "Another reason.".into();
    assert_ne!(
        relabelled.id().expect("identified"),
        id,
        "the verdict is part of the content"
    );
}

#[test]
fn the_verdict_is_stored_under_its_own_words() {
    let run = complete_run();
    let evidence = recording_evidence(&run, None);
    let file = RecordingFile::new("loop:tank", EvidenceSource::SimulatedPlant, run, evidence);
    let value = serde_json::to_value(file).expect("json");
    assert_eq!(
        value["evidence"],
        json!({
            "completeness": "complete",
            "reason": "Complete recording — runtime captured 2 scans. Stop: stopped."
        })
    );
}

#[test]
fn an_unconfirmed_start_and_an_incomplete_recording_stay_labelled_after_a_reload() {
    // A start whose acknowledgement was lost is incomplete whatever the runtime claims (E11.4).
    let unconfirmed = recording_evidence(&complete_run(), Some(UNCONFIRMED_START_RECOVERED));
    assert_eq!(unconfirmed.completeness, Completeness::Incomplete);
    let incomplete: RecordedRun = serde_json::from_value(stopped()).expect("decoded");
    let cases = [
        ("an unconfirmed start", complete_run(), unconfirmed),
        (
            "an incomplete acquisition",
            incomplete.clone(),
            recording_evidence(&incomplete, None),
        ),
        (
            "a complete recording",
            complete_run(),
            recording_evidence(&complete_run(), None),
        ),
    ];
    for (label, run, evidence) in cases {
        let file = RecordingFile::new(
            "loop:tank",
            EvidenceSource::SimulatedPlant,
            run,
            evidence.clone(),
        );
        let id = file.id().expect("identified");
        let text = file.encode().expect("encoded");
        let reloaded = RecordingFile::decode(&text, "loop:tank", &id).expect("decoded");
        assert_eq!(
            reloaded.evidence, evidence,
            "{label} keeps its verdict and its reason"
        );
    }
}

#[test]
fn a_file_without_a_verdict_or_with_one_that_contradicts_its_run_is_neither_written_nor_read() {
    let incomplete: RecordedRun = serde_json::from_value(stopped()).expect("decoded");
    let contradicting = RecordingEvidence {
        completeness: Completeness::Complete,
        reason: "Complete recording.".into(),
    };
    let file = RecordingFile::new(
        "loop:tank",
        EvidenceSource::SimulatedPlant,
        incomplete.clone(),
        contradicting,
    );
    assert!(
        file.encode()
            .expect_err("not written")
            .message
            .contains("not consistent"),
        "a complete verdict on an incomplete acquisition contradicts itself (E4.5)"
    );

    // The same contradiction in a text that carries its own identifier is not read.
    let honest = RecordingFile::new(
        "loop:tank",
        EvidenceSource::SimulatedPlant,
        incomplete.clone(),
        recording_evidence(&incomplete, None),
    );
    let mut forged = serde_json::to_value(&honest).expect("json");
    forged["evidence"]["completeness"] = json!("complete");
    let text = canonical_json(&forged);
    let id = recording_id_for(&text);
    assert!(RecordingFile::decode(&text, "loop:tank", &id)
        .expect_err("not read")
        .message
        .contains("not consistent"));

    // The file of the earlier shape, without a verdict, is not read: no earlier format is (D8).
    let mut earlier = serde_json::to_value(honest).expect("json");
    earlier
        .as_object_mut()
        .expect("an object")
        .remove("evidence");
    let text = canonical_json(&earlier);
    let id = recording_id_for(&text);
    assert!(RecordingFile::decode(&text, "loop:tank", &id)
        .expect_err("not read")
        .message
        .contains("evidence"));
}

#[test]
fn a_kept_recording_keeps_the_model_revision_of_its_run_and_an_older_file_keeps_its_identifier() {
    // B3.6: an isolated simulation's run names its model; the file keeps it, and the revision is part of its content.
    let run: RecordedRun = serde_json::from_value(stopped()).expect("decoded");
    let evidence = recording_evidence(&run, None);
    let file = RecordingFile::new("loop:tank", EvidenceSource::SimulatedPlant, run, evidence);
    let plain_id = file.id().expect("identified");
    assert!(
        !file.encode().expect("encoded").contains("modelRevision"),
        "a file without a model revision writes no such field, so its identifier is unchanged"
    );
    let mut kept = file.clone();
    kept.model_revision = Some("sha256:simulation".into());
    let kept_id = kept.id().expect("identified");
    assert_ne!(kept_id, plain_id, "the revision is part of the content");
    let text = kept.encode().expect("encoded");
    let read = RecordingFile::decode(&text, "loop:tank", &kept_id).expect("decoded");
    assert_eq!(read.model_revision.as_deref(), Some("sha256:simulation"));
    assert_eq!(read, kept, "the file reads back with its revision");
}

#[test]
fn a_recording_file_over_the_storage_limit_is_refused() {
    // Zero bytes are valid text, and the allocation is not touched until it is read.
    let limit = usize::try_from(STUDY_STORAGE_LIMIT_BYTES).expect("addressable");
    let oversize = String::from_utf8(vec![0_u8; limit + 1]).expect("NUL is UTF-8");
    let failure = RecordingFile::decode(
        &oversize,
        "loop:tank",
        "rec-00000000000000000000000000000000",
    )
    .expect_err("over the limit");
    assert!(failure.message.contains("257 MiB"), "{}", failure.message);
}
