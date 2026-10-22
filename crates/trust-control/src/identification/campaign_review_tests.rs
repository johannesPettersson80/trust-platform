//! Tests of the review of an identification campaign (specification 35, B3.2 to B3.4, B3.9, C12 and §0.1a).

#![allow(clippy::float_cmp)] // The review's durations and ranges are compared exactly on purpose.

use serde_json::{json, Value};

use super::*;
use crate::experiment::plan::{ControllerMode, PlanSignal};
use crate::failure::{Cause, NextAction};
use crate::identification::acquisition_settings::AcquisitionMethod;
use crate::recording::EvidenceSource;
use crate::signals::Quality;

const EXAMPLE: &str =
    include_str!("../../../../examples/control-pid-commissioning/control-loop.json");

/// The example loop: a closed loop with a setpoint, output limits 0 to 100 and a declared duration of 16 s.
fn example() -> LoopDefinition {
    LoopDefinition::decode(EXAMPLE).expect("the example loop")
}

/// The example loop after `edit` changed its document.
fn example_with(edit: impl FnOnce(&mut Value)) -> LoopDefinition {
    let mut document: Value = serde_json::from_str(EXAMPLE).expect("the example document");
    edit(&mut document);
    LoopDefinition::decode(&document.to_string()).expect("the edited loop")
}

/// The settings of an automatic campaign of `seconds`, with no amplitude asked for (the declared one, zero).
fn settings(method: AcquisitionMethod, seconds: f64) -> AcquisitionSettings {
    AcquisitionSettings {
        method,
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

/// A live sample of the example loop in Auto, with its setpoint and measurement inside their ranges.
fn auto_sample() -> ExperimentSample {
    let signal = |symbol: &str, value: f64, unit: &str| PlanSignal {
        symbol: symbol.to_owned(),
        address: None,
        value,
        unit: unit.to_owned(),
        quality: Quality::Good,
    };
    ExperimentSample {
        scan: 1,
        source: EvidenceSource::SimulatedPlant,
        controller_mode: ControllerMode::Auto,
        setpoint: Some(signal("P1.Setpoint", 40.0, "°C")),
        process_variable: signal("P1.ProcessValue", 20.0, "°C"),
        manipulated_variable: signal("P1.ManipulatedValue", 30.0, "%"),
        additional: Vec::new(),
    }
}

#[test]
fn a_step_review_in_auto_names_the_setpoint_step_its_limits_and_its_duration() {
    let review = campaign_review(
        &example(),
        &settings(AcquisitionMethod::Automatic, 30.0),
        Some(&auto_sample()),
        None,
    )
    .expect("reviewed");
    assert_eq!(review.kind, ControlExperimentKind::SetpointStep);
    assert!(review.automatic, "Identify chose the method (B3.3)");
    assert!(!review.manual, "a setpoint step stays in Auto");
    assert_eq!(review.execution, Execution::ControlStudio);
    assert_eq!(
        review.input_range,
        Range {
            minimum: 0.0,
            maximum: 100.0
        },
        "the plant input limits of the loop (B3.4 step 1)"
    );
    assert_eq!(
        review.output_range,
        Range {
            minimum: 0.0,
            maximum: 100.0
        },
        "the permitted range of the measured output"
    );
    assert_eq!(
        review.maximum_duration_seconds, 30.0,
        "the campaign's maximum"
    );
    assert_eq!(
        review.amplitude, 0.0,
        "zero: Identify sizes the step from the measured noise after the baseline"
    );
    assert!(review.project_scripted.is_none());
}

#[test]
fn a_closed_loop_review_without_a_live_mode_says_that_a_runtime_is_needed() {
    let refused = campaign_review(
        &example(),
        &settings(AcquisitionMethod::Automatic, 30.0),
        None,
        None,
    )
    .expect_err("Automatic needs the controller's mode");
    assert_eq!(refused.cause, Cause::Runtime);
    assert_eq!(refused.next_action, NextAction::ConnectRuntime);
}

#[test]
fn a_review_refuses_a_loop_whose_plant_input_limit_is_missing_and_names_the_limit() {
    let without_minimum = example_with(|document| {
        document["limits"]
            .as_object_mut()
            .expect("the limits")
            .remove("outputMinimum");
    });
    let refused = campaign_review(
        &without_minimum,
        &settings(AcquisitionMethod::Automatic, 30.0),
        Some(&auto_sample()),
        None,
    )
    .expect_err("the plant input limits are missing");
    assert_eq!(refused.cause, Cause::Settings);
    assert_eq!(refused.next_action, NextAction::OpenLimits);
    assert!(
        refused.message.contains("outputMinimum"),
        "it names the missing limit: {}",
        refused.message
    );
}

#[test]
fn a_project_scripted_review_names_the_activation_the_abort_and_the_baseline() {
    let scripted = example_with(|document| {
        document["signals"]["manipulatedVariable"]["direction"] = json!("read");
        let experiment = &mut document["experiment"];
        experiment["execution"] = json!("project-scripted");
        experiment["baselineSeconds"] = json!(5.0);
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
    });
    let review = campaign_review(
        &scripted,
        &settings(AcquisitionMethod::Automatic, 30.0),
        None,
        None,
    )
    .expect("the project's scenario is reviewed without a live mode (C12)");
    assert_eq!(review.kind, ControlExperimentKind::ProjectSequence);
    assert!(review.automatic);
    assert!(
        !review.manual,
        "the project owns the excitation: no mode change (C12)"
    );
    assert_eq!(review.execution, Execution::ProjectScripted);
    let project = review.project_scripted.expect("the project's scenario");
    assert_eq!(project.baseline_seconds, Some(5.0));
    assert_eq!(
        project.activation,
        Some(ActivationReview {
            symbol: "P1.Experiment.Active".to_owned(),
            address: "%QX0.0".to_owned(),
        })
    );
    assert_eq!(
        project.abort,
        Some(AbortReview {
            symbol: "P1.Experiment.Abort".to_owned(),
            address: "%QX0.1".to_owned(),
            asserted_value: Some(true),
        })
    );
    assert_eq!(
        project.restoration,
        Some(RestorationReview {
            activation_restored_to: false,
            confirmed_after_executions: 3,
        }),
        "the activation is restored to FALSE, confirmed by the third execution (C12, A7)"
    );
    assert_eq!(
        project.stop_conditions,
        vec![
            "Abort".to_owned(),
            format!("{} s timeout", review.maximum_duration_seconds),
            "Loop mode changed".to_owned(),
            "Bad runtime data".to_owned(),
            "Signal force".to_owned(),
            "Signal outside its limits".to_owned(),
            "Runtime fault".to_owned(),
            "Abort or failure asserts P1.Experiment.Abort = true; normal completion does not."
                .to_owned(),
        ],
        "the review names the stop conditions the test's plan names (E5.5, C12)"
    );
}

#[test]
fn a_project_scripted_loop_without_an_activation_is_observation_only() {
    let observed = example_with(|document| {
        document["signals"]["manipulatedVariable"]["direction"] = json!("read");
        document["experiment"]["execution"] = json!("project-scripted");
    });
    let review = campaign_review(
        &observed,
        &settings(AcquisitionMethod::Automatic, 30.0),
        None,
        None,
    )
    .expect("reviewed");
    assert_eq!(review.kind, ControlExperimentKind::Observation);
    let project = review.project_scripted.expect("the project's scenario");
    assert!(project.activation.is_none(), "observation only (C12)");
    assert!(project.abort.is_none());
    assert!(
        project.restoration.is_none(),
        "nothing is activated, so nothing is restored"
    );
    assert_eq!(project.baseline_seconds, None);
}

#[test]
fn a_physical_review_names_the_signal_it_writes_with_its_limits_and_a_simulated_one_shows_no_block()
{
    let loop_def = example();
    let simulated = campaign_review(
        &loop_def,
        &settings(AcquisitionMethod::Automatic, 16.0),
        Some(&auto_sample()),
        None,
    )
    .expect("the simulated review");
    assert_eq!(
        simulated.approval, None,
        "a simulated target shows no block"
    );
    let mut physical_sample = auto_sample();
    physical_sample.source = EvidenceSource::PhysicalPlant;
    let physical = campaign_review(
        &loop_def,
        &settings(AcquisitionMethod::Automatic, 16.0),
        Some(&physical_sample),
        None,
    )
    .expect("the physical review");
    let approval = physical
        .approval
        .expect("a physical target shows the approval block");
    assert_eq!(approval.loop_id, loop_def.id);
    assert_eq!(approval.duration_seconds, 16.0);
    assert_eq!(
        approval.amplitude, 0.0,
        "Identify sizes the step after the baseline"
    );
    assert_eq!(
        approval.signals.len(),
        1,
        "a setpoint step writes the setpoint only"
    );
    let written = &approval.signals[0];
    assert_eq!(written.symbol, "P1.Setpoint");
    assert_eq!(written.unit, "°C");
    assert_eq!((written.minimum, written.maximum), (0.0, 100.0));
    assert_eq!(
        approval.stop_conditions.len(),
        2,
        "{:?}",
        approval.stop_conditions
    );
    assert!(
        approval.restoration.contains("restores the written signal"),
        "{}",
        approval.restoration
    );
}

#[test]
fn the_physical_approval_of_a_project_test_names_its_activation_without_declared_limits() {
    let scripted = example_with(|document| {
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
    });
    let mut physical_sample = auto_sample();
    physical_sample.source = EvidenceSource::PhysicalPlant;
    let review = campaign_review(
        &scripted,
        &settings(AcquisitionMethod::Automatic, 30.0),
        Some(&physical_sample),
        None,
    )
    .expect("the project's scenario is reviewed on the physical target");
    let approval = review.approval.expect("the approval block");
    assert_eq!(approval.signals.len(), 1);
    assert_eq!(approval.signals[0].symbol, "P1.Experiment.Active");
    assert!(
        approval.signals[0].minimum.is_nan() && approval.signals[0].maximum.is_nan(),
        "a Boolean activation declares no limits, so none is shown"
    );
    assert!(
        approval.restoration.contains("returns to FALSE"),
        "{}",
        approval.restoration
    );
    assert_eq!(
        approval.stop_conditions,
        review
            .project_scripted
            .expect("the project's scenario")
            .stop_conditions,
        "the project's stop conditions are the ones the approval names"
    );
}

// C12, Hold parameter: a project test whose loop declares a hold parameter is reviewed for the hold the runtime applies.

/// Lesson 21 (a safe, informative experiment): a project-scripted open-loop test with a 5 s baseline, a declared maximum of 45.1 s and the
/// hold parameter `P1.HoldSeconds`, 20 to 40 s, in a 100 ms loop.
const LESSON_21: &str = include_str!(
    "../../../../examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment/control-loop.json"
);

fn lesson_21() -> LoopDefinition {
    LoopDefinition::decode(LESSON_21).expect("the shipped lesson decodes")
}

#[test]
fn a_project_test_is_reviewed_for_the_baseline_the_applied_hold_and_one_restoration_sample() {
    let lesson = lesson_21();
    let reviewed = |hold: f64| {
        campaign_review(
            &lesson,
            &settings(AcquisitionMethod::Automatic, 45.1),
            None,
            Some(hold),
        )
        .expect("the hold is within its bounds, and the declared maximum records it")
    };
    let review = reviewed(30.0);
    assert_eq!(
        review.maximum_duration_seconds, 35.1,
        "5 s + 30 s + 0.1 s, not the declared 45.1 s"
    );
    let project = review.project_scripted.expect("the project's scenario");
    assert_eq!(project.baseline_seconds, Some(5.0));
    assert_eq!(
        project.stop_conditions[1], "35.1 s timeout",
        "the review names the timeout the test's plan names (E5.5, C12)"
    );
    assert_eq!(reviewed(20.0).maximum_duration_seconds, 25.1);
    assert_eq!(
        reviewed(40.0).maximum_duration_seconds,
        45.1,
        "the longest permitted hold fills the declared maximum"
    );
}

#[test]
fn the_identity_of_a_project_test_names_the_recording_of_its_applied_hold() {
    let lesson = lesson_21();
    let identity = |hold: f64| {
        campaign_review(
            &lesson,
            &settings(AcquisitionMethod::Automatic, 45.1),
            None,
            Some(hold),
        )
        .expect("reviewed")
        .identity
    };
    assert_eq!(
        identity(30.0),
        identity(30.0),
        "the same hold is the same campaign"
    );
    assert_ne!(
        identity(30.0),
        identity(40.0),
        "a longer hold is another campaign, so a start that names the first is refused (E11.8)"
    );
}

#[test]
fn a_project_test_whose_hold_cannot_be_recorded_is_refused_by_its_review() {
    let lesson = lesson_21();
    let review = |seconds: f64, hold: Option<f64>| {
        campaign_review(
            &lesson,
            &settings(AcquisitionMethod::Automatic, seconds),
            None,
            hold,
        )
    };
    for hold in [None, Some(19.0), Some(41.0)] {
        let refused = review(45.1, hold).expect_err("no hold within its declared bounds");
        assert_eq!(
            refused.message,
            "Apply a P1.HoldSeconds value within 20–40 s before reviewing the project test.",
            "{hold:?}"
        );
        assert_eq!(
            (refused.cause, refused.next_action),
            (Cause::Settings, NextAction::None),
            "{hold:?}"
        );
    }
    let refused = review(30.0, Some(30.0)).expect_err("35.1 s exceeds the maximum of 30 s");
    assert_eq!(
        refused.message,
        "A 30 s hold needs 35.1 s, more than the declared 30 s maximum."
    );
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::Settings, NextAction::OpenLimits)
    );
}

#[test]
fn a_physical_review_of_a_project_test_approves_the_recording_of_the_applied_hold() {
    let mut physical_sample = auto_sample();
    physical_sample.source = EvidenceSource::PhysicalPlant;
    let review = campaign_review(
        &lesson_21(),
        &settings(AcquisitionMethod::Automatic, 45.1),
        Some(&physical_sample),
        Some(30.0),
    )
    .expect("the project's scenario is reviewed on the physical target");
    let approval = review.approval.expect("the approval block");
    assert_eq!(approval.duration_seconds, 35.1);
    assert!(
        approval
            .stop_conditions
            .contains(&"35.1 s timeout".to_owned()),
        "{:?}",
        approval.stop_conditions
    );
}

#[test]
fn a_loop_without_a_hold_parameter_is_reviewed_for_its_declared_duration_whatever_hold_is_given() {
    let loop_def = example();
    let review = |hold: Option<f64>| {
        campaign_review(
            &loop_def,
            &settings(AcquisitionMethod::Automatic, 30.0),
            Some(&auto_sample()),
            hold,
        )
        .expect("reviewed")
    };
    assert_eq!(review(Some(30.0)).maximum_duration_seconds, 30.0);
    assert_eq!(review(Some(30.0)), review(None));
}

// Specification 35, C12 (Activation): a project-scripted scenario without an activation binding is observation only, and the project
// test states that in its plan; with one, the plan names the activation and the TRUE the authorized start writes. The words are the
// engine's: a page that inferred them from a missing `activation` would not need this field, and a review that left it out would leave
// the page without the Test signal row.
#[test]
fn project_test_form_states_observation_only_without_activation() {
    let test_signal = |activated: bool| {
        let scripted = example_with(|document| {
            document["signals"]["manipulatedVariable"]["direction"] = json!("read");
            document["experiment"]["execution"] = json!("project-scripted");
            if activated {
                document["experiment"]["activation"] = json!({
                    "symbol": "P1.Experiment.Active",
                    "address": "%QX0.0",
                    "iecType": "BOOL",
                    "direction": "write",
                });
            }
        });
        campaign_review(
            &scripted,
            &settings(AcquisitionMethod::Automatic, 30.0),
            None,
            None,
        )
        .expect("reviewed")
        .project_scripted
        .expect("the project's scenario")
        .test_signal
    };
    assert_eq!(
        test_signal(false).as_deref(),
        Some("Observation only: no signal is written")
    );
    assert_eq!(
        test_signal(true).as_deref(),
        Some("P1.Experiment.Active · TRUE while the test runs")
    );
}
