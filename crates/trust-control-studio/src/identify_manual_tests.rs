//! The manual identification of `controlStudio/identify` (the Recorded-data fit, B3.3, B3.6 and B3.7): a first-order recording is
//! checked, fitted, scored on a second recording and accepted; the accepted model is the study's current model, which Tune reads;
//! Reset clears the manual section; reopening restores the stage; and a cancelled fit keeps the previous state. No runtime runs here.

use std::fs;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::experiment::ControlExperimentKind;
use trust_control::identification::acquisition_settings::AcquisitionMethod;
use trust_control::identification::form_draft::IDENTIFICATION_FORM_KEYS;
use trust_control::identification::model_comparison::ComparisonPresentation;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{OperationState, OperationUpdate};
use trust_control::model::{decode_general_model, GeneralModel};
use trust_control::recording::evidence::recording_evidence;
use trust_control::recording::{EvidenceSource, RecordedRun, RecordingFile};
use trust_control::study::current_model::{current_model, CurrentModelSource};
use trust_control::study::identify::{IdentifyMethod, IdentifySection, IdentifySignals};
use trust_control::study::manual_identification::ManualIdentificationSection;
use trust_control::study::{Study, StudySection};

use super::*;
use crate::commands::{ControlStudio, IDENTIFY};
use crate::identify_import;
use crate::operation::UpdateSink;
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::{loops, storage};

/// The example loop of the identification tests, the runtime it names.
const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");
const LOOP_ID: &str = "control-pid-commissioning";
const RUNTIME_ID: &str = "simulator";
/// The plant input and the measured output of the example loop, by their symbols.
const PLANT_INPUT: &str = "P1.ManipulatedValue";
const PROCESS_VALUE: &str = "P1.ProcessValue";

/// The updates of an engine's operations, in the order the test receives them.
struct Updates(Mutex<mpsc::Sender<OperationUpdate>>);

impl UpdateSink for Updates {
    fn send(&self, update: OperationUpdate) {
        let _ = self.0.lock().send(update);
    }
}

/// An engine whose operation updates the test receives.
fn engine() -> (ControlStudio, mpsc::Receiver<OperationUpdate>) {
    let (sender, receiver) = mpsc::channel();
    (
        ControlStudio::new(Arc::new(Updates(Mutex::new(sender)))),
        receiver,
    )
}

/// The update that ends an operation: its result, its failure or its cancellation.
fn final_update(updates: &mpsc::Receiver<OperationUpdate>) -> OperationUpdate {
    loop {
        let update = updates
            .recv_timeout(Duration::from_secs(30))
            .expect("an update of the operation");
        if update.state != OperationState::Running {
            return update;
        }
    }
}

/// The process value and the manipulated value of sample `index` (every 0.5 s) of the first-order step record of `from` to `to`:
/// the manipulated value steps at 5 s, and the process value starts at 20 and follows it with gain 0.5 and time constant 5 s.
fn step_values(index: u32, from: f64, to: f64) -> (f64, f64) {
    let time = f64::from(index) * 0.5;
    let shape = if time < 5.0 {
        0.0
    } else {
        1.0 - (-(time - 5.0) / 5.0).exp()
    };
    let manipulated = if time < 5.0 { from } else { to };
    (20.0 + 0.5 * (to - from) * shape, manipulated)
}

/// A first-order step record of the example loop, sampled every 0.5 s for 60 s: the process value starts at 20 and, after the
/// manipulated value steps from `from` to `to` at 5 s, follows it with gain 0.5 and time constant 5 s.
fn step_run(name: &str, from: f64, to: f64) -> RecordedRun {
    let samples: Vec<Value> = (0..120_u32)
        .map(|index| {
            let (process, manipulated) = step_values(index, from, to);
            json!({
                "scan": u64::from(index) + 1,
                "runtime_time_ns": 1_000_000_000_u64 + u64::from(index) * 500_000_000,
                "captured_at_ms": u64::from(index) * 500,
                "values": [process, manipulated],
                "forced": [false, false],
                "quality": ["good", "good"]
            })
        })
        .collect();
    serde_json::from_value(json!({
        "recording_id": name, "loop_revision": "sha256:loop-1",
        "bindings": [
            { "id": "processVariable", "address": "%IL0", "iec_type": "LREAL" },
            { "id": "manipulatedVariable", "address": "%QL0", "iec_type": "LREAL" }
        ],
        "stop_reason": "sample_limit",
        "samples": samples,
        "acquisition": { "method": "runtime_scan_buffer", "completeness": "complete", "observedSampleCount": 120,
            "scanRange": { "first": 1, "last": 120 }, "missingScanRanges": [], "droppedObservedSamples": 0,
            "actualCycleTimingAvailable": true, "freshnessAvailable": false, "reason": "" }
    }))
    .expect("a recorded run")
}

/// The Recorded-data fit's form for a first-order model without held-step settings or exclusions.
fn first_order_form() -> Value {
    let values: serde_json::Map<String, Value> = IDENTIFICATION_FORM_KEYS
        .iter()
        .map(|key| ((*key).to_string(), json!("")))
        .collect();
    json!({
        "schemaVersion": 1, "model": "first_order", "heldStep": false, "values": values,
        "excludedStart": "", "excludedEnd": ""
    })
}

/// A project folder of one test with the example loop, removed when the test ends.
struct Project {
    root: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Project {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-identify-manual-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("storage")).expect("the storage folder");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("the loop file");
        Self { root }
    }

    fn loop_definition(&self) -> LoopDefinition {
        loops::open(&self.root, RUNTIME_ID, LOOP_ID).expect("the example loop opens")
    }

    fn study(&self) -> Study {
        let loop_definition = self.loop_definition();
        storage::open_study(&self.root, &loop_definition.id, &loop_definition.revision)
            .expect("the working study")
    }

    /// Keeps the recording of `run` as the study's opened recording, and returns its identifier.
    fn keep(&self, run: RecordedRun) -> String {
        let evidence = recording_evidence(&run, None);
        let mut file = RecordingFile::new(LOOP_ID, EvidenceSource::SimulatedPlant, run, evidence);
        file.opened = true;
        let id = file.id().expect("the recording identifier");
        storage::save_recording(&self.root, &self.study(), &file).expect("the recording is kept");
        id
    }

    /// The request of `action` on the loop; the manual actions need no runtime connection.
    fn request(&self, action: &Value) -> Value {
        json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": RUNTIME_ID,
            "loopId": LOOP_ID,
            "storageFolder": self.root.join("storage").display().to_string(),
            "action": action,
        })
    }
}

/// The engine's answer to `action`, which must be an answer at once: the `result` of a request that succeeded, which `execute` wraps
/// as `{"result": …}`, or the `failure` envelope of a refusal, which the refusal tests read (as `identify_import_tests.rs` does).
fn answer(studio: &ControlStudio, project: &Project, action: &Value) -> Value {
    let envelope = studio
        .execute(IDENTIFY, &[project.request(action)])
        .expect("the engine answers its own method");
    envelope.get("result").cloned().unwrap_or(envelope)
}

/// The result of the manual operation `action` starts, which must finish with a result.
fn finished(
    studio: &ControlStudio,
    updates: &mpsc::Receiver<OperationUpdate>,
    project: &Project,
    action: &Value,
) -> Value {
    let started = answer(studio, project, action);
    assert!(started["operationId"].is_string(), "{started}");
    let end = final_update(updates);
    assert_eq!(end.state, OperationState::Finished, "{end:?}");
    end.result.expect("the operation's result")
}

/// The status of `stage` in an answer's stage list, as the engine writes it (`stages`); `null` when the answer names no such stage.
fn stage_status(answer: &Value, stage: &str) -> Value {
    answer["stages"]
        .as_array()
        .and_then(|stages| stages.iter().find(|entry| entry["stage"] == stage))
        .map_or(Value::Null, |entry| entry["status"].clone())
}

/// The state of the loop's manual section in the study, as its JSON.
fn manual_section_json(project: &Project) -> Option<Value> {
    project
        .study()
        .sections
        .get(ManualIdentificationSection::KEY)
        .cloned()
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

/// The loop of lesson 26: a project-scripted experiment of the simulator, whose Configured method is the project test.
const PROJECT_SCRIPTED_LOOP: &str = include_str!(
    "../../../editors/vscode/media/examples/control/05-experiments-identification-and-validation/26-independent-model-validation/control-loop.json"
);

/// The text of a saved Control Studio recording file (schema 4, B10.2) of the step record `name`, of the project loop's revision: the
/// file an engineer opens with **Open recording…**, which `importRecording` and `manualImport` check (B10.4, A9).
fn recording_text(project: &Project, name: &str, from: f64, to: f64) -> String {
    let loop_definition = project.loop_definition();
    let samples: Vec<Value> = (0..120_u32)
        .map(|index| {
            let (process, manipulated) = step_values(index, from, to);
            json!({
                "elapsedSeconds": f64::from(index) * 0.5,
                "observedAtMs": 500.0 * f64::from(index + 1),
                "actualCycleSeconds": 0.5,
                "scan": index + 1,
                "source": "Simulation",
                "loopRevision": loop_definition.revision,
                "sp": 50.0,
                "pv": process,
                "requestedMv": manipulated,
                "appliedMv": manipulated,
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
        "resultId": format!("result-{name}"),
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
            "selectedIntervals": [{ "startSeconds": 0.0, "endSeconds": 59.5 }],
            "acquisition": {
                "method": "runtime_scan_buffer",
                "completeness": "complete",
                "recordingId": name,
                "observedSampleCount": 120,
                "scanRange": { "first": 1, "last": 120 },
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
    .to_string()
}

#[test]
fn importing_a_recording_selects_it_for_the_next_check_and_names_it() {
    let project = Project::new("import");
    let (studio, updates) = engine();
    let text = recording_text(&project, "imported", 50.0, 60.0);
    let imported = answer(
        &studio,
        &project,
        &json!({"action": "manualImport", "json": text}),
    );
    assert_eq!(imported["phase"], json!("recording-selected"), "{imported}");
    assert_eq!(
        imported["primaryAction"],
        json!("checkRecording"),
        "{imported}"
    );
    assert_eq!(
        imported["message"],
        json!("Recording opened. Check it before fitting a model."),
        "{imported}"
    );
    let recording_id = imported["recordingId"]
        .as_str()
        .expect("the recording the next check reads")
        .to_string();
    assert!(recording_id.starts_with("rec-"), "{imported}");
    assert!(
        project.study().recordings.contains(&recording_id),
        "the study lists the recording"
    );
    let checked = answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": recording_id, "form": first_order_form()}),
    );
    assert_eq!(checked["phase"], json!("qualified"), "{checked}");
    assert_eq!(checked["recordingId"], json!(recording_id), "{checked}");
    let restored = finished(
        &studio,
        &updates,
        &project,
        &json!({"action": "manualRestore"}),
    );
    assert_eq!(restored["recordingId"], json!(recording_id), "{restored}");
}

#[test]
fn a_validation_import_is_selected_for_the_validation_role_and_the_preview_names_no_recording_yet()
{
    let project = Project::new("validation-import");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let (studio, updates) = engine();
    answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    let preview = finished(&studio, &updates, &project, &json!({"action": "manualFit"}));
    assert_eq!(preview["phase"], json!("validation_required"), "{preview}");
    assert!(preview["recordingId"].is_null(), "{preview}");
    let text = recording_text(&project, "validation", 40.0, 55.0);
    let imported = answer(
        &studio,
        &project,
        &json!({"action": "manualImport", "json": text}),
    );
    assert_eq!(imported["phase"], json!("recording-selected"), "{imported}");
    assert_eq!(imported["nextFitRole"], json!("validation"), "{imported}");
    assert_ne!(imported["recordingId"], json!(estimation), "{imported}");
    assert!(imported["recordingId"].is_string(), "{imported}");
}

#[test]
fn a_refused_import_answers_its_sentence_and_keeps_no_recording() {
    let project = Project::new("refused-import");
    let (studio, _updates) = engine();
    let before = project.study().recordings.len();
    let refused = answer(
        &studio,
        &project,
        &json!({"action": "manualImport", "json": "not a recording"}),
    );
    assert!(
        refused["failure"]["message"]
            .as_str()
            .is_some_and(|text| !text.is_empty()),
        "{refused}"
    );
    assert_eq!(
        project.study().recordings.len(),
        before,
        "the study keeps nothing"
    );
}

#[test]
fn a_held_step_or_exclusion_refusal_names_the_form_key_it_refuses() {
    let project = Project::new("form-field");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let (studio, _updates) = engine();
    let mut held = first_order_form();
    held["heldStep"] = json!(true);
    held["model"] = json!("fopdt");
    for key in IDENTIFICATION_FORM_KEYS {
        held["values"][key] = json!("5");
    }
    held["values"]["onsetSamples"] = json!("2.5");
    let refused = answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": held}),
    );
    assert_eq!(
        refused["failure"]["field"],
        json!("onsetSamples"),
        "{refused}"
    );
    assert_eq!(
        refused["failure"]["message"],
        json!("onsetSamples: Enter a whole number from 1 to 100."),
        "{refused}"
    );
    let mut excluded = first_order_form();
    excluded["excludedStart"] = json!("2");
    excluded["excludedEnd"] = json!("1");
    let refused = answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": excluded}),
    );
    assert_eq!(
        refused["failure"]["field"],
        json!("excludedEnd"),
        "{refused}"
    );
}

#[test]
fn a_project_test_while_the_manual_identification_awaits_its_recording_selects_it_and_keeps_no_model(
) {
    let project = Project::new("project-test");
    let estimation = project.keep(step_run("estimation", 50.0, 60.0));
    let (studio, updates) = engine();
    answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": estimation, "form": first_order_form()}),
    );
    finished(&studio, &updates, &project, &json!({"action": "manualFit"}));
    let test_recording = project.keep(step_run("project-test", 40.0, 55.0));
    let work = LoopWork {
        project_root: project.root.clone(),
        storage_folder: project.root.join("storage"),
        pending: Arc::new(PendingReleases::new()),
        loop_definition: project.loop_definition(),
    };
    let kept = keep_project_test_recording(&work, &project.study(), &test_recording)
        .expect("the project test's recording is kept for the check");
    assert_eq!(kept["phase"], json!("recording-selected"), "{kept}");
    assert_eq!(kept["recordingId"], json!(test_recording), "{kept}");
    assert_eq!(kept["nextFitRole"], json!("validation"), "{kept}");
    assert_eq!(kept["primaryAction"], json!("checkRecording"), "{kept}");
    assert_eq!(kept["message"], json!(PROJECT_TEST_NOTE), "{kept}");
    assert!(
        project
            .study()
            .section::<IdentifySection>()
            .expect("the study reads")
            .is_none(),
        "the project test's own model is not kept"
    );
    assert_eq!(
        manual_section_json(&project).expect("the manual section")["phase"],
        json!("selected")
    );
}

#[test]
fn the_project_test_is_the_automatic_or_declared_run_of_a_project_scripted_loop() {
    let project = Project::new("project-loop");
    fs::write(
        project.root.join("control-loop.json"),
        PROJECT_SCRIPTED_LOOP,
    )
    .expect("the lesson's loop file");
    let scripted = loops::open(&project.root, RUNTIME_ID, "control-lesson-26")
        .expect("the project-scripted loop opens");
    // The page's project test asks for the Automatic method, which runs the project's scenario (C12); the declared one does too.
    assert!(is_project_test(&scripted, AcquisitionMethod::Automatic));
    assert!(is_project_test(&scripted, AcquisitionMethod::Configured));
    assert!(!is_project_test(&scripted, AcquisitionMethod::MvStep));
    let example = Project::new("example-loop");
    assert!(!is_project_test(
        &example.loop_definition(),
        AcquisitionMethod::Automatic
    ));
    assert!(!is_project_test(
        &example.loop_definition(),
        AcquisitionMethod::Configured
    ));
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
