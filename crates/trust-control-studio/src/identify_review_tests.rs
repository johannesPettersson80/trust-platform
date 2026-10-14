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

use super::{answer, start_refusal};
use crate::busy_loops::LoopClaims;
use crate::operation::{Operations, UpdateSink};
use crate::runtime_client::runtime_recording::PendingReleases;
use trust_control::experiment::{ControllerMode, ExperimentSample, PlanSignal};
use trust_control::identification::campaign_review::campaign_review;
use trust_control::identification::connected_run::REVIEW_CHANGED_TEXT;
use trust_control::recording::EvidenceSource;
use trust_control::signals::Quality;

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
    assert_eq!(
        reviewed["projectScripted"]["stopConditions"][0],
        json!("Abort"),
        "{reviewed}"
    );
    assert_eq!(
        reviewed["projectScripted"]["stopConditions"][1],
        json!("30 s timeout"),
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

/// A live sample of the example loop in Auto on `source`, with its setpoint and measurement inside their ranges.
fn live(source: EvidenceSource) -> ExperimentSample {
    let signal = |symbol: &str, value: f64, unit: &str| PlanSignal {
        symbol: symbol.to_owned(),
        address: None,
        value,
        unit: unit.to_owned(),
        quality: Quality::Good,
    };
    ExperimentSample {
        scan: 1,
        source,
        controller_mode: ControllerMode::Auto,
        setpoint: Some(signal("P1.Setpoint", 40.0, "°C")),
        process_variable: signal("P1.ProcessValue", 20.0, "°C"),
        manipulated_variable: signal("P1.ManipulatedValue", 30.0, "%"),
        additional: Vec::new(),
    }
}

#[test]
fn the_identity_names_the_campaign_it_reviews_and_changes_with_its_settings() {
    let loop_definition = LoopDefinition::decode(EXAMPLE).expect("the example loop");
    let sample = live(EvidenceSource::SimulatedPlant);
    let first = campaign_review(&loop_definition, &automatic_settings(16.0), Some(&sample))
        .expect("the review of the campaign");
    let again = campaign_review(&loop_definition, &automatic_settings(16.0), Some(&sample))
        .expect("the same review again");
    let longer = campaign_review(&loop_definition, &automatic_settings(20.0), Some(&sample))
        .expect("the review of a longer campaign");
    assert!(first.identity.starts_with("sha256:"), "{}", first.identity);
    assert_eq!(
        first.identity, again.identity,
        "the same campaign has the same identity"
    );
    assert_ne!(
        first.identity, longer.identity,
        "a longer campaign is another campaign"
    );
}

#[test]
fn a_start_is_admitted_with_the_reviewed_identity_and_on_a_physical_target_with_its_approval() {
    let loop_definition = LoopDefinition::decode(EXAMPLE).expect("the example loop");
    let settings = automatic_settings(16.0);
    let simulated = campaign_review(
        &loop_definition,
        &settings,
        Some(&live(EvidenceSource::SimulatedPlant)),
    )
    .expect("the simulated review");
    let physical = campaign_review(
        &loop_definition,
        &settings,
        Some(&live(EvidenceSource::PhysicalPlant)),
    )
    .expect("the physical review");
    assert!(
        simulated.approval.is_none(),
        "a simulated target shows no approval block"
    );
    assert!(
        physical.approval.is_some(),
        "a physical target shows its approval block"
    );
    assert_eq!(
        start_refusal(&simulated, None, false),
        None,
        "pressing the action confirms a simulated start"
    );
    let changed = start_refusal(&simulated, Some("sha256:another-campaign"), false)
        .expect("a changed review is refused");
    assert_eq!(
        changed.message,
        "The campaign changed since it was reviewed: its loop, its mode or its settings differ."
    );
    assert_eq!(changed.detail.as_deref(), Some(REVIEW_CHANGED_TEXT));
    assert_eq!(changed.next_action, NextAction::None);
    let unapproved = start_refusal(&physical, Some(physical.identity.as_str()), false)
        .expect("a physical start needs the approval");
    assert_eq!(unapproved.next_action, NextAction::Approve);
    assert!(
        start_refusal(&physical, None, true).is_some(),
        "an approval covers a reviewed identity, so a start without a review is refused"
    );
    assert_eq!(
        start_refusal(&physical, Some(physical.identity.as_str()), true),
        None,
        "the approved identity starts"
    );
}

#[test]
fn the_start_decodes_the_review_identity_and_the_approval_and_defaults_to_no_approval() {
    let start = |extra: Value| {
        let mut action = json!({
            "action": "identify",
            "settings": {"method": "automatic", "durationSeconds": 30.0, "amplitude": 0.0, "reuseRecordings": false},
        });
        action
            .as_object_mut()
            .expect("an object")
            .extend(extra.as_object().expect("an object").clone());
        serde_json::from_value::<IdentifyRequest>(json!({
            "projectRoot": "/project",
            "runtimeId": "runtime",
            "loopId": "loop",
            "storageFolder": "/storage",
            "action": action,
        }))
        .expect("the start decodes")
        .action
    };
    let approved = start(json!({"review": "sha256:reviewed", "physicalApproved": true}));
    let IdentifyAction::Identify {
        review,
        physical_approved,
        ..
    } = approved
    else {
        panic!("an identify start");
    };
    assert_eq!(review.as_deref(), Some("sha256:reviewed"));
    assert!(physical_approved);
    let plain = start(json!({}));
    let IdentifyAction::Identify {
        review,
        physical_approved,
        ..
    } = plain
    else {
        panic!("an identify start");
    };
    assert_eq!(review, None, "a start that names no review");
    assert!(!physical_approved, "no approval unless the page gives it");
}
