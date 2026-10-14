//! Tests of the Validate method (`validate_action`): what the method refuses at once, before any operation starts and before the
//! runtime is asked anything, and its place among the engine's methods. The run itself is tested with the runtime
//! (`workflow::validate_tests`) and on the real runtime (`tests/control_studio_validate.rs`).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::OperationUpdate;
use trust_control::study::current_model::{CurrentModel, CurrentModelSource};
use trust_control::study::STUDY_FORMAT;

use super::{entry_noise, ValidateWork};
use crate::commands::{ControlStudio, METHODS, VALIDATE};
use crate::operation::UpdateSink;
use crate::runtime_client::fake_runtime::FakeRuntime;
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::storage;
use crate::workflow::apply::test_support::model;

const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A project folder with the example loop, and the arguments of `validate` for it, with `connection` when one is given.
fn example_project(name: &str, connection: Option<Value>) -> (PathBuf, LoopDefinition, Value) {
    let root = std::env::temp_dir().join(format!(
        "trust-control-validate-action-{name}-{}",
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

#[test]
fn validate_is_one_of_the_engine_methods_and_a_malformed_request_is_a_failure() {
    assert!(METHODS.contains(&VALIDATE), "the engine answers {VALIDATE}");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let malformed = studio
        .execute(VALIDATE, &[json!({ "projectRoot": "/x" })])
        .expect("answered");
    assert!(
        malformed["failure"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("invalid arguments")),
        "{malformed}"
    );
    assert!(malformed.get("result").is_none(), "{malformed}");
}

#[test]
fn a_validation_without_a_model_or_a_connection_is_refused_for_the_model_first_and_starts_no_operation(
) {
    // B5.5's order: the missing model is named before the missing runtime connection, so the next action is Identify.
    let (root, _example, request) = example_project("no-connection", None);
    let studio = ControlStudio::new(Arc::new(Ignore));
    let refused = studio.execute(VALIDATE, &[request]).expect("answered");
    assert_eq!(
        refused["failure"]["nextAction"],
        json!("identify"),
        "{refused}"
    );
    assert!(
        studio.operations().status("operation-1").is_err(),
        "a refused validation starts no operation"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_validation_without_a_model_is_refused_before_any_isolated_run_starts() {
    let fake = FakeRuntime::start(|_| None);
    let connection = serde_json::to_value(fake.connection(None)).expect("a connection");
    let (root, _example, request) = example_project("no-model", Some(connection));
    let studio = ControlStudio::new(Arc::new(Ignore));
    let refused = studio.execute(VALIDATE, &[request]).expect("answered");
    assert_eq!(refused["failure"]["nextAction"], "identify", "{refused}");
    assert!(
        fake.requests_of("control.simulation.run.start").is_empty(),
        "a refused validation starts no isolated run"
    );
    assert!(studio.operations().status("operation-1").is_err());
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_study_that_cannot_be_read_ends_the_validation_instead_of_making_the_entry_case_inconclusive() {
    // The study and the noise of the Manual-to-Auto entry case are reads of the validation itself (B5.4): a study that cannot be
    // read fails `entry_noise`, which the validation returns with `?`. Only a planning refusal makes the case Inconclusive.
    let (root, example, _request) = example_project("unreadable-study", None);
    let directory = storage::study_directory(&root, &example.id);
    fs::create_dir_all(&directory).expect("study folder");
    fs::write(
        directory.join("study.json"),
        format!("{{\"format\": {STUDY_FORMAT}}}"),
    )
    .expect("a study that does not decode");
    let work = ValidateWork {
        project_root: root.clone(),
        storage_folder: root.join("storage"),
        pending: Arc::new(PendingReleases::default()),
        loop_definition: example.clone(),
    };
    assert!(
        entry_noise(
            &work,
            &example,
            &CurrentModel {
                source: CurrentModelSource::Supplied,
                model: model(),
                identify: None,
            }
        )
        .is_err(),
        "an unreadable study is a failure of the validation"
    );
    fs::remove_dir_all(&root).expect("clean up");
}
