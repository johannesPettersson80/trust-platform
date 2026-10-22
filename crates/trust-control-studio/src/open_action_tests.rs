//! The open answer (specification 35, §0.1a, A8.2, A8.4 to A8.6, B5.2, B9.4 and C12): the project's loops, a loop's stages, a study of an
//! earlier loop revision, the current identification with its evidence, and the journaled operations that reopening reattaches to.

use std::fs;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::loop_definition::{LoopDefinition, NO_CONTROLLER_LAW};
use trust_control::study::{Page, Study};

use super::*;
use crate::commands::test_support::{example_project, identified_section, Ignore, EXAMPLE};
use crate::commands::{ControlStudio, OPEN};

#[test]
fn open_refuses_a_parked_loop_before_it_reads_or_moves_the_typescript_study() {
    // V6 decision D9: a loop with a parked feature stays on the TypeScript engine, whose study document is its own; opening it
    // through the engine must leave that document where it is. A top-level cascade (C6, the two-loop form) is still parked (V7).
    let root =
        std::env::temp_dir().join(format!("trust-control-open-parked-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("project");
    let mut file: Value = serde_json::from_str(EXAMPLE).expect("example");
    file["cascade"] = json!({ "role": "inner", "outer": "control-pid-outer" });
    fs::write(root.join("control-loop.json"), file.to_string()).expect("loop");
    let example = LoopDefinition::decode(EXAMPLE).expect("example");
    let directory = storage::study_directory(&root, &example.id);
    fs::create_dir_all(&directory).expect("study folder");
    let typescript_study = r#"{"schemaVersion":1,"loop":{"id":"x"},"state":{"page":"model"}}"#;
    fs::write(directory.join("study.json"), typescript_study).expect("study");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let request = json!({ "projectRoot": root.display().to_string(), "runtimeId": example.runtime_id,
        "loopId": example.id, "storageFolder": root.join("storage").display().to_string() });
    let answer = studio.execute(OPEN, &[request]).expect("answered");
    assert_eq!(
        answer["failure"]["cause"], "controller-capability",
        "{answer}"
    );
    assert_eq!(
        fs::read_to_string(directory.join("study.json")).expect("still there"),
        typescript_study,
        "the TypeScript study is neither read into a new study nor moved"
    );
    assert!(
        !directory.join("history").exists(),
        "nothing moved to history"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn open_lists_the_loops_then_opens_one_with_its_stages_and_no_operation() {
    let root = std::env::temp_dir().join(format!("trust-control-open-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("project");
    fs::write(root.join("control-loop.json"), EXAMPLE).expect("loop");
    let example = LoopDefinition::decode(EXAMPLE).expect("example");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let mut request = json!({ "projectRoot": root.display().to_string(), "runtimeId": example.runtime_id, "storageFolder": root.join("storage").display().to_string() });
    let listed = studio
        .execute(OPEN, std::slice::from_ref(&request))
        .expect("answered");
    assert_eq!(listed["result"]["kind"], "loops");
    assert_eq!(listed["result"]["loops"][0]["loopId"], example.id.as_str());
    request["loopId"] = json!(example.id);
    let opened = studio.execute(OPEN, &[request]).expect("answered");
    assert_eq!(opened["result"]["kind"], "opened", "{opened}");
    assert_eq!(opened["result"]["loop"]["id"], example.id.as_str());
    assert_eq!(opened["result"]["stages"][2]["stage"], "Identify");
    assert_eq!(opened["result"]["stages"][2]["status"], "needs-data");
    assert_eq!(opened["result"]["reattached"], json!([]));
    // C12: the answer says a project test only for a loop that declares a project-scripted experiment.
    assert!(opened["result"].get("projectTest").is_none(), "{opened}");
    // A8.6: a study that never applied offers neither Restore nor Keep, and no Apply observation waits to be resumed. Nothing was
    // validated either, so Apply is not offered (B5.4).
    assert_eq!(opened["result"]["applyAvailable"], json!(false));
    assert_eq!(opened["result"]["restoreAvailable"], json!(false));
    assert_eq!(opened["result"]["keepAvailable"], json!(false));
    assert_eq!(opened["result"]["applyObservationResumable"], json!(false));
    // B7: nothing is offered, so no action holds the Validate page's position.
    assert!(opened["result"].get("primaryAction").is_none(), "{opened}");
    assert!(
        !root.join("control-studio").exists(),
        "opening writes nothing"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn open_serves_an_observation_lesson_whose_controller_stages_are_not_applicable() {
    // Specification 35, C12: a loop without a controller law is not parked, so `open` opens it on the engine. Tune, Validate and
    // Apply are Not applicable with the reason, and Identify still runs on it. Heater lesson 04 is such an observation loop.
    const OBSERVATION_LESSON: &str = include_str!(
        "../../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/control-loop.json"
    );
    let root = std::env::temp_dir().join(format!("trust-control-open-c12-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("project");
    fs::write(root.join("control-loop.json"), OBSERVATION_LESSON).expect("loop");
    let lesson = LoopDefinition::decode(OBSERVATION_LESSON).expect("lesson");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let request = json!({ "projectRoot": root.display().to_string(), "runtimeId": lesson.runtime_id,
        "loopId": lesson.id, "storageFolder": root.join("storage").display().to_string() });
    let answer = studio.execute(OPEN, &[request]).expect("answered");
    assert_eq!(answer["result"]["kind"], "opened", "{answer}");
    // C12: the lesson declares a project-scripted experiment, so the answer offers the project test.
    assert_eq!(answer["result"]["projectTest"], true, "{answer}");
    let stages = answer["result"]["stages"].as_array().expect("stages");
    let stage = |name: &str| {
        stages
            .iter()
            .find(|stage| stage["stage"] == name)
            .unwrap_or_else(|| panic!("{name}: {answer}"))
    };
    assert_eq!(stage("Identify")["status"], "needs-data", "{answer}");
    for name in ["Tune", "Validate", "Apply"] {
        assert_eq!(stage(name)["status"], "not-applicable", "{name}: {answer}");
        assert_eq!(stage(name)["label"], "Not applicable", "{name}: {answer}");
        assert_eq!(stage(name)["reason"], NO_CONTROLLER_LAW, "{name}: {answer}");
    }
    assert!(stage("Identify").get("reason").is_none(), "{answer}");
    assert_eq!(
        answer["result"]["stageEvidence"]["controllerStagesApply"],
        json!(false)
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_study_of_an_earlier_loop_revision_opens_at_identify() {
    let (root, example, request) = example_project("page-revision");
    let mut earlier = Study::new(&example.id, "sha256:an-earlier-revision");
    earlier.page = Page::Validate;
    storage::save_study(&root, &earlier, &[]).expect("the earlier study is kept");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio.execute(OPEN, &[request]).expect("answered");
    assert_eq!(
        opened["result"]["page"], "Identify",
        "a study of another loop revision starts again at Identify (A8.2): {opened}"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn reattached_lists_the_unfinished_journaled_recording_by_its_identifier() {
    // A8.5, A8.6: a journaled recording is reattached by its identifier; the list holds journaled operations only.
    use trust_control::experiment::{
        ControlEvidenceSource, RecordingRecoveryBinding, RecordingRecoveryOwner,
    };
    let owner = RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: "recording-1".into(),
        project_id: "project:sha256:abc".into(),
        runtime_id: "runtime:local".into(),
        loop_id: "loop:tank".into(),
        loop_revision: "sha256:loop".into(),
        source: ControlEvidenceSource::Simulation,
        program_revision: None,
        start_acknowledgement_unconfirmed: None,
        bindings: vec![RecordingRecoveryBinding {
            id: "processVariable".into(),
            address: Some("%IL0".into()),
            symbol: None,
            iec_type: "LREAL".into(),
        }],
    };
    let entries = [JournalEntry::RecordingOwner { owner }];
    assert_eq!(
        journaled_operations(&entries),
        vec!["recording-1".to_owned()]
    );
    assert_eq!(journaled_operations(&[]), Vec::<String>::new());
}

#[test]
fn open_holds_a_current_identification_with_its_evidence_and_none_after_reset() {
    // A8.6, B9.4: a reopened study with a current model holds its identification and says it has a model, so the page enables
    // Tune without deducing it. Reset starts a new study, which holds neither.
    let (root, example, request) = example_project("identification");
    let mut study = Study::new(&example.id, &example.revision);
    study
        .set_section(&identified_section(&example, false))
        .expect("the Identify section is kept");
    storage::save_study(&root, &study, &[]).expect("the study is kept");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, std::slice::from_ref(&request))
        .expect("answered");
    assert_eq!(
        opened["result"]["stageEvidence"]["hasModel"],
        json!(true),
        "{opened}"
    );
    let identification = &opened["result"]["identification"];
    assert_eq!(
        identification["modelRevision"], "model:fitted:1",
        "{opened}"
    );
    assert_eq!(identification["provisional"], json!(true), "{opened}");
    assert!(
        opened["result"].get("tune").is_none(),
        "a study with no candidate holds none: {opened}"
    );
    assert!(
        identification["plot"]["timeSeconds"]
            .as_array()
            .is_some_and(|times| !times.is_empty()),
        "the answer holds the plot: {opened}"
    );
    storage::start_new_study(&root, &study).expect("the study is reset");
    let reset = studio.execute(OPEN, &[request]).expect("answered");
    assert_eq!(
        reset["result"]["stageEvidence"]["hasModel"],
        json!(false),
        "{reset}"
    );
    assert!(reset["result"].get("identification").is_none(), "{reset}");
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn open_holds_a_model_that_validate_qualified_as_not_provisional() {
    // B5.2, A8.6: a model whose independent check passed names its validation recording, and reopens as not provisional.
    let (root, example, request) = example_project("identification-qualified");
    let mut study = Study::new(&example.id, &example.revision);
    study
        .set_section(&identified_section(&example, true))
        .expect("the qualified Identify section is kept");
    storage::save_study(&root, &study, &[]).expect("the study is kept");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio.execute(OPEN, &[request]).expect("answered");
    assert_eq!(
        opened["result"]["identification"]["provisional"],
        json!(false),
        "{opened}"
    );
    fs::remove_dir_all(&root).expect("clean up");
}
