//! Tests of the Keep method (`keep_action`): what it refuses at once while the study offers no Keep (B6.5), the request it decodes, and
//! the shape of its review and outcome answers. The review and the confirmation run against the runtime and the language tool
//! (`workflow::keep_tests`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{KeepRequest, OperationUpdate};
use trust_control::study::Study;

use super::{keep_outcome_json, review_json};
use crate::commands::{ControlStudio, KEEP, METHODS};
use crate::operation::UpdateSink;
use crate::storage;
use crate::workflow::keep::{FileChange, KeepOutcome, KeepReview};

const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A project folder with the example loop, and the arguments of `keep` for it, with `connection` when one is given.
fn example_project(name: &str, connection: Option<Value>) -> (PathBuf, LoopDefinition, Value) {
    let root = std::env::temp_dir().join(format!(
        "trust-control-keep-action-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("project");
    fs::write(root.join("control-loop.json"), EXAMPLE).expect("loop");
    let example = LoopDefinition::decode(EXAMPLE).expect("example");
    let mut request = json!({
        "projectRoot": root.display().to_string(),
        "runtimeId": example.runtime_id,
        "loopId": example.id,
        "storageFolder": root.join("storage").display().to_string(),
    });
    if let Some(connection) = connection {
        request["connection"] = connection;
    }
    (root, example, request)
}

/// The study of the example loop, with an Apply section made of one applied Apply whose result passed, and the `restorations` given.
#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn store_applied_section(root: &Path, example: &LoopDefinition, restorations: Value) {
    let section = json!({
        "applications": [{
            "checkpoint": "cp-1",
            "validationIdentity": "validation:1",
            "settingsApplied": true,
            "result": {
                "status": "passed",
                "reason": "The response was assessed.",
                "transfers": [],
                "recoveryConfirmed": false,
                "target": 38.0,
                "modelRevision": "model:1",
                "channels": [],
                "criteria": {
                    "target": 38.0,
                    "durationSeconds": 60.0,
                    "settlingTimeSeconds": 30.0,
                    "maximumError": 0.2,
                    "taskPeriodMs": 100.0,
                    "processRange": { "minimum": 0.0, "maximum": 80.0 },
                    "outputLimits": { "minimum": 0.0, "maximum": 100.0 }
                }
            }
        }],
        "restorations": restorations,
        "keeps": []
    });
    let mut study = Study::new(&example.id, &example.revision);
    study.sections.insert("apply".to_string(), section);
    storage::save_study(root, &study, &[]).expect("the study is saved");
}

#[test]
fn keep_is_one_of_the_engine_methods() {
    assert!(METHODS.contains(&KEEP), "the engine answers {KEEP}");
}

#[test]
fn keep_is_refused_at_once_when_no_apply_has_applied_its_settings() {
    let (root, _example, request) =
        example_project("none", Some(json!({ "endpoint": "tcp://127.0.0.1:1" })));
    let studio = ControlStudio::new(Arc::new(Ignore));
    let refused = studio.execute(KEEP, &[request]).expect("answered");
    assert!(
        refused["failure"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Keep in project is not available")),
        "{refused}"
    );
    assert!(refused.get("result").is_none(), "{refused}");
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn keep_is_offered_after_an_applied_apply_and_then_needs_a_runtime() {
    let (root, example, request) = example_project("offered", None);
    store_applied_section(&root, &example, json!([]));
    let studio = ControlStudio::new(Arc::new(Ignore));
    let refused = studio.execute(KEEP, &[request]).expect("answered");
    assert_eq!(
        refused["failure"]["nextAction"], "connect-runtime",
        "the availability passed, and the running values need the runtime: {refused}"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn keep_is_refused_after_a_restore_of_the_applied_settings() {
    let (root, example, request) =
        example_project("restored", Some(json!({ "endpoint": "tcp://127.0.0.1:1" })));
    store_applied_section(
        &root,
        &example,
        json!([{ "checkpoint": "cp-1", "restored": true, "retired": false, "reason": "The previous settings were confirmed back." }]),
    );
    let studio = ControlStudio::new(Arc::new(Ignore));
    let refused = studio.execute(KEEP, &[request]).expect("answered");
    assert!(
        refused["failure"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Keep in project is not available")),
        "Keep is not offered after a Restore (B6.5): {refused}"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_keep_request_decodes_its_unsaved_files_and_language_tool_and_defaults_them() {
    let request: KeepRequest = serde_json::from_value(json!({
        "projectRoot": "/project",
        "runtimeId": "simulator",
        "loopId": "tank",
        "storageFolder": "/journal",
        "unsavedFiles": ["Main.st"],
        "languageTool": "/opt/trust-lsp",
    }))
    .expect("decoded");
    assert_eq!(request.unsaved_files, vec!["Main.st".to_string()]);
    assert_eq!(request.language_tool.as_deref(), Some("/opt/trust-lsp"));
    let defaults: KeepRequest = serde_json::from_value(json!({
        "projectRoot": "/project",
        "runtimeId": "simulator",
        "loopId": "tank",
        "storageFolder": "/journal",
    }))
    .expect("decoded");
    assert_eq!(defaults.unsaved_files, [] as [std::string::String; 0]);
    assert!(defaults.language_tool.is_none());
    assert!(!defaults.physical_approved);
}

#[test]
fn the_review_answer_names_the_record_the_values_the_fields_the_files_and_the_identity() {
    let review = KeepReview {
        record: "Main.TIC101_Parameters".to_string(),
        values: BTreeMap::from([("Kp".to_string(), 2.5)]),
        kept: vec!["Kp".to_string()],
        unknown: Vec::new(),
        files: vec![FileChange {
            path: PathBuf::from("/project/Main.st"),
            before: Some("old text".to_string()),
            after: "new text".to_string(),
        }],
        identity: "sha256:review".to_string(),
    };
    let answer = review_json(&review).expect("the review encodes");
    assert_eq!(answer["record"], "Main.TIC101_Parameters");
    assert_eq!(answer["values"]["Kp"], 2.5);
    assert_eq!(answer["kept"][0], "Kp");
    assert_eq!(answer["files"][0]["path"], "/project/Main.st");
    assert_eq!(answer["files"][0]["after"], "new text");
    assert_eq!(answer["identity"], "sha256:review");
}

#[test]
fn the_outcome_answer_names_the_fields_kept_and_those_without_a_setting() {
    let outcome = KeepOutcome {
        record: "Main.TIC101_Parameters".to_string(),
        kept: vec!["Kp".to_string(), "Ki".to_string()],
        unknown: vec!["Kd".to_string()],
    };
    let answer = keep_outcome_json(&outcome).expect("the result encodes");
    assert_eq!(answer["record"], "Main.TIC101_Parameters");
    assert_eq!(answer["kept"], json!(["Kp", "Ki"]));
    assert_eq!(answer["unknown"], json!(["Kd"]));
    assert_eq!(
        answer["reason"],
        "Kept in the project: Kp, Ki of Main.TIC101_Parameters. A restart or a new download keeps them. The equipment has no setting for Kd; copy it into the PLC code by hand.",
        "the sentence of keepInProject.ts (B6.5)"
    );
}
