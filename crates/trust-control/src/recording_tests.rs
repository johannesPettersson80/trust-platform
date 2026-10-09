//! Annex E4 and specification 35, A7 and A8.3: recording requests, recorded runs and their files.

use serde_json::json;

use super::*;

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
            },
            SignalBinding {
                id: "controllerMode".into(),
                address: None,
                symbol: Some("P1.AutoMode".into()),
                iec_type: "BOOL".into(),
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

#[test]
fn a_recording_file_is_named_by_its_content_and_read_back_only_under_that_name() {
    let run: RecordedRun = serde_json::from_value(stopped()).expect("decoded");
    let file = RecordingFile::new("loop:tank", EvidenceSource::SimulatedPlant, run);
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
}
