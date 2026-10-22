//! Tests of `controlStudio/run` (specification 35, B8 and §0.1a): the order and the stops that need no runtime. A run that identifies
//! or validates needs a runtime, so its campaign and its isolated simulations are tested with the runtime's own tests, not here.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use trust_control::failure::{Cause, NextAction};
use trust_control::identification::acquisition_settings::{AcquisitionMethod, AcquisitionSettings};
use trust_control::identification::method_applicability::MethodRefusal;
use trust_control::identification::texts::method_refusal_text;
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
            inputs: Some(manual),
            change: None,
            channel_selection: None,
            storage_folder: Some(project.root.join("storage").display().to_string()),
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
    // B8: the engine asks for the settings in its own sentence; the page writes none (the manual settings are asked for before any stage
    // runs, so no refusal carries the sentence).
    assert_eq!(
        result["needsMessage"],
        json!("Enter the manual settings to tune the controller."),
        "{result}"
    );
    assert!(
        result.get("needsField").is_none(),
        "no form field concerns the manual settings: {result}"
    );
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
fn a_run_on_a_stopped_runtime_still_asks_for_the_limits_before_it_requires_the_runtime() {
    let mut document: Value = serde_json::from_str(EXAMPLE).expect("the example document");
    document["experiment"]
        .as_object_mut()
        .expect("the experiment")
        .remove("durationSeconds");
    let project = Project::new("run-limits-stopped", &document.to_string());
    let studio = ControlStudio::new(Arc::new(Ignore));
    // No connection: the page sends none while no runtime runs (E11.1).
    let update = run_to_end(&studio, &project.run_request("identify", None));
    assert_eq!(
        update.state,
        OperationState::Finished,
        "a missing limit is the run's input to ask for, whether or not a runtime runs: {update:?}"
    );
    let result = update.result.expect("the run's result");
    assert_eq!(result["needs"], json!("limits"), "{result}");
}

#[test]
fn a_run_that_needs_limits_answers_the_sentence_the_editor_shows() {
    // B8, B3.2: a loop without a maximum duration asks for it in the engine's own sentence, and names the duration's field, so the Limits
    // editor shows that sentence under that field and the page writes none. A bare code would leave the page to word the request (it wrote
    // "Enter the maximum experiment duration." itself), and a sentence without the field would leave it to guess which row to mark.
    let mut document: Value = serde_json::from_str(EXAMPLE).expect("the example document");
    document["experiment"]
        .as_object_mut()
        .expect("the experiment")
        .remove("durationSeconds");
    let project = Project::new("run-limits-words", &document.to_string());
    let studio = ControlStudio::new(Arc::new(Ignore));
    let update = run_to_end(&studio, &project.run_request("identify", None));
    assert_eq!(update.state, OperationState::Finished, "{update:?}");
    let result = update.result.expect("the run's result");
    assert_eq!(result["needs"], json!("limits"), "{result}");
    assert_eq!(
        result["needsMessage"],
        json!("Set the maximum experiment duration in Limits before identification."),
        "{result}"
    );
    assert_eq!(result["needsField"], json!("durationSeconds"), "{result}");
}

#[test]
fn a_recorded_method_the_loop_cannot_run_is_refused_before_the_runtime_is_required() {
    // The loop declares no Manual command, so the open-loop step cannot run (B3.2); the study records the engineer's choice of it.
    let mut document: Value = serde_json::from_str(EXAMPLE).expect("the example document");
    let controller = document["controller"]
        .as_object_mut()
        .expect("the controller");
    for key in [
        "manualOutputSignal",
        "manualOutputAddress",
        "manualOutputType",
    ] {
        controller.remove(key);
    }
    let project = Project::new("run-method-stopped", &document.to_string());
    let step = InputsSection {
        acquisition: Some(AcquisitionSettings {
            method: AcquisitionMethod::MvStep,
            duration_seconds: 16.0,
            amplitude: 0.0,
            setpoint_signal_id: None,
            relay_direction: None,
            reuse_recordings: false,
            input_range: None,
            output_range: None,
            input_ranges: None,
            output_ranges: None,
        }),
        ..InputsSection::default()
    };
    inputs_action::answer(
        &LoopClaims::default(),
        InputsRequest {
            project_root: project.root.display().to_string(),
            runtime_id: project.loop_definition.runtime_id.clone(),
            loop_id: project.loop_definition.id.clone(),
            inputs: Some(step),
            change: None,
            channel_selection: None,
            storage_folder: Some(project.root.join("storage").display().to_string()),
        },
    )
    .expect("recorded");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let update = run_to_end(&studio, &project.run_request("identify", None));
    assert_eq!(update.state, OperationState::Failed, "{update:?}");
    let failure = update.failure.expect("the run's failure");
    assert_eq!(
        failure.message,
        method_refusal_text(MethodRefusal::OpenLoopBindings),
        "the refusal is the sentence the Method list gives the method, not the missing runtime"
    );
    assert_eq!(failure.cause, Cause::Settings);
    assert_eq!(failure.next_action, NextAction::ChooseMethod);
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
        needs_message: None,
        needs_field: None,
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
    assert!(value.get("needsMessage").is_none(), "{value}");
    assert!(value.get("needsField").is_none(), "{value}");
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
