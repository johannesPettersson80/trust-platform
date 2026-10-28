//! The engine tests of the recording and model imports and of the fit of an opened recording (`importRecording`,
//! `importModel`, `openRecording` of an opened recording; specification 35, §0.1a, B3.6, B3.7, B10.2 and A9). They use the
//! fixtures of `identify_action_tests.rs`, and no runtime runs here.

use super::*;
use crate::commands::ControlStudio;
use std::collections::BTreeMap;
use trust_control::identification::connected_record::connected_identification_record;
use trust_control::identification::connected_record::{
    RunConditions, RunEvidence, ValidationChannel, ValidationRun, ValidationSample, ValidationValue,
};
use trust_control::identification::recorded_limits::evaluate_recorded_limits;
use trust_control::recording::channels::ChannelQuality;
use trust_control::recording::channels::{
    MANIPULATED_VARIABLE_BINDING, PROCESS_VARIABLE_BINDING, SETPOINT_BINDING,
};
use trust_control::recording::evidence::recording_evidence;
use trust_control::recording::opened::opened_plan;
use trust_control::recording::{
    Acquisition, Completeness, RecordedRun, RecordedSample, RecordingFile, ScanRange, StopReason,
};
use trust_control::signals::{Quality, SignalBinding};
use trust_control::study::inputs::InputsSection;
use trust_control::validation::isolated_run_sampling::recorded_sample_times;
use trust_control::validation::run_validation::{
    evaluate_recorded_run, RunCheck, RunRequirement, Window,
};

/// Answers an import request as this module reads it: the `result` of a request that succeeded, which `execute` wraps as
/// `{"result": …}`, or the `failure` envelope of a refusal, which `refusal` reads.
fn identify(studio: &ControlStudio, request: Value) -> Value {
    let envelope = super::identify(studio, request);
    envelope.get("result").cloned().unwrap_or(envelope)
}

/// The command and the measurement of the example loop's plant over `count` samples, one second apart: a setpoint step of `size`
/// percent at sample `step_at` (counted from zero) and held to the end, with the first-order response of gain two and a time
/// constant of five seconds around 50 °C. A single step is what a `setpoint_step` record holds (B5.2), and the validated fit reads
/// its held step: a pseudo-random command that changes every five samples leaves a held step of fifteen samples, below the twenty
/// that a fit needs (`minimum_samples`).
fn plant_response(count: u32, step_at: u32, size: f64) -> Vec<(f64, f64)> {
    let alpha = (-1.0_f64 / 5.0).exp();
    let mut process = 50.0_f64;
    (0..count)
        .map(|index| {
            let command = if index < step_at { 50.0 } else { 50.0 + size };
            process = alpha * process + (1.0 - alpha) * (50.0 + 2.0 * (command - 50.0));
            (command, process)
        })
        .collect()
}

/// A saved result of the example loop with `count` runtime samples, one second apart: the record of a plant that an opened file
/// keeps. Its command is a setpoint step of five percent at the tenth sample (`plant_response`). It is complete and steady.
fn plant_result(loop_definition: &LoopDefinition, count: u32) -> Value {
    let samples: Vec<Value> = (1..=count)
        .zip(plant_response(count, 10, 5.0))
        .map(|(scan, (command, process))| {
            json!({
                "elapsedSeconds": f64::from(scan - 1),
                "observedAtMs": 1000.0 * f64::from(scan),
                "actualCycleSeconds": 1.0,
                "scan": scan,
                "source": "Simulation",
                "loopRevision": loop_definition.revision,
                "sp": 50.0,
                "pv": process,
                "requestedMv": command,
                "appliedMv": command,
                "units": { "sp": "°C", "pv": "°C", "mv": "%" },
                "quality": "good",
                "freshnessMs": null,
                "controllerMode": "Auto",
                "outputLimits": { "minimum": 0.0, "maximum": 100.0 },
                "safetyState": "Normal",
                "events": []
            })
        })
        .collect();
    json!({
        "schemaVersion": 4,
        "resultId": "result-plant",
        "resultRevision": 1,
        "createdAt": "2026-10-10T12:00:00.000Z",
        "projectId": loop_definition.project_id,
        "source": "Simulation",
        "loop": { "id": LOOP_ID, "revision": loop_definition.revision },
        "controller": { "instance": "P1.Controller", "revision": "sha256:controller-plant" },
        "software": {
            "extensionVersion": "0.25.0",
            "numericalAdapters": { "trust-control-math": "1.0.0" }
        },
        "units": { "sp": "°C", "pv": "°C", "mv": "%" },
        "recording": {
            "rawSamples": samples,
            "selectedIntervals": [{ "startSeconds": 0.0, "endSeconds": f64::from(count - 1) }],
            "acquisition": {
                "method": "runtime_scan_buffer",
                "completeness": "complete",
                "recordingId": "recording-plant",
                "programRevision": "sha256:program-plant",
                "observedSampleCount": count,
                "scanRange": { "first": 1, "last": count },
                "missingScanRanges": [],
                "droppedObservedSamples": 0,
                "actualCycleTimingAvailable": true,
                "freshnessAvailable": false,
                "reason": "Complete recording."
            }
        },
        "evidence": { "observation": "Observed", "model": "Not evaluated" },
        "conclusion": "Recorded for identification."
    })
}

#[test]
fn import_recording_keeps_a_checked_file_as_an_opened_recording_of_the_study() {
    let project = Project::new("import-kept");
    let (studio, updates) = engine();
    let file = plant_result(&project.loop_definition(), 60).to_string();
    let answer = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": file })),
    );
    assert_eq!(answer["status"], "checked", "{answer}");
    // §0.1a: the answer names its kind, which the page shows the kept recording by.
    assert_eq!(answer["kind"], "importedRecording", "{answer}");
    assert_eq!(answer["source"], "Simulation", "{answer}");
    // The fit the import names ends before the test reads the study, and before the project folder is removed.
    let _import_fit = final_update(&updates);
    let recording_id = answer["recordingId"]
        .as_str()
        .expect("the kept recording")
        .to_string();
    assert!(recording_id.starts_with("rec-"), "{answer}");
    let kept = storage::read_recording(&project.root, LOOP_ID, &recording_id)
        .expect("the kept file reads back");
    assert!(kept.opened, "the recording carries the opened marker (A9)");
    assert_eq!(
        kept.source,
        EvidenceSource::SimulatedPlant,
        "its original source"
    );
    assert_eq!(kept.run.samples.len(), 60);
    assert_eq!(
        project.working_study().recordings,
        [recording_id.as_str()],
        "the study lists the recording"
    );
    assert!(
        answer["operationId"].is_string(),
        "a checked recording is fitted at once, and the answer names the fit (B3.6): {answer}"
    );
}

#[test]
fn a_checked_recording_is_fitted_by_the_answer_of_its_import_and_the_fit_is_kept() {
    let project = Project::new("import-fits-at-once");
    let (studio, updates) = engine();
    let loop_definition = project.loop_definition();
    let answer = identify(
        &studio,
        project.request(json!({
            "action": "importRecording",
            "json": plant_result(&loop_definition, 240).to_string()
        })),
    );
    assert_eq!(answer["status"], "checked", "{answer}");
    let recording_id = answer["recordingId"]
        .as_str()
        .expect("the kept recording")
        .to_string();
    let operation_id = answer["operationId"]
        .as_str()
        .expect("the import names the fit it started")
        .to_string();
    let update = final_update(&updates);
    assert_eq!(update.operation_id, operation_id, "{answer}");
    assert_eq!(
        update.state,
        OperationState::Finished,
        "{:?}",
        update.failure
    );
    let result = update.result.expect("the fit of the recording");
    assert_eq!(result["recordingId"], recording_id.as_str(), "{result}");
    let stored = storage::read_identify(&project.root, &project.working_study())
        .expect("the study reads")
        .expect("the fit is kept");
    assert_eq!(
        stored.section.signals.outputs,
        ["P1.ProcessValue".to_string()],
        "the fit reads the loop's own measured output"
    );
}

#[test]
fn import_recording_refuses_a_file_in_other_units_and_keeps_nothing() {
    let project = Project::new("import-refused");
    let (studio, _updates) = engine();
    let mut file = plant_result(&project.loop_definition(), 20);
    file["units"]["mv"] = json!("V");
    let answer = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": file.to_string() })),
    );
    let (message, next_action) = refusal(&answer);
    assert!(!message.is_empty(), "{answer}");
    assert_eq!(next_action, "none", "{answer}");
    assert!(
        !study_directory(&project.root, LOOP_ID).exists(),
        "a refused file keeps nothing: no study folder is written"
    );
    assert_eq!(project.working_study().recordings, Vec::<String>::new());
}

#[test]
fn an_opened_recording_is_fitted_from_its_own_samples_and_reopening_shows_the_fit() {
    let project = Project::new("opened-fit");
    let (studio, updates) = engine();
    let loop_definition = project.loop_definition();
    let imported = identify(
        &studio,
        project.request(json!({
            "action": "importRecording",
            "json": plant_result(&loop_definition, 240).to_string()
        })),
    );
    let recording_id = imported["recordingId"]
        .as_str()
        .expect("the kept recording")
        .to_string();
    // The engine fits the checked recording within the import (B3.6); the fit ends before the page's own fit can start.
    let _import_fit = final_update(&updates);
    let started = identify(&studio, project.request(open_recording(&recording_id)));
    assert!(started["operationId"].is_string(), "{started}");
    let update = final_update(&updates);
    assert_eq!(
        update.state,
        OperationState::Finished,
        "{:?}",
        update.failure
    );
    let result = update.result.expect("the fit of the opened recording");
    assert_eq!(result["recordingId"], recording_id.as_str(), "{result}");
    assert_eq!(
        result["provisional"], true,
        "a fit from one recording is provisional (B3.5)"
    );
    let study = project.working_study();
    let stored = storage::read_identify(&project.root, &study)
        .expect("the study reads")
        .expect("the fit is kept, so reopening shows it");
    assert_eq!(
        stored.estimation.id().expect("its identifier"),
        recording_id
    );
    assert_eq!(
        stored.section.method,
        IdentifyMethod::Opened(ControlExperimentKind::Observation)
    );
    assert_eq!(stored.section.source, EvidenceSource::SimulatedPlant);
}

/// A supplied transfer function `2 / (5s + 1)` with a delay of one second, from `u` in % to `y` in °C, as a model file writes it.
fn supplied_model(source: &str, validation: &str) -> Value {
    json!({
        "kind": "transfer-function", "revision": "provided-course-model", "source": source,
        "validation": validation, "evidence": ["independent-course-measurement"], "delaySeconds": 1.0,
        "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0.0, "maximum": 100.0 } }],
        "outputs": [{ "id": "y", "unit": "°C", "operatingRange": { "minimum": 0.0, "maximum": 100.0 } }],
        "operatingPoint": { "inputs": [20.0], "outputs": [20.0] },
        "numerator": [2.0], "denominator": [5.0, 1.0]
    })
}

#[test]
fn import_model_keeps_an_accepted_model_as_the_study_model_and_answers_its_equation() {
    let project = Project::new("import-model");
    let (studio, _updates) = engine();
    let answer = identify(
        &studio,
        project.request(json!({
            "action": "importModel",
            "json": supplied_model("provided", "validated").to_string()
        })),
    );
    assert_eq!(
        answer["equation"], "G(s) = 2 / (5s + 1), delay 1 s",
        "{answer}"
    );
    assert_eq!(
        answer["modelType"], "Transfer function, continuous time",
        "{answer}"
    );
    assert_eq!(
        answer["provisional"], false,
        "a model with independent validation is not provisional (B3.7)"
    );
    assert_eq!(answer["modelRevision"], "provided-course-model", "{answer}");
    let kept = storage::read_supplied_model(&project.working_study())
        .expect("the study reads")
        .expect("the model is kept, so reopening shows it");
    assert_eq!(kept.model.identity().revision, "provided-course-model");
    assert!(
        nothing_started(&studio),
        "a supplied model starts no campaign (B3.7)"
    );
}

#[test]
fn import_model_refuses_a_model_that_fails_its_checks_and_leaves_the_study_unchanged() {
    let project = Project::new("import-model-refused");
    let (studio, _updates) = engine();
    let accepted = identify(
        &studio,
        project.request(json!({
            "action": "importModel",
            "json": supplied_model("provided", "unvalidated").to_string()
        })),
    );
    assert_eq!(accepted["provisional"], true, "{accepted}");
    let study_file = study_directory(&project.root, LOOP_ID).join("study.json");
    let before = fs::read(&study_file).expect("the study file");

    let not_json = identify(
        &studio,
        project.request(json!({ "action": "importModel", "json": "not json" })),
    );
    assert_eq!(
        refusal(&not_json),
        ("Choose a valid plant model JSON file.", "none")
    );
    let fitted = identify(
        &studio,
        project.request(json!({
            "action": "importModel",
            "json": supplied_model("fitted", "unvalidated").to_string()
        })),
    );
    let (message, next_action) = refusal(&fitted);
    assert!(
        message.starts_with("The supplied model of the study is not valid"),
        "{fitted}"
    );
    assert_eq!(next_action, "none", "{fitted}");
    let too_large = identify(
        &studio,
        project.request(json!({ "action": "importModel", "json": " ".repeat(256_001) })),
    );
    assert_eq!(
        refusal(&too_large).0,
        "Choose a plant model JSON file of at most 256 kB."
    );

    assert_eq!(
        fs::read(&study_file).expect("the study file"),
        before,
        "a refused model leaves the study as it was"
    );
    let kept = storage::read_supplied_model(&project.working_study())
        .expect("the study reads")
        .expect("the earlier model is still kept");
    assert_eq!(kept.model.identity().revision, "provided-course-model");
}

#[test]
fn import_model_refuses_a_changed_model_under_the_current_revision_and_keeps_the_study() {
    let project = Project::new("import-model-changed");
    let (studio, _updates) = engine();
    let accepted = identify(
        &studio,
        project.request(json!({
            "action": "importModel",
            "json": supplied_model("provided", "unvalidated").to_string()
        })),
    );
    assert_eq!(
        accepted["modelRevision"], "provided-course-model",
        "{accepted}"
    );
    let study_file = study_directory(&project.root, LOOP_ID).join("study.json");
    let before = fs::read(&study_file).expect("the study file");

    let mut changed = supplied_model("provided", "unvalidated");
    changed["numerator"] = json!([3.0]);
    let answer = identify(
        &studio,
        project.request(json!({ "action": "importModel", "json": changed.to_string() })),
    );
    assert_eq!(
        refusal(&answer),
        ("A changed model needs a new revision.", "none"),
        "{answer}"
    );
    assert_eq!(
        fs::read(&study_file).expect("the study file"),
        before,
        "a changed model under the current revision leaves the study as it was (A8.4)"
    );
    assert!(
        nothing_started(&studio),
        "a supplied model starts no campaign (B3.7)"
    );
}

#[test]
fn import_model_accepts_the_same_model_under_the_current_revision_and_changes_nothing() {
    let project = Project::new("import-model-same");
    let (studio, _updates) = engine();
    identify(
        &studio,
        project.request(json!({
            "action": "importModel",
            "json": supplied_model("provided", "unvalidated").to_string()
        })),
    );
    let study_file = study_directory(&project.root, LOOP_ID).join("study.json");
    let before = fs::read(&study_file).expect("the study file");

    let again = identify(
        &studio,
        project.request(json!({
            "action": "importModel",
            "json": supplied_model("provided", "unvalidated").to_string()
        })),
    );
    assert_eq!(again["modelRevision"], "provided-course-model", "{again}");
    assert_eq!(again["provisional"], true, "{again}");
    assert_eq!(
        fs::read(&study_file).expect("the study file"),
        before,
        "the same model under the same revision changes nothing (A8.4)"
    );
}

/// A connected record of the example loop's plant, as an archived run of a saved result: `count` runtime samples, one second
/// apart, of the response of `plant_response` to a setpoint step of `size` percent at sample `step_at`. The record is what the
/// engine's builder makes (`connected_identification_record`), so the file holds what Control Studio keeps for a run it took.
fn connected_record_json(
    loop_definition: &LoopDefinition,
    recording_id: &str,
    step_at: u32,
    size: f64,
    count: u32,
) -> Value {
    let samples: Vec<RecordedSample> = (1..=count)
        .zip(plant_response(count, step_at, size))
        .map(|(scan, (command, process))| RecordedSample {
            scan: u64::from(scan),
            runtime_time_ns: Some(i64::from(scan - 1) * 1_000_000_000),
            captured_at_ms: 1_000 * u64::from(scan),
            values: vec![Some(50.0), Some(process), Some(command)],
            forced: vec![false; 3],
            quality: vec![Quality::Good; 3],
            gap: false,
        })
        .collect();
    let signals = &loop_definition.signals;
    let bind = |id: &str, symbol: &str| SignalBinding {
        id: id.to_string(),
        address: None,
        symbol: Some(symbol.to_string()),
        iec_type: "REAL".to_string(),
        unit: None,
    };
    let run = RecordedRun {
        recording_id: recording_id.to_string(),
        loop_revision: loop_definition.revision.clone(),
        program_revision: Some("sha256:program-plant".to_string()),
        bindings: vec![
            bind(
                SETPOINT_BINDING,
                signals
                    .setpoint
                    .as_ref()
                    .expect("the example loop has a setpoint")
                    .symbol
                    .as_str(),
            ),
            bind(
                PROCESS_VARIABLE_BINDING,
                signals.process_variable.symbol.as_str(),
            ),
            bind(
                MANIPULATED_VARIABLE_BINDING,
                signals.manipulated_variable.symbol.as_str(),
            ),
        ],
        stop_reason: StopReason::Stopped,
        samples,
        acquisition: Acquisition {
            method: "runtime_scan_buffer".to_string(),
            completeness: Completeness::Complete,
            observed_sample_count: u64::from(count),
            scan_range: Some(ScanRange {
                first: 1,
                last: u64::from(count),
            }),
            missing_scan_ranges: Vec::new(),
            dropped_observed_samples: 0,
            actual_cycle_timing_available: true,
            freshness_available: false,
            reason: "Complete recording.".to_string(),
        },
    };
    let file = RecordingFile::new(
        &loop_definition.id,
        EvidenceSource::SimulatedPlant,
        run.clone(),
        recording_evidence(&run, None),
    );
    let plan = opened_plan(&file, loop_definition);
    let record = connected_identification_record(
        loop_definition,
        &run,
        None,
        &plan,
        0.0,
        None,
        evaluate_recorded_limits,
    )
    .expect("the connected record of the archived run");
    serde_json::to_value(&record).expect("the record as JSON")
}

/// `plant_result` with an engineer's study whose one completed campaign holds `runs`: a schema 4 file of archived runs.
fn archive_result(loop_definition: &LoopDefinition, runs: &[Value]) -> Value {
    let mut file = plant_result(loop_definition, 60);
    file["engineerStudy"] = json!({
        "campaigns": [{ "id": "campaign-archive", "status": "completed", "runs": runs }]
    });
    file
}

#[test]
fn a_file_of_two_archived_runs_is_kept_and_fitted_with_independent_validation() {
    let project = Project::new("archive-pair");
    let (studio, _updates) = engine();
    let loop_definition = project.loop_definition();
    let file = archive_result(
        &loop_definition,
        &[
            connected_record_json(&loop_definition, "recording-estimation", 10, 5.0, 240),
            connected_record_json(&loop_definition, "recording-validation", 20, -5.0, 240),
        ],
    );
    let answer = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": file.to_string() })),
    );
    assert_eq!(answer["status"], "checked", "{answer}");
    assert_eq!(
        answer["identification"]["provisional"], false,
        "a model checked on an independent recording is not provisional (B3.5)"
    );
    assert_eq!(
        answer["validation"], "The model was checked against an independent recording.",
        "{answer}"
    );
    assert_eq!(answer["recordingCatalog"]["kind"], "archive", "{answer}");
    let study = project.working_study();
    assert_eq!(
        study.recordings.len(),
        2,
        "both archived runs are kept as recordings"
    );
    let stored = storage::read_identify(&project.root, &study)
        .expect("the study reads")
        .expect("the fit is kept, so reopening shows it");
    assert!(
        stored.validation.is_some(),
        "the fit keeps its independent recording"
    );
    assert_eq!(
        answer["recordingId"],
        stored.section.estimation_recording_id.as_str(),
        "{answer}"
    );
    assert!(nothing_started(&studio), "an import starts no operation");
}

#[test]
fn a_file_with_one_archived_run_keeps_it_and_asks_for_a_second_recording() {
    let project = Project::new("archive-one");
    let (studio, _updates) = engine();
    let loop_definition = project.loop_definition();
    let file = archive_result(
        &loop_definition,
        &[connected_record_json(
            &loop_definition,
            "recording-only",
            10,
            5.0,
            240,
        )],
    );
    let answer = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": file.to_string() })),
    );
    assert_eq!(answer["status"], "recording-needed", "{answer}");
    assert_eq!(
        answer["reason"], "Open an independent recording to check the plant model.",
        "{answer}"
    );
    assert_eq!(
        answer["recordingCatalog"]["selection"]["outputs"],
        json!(["P1.ProcessValue"]),
        "the loop's measurement is the default output"
    );
    assert!(
        answer["recordingCatalog"]["channels"]
            .as_array()
            .is_some_and(|channels| channels
                .iter()
                .any(|channel| channel["id"] == "P1.ProcessValue" && channel["unit"] == "°C")),
        "the catalogue names the channel with its unit: {answer}"
    );
    assert_eq!(
        project.working_study().recordings.len(),
        1,
        "the archived run is kept, so a second recording can be checked against it"
    );
    assert!(nothing_started(&studio), "an import starts no operation");
}

#[test]
fn a_second_file_with_one_archived_run_pairs_with_the_kept_run_of_the_first_and_is_fitted() {
    let project = Project::new("archive-second");
    let (studio, _updates) = engine();
    let loop_definition = project.loop_definition();
    let first_file = archive_result(
        &loop_definition,
        &[connected_record_json(
            &loop_definition,
            "recording-first",
            10,
            5.0,
            240,
        )],
    );
    let first = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": first_file.to_string() })),
    );
    assert_eq!(first["status"], "recording-needed", "{first}");
    assert_eq!(project.working_study().recordings.len(), 1, "{first}");

    let second_file = archive_result(
        &loop_definition,
        &[connected_record_json(
            &loop_definition,
            "recording-second",
            20,
            -5.0,
            240,
        )],
    );
    let second = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": second_file.to_string() })),
    );
    assert_eq!(second["status"], "checked", "{second}");
    assert_eq!(
        second["identification"]["provisional"], false,
        "the second run pairs with the first, so the fit is checked on an independent recording (B5.2)"
    );
    assert_eq!(
        second["recordingId"], first["recordingId"],
        "the first run, kept by the earlier import, is the estimation (openedRecordingIdentification.ts)"
    );
    let study = project.working_study();
    assert_eq!(study.recordings.len(), 2, "both files' runs are kept");
    let stored = storage::read_identify(&project.root, &study)
        .expect("the study reads")
        .expect("the fit is kept");
    assert_ne!(
        stored.section.validation_recording_id.as_deref(),
        Some(
            first["recordingId"]
                .as_str()
                .expect("the first recording id")
        ),
        "the validation is the second file's run"
    );
    assert!(nothing_started(&studio), "an import starts no operation");
}

#[test]
fn a_single_recording_answers_its_channels_and_the_study_keeps_the_selection() {
    let project = Project::new("import-catalogue");
    let (studio, updates) = engine();
    let answer = identify(
        &studio,
        project.request(json!({
            "action": "importRecording",
            "json": plant_result(&project.loop_definition(), 60).to_string()
        })),
    );
    assert_eq!(answer["status"], "checked", "{answer}");
    // The fit the import names ends before the test reads the study, and before the project folder is removed.
    let _import_fit = final_update(&updates);
    assert_eq!(
        answer["recordingCatalog"]["selection"],
        json!({ "inputs": ["P1.ManipulatedValue"], "outputs": ["P1.ProcessValue"] }),
        "the loop's own signals are the default selection"
    );
    let channels = answer["recordingCatalog"]["channels"]
        .as_array()
        .expect("the catalogue's channels");
    for (id, unit) in [("P1.ManipulatedValue", "%"), ("P1.ProcessValue", "°C")] {
        assert!(
            channels
                .iter()
                .any(|channel| channel["id"] == id && channel["unit"] == unit),
            "{id} with {unit} is in {answer}"
        );
    }
    let inputs = project
        .working_study()
        .section::<InputsSection>()
        .expect("the study reads")
        .expect("the study keeps its inputs");
    assert_eq!(
        inputs
            .recording_selection
            .map(|selection| selection.outputs),
        Some(vec!["P1.ProcessValue".to_string()]),
        "reopening shows the selection the study keeps"
    );
}

#[test]
fn open_recording_fits_the_selected_signals_and_refuses_a_channel_the_recording_lacks() {
    let project = Project::new("open-selection");
    let (studio, updates) = engine();
    let loop_definition = project.loop_definition();
    let imported = identify(
        &studio,
        project.request(json!({
            "action": "importRecording",
            "json": plant_result(&loop_definition, 240).to_string()
        })),
    );
    let recording_id = imported["recordingId"]
        .as_str()
        .expect("the kept recording")
        .to_string();
    // The engine fits the checked recording within the import (B3.6); the fit ends before the page's own fit can start.
    let _import_fit = final_update(&updates);
    let started = identify(
        &studio,
        project.request(json!({
            "action": "openRecording",
            "recordingId": recording_id,
            "inputs": ["P1.ManipulatedValue"],
            "outputs": ["P1.ProcessValue"]
        })),
    );
    assert!(started["operationId"].is_string(), "{started}");
    let update = final_update(&updates);
    assert_eq!(
        update.state,
        OperationState::Finished,
        "{:?}",
        update.failure
    );
    let stored = storage::read_identify(&project.root, &project.working_study())
        .expect("the study reads")
        .expect("the fit of the selected signals is kept");
    assert_eq!(
        stored.section.signals.outputs,
        ["P1.ProcessValue".to_string()],
        "the fit names the output it used"
    );
    let refused = identify(
        &studio,
        project.request(json!({
            "action": "openRecording",
            "recordingId": recording_id,
            "outputs": ["P1.Missing"]
        })),
    );
    assert_eq!(
        refusal(&refused),
        (
            "Choose input and output signals present in this recording.",
            "none"
        ),
        "{refused}"
    );
}

/// The project revision and the model revision of the isolated simulation the runs below ran on. The model revision is a digest,
/// as the runtime names a simulation's configuration; no connected record has one.
const ISOLATED_PROGRAM: &str = "sha256:program-simulated";
const ISOLATED_MODEL: &str = "sha256:simulation-plant";

/// An isolated simulation's run of the example loop, as an archived run of a saved result: `count` grid points one second apart,
/// with the setpoint step response of `plant_response` to a step of `size` percent at sample `step_at`. The frozen request, the run
/// and its verdict are written as the runtime writes them (E8.4), so `decode_isolated_run` reads the run as the engine's import
/// does (B10.4).
fn isolated_record_json(
    loop_definition: &LoopDefinition,
    id: &str,
    seed: u64,
    step_at: u32,
    size: f64,
    count: u32,
) -> Value {
    let (input, output) = (
        &loop_definition.signals.manipulated_variable,
        &loop_definition.signals.process_variable,
    );
    let samples: Vec<ValidationSample> = (0..count)
        .zip(plant_response(count, step_at, size))
        .map(|(index, (command, process))| ValidationSample {
            time_seconds: f64::from(index),
            sequence: u64::from(index) + 1,
            values: BTreeMap::from([
                (
                    input.symbol.clone(),
                    ValidationValue {
                        value: Some(command),
                        quality: ChannelQuality::Good,
                    },
                ),
                (
                    output.symbol.clone(),
                    ValidationValue {
                        value: Some(process),
                        quality: ChannelQuality::Good,
                    },
                ),
            ]),
        })
        .collect();
    let duration_ms = u64::from(count - 1) * 1_000;
    let run = ValidationRun {
        id: id.to_string(),
        evidence: RunEvidence::StExecuted,
        parameters: BTreeMap::new(),
        conditions: RunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            duration_seconds: f64::from(count - 1),
            sample_times_seconds: recorded_sample_times(1_000, duration_ms, 1),
            seed: u32::try_from(seed).expect("a small seed"),
            model_revision: ISOLATED_MODEL.to_string(),
            program_revision: ISOLATED_PROGRAM.to_string(),
            parameter_schema_revision: loop_definition.revision.clone(),
        },
        channels: vec![
            ValidationChannel {
                id: input.symbol.clone(),
                unit: input.unit.clone(),
            },
            ValidationChannel {
                id: output.symbol.clone(),
                unit: output.unit.clone(),
            },
        ],
        samples,
    };
    let requirement = RunRequirement {
        id: "output-limits".to_string(),
        channel: output.symbol.clone(),
        window: Window {
            start: 0.0,
            end: f64::from(count - 1),
        },
        check: RunCheck::Bounds {
            minimum: 0.0,
            maximum: 100.0,
        },
    };
    let evaluation = evaluate_recorded_run(&run, std::slice::from_ref(&requirement), false)
        .expect("the isolated run is valid");
    let request = json!({
        "execution": "isolated-simulation",
        "expected_project_revision": ISOLATED_PROGRAM,
        "step_ms": 1_000,
        "duration_ms": duration_ms,
        "seed": seed,
        "max_total_cycles": count,
        "wall_budget_ms": 60_000,
        "initial_values": {},
        "events": [],
        "observations": [
            { "symbol": input.symbol, "unit": input.unit },
            { "symbol": output.symbol, "unit": output.unit }
        ],
        "parameter_overrides": []
    });
    json!({
        "projectRevision": ISOLATED_PROGRAM,
        "request": request,
        "run": serde_json::to_value(&run).expect("the run as JSON"),
        "evaluation": serde_json::to_value(&evaluation).expect("the evaluation as JSON"),
    })
}

#[test]
fn a_file_of_two_isolated_runs_is_kept_and_fitted_with_independent_validation() {
    let project = Project::new("isolated-pair");
    let (studio, _updates) = engine();
    let loop_definition = project.loop_definition();
    let file = archive_result(
        &loop_definition,
        &[
            isolated_record_json(&loop_definition, "isolated-estimation", 1, 10, 5.0, 240),
            isolated_record_json(&loop_definition, "isolated-validation", 7, 20, -5.0, 240),
        ],
    );
    let answer = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": file.to_string() })),
    );
    assert_eq!(answer["status"], "checked", "{answer}");
    assert_eq!(
        answer["identification"]["provisional"], false,
        "a model checked on an independent isolated run is not provisional (B3.5, B5.2)"
    );
    assert_eq!(
        answer["validation"], "The model was checked against an independent recording.",
        "{answer}"
    );
    let study = project.working_study();
    assert_eq!(
        study.recordings.len(),
        2,
        "both isolated runs are kept as recordings"
    );
    for id in &study.recordings {
        let kept =
            storage::read_recording(&project.root, LOOP_ID, id).expect("the kept file reads back");
        assert_eq!(
            kept.run.acquisition.method, "isolated-simulation",
            "the recording keeps its own acquisition (E8.4)"
        );
        assert_eq!(
            kept.model_revision.as_deref(),
            Some(ISOLATED_MODEL),
            "the file keeps the model revision of its isolated run (B3.6)"
        );
    }
    assert!(nothing_started(&studio), "an import starts no operation");
}

#[path = "identify_import_opened_tests.rs"]
mod opened_tests;
