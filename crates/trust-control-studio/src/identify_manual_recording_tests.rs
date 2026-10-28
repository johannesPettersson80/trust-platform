//! The recordings that the manual identification checks, the refusals of its form, and the project test that supplies its independent
//! recording (the Recorded-data fit, B3.3, B3.6, B3.7 and C12). No runtime runs here.

use std::fs;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::identification::acquisition_settings::AcquisitionMethod;
use trust_control::identification::form_draft::IDENTIFICATION_FORM_KEYS;
use trust_control::study::identify::IdentifySection;

use super::test_support::{
    answer, engine, finished, first_order_form, manual_section_json, step_run, step_values,
    Project, LOOP_ID, RUNTIME_ID,
};
use super::*;
use crate::loops;
use crate::runtime_client::runtime_recording::PendingReleases;

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

/// B3.6: a check that names no recording, while the study keeps no opened recording the check could read, is refused by the engine
/// with its sentence, and keeps nothing.
#[test]
fn a_check_that_names_no_kept_recording_is_refused_with_its_sentence_and_keeps_nothing() {
    let project = Project::new("check-without-recording");
    let (studio, _updates) = engine();
    let before = project.study().recordings.len();
    let refused = answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "form": first_order_form()}),
    );
    assert_eq!(
        refused["failure"]["message"],
        json!("Open a recording before checking it."),
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
fn a_held_step_refuses_a_model_it_does_not_allow_at_edit_and_accepts_the_integrating_form() {
    // N2.3a and the page's model choices: a held step fits the first-order-with-delay, the integrating and the two-lag forms. Any
    // other model is refused at the edit, at the model field, and nothing is stored; the integrating form is accepted.
    let project = Project::new("held-model");
    let (studio, _updates) = engine();
    let mut held = first_order_form();
    held["heldStep"] = json!(true);
    held["model"] = json!("automatic");
    let refused = answer(
        &studio,
        &project,
        &json!({"action": "manualEdit", "form": held}),
    );
    assert_eq!(refused["failure"]["field"], json!("model"), "{refused}");
    assert_eq!(
        refused["failure"]["message"],
        json!("A held step fits a first-order-with-delay, an integrating or a two-lag model."),
        "{refused}"
    );
    assert_eq!(
        manual_section_json(&project),
        None,
        "the refused form is not stored"
    );
    held["model"] = json!("integrating");
    let accepted = answer(
        &studio,
        &project,
        &json!({"action": "manualEdit", "form": held}),
    );
    assert!(accepted["failure"].is_null(), "{accepted}");
    assert_eq!(
        accepted["form"]["model"],
        json!("integrating"),
        "{accepted}"
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
    let kept = keep_project_test_recording(
        &work,
        &project.study(),
        &test_recording,
        Some(CampaignRestoration::Restored),
    )
    .expect("the project test's recording is kept for the check");
    assert_eq!(kept["phase"], json!("recording-selected"), "{kept}");
    assert_eq!(kept["recordingId"], json!(test_recording), "{kept}");
    assert_eq!(kept["nextFitRole"], json!("validation"), "{kept}");
    assert_eq!(kept["primaryAction"], json!("checkRecording"), "{kept}");
    assert_eq!(kept["message"], json!(PROJECT_TEST_NOTE), "{kept}");
    // C12: the test ended with its outputs restored, and the answer says so in the engine's words (a page that composed the sentence
    // from the restoration would find none here, because this answer carries no restoration).
    assert_eq!(
        kept["testOutcome"],
        json!("Project test completed. Outputs restored."),
        "{kept}"
    );
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

/// B3.6: a check that names no recording reads the study's last opened recording, as `openRecording` does, so that the page keeps no
/// recording identity for it. Catches a check that is refused although the study keeps recordings, and a check that reads the first
/// opened recording, or any other, instead of the last.
#[test]
fn manual_qualify_takes_the_last_opened_recording_when_none_is_named() {
    let project = Project::new("check-last-opened");
    let first = project.keep(step_run("first", 50.0, 60.0));
    let last = project.keep(step_run("last", 40.0, 55.0));
    assert_ne!(first, last, "the two runs are two recordings");
    let (studio, _updates) = engine();
    let checked = answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "form": first_order_form()}),
    );
    assert_eq!(checked["phase"], json!("qualified"), "{checked}");
    assert_eq!(
        checked["recordingId"],
        json!(last),
        "the check read the last opened recording: {checked}"
    );
    // A recording the request names is still the one checked.
    let named = answer(
        &studio,
        &project,
        &json!({"action": "manualQualify", "recordingId": first, "form": first_order_form()}),
    );
    assert_eq!(named["recordingId"], json!(first), "{named}");
}
