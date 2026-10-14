//! Tests of the `feedback` tool (N4.3, N4.4): the loop that a request closes, its channels, its checked grid and the answer
//! of a transfer-function loop.

use std::time::Duration;

use serde_json::{json, Value};

use super::*;
use crate::analysis::budget::ANALYSIS_BUDGET;
use crate::analysis::general_feedback::{ClosedLoop, ControllerSource};
use crate::model::decode_general_model;

fn channel(id: &str, unit: &str) -> Value {
    json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -100.0, "maximum": 100.0 } })
}

/// The lag 1/(s + 1) from `u` to `y`, in volts.
fn lag_plant() -> GeneralModel {
    decode_general_model(&json!({
        "revision": "plant-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "kind": "transfer-function",
        "inputs": [channel("u", "V")], "outputs": [channel("y", "V")],
        "numerator": [1.0], "denominator": [1.0, 1.0]
    }))
    .expect("a valid plant")
}

/// The gain 2 from `e` to `c`, with its output in the given unit.
fn gain_controller(output_unit: &str) -> GeneralModel {
    decode_general_model(&json!({
        "revision": "controller-1", "source": "provided", "validation": "unvalidated", "evidence": [], "delaySeconds": 0.0,
        "kind": "transfer-function",
        "inputs": [channel("e", "V")], "outputs": [channel("c", output_unit)],
        "numerator": [2.0], "denominator": [1.0]
    }))
    .expect("a valid controller")
}

#[test]
fn the_default_grid_is_the_c23_grid_of_the_loop() {
    let plant = lag_plant();
    let controller = gain_controller("V");
    let definition =
        feedback_definition(&plant, Some(&controller), None, &AnalysisInputs::default())
            .expect("the loop");
    let grid = feedback_grid(&definition, None).expect("the default grid");
    assert_eq!(grid.points, 128);
    assert!((grid.minimum_frequency - 0.1).abs() < 1e-12);
    assert!((grid.maximum_frequency - 100.0).abs() < 1e-12);
}

#[test]
fn a_grid_outside_the_rules_of_n4_3_is_refused_with_its_points_named() {
    let plant = lag_plant();
    let controller = gain_controller("V");
    let definition =
        feedback_definition(&plant, Some(&controller), None, &AnalysisInputs::default())
            .expect("the loop");
    let too_many = FrequencyGrid {
        minimum_frequency: 0.1,
        maximum_frequency: 100.0,
        points: 513,
    };
    let error = feedback_grid(&definition, Some(too_many)).expect_err("513 points");
    assert!(error.message.contains("512"), "{}", error.message);
    let one_point = FrequencyGrid {
        points: 1,
        ..too_many
    };
    assert!(feedback_grid(&definition, Some(one_point)).is_err());
}

#[test]
fn a_named_channel_must_exist_and_the_units_at_the_junctions_must_agree() {
    let plant = lag_plant();
    let controller = gain_controller("V");
    let unknown = AnalysisInputs {
        plant_channel: Some(ChannelPair {
            input_id: "nope".to_string(),
            output_id: "y".to_string(),
        }),
        ..AnalysisInputs::default()
    };
    let error = feedback_definition(&plant, Some(&controller), None, &unknown)
        .expect_err("an unknown plant channel");
    assert!(error.message.contains("plantChannel"), "{}", error.message);
    let mismatched = gain_controller("Nm");
    let error = feedback_definition(&plant, Some(&mismatched), None, &AnalysisInputs::default())
        .expect_err("a unit mismatch");
    assert!(error.message.contains("output unit"), "{}", error.message);
}

#[test]
fn the_answer_of_a_lag_loop_with_gain_two_reports_its_stable_closed_loop() {
    let plant = lag_plant();
    let controller = gain_controller("V");
    let definition =
        feedback_definition(&plant, Some(&controller), None, &AnalysisInputs::default())
            .expect("the loop");
    let elapsed = || Duration::from_secs(0);
    let cancelled = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancelled);
    let answer =
        feedback_answer(&definition, &AnalysisInputs::default(), &budget).expect("the answer");
    assert_eq!(answer.grid.points, 128);
    assert_eq!(answer.controller_source, ControllerSource::Model);
    // The open loop 2/(s + 1) has a gain crossover and no phase crossover: the phase margin is named, the gain margin is not infinite.
    assert!(
        answer.summary[0].contains("phase margin"),
        "{:?}",
        answer.summary
    );
    // Its contour does not encircle −1, and its open loop has no right-half-plane pole.
    assert!(
        answer.summary[1].starts_with("Nyquist contour: P = 0 open-loop right-half-plane poles."),
        "{:?}",
        answer.summary
    );
    assert!(
        answer.summary[1].ends_with("the closed loop is stable."),
        "{:?}",
        answer.summary
    );
    assert_eq!(answer.details.len(), 3);
    match &answer.closed_loop {
        ClosedLoop::Qualified { poles, stable, .. } => {
            assert!(*stable);
            assert_eq!(poles.len(), 1);
            // s + 1 + 2 = 0, so the closed-loop pole is −3.
            assert!((poles[0].real + 3.0).abs() < 1e-6);
        }
        ClosedLoop::Unavailable { reason } => panic!("the closed loop is available: {reason}"),
    }
}

#[test]
fn a_loop_without_a_controller_or_a_declared_law_asks_for_the_loop_setup() {
    let error = feedback_definition(&lag_plant(), None, None, &AnalysisInputs::default())
        .expect_err("no controller");
    assert_eq!(error.next_action, NextAction::OpenLoopSetup);
}
