//! The run context of a recording (specification 35, B10.2, B10.3 and B10.5) through the study's own files: the retention keeps the
//! experiments a run noted in the file of the recording that follows, and the JSON export writes the context of every recording the
//! study keeps, so a recording from an aborted test is exported with its stop reason and its restoration. They use the fixtures of
//! `identify_action_tests.rs`, and no runtime runs here.

use super::*;
use trust_control::export::{ExportFormat, ExportRequest};
use trust_control::recording::context::{saved_context, ExperimentContext, RestorationResult};

/// A saved result in the form the extension wrote, whose run was a manual-output step on a simulated target that the engineer aborted
/// (B10.3).
fn aborted_step() -> Value {
    json!({
        "source": "Simulation",
        "experiment": {
            "kind": "mv_step", "affectedSignal": "P1.ManipulatedValue", "address": "%QL0", "requestedValue": 25,
            "appliedValue": 25, "durationMs": 10_000, "trajectory": [{ "atMs": 0, "value": 25 }],
            "stopConditions": ["Abort"], "minimum": 0, "maximum": 100, "approval": "Simulation confirmation",
            "status": "aborted", "stopReason": "user_abort", "aborted": true,
            "restoration": { "value": 20, "status": "confirmed" }
        },
        "recording": { "rawSamples": [] }
    })
}

/// The experiments of `aborted_step`, which a run notes as its hold ends.
fn aborted_experiments() -> Vec<ExperimentContext> {
    saved_context(&aborted_step(), EvidenceSource::SimulatedPlant)
        .expect("the fixture states its experiment")
        .experiments
}

#[test]
fn the_study_keeps_the_experiments_a_run_noted_in_the_file_of_the_recording_that_follows() {
    let project = Project::new("retention-context");
    let source = EvidenceSource::SimulatedPlant;
    let mut retention =
        StudyRetention::new(&project.root, LOOP_ID, source, project.working_study());
    for experiment in aborted_experiments() {
        retention.note_experiment(experiment);
    }
    let run = recorded_run();
    retention
        .retain(&run, &recording_evidence(&run, None))
        .expect("kept");
    let file_id = retention
        .kept_file_id("recording-1")
        .expect("the file of the run")
        .to_string();
    let file = storage::read_recording(&project.root, LOOP_ID, &file_id).expect("it reads back");
    let context = file.context.expect("the file keeps the context of its run");
    let [experiment] = context.experiments.as_slice() else {
        panic!("one experiment: {context:?}");
    };
    assert_eq!(experiment.stop_reason.as_deref(), Some("user_abort"));
    assert!(experiment.aborted, "the abort state is kept (B10.2)");
    assert_eq!(experiment.restoration.status, RestorationResult::Confirmed);
    assert_eq!(context.plant, None, "the run noted no plant");

    // The notes belong to the recording that was kept: the next recording, of another run, keeps none.
    let newer = newer_recorded_run();
    retention
        .retain(&newer, &recording_evidence(&newer, None))
        .expect("kept");
    let later_id = retention
        .kept_file_id("recording-2")
        .expect("the file of the later run")
        .to_string();
    let later = storage::read_recording(&project.root, LOOP_ID, &later_id).expect("it reads back");
    assert_eq!(later.context, None);
}

#[test]
fn a_recording_is_kept_whatever_its_context_says_and_a_part_that_breaks_a_rule_is_left_out() {
    let project = Project::new("retention-broken-context");
    let source = EvidenceSource::SimulatedPlant;
    let mut retention =
        StudyRetention::new(&project.root, LOOP_ID, source, project.working_study());
    let mut broken = aborted_experiments().remove(0);
    broken.minimum = broken.maximum + 1.0;
    retention.note_experiment(broken);
    let run = recorded_run();
    retention
        .retain(&run, &recording_evidence(&run, None))
        .expect("the recording is kept");
    let file_id = retention
        .kept_file_id("recording-1")
        .expect("the file of the run")
        .to_string();
    let file = storage::read_recording(&project.root, LOOP_ID, &file_id).expect("it reads back");
    assert_eq!(
        file.context, None,
        "an experiment with unordered bounds is not kept"
    );
    assert_eq!(file.run, run, "the run is kept as the runtime returned it");
}

#[test]
fn the_json_export_writes_the_context_of_every_recording_the_study_keeps() {
    let project = Project::new("export-context");
    project.keep_identify_result(PLANT_INPUT);
    let export = || -> Value {
        let answer = crate::export_action::answer(ExportRequest {
            project_root: project.root.display().to_string(),
            runtime_id: RUNTIME_ID.to_string(),
            loop_id: LOOP_ID.to_string(),
            format: ExportFormat::Json,
            recording_id: None,
        })
        .expect("the JSON export");
        serde_json::from_str(answer["text"].as_str().expect("the JSON text"))
            .expect("the export reads back as JSON")
    };
    assert!(
        export().get("recordings").is_none(),
        "a study whose runs state no context exports none"
    );

    // An aborted test: its recording is kept with no result that refers to it, and its context goes with it (B10.3).
    let mut retention = StudyRetention::new(
        &project.root,
        LOOP_ID,
        EvidenceSource::SimulatedPlant,
        project.working_study(),
    );
    for experiment in aborted_experiments() {
        retention.note_experiment(experiment);
    }
    let run = newer_recorded_run();
    retention
        .retain(&run, &recording_evidence(&run, None))
        .expect("the aborted test's recording is kept");
    let aborted_id = retention
        .kept_file_id("recording-2")
        .expect("the file of the run")
        .to_string();
    let exported = export();
    let recordings = exported["recordings"]
        .as_array()
        .expect("the recordings of the study");
    let [entry] = recordings.as_slice() else {
        panic!("only the recording with a context has an entry: {recordings:?}");
    };
    assert_eq!(entry["recordingId"], json!(aborted_id));
    assert_eq!(entry["source"], "simulated-plant");
    assert_eq!(entry["experiments"][0]["status"], "aborted");
    assert_eq!(entry["experiments"][0]["stopReason"], "user_abort");
    assert_eq!(entry["experiments"][0]["aborted"], true);
    assert_eq!(
        entry["experiments"][0]["restoration"]["status"],
        "confirmed"
    );
    assert_eq!(
        entry["experiments"][0]["trajectory"][0]["value"], 25.0,
        "the complete commanded trajectory is exported"
    );
    assert_eq!(
        exported["kind"], "opened",
        "the export is still the opened result of the loop"
    );
}
