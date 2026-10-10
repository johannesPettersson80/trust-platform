//! Rule tests of the connected identification record (`connectedIdentificationRecord.ts`). No `TypeScript` test calls the builder
//! or the decoder directly, so the rules are worked out from the `TypeScript` and specification 35 (A4.4, A11.1, A11.2, B3.4).
//! The evaluator is a stand-in that keeps the checks it is given, as the V3 evaluation keeps its requirements.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::*;
use crate::experiment::plan::ControlExperimentPlan;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::recording::channels::ChannelQuality;
use crate::recording::fixtures::{binding, loop_definition, run, sample};
use crate::recording::samples::invalid_samples;
use crate::recording::{Completeness, RecordedRun};
use crate::signals::Quality;

/// The evaluation of the stand-in evaluator: its status and the checks it was given, as the V3 evaluation keeps its requirements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StubEvaluation {
    status: String,
    specification: Vec<BoundsCheck>,
}

/// The stand-in evaluator: every run passes, and its specification is the checks it was given.
#[allow(clippy::unnecessary_wraps)] // It stands in for the evaluator callback, which returns a Result.
fn stub(_run: &ValidationRun, checks: &[BoundsCheck]) -> Result<StubEvaluation, Failure> {
    Ok(StubEvaluation {
        status: "Pass".to_string(),
        specification: checks.to_vec(),
    })
}

/// Whether two numbers agree within the tolerance of these tests.
fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9
}

/// Whether two optional numbers agree: both missing, or both present and close.
fn same_option(value: Option<f64>, expected: Option<f64>) -> bool {
    match (value, expected) {
        (Some(left), Some(right)) => close(left, right),
        (None, None) => true,
        _ => false,
    }
}

/// Whether `values` holds `symbol` with `value` and `quality`.
fn holds(
    values: &BTreeMap<String, ValidationValue>,
    symbol: &str,
    value: Option<f64>,
    quality: ChannelQuality,
) -> bool {
    values
        .get(symbol)
        .is_some_and(|read| read.quality == quality && same_option(read.value, value))
}

/// `value` with the field at `path` replaced by `to`.
fn altered(value: &serde_json::Value, path: &[&str], to: serde_json::Value) -> serde_json::Value {
    let mut copy = value.clone();
    let mut field = &mut copy;
    for key in path {
        field = &mut field[*key];
    }
    *field = to;
    copy
}

/// The complete recording of the fixture loop: three scans 10 ms apart, the first at runtime time 2 s.
fn recording() -> RecordedRun {
    run(
        vec![
            binding("setpoint", "%ML0", "LREAL"),
            binding("processVariable", "%IL0", "LREAL"),
            binding("manipulatedVariable", "%QL0", "LREAL"),
        ],
        vec![
            sample(
                11,
                Some(2_000_000_000),
                &[Some(50.0), Some(48.0), Some(32.0)],
                &[Quality::Good; 3],
            ),
            sample(
                12,
                Some(2_010_000_000),
                &[Some(50.0), Some(48.5), Some(33.0)],
                &[Quality::Good; 3],
            ),
            sample(
                13,
                Some(2_020_000_000),
                &[Some(50.0), Some(49.0), Some(34.0)],
                &[Quality::Good; 3],
            ),
        ],
    )
}

/// The reviewed plan of a setpoint step on the fixture loop, run on a simulated or a physical target.
fn plan(source: &str) -> ControlExperimentPlan {
    serde_json::from_value(json!({
        "id": "experiment:tank:11:setpoint_step",
        "source": source,
        "kind": "setpoint_step",
        "affectedSignal": "LevelSP",
        "valueType": "LREAL",
        "from": 50.0,
        "to": 55.0,
        "unit": "%",
        "minimum": 0.0,
        "maximum": 100.0,
        "durationMs": 20.0,
        "expectedScan": 11,
        "stopConditions": [],
        "restorationValue": 50.0,
        "requiresPhysicalApproval": false,
        "trajectory": []
    }))
    .expect("the fixture plan decodes")
}

/// Builds the record of `recorded` on the fixture loop, with the stand-in evaluator and no start failure or baseline.
fn record_of(
    loop_def: &LoopDefinition,
    recorded: &RecordedRun,
    reviewed: &ControlExperimentPlan,
) -> Result<RuntimeVerificationResult<StubEvaluation>, Failure> {
    connected_identification_record(loop_def, recorded, None, reviewed, 0.0, None, stub)
}

#[test]
fn an_incomplete_recording_is_refused_as_identify_evidence() {
    let mut recorded = recording();
    recorded.acquisition.completeness = Completeness::Incomplete;
    recorded.acquisition.reason = "Scan 12 is missing.".to_string();
    let Err(failure) = record_of(&loop_definition(), &recorded, &plan("simulation")) else {
        panic!("an incomplete recording is refused");
    };
    assert_eq!(
        failure.message,
        "Identification requires the complete recording from the connected runtime."
    );
    assert_eq!(
        failure.detail.as_deref(),
        Some(
            "Identification requires the complete recording from the connected runtime. \
             Incomplete recording — Scan 12 is missing. \
             The incomplete recording is kept as evidence, and Identify plant records a new one."
        )
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::ModelOrData, NextAction::Identify)
    );
}

#[test]
fn an_unconfirmed_start_refuses_even_a_complete_recording() {
    let Err(failure) = connected_identification_record(
        &loop_definition(),
        &recording(),
        Some("no reply within 2 s"),
        &plan("simulation"),
        0.0,
        None,
        stub,
    ) else {
        panic!("an unconfirmed start is refused");
    };
    assert_eq!(
        failure.message,
        "Identification requires the complete recording from the connected runtime."
    );
    assert!(failure.detail.as_deref().is_some_and(|detail| {
        detail.contains(
            "Start acknowledgement unconfirmed (no reply within 2 s); stopped data retained.",
        )
    }));
    assert_eq!(failure.next_action, NextAction::Identify);
}

#[test]
fn the_time_starts_at_the_first_runtime_scan() {
    let Ok(record) = record_of(&loop_definition(), &recording(), &plan("simulation")) else {
        panic!("the complete recording builds");
    };
    let samples = &record.run.samples;
    let times: Vec<f64> = samples
        .iter()
        .map(|measured| measured.time_seconds)
        .collect();
    let scans: Vec<u64> = samples.iter().map(|measured| measured.sequence).collect();
    assert_eq!(scans, vec![11, 12, 13]);
    assert_eq!(times.len(), 3);
    assert!(
        close(times[0], 0.0) && close(times[1], 0.01) && close(times[2], 0.02),
        "the times are {times:?}"
    );
    assert!(close(record.run.conditions.duration_seconds, 0.02));
    assert_eq!(record.run.conditions.sample_times_seconds.len(), 3);
}

#[test]
fn a_sample_without_runtime_time_is_refused_as_invalid() {
    let mut recorded = recording();
    recorded.samples[1].runtime_time_ns = None;
    let Err(failure) = record_of(&loop_definition(), &recorded, &plan("simulation")) else {
        panic!("a sample without runtime time is refused");
    };
    assert_eq!(failure, invalid_samples());
}

#[test]
fn the_baseline_fields_are_written_only_when_they_apply() {
    let cases = [
        (0.0, None, None, None),
        (0.0, Some(5.0), None, Some(5.0)),
        (2.0, Some(1.0), Some(2.0), None),
        (2.0, Some(4.0), Some(2.0), Some(4.0)),
    ];
    for (start, end, expected_start, expected_end) in cases {
        let Ok(record) = connected_identification_record(
            &loop_definition(),
            &recording(),
            None,
            &plan("simulation"),
            start,
            end,
            stub,
        ) else {
            panic!("the complete recording builds");
        };
        assert!(
            same_option(record.request.baseline_start_seconds, expected_start),
            "the start {start} is written as {:?}",
            record.request.baseline_start_seconds
        );
        assert!(
            same_option(record.request.baseline_end_seconds, expected_end),
            "the end {end:?} is written as {:?}",
            record.request.baseline_end_seconds
        );
    }
}

#[test]
fn a_record_without_its_plant_input_limit_is_refused_and_names_the_limit() {
    let mut loop_def = loop_definition();
    loop_def.limits.output_maximum = None;
    let Err(failure) = record_of(&loop_def, &recording(), &plan("simulation")) else {
        panic!("the plant input limit is needed to check the run");
    };
    assert_eq!(failure.next_action, NextAction::OpenLimits);
    assert_eq!(
        failure.message,
        "The plant input limits are missing: limits.outputMaximum must be set, with the minimum below the maximum."
    );
}

#[test]
fn the_bounds_cover_the_plant_input_and_the_measured_output_only() {
    let mut loop_def = loop_definition();
    loop_def.limits.output_maximum = Some(80.0);
    loop_def.signals.manipulated_variable.range.maximum = 90.0;
    let Ok(record) = record_of(&loop_def, &recording(), &plan("simulation")) else {
        panic!("the complete recording builds");
    };
    let checks = &record.evaluation.specification;
    let ids: Vec<&str> = checks.iter().map(|check| check.id.as_str()).collect();
    assert_eq!(ids, ["bounds:LevelPV", "bounds:ValveMV"]);
    assert!(checks.iter().all(|check| check.kind == BoundsKind::Bounds));
    assert!(checks
        .iter()
        .all(|check| close(check.window.start, 0.0) && close(check.window.end, 0.02)));
    assert!(
        close(checks[0].minimum, 0.0) && close(checks[0].maximum, 100.0),
        "the process value is bounded by its range"
    );
    assert!(
        close(checks[1].minimum, 0.0) && close(checks[1].maximum, 80.0),
        "the valve is bounded by the output limits, not by its range"
    );
}

#[test]
fn the_request_names_the_loop_the_plan_and_the_evidence_of_the_recording() {
    let reviewed = plan("simulation");
    let Ok(record) = record_of(&loop_definition(), &recording(), &reviewed) else {
        panic!("the complete recording builds");
    };
    let request = &record.request;
    assert_eq!(request.execution, ConnectedExecution::ConnectedRuntime);
    assert_eq!(request.loop_revision, "sha256:loop-1");
    assert_eq!(request.experiment, reviewed);
    assert_eq!(request.acquisition.recording_id, "rec-fixture");
    assert_eq!(request.acquisition.program_revision, "sha256:program");
    assert_eq!(request.acquisition.method, "runtime_scan_buffer");
    assert_eq!(request.acquisition.completeness, Completeness::Complete);
    assert_eq!(request.acquisition.observed_sample_count, 3);
    assert_eq!(
        request.acquisition.reason,
        "Complete recording — runtime captured 3 scans. Stop: stopped."
    );
    assert_eq!(record.project_revision, "sha256:program");
    assert_eq!(record.run.id, "rec-fixture");
}

#[test]
fn each_value_is_read_as_the_recording_holds_it() {
    let mut recorded = recording();
    recorded.samples[1].forced[2] = true;
    let Ok(record) = record_of(&loop_definition(), &recorded, &plan("simulation")) else {
        panic!("the complete recording builds");
    };
    let first = &record.run.samples[0].values;
    let second = &record.run.samples[1].values;
    assert!(holds(first, "LevelSP", Some(50.0), ChannelQuality::Good));
    assert!(holds(first, "LevelPV", Some(48.0), ChannelQuality::Good));
    assert!(holds(first, "ValveMV", Some(32.0), ChannelQuality::Good));
    // A forced valve makes its whole sample forced (A4.2); the values stay the ones the runtime recorded.
    assert!(holds(second, "LevelSP", Some(50.0), ChannelQuality::Forced));
    assert!(holds(second, "LevelPV", Some(48.5), ChannelQuality::Forced));
    assert!(holds(second, "ValveMV", Some(33.0), ChannelQuality::Forced));
}

#[test]
fn the_run_holds_its_channels_first_values_and_the_evidence_of_the_plan() {
    let Ok(record) = record_of(&loop_definition(), &recording(), &plan("simulation")) else {
        panic!("the complete recording builds");
    };
    let validation = &record.run;
    let channels: Vec<&str> = validation
        .channels
        .iter()
        .map(|channel| channel.id.as_str())
        .collect();
    assert_eq!(channels, ["LevelPV", "ValveMV", "LevelSP"]);
    assert_eq!(validation.evidence, RunEvidence::StExecuted);
    assert_eq!(validation.conditions.seed, 1);
    assert_eq!(
        validation.conditions.model_revision,
        "connected-plant:sha256:program"
    );
    assert_eq!(validation.conditions.program_revision, "sha256:program");
    assert_eq!(
        validation.conditions.parameter_schema_revision,
        "sha256:loop-1"
    );
    let initial = &validation.conditions.initial_state;
    assert_eq!(initial.len(), 3);
    assert!(close(initial["LevelPV"], 48.0));
    assert!(close(initial["ValveMV"], 32.0));
    assert!(close(initial["LevelSP"], 50.0));

    let mut seeded = plan("physical");
    seeded.seed = Some(7);
    let Ok(physical) = record_of(&loop_definition(), &recording(), &seeded) else {
        panic!("the complete recording builds");
    };
    assert_eq!(physical.run.evidence, RunEvidence::PhysicalRecorded);
    assert_eq!(physical.run.conditions.seed, 7);
}

#[test]
fn a_refusal_of_the_evaluator_is_returned_unchanged() {
    let refusal = Failure::new(
        "Requirement IDs must be unique.",
        Cause::Unclassified,
        NextAction::None,
    );
    let Err(failure) = connected_identification_record(
        &loop_definition(),
        &recording(),
        None,
        &plan("simulation"),
        0.0,
        None,
        |_, _| Err::<StubEvaluation, Failure>(refusal.clone()),
    ) else {
        panic!("the evaluator's refusal is returned");
    };
    assert_eq!(failure, refusal);
}

#[test]
fn the_decoder_accepts_the_record_it_wrote_and_refuses_an_altered_one() {
    let Ok(record) = record_of(&loop_definition(), &recording(), &plan("simulation")) else {
        panic!("the complete recording builds");
    };
    let Ok(stored) = serde_json::to_value(&record) else {
        panic!("the record encodes");
    };
    let reads = |value: &serde_json::Value| decode_connected_identification_record(value, stub);
    assert_eq!(reads(&stored), Some(record.clone()));

    let altered_evaluation = altered(&stored, &["evaluation", "status"], json!("Fail"));
    assert!(
        reads(&altered_evaluation).is_none(),
        "an altered evaluation is refused"
    );
    let other_recording = altered(
        &stored,
        &["request", "acquisition", "recordingId"],
        json!("rec-other"),
    );
    assert!(
        reads(&other_recording).is_none(),
        "another recording is refused"
    );
    let other_program = altered(
        &stored,
        &["run", "conditions", "programRevision"],
        json!("sha256:other"),
    );
    assert!(
        reads(&other_program).is_none(),
        "another program revision is refused"
    );
    let other_evidence = altered(&stored, &["run", "evidence"], json!("physical-recorded"));
    assert!(
        reads(&other_evidence).is_none(),
        "evidence of another source is refused"
    );
    let incomplete = altered(
        &stored,
        &["request", "acquisition", "completeness"],
        json!("incomplete"),
    );
    assert!(
        reads(&incomplete).is_none(),
        "an incomplete acquisition is refused"
    );
}

#[test]
#[allow(clippy::too_many_lines)] // The stored shape is written out in full by hand, so a renamed field is caught.
fn a_record_in_the_stored_shape_decodes_and_is_written_back_unchanged() {
    // The stored shape of `connectedIdentificationRecord.ts`, written by hand: the acquisition's fields sit flat beside the
    // recording's identifier and the program revision, and no field is nested.
    let stored = json!({
        "projectRevision": "sha256:program",
        "request": {
            "execution": "connected-runtime",
            "loop_revision": "sha256:loop-1",
            "acquisition": {
                "recordingId": "rec-fixture",
                "programRevision": "sha256:program",
                "method": "runtime_scan_buffer",
                "completeness": "complete",
                "observedSampleCount": 3,
                "scanRange": { "first": 11, "last": 13 },
                "missingScanRanges": [],
                "droppedObservedSamples": 0,
                "actualCycleTimingAvailable": true,
                "freshnessAvailable": false,
                "reason": "Complete recording — runtime captured 3 scans. Stop: stopped."
            },
            "experiment": {
                "id": "experiment:tank:11:setpoint_step",
                "source": "simulation",
                "kind": "setpoint_step",
                "affectedSignal": "LevelSP",
                "valueType": "LREAL",
                "from": 50.0,
                "to": 55.0,
                "unit": "%",
                "minimum": 0.0,
                "maximum": 100.0,
                "durationMs": 20.0,
                "expectedScan": 11,
                "stopConditions": [],
                "restorationValue": 50.0,
                "requiresPhysicalApproval": false,
                "trajectory": []
            }
        },
        "run": {
            "id": "rec-fixture",
            "evidence": "ST-executed",
            "parameters": {},
            "conditions": {
                "initialState": { "LevelSP": 50.0, "LevelPV": 48.0, "ValveMV": 32.0 },
                "events": [],
                "durationSeconds": 0.02,
                "sampleTimesSeconds": [0.0, 0.01, 0.02],
                "seed": 1,
                "modelRevision": "connected-plant:sha256:program",
                "programRevision": "sha256:program",
                "parameterSchemaRevision": "sha256:loop-1"
            },
            "channels": [
                { "id": "LevelPV", "unit": "%" },
                { "id": "ValveMV", "unit": "%" },
                { "id": "LevelSP", "unit": "%" }
            ],
            "samples": [
                {
                    "timeSeconds": 0.0,
                    "sequence": 11,
                    "values": {
                        "LevelSP": { "value": 50.0, "quality": "good" },
                        "LevelPV": { "value": 48.0, "quality": "good" },
                        "ValveMV": { "value": 32.0, "quality": "good" }
                    }
                },
                {
                    "timeSeconds": 0.01,
                    "sequence": 12,
                    "values": {
                        "LevelSP": { "value": 50.0, "quality": "good" },
                        "LevelPV": { "value": 48.5, "quality": "good" },
                        "ValveMV": { "value": 33.0, "quality": "good" }
                    }
                },
                {
                    "timeSeconds": 0.02,
                    "sequence": 13,
                    "values": {
                        "LevelSP": { "value": 50.0, "quality": "good" },
                        "LevelPV": { "value": 49.0, "quality": "good" },
                        "ValveMV": { "value": 34.0, "quality": "good" }
                    }
                }
            ]
        },
        "evaluation": {
            "status": "Pass",
            "specification": [
                {
                    "id": "bounds:LevelPV",
                    "kind": "bounds",
                    "channel": "LevelPV",
                    "window": { "start": 0.0, "end": 0.02 },
                    "minimum": 0.0,
                    "maximum": 100.0
                },
                {
                    "id": "bounds:ValveMV",
                    "kind": "bounds",
                    "channel": "ValveMV",
                    "window": { "start": 0.0, "end": 0.02 },
                    "minimum": 0.0,
                    "maximum": 100.0
                }
            ]
        }
    });
    let Some(record) = decode_connected_identification_record(&stored, stub) else {
        panic!("the record in the stored shape decodes");
    };
    assert_eq!(record.request.acquisition.recording_id, "rec-fixture");
    assert_eq!(record.request.acquisition.method, "runtime_scan_buffer");
    assert_eq!(record.run.samples.len(), 3);
    let Ok(written) = serde_json::to_value(record) else {
        panic!("the record encodes");
    };
    assert_eq!(
        written, stored,
        "the record is written back as it was stored"
    );
    let extra = altered(&stored, &["request", "acquisition", "extra"], json!(true));
    assert!(
        decode_connected_identification_record(&extra, stub).is_none(),
        "an unknown field of the acquisition is refused"
    );
}
