//! The manual identification of `controlStudio/identify` (the Recorded-data fit, B3.3, B3.6 and B3.7): a first-order recording is
//! checked, fitted, scored on a second recording and accepted; the accepted model is the study's current model, which Tune reads;
//! Reset clears the manual section; reopening restores the stage; and a cancelled fit keeps the previous state. No runtime runs here.

use std::fs;
use std::sync::{mpsc, Arc};

use serde_json::{json, Value};
use trust_control::experiment::ControlExperimentKind;
use trust_control::identification::model_comparison::ComparisonPresentation;
use trust_control::messages::OperationUpdate;
use trust_control::model::{decode_general_model, GeneralModel};
use trust_control::recording::EvidenceSource;
use trust_control::study::current_model::{current_model, CurrentModelSource};
use trust_control::study::identify::{IdentifyMethod, IdentifySection, IdentifySignals};

use super::test_support::{
    answer, engine, finished, first_order_form, manual_section_json, step_run, Project, LOOP_ID,
    RUNTIME_ID,
};
use super::*;
use crate::commands::{ControlStudio, OPEN};
use crate::identify_import;
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::storage;

/// The plant input and the measured output of the example loop, by their symbols.
const PLANT_INPUT: &str = "P1.ManipulatedValue";
const PROCESS_VALUE: &str = "P1.ProcessValue";

/// The status of `stage` in an answer's stage list, as the engine writes it (`stages`); `null` when the answer names no such stage.
fn stage_status(answer: &Value, stage: &str) -> Value {
    answer["stages"]
        .as_array()
        .and_then(|stages| stages.iter().find(|entry| entry["stage"] == stage))
        .map_or(Value::Null, |entry| entry["status"].clone())
}

/// The `controlStudio/open` answer of the example loop of `project`.
fn opened(studio: &ControlStudio, project: &Project) -> Value {
    let envelope = studio
        .execute(
            OPEN,
            &[json!({
                "projectRoot": project.root.display().to_string(),
                "runtimeId": RUNTIME_ID,
                "loopId": LOOP_ID,
                "storageFolder": project.root.join("storage").display().to_string(),
            })],
        )
        .expect("the engine answers its own method");
    envelope["result"].clone()
}

#[test]
fn a_manual_fit_of_a_first_order_recording_is_accepted_and_becomes_the_current_model_that_tune_reads(
) {
    let project = Project::new("accept");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let validation = project.keep(step_run("validation", 40.0, 55.0));
    let (studio, updates) = engine();

    let edited = answer(
        &studio,
        &project,
        &json!({"action": "manualEdit", "form": first_order_form()}),
    );
    assert_eq!(edited["phase"], json!("editing"), "{edited}");

    let checked = answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    assert_eq!(checked["phase"], json!("qualified"), "{checked}");
    assert_eq!(checked["primaryAction"], json!("fitPlant"), "{checked}");

    let preview = finished(&studio, &updates, &project, &json!({"action": "manualFit"}));
    assert_eq!(preview["phase"], json!("validation_required"), "{preview}");
    assert_eq!(
        preview["primaryAction"],
        json!("openValidationRecording"),
        "{preview}"
    );

    let scored = answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": validation, "form": first_order_form()}),
    );
    assert_eq!(
        scored["primaryAction"],
        json!("compareWithModel"),
        "{scored}"
    );

    let review = finished(&studio, &updates, &project, &json!({"action": "manualFit"}));
    assert_eq!(review["phase"], json!("acceptance_required"), "{review}");
    assert_eq!(review["acceptanceReady"], json!(true), "{review}");

    let accepted = answer(&studio, &project, &json!({"action": "manualAccept"}));
    assert_eq!(accepted["phase"], json!("accepted"), "{accepted}");
    assert_eq!(
        accepted["primaryActionLabel"],
        json!("Model ready"),
        "{accepted}"
    );

    let study = project.study();
    let current = current_model(&study)
        .expect("the study reads")
        .expect("a current model");
    assert_eq!(current.source, CurrentModelSource::Identify);
    assert_eq!(study.model_source, Some(CurrentModelSource::Identify));
}

#[test]
fn reopening_restores_the_accepted_manual_identification_and_reset_clears_it() {
    let project = Project::new("reopen");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let validation = project.keep(step_run("validation", 40.0, 55.0));
    let (studio, updates) = engine();
    answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    finished(&studio, &updates, &project, &json!({"action": "manualFit"}));
    answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": validation, "form": first_order_form()}),
    );
    finished(&studio, &updates, &project, &json!({"action": "manualFit"}));
    answer(&studio, &project, &json!({"action": "manualAccept"}));

    // A new engine process reads the study again: the replay restores the accepted stage from the kept recordings.
    let (reopened_studio, reopened_updates) = engine();
    let restored = finished(
        &reopened_studio,
        &reopened_updates,
        &project,
        &json!({"action": "manualRestore"}),
    );
    assert_eq!(restored["phase"], json!("accepted"), "{restored}");
    assert_eq!(
        restored["stageEvidence"]["hasModel"],
        json!(true),
        "{restored}"
    );

    let reset = answer(&reopened_studio, &project, &json!({"action": "reset"}));
    assert!(reset["stages"].is_array(), "{reset}");
    assert_eq!(manual_section_json(&project), None);
    let empty = finished(
        &reopened_studio,
        &reopened_updates,
        &project,
        &json!({"action": "manualRestore"}),
    );
    assert_eq!(empty["primaryAction"], json!("openRecording"), "{empty}");
    assert_eq!(empty["stageEvidence"]["hasModel"], json!(false), "{empty}");
}

#[test]
fn a_cancelled_fit_keeps_the_previous_state_and_a_fit_that_runs_keeps_its_result() {
    let project = Project::new("cancel");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let (studio, _updates) = engine();
    answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    let before = manual_section_json(&project).expect("the section after the check");

    let loop_definition = project.loop_definition();
    let work = LoopWork {
        project_root: project.root.clone(),
        storage_folder: project.root.join("storage"),
        pending: Arc::new(PendingReleases::new()),
        loop_definition,
    };
    let cancelled = fit_unless_cancelled(&work, &|| true);
    assert!(
        cancelled.is_err(),
        "a cancelled fit answers its cancellation"
    );
    assert_eq!(manual_section_json(&project), Some(before.clone()));

    let fitted = fit_unless_cancelled(&work, &|| false).expect("a fit that runs answers");
    assert_eq!(fitted["phase"], json!("validation_required"), "{fitted}");
    assert_ne!(manual_section_json(&project), Some(before));
}

/// Runs the Recorded-data fit of two first-order recordings to the accepted stage of a fresh study.
fn accept_first_order(
    project: &Project,
    studio: &ControlStudio,
    updates: &mpsc::Receiver<OperationUpdate>,
) {
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let validation = project.keep(step_run("validation", 40.0, 55.0));
    answer(
        studio,
        project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    finished(studio, updates, project, &json!({"action": "manualFit"}));
    answer(
        studio,
        project,
        &json!({"action": "manualQualify", "recordingId": validation, "form": first_order_form()}),
    );
    finished(studio, updates, project, &json!({"action": "manualFit"}));
    let accepted = answer(studio, project, &json!({"action": "manualAccept"}));
    assert_eq!(accepted["phase"], json!("accepted"), "{accepted}");
}

/// A first-order model of the example loop from the manipulated value to the process value, as the automatic identification keeps it
/// (the fixture of `identify_action_tests.rs`).
fn fitted_model() -> GeneralModel {
    let channel = |id: &str, unit: &str| json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -10.0, "maximum": 10.0 } });
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "model:fitted:1", "source": "fitted",
        "validation": "unvalidated", "evidence": ["sha256:recording"], "delaySeconds": 0.0,
        "inputs": [channel("command", "V")], "outputs": [channel("position", "m")],
        "numerator": [1.0], "denominator": [1.0, 1.0]
    }))
    .expect("a fitted model")
}

/// What the page shows of the fitted model (the presentation of `identify_action_tests.rs`).
fn presentation() -> ComparisonPresentation {
    trust_control::json::decode_value(json!({
        "equation": "G(s) = 1 / (1s + 1)",
        "units": { "input": "V", "output": "m", "gain": "m/V", "time": "s" },
        "parameters": [
            { "name": "Gain", "value": 1.0, "unit": "m/V" },
            { "name": "Time constant 1", "value": 1.0, "unit": "s" },
            { "name": "Dead time", "value": 0.0, "unit": "s" }
        ],
        "poles": [-1.0], "zeros": [], "delaySeconds": 0.0,
        "uncertainty": { "label": "Sample-resolution envelope from the estimation record; not statistical confidence" },
        "estimation": { "normalizedError": 0.1 }, "validation": {},
        "predictedResponse": [
            { "timeSeconds": 0.0, "measured": 0.0, "predicted": 0.0, "residual": 0.0 },
            { "timeSeconds": 2.0, "measured": 1.0, "predicted": 0.9, "residual": 0.1 }
        ],
        "validityRange": { "inputMinimum": 0.0, "inputMaximum": 1.0, "outputMinimum": 0.0, "outputMaximum": 1.0 },
        "warnings": []
    }))
    .expect("the presentation decodes")
}

/// A supplied model of the course's form, checked as B3.7 requires (the fixture of `identify_import_tests.rs`).
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
fn an_automatic_identify_result_clears_the_manual_section_in_the_same_save() {
    let project = Project::new("automatic-keep");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let (studio, _updates) = engine();
    answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    assert!(
        manual_section_json(&project).is_some(),
        "the manual section before the automatic result"
    );
    let section = IdentifySection::new(
        IdentifyMethod::Live(ControlExperimentKind::MvStep),
        EvidenceSource::SimulatedPlant,
        IdentifySignals {
            inputs: vec![PLANT_INPUT.to_string()],
            outputs: vec![PROCESS_VALUE.to_string()],
            setpoint: None,
        },
        &estimation,
        None,
        fitted_model(),
        presentation(),
    );
    storage::save_identify(&project.root, &project.study(), &section, &[])
        .expect("the automatic result is kept");
    assert_eq!(
        manual_section_json(&project),
        None,
        "the same save cleared the manual section"
    );
    let study = project.study();
    assert_eq!(study.model_source, Some(CurrentModelSource::Identify));
    assert!(current_model(&study).expect("the study reads").is_some());
}

#[test]
fn a_supplied_model_clears_the_manual_section_in_the_same_save() {
    let project = Project::new("supplied");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let (studio, _updates) = engine();
    answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    assert!(
        manual_section_json(&project).is_some(),
        "the manual section before the supplied model"
    );
    identify_import::import_model(
        &project.root,
        &project.loop_definition(),
        &supplied_model("provided", "validated").to_string(),
    )
    .expect("the supplied model is kept");
    assert_eq!(
        manual_section_json(&project),
        None,
        "the same save cleared the manual section"
    );
    assert_eq!(
        project.study().model_source,
        Some(CurrentModelSource::Supplied)
    );
}

#[test]
fn a_manual_accept_keeps_the_accepted_section_beside_the_model_it_made_current() {
    let project = Project::new("accept-keeps");
    let (studio, updates) = engine();
    accept_first_order(&project, &studio, &updates);
    let kept = manual_section_json(&project).expect("the accepted section stays in the study");
    assert_eq!(kept["phase"], json!("accepted"), "{kept}");
    let study = project.study();
    assert_eq!(study.model_source, Some(CurrentModelSource::Identify));
    assert!(current_model(&study).expect("the study reads").is_some());
}

#[test]
fn the_manual_answers_name_the_study_stages_so_tune_is_available_only_after_accept() {
    let project = Project::new("stage-evidence");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let validation = project.keep(step_run("validation", 40.0, 55.0));
    let (studio, updates) = engine();
    answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    finished(&studio, &updates, &project, &json!({"action": "manualFit"}));
    answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": validation, "form": first_order_form()}),
    );
    let review = finished(&studio, &updates, &project, &json!({"action": "manualFit"}));
    // Before acceptance the study holds no current model (B3.7), so Tune needs its data and the page keeps Tune disabled.
    assert_eq!(
        review["stageEvidence"]["hasModel"],
        json!(false),
        "{review}"
    );
    assert_eq!(
        stage_status(&review, "Tune"),
        json!("needs-data"),
        "{review}"
    );

    let accepted = answer(&studio, &project, &json!({"action": "manualAccept"}));
    // After acceptance the accepted model is the study's current model: Identify is complete and Tune is ready.
    assert_eq!(
        accepted["stageEvidence"]["hasModel"],
        json!(true),
        "{accepted}"
    );
    assert_eq!(
        stage_status(&accepted, "Identify"),
        json!("complete"),
        "{accepted}"
    );
    assert_eq!(
        stage_status(&accepted, "Tune"),
        json!("ready"),
        "{accepted}"
    );
}

// Specification 35, B3.8; Identify page specification, "Identified model and plot": the review's presentation carries the engine's words
// for the fitted model (its type, its key numbers and its equation's fraction), which the page shows and does not compose.
#[test]
fn the_fitted_model_is_shown_with_the_engines_words() {
    use trust_control::identification::presentation::{
        identified_equation_fraction, identified_model_key_numbers, identified_model_type_text,
        IdentifiedModelPresentation,
    };
    let project = Project::new("words");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let (studio, updates) = engine();
    answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    let preview = finished(&studio, &updates, &project, &json!({"action": "manualFit"}));
    let shown: IdentifiedModelPresentation =
        serde_json::from_value(preview["presentation"].clone())
            .unwrap_or_else(|error| panic!("the presentation reads: {error}: {preview}"));
    assert_eq!(
        shown.model_type,
        Some(identified_model_type_text(&shown)),
        "{preview}"
    );
    assert_eq!(
        shown.key_numbers,
        Some(identified_model_key_numbers(&shown)),
        "{preview}"
    );
    assert_eq!(
        shown.equation_fraction,
        identified_equation_fraction(&shown),
        "{preview}"
    );
    assert!(
        shown.equation_fraction.is_some(),
        "the fitted model has a gain: {preview}"
    );
    assert!(
        shown
            .key_numbers
            .as_deref()
            .is_some_and(|line| line.starts_with("Gain ")),
        "{preview}"
    );
}

// Specification 35, B3.3: the evidence rows of a fit are the engine's numbers. The answer states the largest absolute residual of the
// fit's samples beside the fit's root mean square error, and the page computes neither; an answer that shows no fit states none.
#[test]
fn the_fit_answer_states_the_largest_absolute_residual_of_its_samples() {
    let project = Project::new("residual");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let (studio, updates) = engine();
    let checked = answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    assert!(
        checked.get("maximumAbsoluteResidual").is_none(),
        "a check shows no fit: {checked}"
    );
    let preview = finished(&studio, &updates, &project, &json!({"action": "manualFit"}));
    let residuals: Vec<f64> = preview["fit"]["points"]
        .as_array()
        .expect("the fit holds its samples")
        .iter()
        .map(|point| point["residual"].as_f64().expect("a residual").abs())
        .collect();
    assert!(!residuals.is_empty(), "{preview}");
    let largest = residuals.iter().copied().fold(0.0, f64::max);
    assert_eq!(
        preview["maximumAbsoluteResidual"].as_f64(),
        Some(largest),
        "{preview}"
    );
}

// Specification 35, B3.3: a study that keeps no manual section answers the default form, so the page shows the engine's numbers.
#[test]
fn restoring_a_study_without_a_manual_section_answers_the_default_form() {
    use trust_control::identification::form_draft::default_form;
    let project = Project::new("default-form");
    let (studio, updates) = engine();
    let restored = finished(
        &studio,
        &updates,
        &project,
        &json!({"action": "manualRestore"}),
    );
    assert_eq!(
        restored["form"],
        serde_json::to_value(default_form()).expect("the form encodes"),
        "{restored}"
    );
    assert_eq!(
        restored["primaryAction"],
        json!("openRecording"),
        "{restored}"
    );
}

// Specification 35, B3.3: a reopened study answers the form it kept, so the held-step controls show the numbers the engineer typed
// and not the default numbers.
#[test]
fn reopening_a_study_answers_the_form_it_kept_and_not_the_default_numbers() {
    use trust_control::identification::form_draft::default_form;
    let project = Project::new("kept-form");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let (studio, _updates) = engine();
    let mut typed = first_order_form();
    typed["values"]["gainStep"] = json!("0.25");
    answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": typed}),
    );

    // A new engine process reads the study again.
    let (reopened, updates) = engine();
    let restored = finished(
        &reopened,
        &updates,
        &project,
        &json!({"action": "manualRestore"}),
    );
    assert_eq!(restored["form"], typed, "{restored}");
    assert_ne!(
        restored["form"],
        serde_json::to_value(default_form()).expect("the form encodes"),
        "{restored}"
    );
}

/// A8.6, B3.3: the open answer carries the Recorded-data fit the study keeps, so the page asks for no replay. A study that keeps none
/// carries the engine's start state with its default form; a study that keeps an accepted fit carries that stage, replayed from the
/// kept recordings, equal to what the engine's `manualRestore` answers; and a reset answers the start state of the new study. Catches
/// an open answer without the state (the page would have to decide when to replay), and a reset that leaves the page the old state.
#[test]
fn open_answer_carries_the_recorded_data_fit_state() {
    use trust_control::identification::form_draft::default_form;
    let project = Project::new("open-fit-state");
    let (studio, updates) = engine();
    let default = serde_json::to_value(default_form()).expect("the form encodes");
    let fresh = opened(&studio, &project);
    assert_eq!(
        fresh["manualIdentification"]["primaryAction"],
        json!("openRecording"),
        "{fresh}"
    );
    assert_eq!(fresh["manualIdentification"]["form"], default, "{fresh}");

    accept_first_order(&project, &studio, &updates);
    // A new engine process reads the study again.
    let (reopened_studio, reopened_updates) = engine();
    let reopened = opened(&reopened_studio, &project);
    let state = &reopened["manualIdentification"];
    assert_eq!(state["phase"], json!("accepted"), "{reopened}");
    assert_eq!(
        state["primaryActionLabel"],
        json!("Model ready"),
        "{reopened}"
    );
    assert_eq!(
        state["stageEvidence"]["hasModel"],
        json!(true),
        "{reopened}"
    );
    let restored = finished(
        &reopened_studio,
        &reopened_updates,
        &project,
        &json!({"action": "manualRestore"}),
    );
    assert_eq!(
        *state, restored,
        "the open answer is the restoration's answer"
    );

    let reset = answer(&reopened_studio, &project, &json!({"action": "reset"}));
    let start = &reset["manualIdentification"];
    assert_eq!(start["primaryAction"], json!("openRecording"), "{reset}");
    assert_eq!(start["form"], default, "{reset}");
    assert_eq!(start["hasRecording"], json!(false), "{reset}");
}

/// A8.6: a saved fit whose recording is gone cannot be replayed. The open answer still opens the loop, and carries the fit as a failed
/// stage that says why and offers to open a recording, so the sentence is not lost. Catches an open that fails for it.
#[test]
fn an_open_answer_names_a_saved_fit_that_cannot_be_replayed_and_still_opens() {
    let project = Project::new("open-fit-lost-recording");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let (studio, _updates) = engine();
    let checked = answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    assert_eq!(checked["phase"], json!("qualified"), "{checked}");
    fs::remove_file(
        storage::study_directory(&project.root, LOOP_ID)
            .join("recordings")
            .join(format!("{estimation}.json")),
    )
    .expect("the recording file is removed");
    let (reopened_studio, _updates) = engine();
    let reopened = opened(&reopened_studio, &project);
    assert_eq!(reopened["kind"], json!("opened"), "{reopened}");
    let state = &reopened["manualIdentification"];
    assert_eq!(state["status"], json!("failed"), "{reopened}");
    assert_eq!(state["primaryAction"], json!("openRecording"), "{reopened}");
    assert!(
        state["reason"]
            .as_str()
            .is_some_and(|reason| !reason.is_empty()),
        "the answer says why: {reopened}"
    );
}
