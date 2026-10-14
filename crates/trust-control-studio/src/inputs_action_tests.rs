//! Tests of `controlStudio/inputs` and of what `controlStudio/open` answers for the recorded inputs (specification 35, A8.1, A8.4,
//! A8.6 and §0.1a). They use the example loop, a closed loop with a setpoint, and no runtime.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::failure::NextAction;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{InputsRequest, OperationUpdate, TuneChoice, TuneMethod};
use trust_control::study::inputs::InputsSection;
use trust_control::tuning::requirements::TuningRequirements;

use super::answer;
use crate::busy_loops::LoopClaims;
use crate::commands::{ControlStudio, OPEN};
use crate::operation::UpdateSink;
use crate::storage;

const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A project folder holding the example loop, removed when the test ends.
struct Project {
    root: PathBuf,
    loop_definition: LoopDefinition,
}

impl Project {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-control-studio-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("project");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("loop");
        let loop_definition = LoopDefinition::decode(EXAMPLE).expect("example");
        Self {
            root,
            loop_definition,
        }
    }

    fn inputs_request(&self, inputs: InputsSection) -> InputsRequest {
        InputsRequest {
            project_root: self.root.display().to_string(),
            runtime_id: self.loop_definition.runtime_id.clone(),
            loop_id: self.loop_definition.id.clone(),
            inputs,
        }
    }

    fn open_request(&self) -> Value {
        json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": self.loop_definition.runtime_id,
            "loopId": self.loop_definition.id,
            "storageFolder": self.root.join("storage").display().to_string(),
        })
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Requirements that are valid and that differ by their change: `change` is the setpoint change of the test.
fn requirements(change: f64) -> TuningRequirements {
    TuningRequirements {
        reference_change: change,
        maximum_error: 0.4,
        settling_band: 0.1,
        settling_time_seconds: 45.0,
        duration_seconds: 90.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
    }
}

fn with_requirements(change: f64) -> InputsSection {
    InputsSection {
        requirements: Some(requirements(change)),
        ..InputsSection::default()
    }
}

#[test]
fn recorded_inputs_are_answered_by_open_and_the_stages_are_answered_at_once() {
    let project = Project::new("inputs-open");
    let inputs = InputsSection {
        method: Some(TuneMethod::Simc),
        choice: Some(TuneChoice::Faster),
        ..InputsSection::default()
    };
    let answered =
        answer(&LoopClaims::default(), project.inputs_request(inputs)).expect("recorded");
    assert_eq!(
        answered["invalidated"]["validation"],
        json!(false),
        "no requirements were held before or after: nothing is invalidated"
    );
    assert!(answered["stages"].is_array(), "{answered}");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(OPEN, &[project.open_request()])
        .expect("answered");
    assert_eq!(
        opened["result"]["inputs"]["method"],
        json!("SIMC"),
        "{opened}"
    );
    assert_eq!(
        opened["result"]["inputs"]["choice"],
        json!("faster"),
        "{opened}"
    );
    assert_eq!(opened["result"]["page"], json!("Identify"), "{opened}");
}

#[test]
fn a_change_of_the_requirements_is_reported_as_invalidating_the_validations() {
    let project = Project::new("inputs-invalidate");
    let claims = LoopClaims::default();
    let first = answer(&claims, project.inputs_request(with_requirements(2.0))).expect("recorded");
    assert_eq!(
        first["invalidated"]["validation"],
        json!(true),
        "requirements recorded where none were held are a change"
    );
    let same = answer(&claims, project.inputs_request(with_requirements(2.0))).expect("recorded");
    assert_eq!(
        same["invalidated"]["validation"],
        json!(false),
        "the same requirements invalidate nothing"
    );
    let changed =
        answer(&claims, project.inputs_request(with_requirements(3.0))).expect("recorded");
    assert_eq!(changed["invalidated"]["validation"], json!(true));
}

#[test]
fn the_inputs_cannot_change_while_an_operation_of_the_loop_runs() {
    let project = Project::new("inputs-busy");
    let claims = LoopClaims::default();
    let busy = claims
        .claim(
            &project.loop_definition.project_id,
            &project.loop_definition.id,
            "Identify plant",
        )
        .expect("claimed");
    assert!(
        answer(&claims, project.inputs_request(InputsSection::default())).is_err(),
        "the page's inputs are read-only while the operation runs"
    );
    drop(busy);
    assert!(
        answer(&claims, project.inputs_request(InputsSection::default())).is_ok(),
        "once the operation ends, the inputs change again"
    );
}

#[test]
fn an_invalid_requirement_is_refused_with_its_next_action_and_nothing_is_recorded() {
    let project = Project::new("inputs-invalid");
    let mut bad = requirements(2.0);
    bad.settling_band = 0.4;
    let refused = answer(
        &LoopClaims::default(),
        project.inputs_request(InputsSection {
            requirements: Some(bad),
            ..InputsSection::default()
        }),
    )
    .expect_err("a settling band not below the error limit");
    assert_eq!(refused.next_action, NextAction::OpenRequirements);
    let study = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .expect("the study");
    assert_eq!(
        study.section::<InputsSection>().expect("read"),
        None,
        "a refused section is not kept"
    );
}

#[test]
fn the_request_decodes_as_the_page_writes_it_and_refuses_an_unknown_field() {
    let request = json!({
        "projectRoot": "/project",
        "runtimeId": "simulator",
        "loopId": "control-pid-commissioning",
        "inputs": {
            "method": "SIMC",
            "choice": "balanced",
            "acquisition": {
                "method": "automatic",
                "durationSeconds": 30.0,
                "amplitude": 0.0,
                "reuseRecordings": false,
            },
        },
    });
    let decoded: InputsRequest =
        trust_control::json::decode_value(request.clone()).expect("the page's request");
    assert_eq!(decoded.inputs.method, Some(TuneMethod::Simc));
    assert_eq!(decoded.inputs.choice, Some(TuneChoice::Balanced));
    assert!(decoded.inputs.acquisition.is_some());
    let mut unknown = request;
    unknown["bogus"] = json!(1);
    assert!(
        trust_control::json::decode_value::<InputsRequest>(unknown).is_err(),
        "a field the request does not know is refused (deny_unknown_fields)"
    );
}
