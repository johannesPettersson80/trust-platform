//! The recording an opened file becomes (`opened.rs`): the samples the file keeps with its source and its opened marker, the
//! refusal of a file the check does not accept, and the observation its fit reads (B3.6, C12, A9).

use serde_json::{json, Value};

use super::*;
use crate::recording::context::RestorationResult;
use crate::recording::fixtures;

/// One raw sample at scan `scan` of the fixture loop, one second after the one before it. The process value rises with the
/// scan, so the test can tell the samples apart; the command is the applied value of 40 %.
fn sample_json(scan: u32) -> Value {
    let seconds = f64::from(scan - 1);
    json!({
        "elapsedSeconds": seconds,
        "observedAtMs": 1000.0 * seconds + 1000.0,
        "actualCycleSeconds": 1.0,
        "scan": scan,
        "source": "Simulation",
        "loopRevision": "sha256:loop-1",
        "sp": 50.0,
        "pv": 50.0 + f64::from(scan),
        "requestedMv": 40.0,
        "appliedMv": 40.0,
        "units": { "sp": "%", "pv": "%", "mv": "%" },
        "quality": "good",
        "freshnessMs": null,
        "controllerMode": "Auto",
        "outputLimits": { "minimum": 0.0, "maximum": 100.0 },
        "safetyState": "Normal",
        "events": []
    })
}

/// The acquisition of a complete scan buffer of `count` samples, taken by runtime recording `recording-7` on program
/// revision `sha256:program-1`.
fn acquisition_json(count: u64) -> Value {
    json!({
        "method": "runtime_scan_buffer",
        "completeness": "complete",
        "recordingId": "recording-7",
        "programRevision": "sha256:program-1",
        "observedSampleCount": count,
        "scanRange": { "first": 1, "last": count },
        "missingScanRanges": [],
        "droppedObservedSamples": 0,
        "actualCycleTimingAvailable": true,
        "freshnessAvailable": false,
        "reason": "Complete recording."
    })
}

/// A complete, steady schema 4 result of the fixture loop: five runtime samples, scans 1 to 5 (B10.2, B10.4).
fn saved_result() -> Value {
    let samples: Vec<Value> = (1_u32..=5).map(sample_json).collect();
    json!({
        "schemaVersion": 4,
        "resultId": "result-1",
        "resultRevision": 3,
        "createdAt": "2026-10-10T12:00:00.000Z",
        "projectId": "project:fixture",
        "source": "Simulation",
        "loop": { "id": "loop:tank", "revision": "sha256:loop-1" },
        "controller": { "instance": "TankController", "revision": "sha256:controller-1" },
        "software": {
            "extensionVersion": "0.25.0",
            "numericalAdapters": { "trust-control-math": "1.0.0" }
        },
        "units": { "sp": "%", "pv": "%", "mv": "%" },
        "recording": {
            "rawSamples": samples,
            "selectedIntervals": [{ "startSeconds": 0.0, "endSeconds": 4.0 }],
            "acquisition": acquisition_json(5)
        },
        "evidence": { "observation": "Observed", "model": "Not evaluated" },
        "conclusion": "Recorded for identification."
    })
}

#[test]
fn a_checked_file_is_kept_as_an_opened_recording_with_its_source_and_its_runtime_values() {
    let loop_definition = fixtures::loop_definition();
    let (check, file) =
        opened_recording(&saved_result().to_string(), &loop_definition).expect("the file is kept");
    assert_eq!(check.result_id, "result-1");
    assert_eq!(check.samples, 5);
    assert!(file.opened, "the recording carries the opened marker (A9)");
    assert_eq!(
        file.source,
        EvidenceSource::SimulatedPlant,
        "the original source is kept"
    );
    assert_eq!(file.loop_id, loop_definition.id);
    assert_eq!(
        file.run.recording_id, "recording-7",
        "the runtime recording the file names"
    );
    assert_eq!(
        file.run.program_revision.as_deref(),
        Some("sha256:program-1")
    );
    assert_eq!(
        file.evidence.completeness,
        Completeness::Complete,
        "a complete steady acquisition is complete evidence"
    );
    assert_eq!(file.run.samples.len(), 5);
    assert_eq!(file.run.samples[0].runtime_time_ns, Some(0));
    assert_eq!(file.run.samples[4].runtime_time_ns, Some(4_000_000_000));
    let column = |id: &str| {
        file.run
            .bindings
            .iter()
            .position(|binding| binding.id == id)
            .expect("the binding of the signal")
    };
    assert_eq!(
        file.run.samples[1].values[column(PROCESS_VARIABLE_BINDING)],
        Some(52.0)
    );
    assert_eq!(
        file.run.samples[1].values[column(MANIPULATED_VARIABLE_BINDING)],
        Some(40.0)
    );
    assert!(
        file.run.samples[1]
            .quality
            .iter()
            .all(|quality| *quality == Quality::Good),
        "a good sample of good values"
    );
}

#[test]
fn a_file_the_check_refuses_is_not_kept() {
    let loop_definition = fixtures::loop_definition();
    let refusal =
        opened_recording("{", &loop_definition).expect_err("a file that is not JSON is refused");
    assert_eq!(
        refusal.0,
        "Choose a valid Control Studio recording or result JSON file."
    );
    let mut other_units = saved_result();
    other_units["units"]["mv"] = json!("V");
    assert!(
        opened_recording(&other_units.to_string(), &loop_definition).is_err(),
        "a recording in other units than the loop's is refused with no conversion"
    );
}

#[test]
fn the_fit_of_an_opened_recording_is_an_observation_that_writes_nothing() {
    let loop_definition = fixtures::loop_definition();
    let (_, file) =
        opened_recording(&saved_result().to_string(), &loop_definition).expect("the file is kept");
    let plan = opened_plan(&file, &loop_definition);
    assert_eq!(
        plan.kind,
        ControlExperimentKind::Observation,
        "the file names no experiment that Control Studio ran"
    );
    assert_eq!(plan.source, PlanSource::Simulation);
    assert_eq!(
        plan.affected_signal,
        loop_definition.signals.manipulated_variable.symbol
    );
    assert_eq!(plan.from, ExperimentValue::Number(40.0));
    assert_eq!(plan.to, plan.from, "nothing moves");
    assert_eq!(plan.restoration_value, plan.from, "nothing is restored");
    assert_eq!(plan.minimum, 40.0);
    assert_eq!(plan.maximum, 40.0);
    assert_eq!(plan.expected_scan, 1);
    assert!(
        (plan.duration_ms - 4000.0).abs() < 1e-9,
        "{}",
        plan.duration_ms
    );
    assert!(plan.trajectory.is_empty(), "no point is written");
}

/// The experiment a saved result states of the run that made its recording, as the extension wrote it (B10.2): a completed
/// setpoint step of the fixture loop.
fn experiment_json() -> Value {
    json!({
        "kind": "setpoint_step", "affectedSignal": "LevelSP", "address": "%ML0", "requestedValue": 55,
        "appliedValue": 55, "durationMs": 4000,
        "trajectory": [{ "atMs": 0, "value": 55 }], "stopConditions": ["Abort"],
        "minimum": 0, "maximum": 100, "approval": "Simulation confirmation", "status": "completed",
        "stopReason": "duration_complete", "aborted": false,
        "restoration": { "value": 50, "status": "confirmed" }
    })
}

/// The plant a saved result states, as the extension wrote it for a Simulation result (B10.2).
fn plant_json() -> Value {
    json!({
        "source": "Simulation", "id": "tank", "configurationRevision": "sha256:plant-1",
        "configuration": { "id": "tank", "model": "first_order_dead_time" }, "model": "first_order_dead_time",
        "effectiveSeed": "42", "uncertainties": [], "scheduledEvents": [], "actuator": "Not declared",
        "validityRange": "PV 0…100 %; MV 0…100 %"
    })
}

#[test]
fn an_opened_recording_keeps_the_experiment_and_the_plant_its_file_states() {
    let loop_definition = fixtures::loop_definition();
    let mut result = saved_result();
    result["experiment"] = experiment_json();
    result["plant"] = plant_json();
    let (_, file) =
        opened_recording(&result.to_string(), &loop_definition).expect("the file is kept");
    let context = file
        .context
        .clone()
        .expect("the file states its experiment and its plant");
    let [experiment] = context.experiments.as_slice() else {
        panic!("one experiment: {context:?}");
    };
    assert_eq!(experiment.kind, ControlExperimentKind::SetpointStep);
    assert_eq!(experiment.stop_reason.as_deref(), Some("duration_complete"));
    assert_eq!(experiment.restoration.status, RestorationResult::Confirmed);
    let plant = context.plant.expect("the plant");
    assert_eq!(plant.id, "tank");
    assert_eq!(plant.effective_seed.as_deref(), Some("42"));
    let id = file.id().expect("a file with a context is written");
    let (_, plain) =
        opened_recording(&saved_result().to_string(), &loop_definition).expect("the file is kept");
    assert_eq!(plain.context, None, "a file that states none keeps none");
    assert_ne!(
        plain.id().expect("identified"),
        id,
        "the context is part of the recording's content"
    );
}

#[test]
fn an_opened_file_with_a_context_it_cannot_read_is_still_kept() {
    let loop_definition = fixtures::loop_definition();
    let mut result = saved_result();
    // The file states its experiment's status and nothing the engine could keep: the file is kept, and the context is not.
    result["experiment"] = json!({ "status": "completed" });
    let (_, file) =
        opened_recording(&result.to_string(), &loop_definition).expect("the file is kept");
    assert_eq!(file.context, None);
}
