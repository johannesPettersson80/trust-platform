//! Tests of `controlStudio/run` (specification 35, B8 and §0.1a): the order and the stops that need no runtime. A run that identifies
//! or validates needs a runtime, so its campaign and its isolated simulations are tested with the runtime's own tests, not here.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use trust_control::failure::{Cause, NextAction};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{
    InputsRequest, OperationState, OperationUpdate, RunRequest, RunResult, TuneMethod,
};
use trust_control::stages::StageEvidence;
use trust_control::study::inputs::InputsSection;
use trust_control::study::tune::TuneSection;
use trust_control::study::Page;

use crate::busy_loops::LoopClaims;
use crate::commands::{ControlStudio, RUN};
use crate::inputs_action;
use crate::operation::UpdateSink;
use crate::storage;

const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A project folder holding the loop `document`, removed when the test ends.
struct Project {
    root: PathBuf,
    loop_definition: LoopDefinition,
}

impl Project {
    fn new(name: &str, document: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-control-studio-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("project");
        fs::write(root.join("control-loop.json"), document).expect("loop");
        let loop_definition = LoopDefinition::decode(document).expect("the loop");
        Self {
            root,
            loop_definition,
        }
    }

    fn example(name: &str) -> Self {
        Self::new(name, EXAMPLE)
    }

    /// The arguments of `controlStudio/run` through `through`, with `connection` when one is given.
    fn run_request(&self, through: &str, connection: Option<Value>) -> Value {
        let mut request = json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": self.loop_definition.runtime_id,
            "loopId": self.loop_definition.id,
            "storageFolder": self.root.join("storage").display().to_string(),
            "through": through,
        });
        if let Some(connection) = connection {
            request["connection"] = connection;
        }
        request
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Starts the run and waits for its operation to end: the operation's final update.
fn run_to_end(studio: &ControlStudio, request: &Value) -> OperationUpdate {
    let answered = studio
        .execute(RUN, std::slice::from_ref(request))
        .expect("answered");
    let id = answered["result"]["operationId"]
        .as_str()
        .expect("the operation's identifier")
        .to_owned();
    for _ in 0..1000 {
        let update = studio.operations().status(&id).expect("the operation");
        if update.state != OperationState::Running {
            return update;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the run did not end")
}

#[test]
fn the_manual_method_without_its_settings_ends_the_run_before_any_stage_with_the_input_it_needs() {
    let project = Project::example("run-manual");
    let manual = InputsSection {
        method: Some(TuneMethod::Manual),
        ..InputsSection::default()
    };
    inputs_action::answer(
        &LoopClaims::default(),
        InputsRequest {
            project_root: project.root.display().to_string(),
            runtime_id: project.loop_definition.runtime_id.clone(),
            loop_id: project.loop_definition.id.clone(),
            inputs: manual,
        },
    )
    .expect("recorded");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let update = run_to_end(&studio, &project.run_request("tune", None));
    assert_eq!(
        update.state,
        OperationState::Finished,
        "a needed input is a result, not a failure: {update:?}"
    );
    assert_eq!(
        update.stage, None,
        "the run entered no stage before it asked"
    );
    let result = update.result.expect("the run's result");
    assert_eq!(result["needs"], json!("manual-settings"), "{result}");
    let study = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .expect("the study");
    assert_eq!(
        study.section::<TuneSection>().expect("read"),
        None,
        "the run stopped before Tune, so no design was kept"
    );
}

#[test]
fn a_loop_without_its_experiment_duration_ends_the_run_at_identify_with_the_limits_it_needs() {
    let mut document: Value = serde_json::from_str(EXAMPLE).expect("the example document");
    document["experiment"]
        .as_object_mut()
        .expect("the experiment")
        .remove("durationSeconds");
    let project = Project::new("run-limits", &document.to_string());
    let studio = ControlStudio::new(Arc::new(Ignore));
    let connection = json!({ "endpoint": "tcp://127.0.0.1:9" });
    let update = run_to_end(&studio, &project.run_request("identify", Some(connection)));
    assert_eq!(
        update.state,
        OperationState::Finished,
        "the limits are checked before the runtime is reached: {update:?}"
    );
    assert_eq!(
        update.stage,
        Some(Page::Identify),
        "the run stopped in Identify, and its final update names that stage"
    );
    let result = update.result.expect("the run's result");
    assert_eq!(result["needs"], json!("limits"), "{result}");
    assert_eq!(
        result["stageEvidence"]["hasModel"],
        json!(false),
        "{result}"
    );
    assert!(
        result.get("identification").is_none(),
        "the identification stage never ran, so the answer carries none: {result}"
    );
}

#[test]
fn the_answer_carries_the_identification_the_run_made_and_the_evidence_the_page_gates_on() {
    let answer = RunResult {
        stages: Vec::new(),
        stage_evidence: StageEvidence {
            has_model: true,
            ..StageEvidence::default()
        },
        identification: Some(json!({ "modelType": "first-order" })),
        needs: None,
    };
    let value = serde_json::to_value(&answer).expect("the answer");
    assert_eq!(
        value["identification"]["modelType"],
        json!("first-order"),
        "{value}"
    );
    assert_eq!(value["stageEvidence"]["hasModel"], json!(true), "{value}");
    assert_eq!(
        value["stageEvidence"]["hasCandidate"],
        json!(false),
        "{value}"
    );
    assert!(value.get("needs").is_none(), "{value}");
}

#[test]
fn a_stage_that_needs_a_runtime_without_one_fails_the_run_and_keeps_nothing() {
    let project = Project::example("run-runtime");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let update = run_to_end(&studio, &project.run_request("identify", None));
    assert_eq!(update.state, OperationState::Failed, "{update:?}");
    let failure = update.failure.expect("the run's failure");
    assert_eq!(failure.cause, Cause::Runtime);
    assert_eq!(failure.next_action, NextAction::ConnectRuntime);
    assert!(
        !project.root.join("control-studio").exists(),
        "a stopped run keeps no result: nothing was recorded"
    );
}

#[test]
fn a_run_names_its_stage_through_a_kebab_case_word_and_refuses_an_unknown_one() {
    let project = Project::example("run-word");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let unknown = studio
        .execute(RUN, &[project.run_request("apply", None)])
        .expect("answered");
    assert!(
        unknown.get("failure").is_some(),
        "Apply is not a stage of a run (B8): {unknown}"
    );
}

#[test]
fn the_run_request_names_identify_again_and_absent_it_reuses_the_current_model() {
    let mut request = json!({
        "projectRoot": "/project",
        "runtimeId": "runtime-1",
        "loopId": "loop-1",
        "storageFolder": "",
        "through": "identify",
    });
    let reused: RunRequest = serde_json::from_value(request.clone()).expect("the request");
    assert!(
        !reused.identify_again,
        "absent, the current model is reused"
    );
    request["identifyAgain"] = json!(true);
    let again: RunRequest = serde_json::from_value(request).expect("the request");
    assert!(
        again.identify_again,
        "Identify plant asks for a new campaign"
    );
}
