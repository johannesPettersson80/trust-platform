//! The state-feedback designs, asserted on the loops and models of the TypeScript numerics and loop-setup tests
//! (`control-studio-numerics.test.ts`, `control-studio-loop-setup-state-feedback.test.ts`, `control-studio-loop-setup-lqg.test.ts`,
//! and the design rules of `stateControllerDesign.ts`). The setup generators and the webviews are not ported here.

use serde_json::{json, Value};
use trust_control_math::lqr::FiniteHorizon;

use super::*;
use crate::tuning::state_controller::fixtures::{
    loop_with_law, scalar_model, transfer_function_model, two_tank_model, two_tank_model_with_delay,
};
use crate::tuning::state_controller::preferences::StateDesignPreferences;
use crate::tuning::state_controller::{state_controller_poles, texts};

/// The text of a refusal as the engineer reads it: its whole text when it says more than its first sentence.
fn full_text(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

/// The state-feedback law of the two-tank model, with two declared poles.
fn two_tank_feedback(poles: &Value) -> Value {
    json!({ "family": "state-feedback", "convention": "sampled-deviation-negative-feedback",
        "gainSymbols": [["P1.K1", "P1.K2"]], "referenceGainSymbols": [["P1.N"]], "desiredPoles": poles })
}

/// The LQR law of one state and one input.
fn scalar_lqr() -> Value {
    json!({ "family": "LQR", "convention": "sampled-deviation-negative-feedback",
        "gainSymbols": [["P1.K"]], "referenceGainSymbols": [["P1.N"]] })
}

/// The parameter units of the two-tank laws: a gain or reference is in m3/s per m.
const TWO_TANK_UNITS: [(&str, &str); 3] =
    [("P1.K1", "m3/s/m"), ("P1.K2", "m3/s/m"), ("P1.N", "m3/s/m")];

#[test]
fn state_feedback_places_the_declared_poles_and_tracks_the_reference_on_the_controlled_output() {
    let loop_definition = loop_with_law(
        &two_tank_feedback(&json!([0.5, 0.6])),
        &TWO_TANK_UNITS,
        1000.0,
    );
    let model = two_tank_model();
    let design = design_state_controller(&loop_definition, &model, 60.0, None, None, &[0])
        .expect("the declared poles are inside the unit circle");
    assert_eq!(design.status, DesignStatus::Provisional);
    assert!(
        !design.candidate.applied,
        "a design never applies its candidate"
    );
    assert!(design.actual_st_verify);
    assert_eq!(
        design.selected_model.provenance,
        "declared-fitted-validated"
    );
    let keys: Vec<&str> = design
        .candidate
        .parameters
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["P1.K1", "P1.K2", "P1.N"]);

    let mut poles: Vec<f64> = design
        .closed_loop_poles
        .iter()
        .map(|pole| pole.real)
        .collect();
    poles.sort_by(f64::total_cmp);
    assert!(
        (poles[0] - 0.5).abs() <= 1e-9 && (poles[1] - 0.6).abs() <= 1e-9,
        "{poles:?}"
    );

    // The steady state of x = (A − BK)x + BNr with r = 1 must hold the first output at the reference: C is the identity,
    // so that output is x₀. The 2×2 system is solved by Cramer's rule.
    let (k1, k2) = (
        design.candidate.parameters["P1.K1"],
        design.candidate.parameters["P1.K2"],
    );
    let n = design.candidate.parameters["P1.N"];
    let (a, b) = ([[0.96, 0.04], [0.0, 0.96]], [0.0, 2.0]);
    let closed = [
        [a[0][0] - b[0] * k1, a[0][1] - b[0] * k2],
        [a[1][0] - b[1] * k1, a[1][1] - b[1] * k2],
    ];
    let m = [
        [1.0 - closed[0][0], -closed[0][1]],
        [-closed[1][0], 1.0 - closed[1][1]],
    ];
    let rhs = [b[0] * n, b[1] * n];
    let determinant = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    let first_output = (rhs[0] * m[1][1] - m[0][1] * rhs[1]) / determinant;
    assert!(
        (first_output - 1.0).abs() <= 1e-9,
        "the steady output {first_output} must equal the unit reference"
    );
}

#[test]
fn stored_state_weights_replace_the_automatic_ones_and_raise_the_lqr_gain() {
    let loop_definition = loop_with_law(
        &scalar_lqr(),
        &[("P1.K", "m3/s/m"), ("P1.N", "m3/s/m")],
        1000.0,
    );
    let model = scalar_model();
    let automatic = design_state_controller(&loop_definition, &model, 60.0, None, None, &[0])
        .expect("the automatic weights design the law");
    let stored = StateDesignPreferences {
        state_weights: Some(vec![vec![9.0]]),
        ..Default::default()
    };
    let heavier =
        design_state_controller(&loop_definition, &model, 60.0, None, Some(&stored), &[0])
            .expect("the stored weight designs the law");
    let gain = automatic.candidate.parameters["P1.K"];
    let heavier_gain = heavier.candidate.parameters["P1.K"];
    assert!(
        gain > 0.0,
        "the gain of a stable first-order plant is positive: {gain}"
    );
    assert!(
        heavier_gain > gain,
        "a stronger state weight raises the gain: {gain} then {heavier_gain}"
    );
}

#[test]
fn a_finite_horizon_keeps_every_stage_and_its_first_gain_is_the_designed_gain() {
    let loop_definition = loop_with_law(
        &scalar_lqr(),
        &[("P1.K", "m3/s/m"), ("P1.N", "m3/s/m")],
        1000.0,
    );
    let stored = StateDesignPreferences {
        finite_horizon: Some(FiniteHorizon {
            steps: 2,
            terminal_weights: vec![vec![1.0]],
        }),
        ..Default::default()
    };
    let design = design_state_controller(
        &loop_definition,
        &scalar_model(),
        60.0,
        None,
        Some(&stored),
        &[0],
    )
    .expect("the finite horizon designs the law");
    let horizon = design.horizon.expect("a finite horizon keeps its stages");
    assert_eq!(horizon.steps, 2);
    assert_eq!(horizon.gain_sequence.len(), 2);
    assert_eq!(horizon.riccati_sequence.len(), 3);
    assert_eq!(horizon.policy, HorizonPolicy::RepeatFirstFiniteGain);
    assert!(
        (horizon.gain_sequence[0][0][0] - design.candidate.parameters["P1.K"]).abs() <= 1e-12,
        "the candidate is the first finite gain"
    );
}

#[test]
fn an_lqg_law_designs_the_state_and_observer_gains_on_the_declared_estimator_model() {
    // Both states are measured (C is the identity), so the observer gain is a 2×2 matrix of dimensionless entries.
    let law = json!({ "family": "LQG", "convention": "sampled-deviation-negative-feedback",
        "gainSymbols": [["P1.K1", "P1.K2"]], "referenceGainSymbols": [["P1.N"]],
        "observerGainSymbols": [["P1.L11", "P1.L12"], ["P1.L21", "P1.L22"]],
        "estimatorModel": { "a": [[0.96, 0.04], [0.0, 0.96]], "b": [[0.0], [2.0]],
            "c": [[1.0, 0.0], [0.0, 1.0]], "sampleTimeSeconds": 1.0 },
        "processNoise": [[1e-6, 0.0], [0.0, 1e-6]], "measurementNoise": [[1e-6, 0.0], [0.0, 1e-6]] });
    let units = [
        ("P1.K1", "m3/s/m"),
        ("P1.K2", "m3/s/m"),
        ("P1.N", "m3/s/m"),
        ("P1.L11", "1"),
        ("P1.L12", "1"),
        ("P1.L21", "1"),
        ("P1.L22", "1"),
    ];
    let loop_definition = loop_with_law(&law, &units, 1000.0);
    let design =
        design_state_controller(&loop_definition, &two_tank_model(), 60.0, None, None, &[0])
            .expect("the LQG law designs on its declared model");
    let keys: Vec<&str> = design
        .candidate
        .parameters
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        ["P1.K1", "P1.K2", "P1.L11", "P1.L12", "P1.L21", "P1.L22", "P1.N"]
    );
    assert_eq!(
        design.closed_loop_poles.len(),
        4,
        "two controller and two estimator poles"
    );
    assert!(design
        .closed_loop_poles
        .iter()
        .all(|pole| pole.real.hypot(pole.imaginary) < 1.0));
}

#[test]
fn an_lqr_law_cannot_change_into_an_lqg_law_without_its_own_adapter() {
    let loop_definition = loop_with_law(
        &scalar_lqr(),
        &[("P1.K", "m3/s/m"), ("P1.N", "m3/s/m")],
        1000.0,
    );
    let failure = design_state_controller(
        &loop_definition,
        &scalar_model(),
        60.0,
        Some(StateControllerFamily::Lqg),
        None,
        &[0],
    )
    .expect_err("the estimator structure changes");
    assert_eq!(full_text(&failure), texts::ESTIMATOR_STRUCTURE_CHANGE);
}

#[test]
fn an_lqg_estimator_that_is_not_the_selected_model_is_refused() {
    // The declared estimator's first state coefficient is 0.5, where the selected model has 0.96.
    let law = json!({ "family": "LQG", "convention": "sampled-deviation-negative-feedback",
        "gainSymbols": [["P1.K1", "P1.K2"]], "referenceGainSymbols": [["P1.N"]],
        "observerGainSymbols": [["P1.L11", "P1.L12"], ["P1.L21", "P1.L22"]],
        "estimatorModel": { "a": [[0.5, 0.04], [0.0, 0.96]], "b": [[0.0], [2.0]],
            "c": [[1.0, 0.0], [0.0, 1.0]], "sampleTimeSeconds": 1.0 },
        "processNoise": [[1e-6, 0.0], [0.0, 1e-6]], "measurementNoise": [[1e-6, 0.0], [0.0, 1e-6]] });
    let units = [
        ("P1.K1", "m3/s/m"),
        ("P1.K2", "m3/s/m"),
        ("P1.N", "m3/s/m"),
        ("P1.L11", "1"),
        ("P1.L12", "1"),
        ("P1.L21", "1"),
        ("P1.L22", "1"),
    ];
    let loop_definition = loop_with_law(&law, &units, 1000.0);
    let failure =
        design_state_controller(&loop_definition, &two_tank_model(), 60.0, None, None, &[0])
            .expect_err("the estimator runs another model");
    assert_eq!(full_text(&failure), texts::ESTIMATOR_MODEL_DIFFERS);
}

#[test]
fn desired_poles_outside_the_unit_circle_are_refused() {
    let loop_definition = loop_with_law(
        &two_tank_feedback(&json!([1.2, 0.5])),
        &TWO_TANK_UNITS,
        1000.0,
    );
    let failure =
        design_state_controller(&loop_definition, &two_tank_model(), 60.0, None, None, &[0])
            .expect_err("a pole at 1.2 is unstable");
    assert_eq!(full_text(&failure), texts::STRICT_UNIT_CIRCLE_POLES);
}

#[test]
fn a_transfer_function_or_a_delayed_model_is_not_designed_on() {
    let loop_definition = loop_with_law(
        &scalar_lqr(),
        &[("P1.K", "m3/s/m"), ("P1.N", "m3/s/m")],
        1000.0,
    );
    let transfer = design_state_controller(
        &loop_definition,
        &transfer_function_model(),
        60.0,
        None,
        None,
        &[0],
    )
    .expect_err("a transfer function has no state-space coordinates");
    assert_eq!(full_text(&transfer), texts::NEEDS_BINDING);
    let delayed = design_state_controller(
        &loop_definition,
        &two_tank_model_with_delay(0.5),
        60.0,
        None,
        None,
        &[0],
    )
    .expect_err("a delayed model is outside the state-feedback design");
    assert_eq!(full_text(&delayed), texts::NEEDS_BINDING);
}

#[test]
fn the_controlled_objectives_must_match_the_inputs() {
    let loop_definition = loop_with_law(
        &scalar_lqr(),
        &[("P1.K", "m3/s/m"), ("P1.N", "m3/s/m")],
        1000.0,
    );
    let failure = design_state_controller(&loop_definition, &scalar_model(), 60.0, None, None, &[])
        .expect_err("one input needs one controlled objective");
    assert_eq!(full_text(&failure), texts::ONE_OBJECTIVE_PER_INPUT);
}

#[test]
fn the_closed_loop_poles_come_from_the_mapped_values_and_a_pin_that_destabilises_is_refused() {
    let loop_definition = loop_with_law(
        &two_tank_feedback(&json!([0.5, 0.6])),
        &TWO_TANK_UNITS,
        1000.0,
    );
    let model = two_tank_model();
    let design = design_state_controller(&loop_definition, &model, 60.0, None, None, &[0])
        .expect("the declared poles design the law");
    let recomputed = state_controller_poles(&loop_definition, &model, &design.candidate.parameters)
        .expect("the designed values are stable");
    let mut real: Vec<f64> = recomputed.iter().map(|pole| pole.real).collect();
    real.sort_by(f64::total_cmp);
    assert_eq!(real.len(), 2);
    assert!(
        (real[0] - 0.5).abs() <= 1e-9 && (real[1] - 0.6).abs() <= 1e-9,
        "{real:?}"
    );

    // With K = (−2, 0) the closed loop is [[0.96, 0.04], [4, 0.96]], whose eigenvalues are 1.36 and 0.56: unstable.
    let mut pinned = design.candidate.parameters.clone();
    pinned.insert("P1.K1".to_string(), -2.0);
    pinned.insert("P1.K2".to_string(), 0.0);
    let failure = state_controller_poles(&loop_definition, &model, &pinned)
        .expect_err("the pinned gain is unstable");
    assert_eq!(full_text(&failure), texts::NOT_STRICTLY_STABLE);
}
