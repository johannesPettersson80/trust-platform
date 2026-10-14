//! Tests of the review that `controlStudio/identify` answers with `action: "review"` (specification 35, §0.1a, B3.2 to B3.4 and C12):
//! the campaign Identify would run, answered with nothing started and nothing written. They need no runtime: a project-scripted
//! experiment's review is given by the loop alone, and a closed-loop review without a runtime says that one is needed.

#![allow(clippy::float_cmp)] // The review's duration is compared exactly on purpose.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::failure::{Failure, NextAction};
use trust_control::identification::acquisition_settings::{AcquisitionMethod, AcquisitionSettings};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{IdentifyAction, IdentifyRequest, OperationUpdate};

use super::answer;
use crate::busy_loops::LoopClaims;
use crate::operation::{Operations, UpdateSink};
use crate::runtime_client::runtime_recording::PendingReleases;

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

    /// The review of `settings` on this project's loop, with no runtime connection.
    fn review(&self, settings: AcquisitionSettings) -> Result<Value, Failure> {
        answer(
            &Operations::new(Arc::new(Ignore)),
            &Arc::new(PendingReleases::default()),
            &LoopClaims::default(),
            IdentifyRequest {
                project_root: self.root.display().to_string(),
                runtime_id: self.loop_definition.runtime_id.clone(),
                loop_id: self.loop_definition.id.clone(),
                storage_folder: self.root.join("storage").display().to_string(),
                connection: None,
                action: IdentifyAction::Review { settings },
            },
        )
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn automatic_settings(seconds: f64) -> AcquisitionSettings {
    AcquisitionSettings {
        method: AcquisitionMethod::Automatic,
        duration_seconds: seconds,
        amplitude: 0.0,
        setpoint_signal_id: None,
        relay_direction: None,
        reuse_recordings: false,
        input_range: None,
        output_range: None,
        input_ranges: None,
        output_ranges: None,
    }
}

/// The example loop as a project-scripted experiment with an activation and an abort (C12).
fn project_scripted_document() -> String {
    let mut document: Value = serde_json::from_str(EXAMPLE).expect("the example document");
    document["signals"]["manipulatedVariable"]["direction"] = json!("read");
    let experiment = &mut document["experiment"];
    experiment["execution"] = json!("project-scripted");
    experiment["activation"] = json!({
        "symbol": "P1.Experiment.Active",
        "address": "%QX0.0",
        "iecType": "BOOL",
        "direction": "write",
    });
    experiment["abort"] = json!({
        "symbol": "P1.Experiment.Abort",
        "address": "%QX0.1",
        "iecType": "BOOL",
        "direction": "write",
        "assertedValue": true,
    });
    document.to_string()
}

#[test]
fn the_review_of_a_project_scripted_experiment_names_its_activation_and_writes_nothing() {
    let project = Project::new("review-scripted", &project_scripted_document());
    let reviewed = project
        .review(automatic_settings(30.0))
        .expect("the project's scenario is reviewed without a runtime");
    assert_eq!(
        reviewed["execution"],
        json!("project-scripted"),
        "{reviewed}"
    );
    assert_eq!(reviewed["automatic"], json!(true), "{reviewed}");
    assert_eq!(reviewed["manual"], json!(false), "{reviewed}");
    assert_eq!(
        reviewed["projectScripted"]["activation"]["symbol"],
        json!("P1.Experiment.Active"),
        "{reviewed}"
    );
    assert_eq!(
        reviewed["projectScripted"]["abort"]["assertedValue"],
        json!(true),
        "{reviewed}"
    );
    assert_eq!(
        reviewed["maximumDurationSeconds"].as_f64(),
        Some(30.0),
        "{reviewed}"
    );
    assert!(
        !project.root.join("control-studio").exists(),
        "a review starts nothing and writes nothing"
    );
}

#[test]
fn a_review_of_a_closed_loop_without_a_runtime_says_that_one_is_needed() {
    let project = Project::new("review-runtime", EXAMPLE);
    let refused = project
        .review(automatic_settings(30.0))
        .expect_err("Automatic chooses from the controller's mode");
    assert_eq!(refused.next_action, NextAction::ConnectRuntime);
    assert!(
        !project.root.join("control-studio").exists(),
        "a refused review writes nothing either"
    );
}
