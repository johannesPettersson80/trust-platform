//! Tests of the structure comparison: the inclusive residual limits and the final window, the recommendation and its strict
//! improvement bound, and the choice that is accepted only when it is the recommendation. The TypeScript cases are
//! "supports the added lag when the simpler fit fails the declared task and improvement exceeds measurement uncertainty",
//! "rejects an unsupported extra state without altering the retained comparison", and "keeps the simplest adequate
//! first-order fit without confusing measurement and approximation bounds".

#![allow(clippy::float_cmp)] // The default requirements are compared with the values they are declared as.

use trust_control_math::identify::step_response::StepResponseWindow;

use super::{
    assess_structure, choose_structure, recommend_structure, rms_improvement, ModelAdequacy,
    StructureChoice, StructureRecommendation, DEFAULT_MODEL_ADEQUACY,
};
use crate::identification::evidence::ModelFitPoint;
use crate::identification::sensor_lag_texts::{
    CALCULATE_FIRST, FIRST_ORDER_ADEQUATE, STRUCTURE_NOT_MET,
};

/// A window from `start` to `end` with the given mean; only its bounds in time are read by the assessment.
fn window(start: f64, end: f64) -> StepResponseWindow {
    StepResponseWindow {
        start_seconds: start,
        end_seconds: end,
        count: 0,
        mean: 0.0,
        minimum: 0.0,
        maximum: 0.0,
        range: 0.0,
        slope_per_second: 0.0,
    }
}

/// Ten samples a second apart, with the residual `early` before t = 8 s and `final_residual` in the final window [8, 10).
fn points(early: f64, final_residual: f64) -> Vec<ModelFitPoint> {
    (0..10)
        .map(|index| {
            let time_seconds = f64::from(index);
            let residual = if time_seconds >= 8.0 {
                final_residual
            } else {
                early
            };
            ModelFitPoint {
                time_seconds,
                measured: 0.0,
                predicted: 0.0,
                residual,
            }
        })
        .collect()
}

#[test]
fn the_default_requirements_are_the_declared_pair() {
    assert_eq!(DEFAULT_MODEL_ADEQUACY.maximum_absolute_residual, 0.2);
    assert_eq!(DEFAULT_MODEL_ADEQUACY.maximum_final_error, 0.1);
}

#[test]
fn assessment_uses_inclusive_limits_over_the_final_window() {
    // Every residual is 0.2 and the final window's mean residual is 0.1: both are on the limits, so the structure is adequate.
    let assessment = assess_structure(
        &points(0.2, 0.1),
        &window(8.0, 10.0),
        DEFAULT_MODEL_ADEQUACY,
    );
    assert!(assessment.adequate, "{assessment:?}");
    assert!((assessment.maximum_absolute_residual - 0.2).abs() < 1e-12);
    assert!((assessment.final_error - 0.1).abs() < 1e-12);
    // A tighter residual limit makes the same residuals inadequate.
    let tighter = ModelAdequacy {
        maximum_absolute_residual: 0.19,
        ..DEFAULT_MODEL_ADEQUACY
    };
    assert!(!assess_structure(&points(0.2, 0.1), &window(8.0, 10.0), tighter).adequate);
    // The final window's error is the mean over the window, so a large early residual alone does not fail the final check.
    let early_only = assess_structure(
        &points(0.2, 0.0),
        &window(8.0, 10.0),
        DEFAULT_MODEL_ADEQUACY,
    );
    assert!(early_only.adequate);
    assert!(early_only.final_error < 1e-12);
}

#[test]
fn an_empty_final_window_is_not_adequate() {
    let assessment = assess_structure(
        &points(0.0, 0.0),
        &window(20.0, 30.0),
        DEFAULT_MODEL_ADEQUACY,
    );
    assert!(!assessment.adequate);
}

#[test]
fn the_recommendation_follows_adequacy_and_a_strict_improvement_bound() {
    let adequate = assess_structure(
        &points(0.0, 0.0),
        &window(8.0, 10.0),
        DEFAULT_MODEL_ADEQUACY,
    );
    let inadequate = assess_structure(
        &points(0.5, 0.5),
        &window(8.0, 10.0),
        DEFAULT_MODEL_ADEQUACY,
    );
    assert_eq!(
        recommend_structure(&adequate, &inadequate, 0.0, 0.05),
        StructureRecommendation::FirstOrder
    );
    // The first-order model fails and the lag is adequate: the improvement must be strictly above the measurement bound.
    let lag = assess_structure(
        &points(0.0, 0.0),
        &window(8.0, 10.0),
        DEFAULT_MODEL_ADEQUACY,
    );
    assert_eq!(
        recommend_structure(&inadequate, &lag, 0.06, 0.05),
        StructureRecommendation::AdditionalLag
    );
    assert_eq!(
        recommend_structure(&inadequate, &lag, 0.05, 0.05),
        StructureRecommendation::Neither
    );
    assert_eq!(
        recommend_structure(&inadequate, &inadequate, 0.5, 0.05),
        StructureRecommendation::Neither
    );
    assert!((rms_improvement(&inadequate, &lag) - inadequate.root_mean_square_error).abs() < 1e-12);
}

#[test]
fn a_choice_is_accepted_only_when_it_is_the_recommendation() {
    assert_eq!(
        choose_structure(
            Some(StructureRecommendation::AdditionalLag),
            StructureChoice::AdditionalLag
        ),
        Ok(StructureChoice::AdditionalLag)
    );
    assert_eq!(
        choose_structure(
            Some(StructureRecommendation::FirstOrder),
            StructureChoice::FirstOrder
        ),
        Ok(StructureChoice::FirstOrder)
    );
    let refused = |recommended, choice| {
        choose_structure(recommended, choice)
            .expect_err("refused")
            .0
    };
    assert_eq!(
        refused(
            Some(StructureRecommendation::FirstOrder),
            StructureChoice::AdditionalLag
        ),
        FIRST_ORDER_ADEQUATE
    );
    assert_eq!(
        refused(
            Some(StructureRecommendation::Neither),
            StructureChoice::FirstOrder
        ),
        STRUCTURE_NOT_MET
    );
    assert_eq!(
        refused(
            Some(StructureRecommendation::AdditionalLag),
            StructureChoice::FirstOrder
        ),
        STRUCTURE_NOT_MET
    );
    assert_eq!(refused(None, StructureChoice::FirstOrder), CALCULATE_FIRST);
}
