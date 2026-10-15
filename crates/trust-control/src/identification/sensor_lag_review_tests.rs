//! Tests of the sensor-lag comparison (`sensorLagComparison.ts`, the lesson 24 case of the course). The record is the
//! lesson's two-store model in closed form: a water store of 5 s and a probe store of 2 s behind a 1 s delay, at 0.2 °C
//! per % over a 10-point step from 35 °C, with the step at 5.1 s and the restoration at 45.1 s, as the lesson's saved
//! recording. The lesson's limit is a residual of 0.02 °C with a measurement bound of 0.02 °C.

use serde_json::json;
use trust_control_math::identify::{LowOrderModelKind, SampleQuality};

use super::*;
use crate::identification::bounded_step_review::{
    DISTINCT_IDENTITIES_REFUSAL, DISTINCT_VALIDATION_STEP_REFUSAL,
};
use crate::identification::data::DataSource;
use crate::identification::plant::Partition;
use crate::identification::presentation::identified_model_presentation;
use crate::identification::review::prepare_identification_review_with_structure;
use crate::identification::sensor_lag_comparison::{
    sensor_lag_settings_problem, ModelAdequacy, StructureRecommendation, DEFAULT_MODEL_ADEQUACY,
};
use crate::identification::sensor_lag_texts::{
    CALCULATE_FIRST, CHOOSE_BEFORE_VALIDATION, FIRST_ORDER_ADEQUATE, REFERENCE_CHANGED,
    STRUCTURE_NOT_MET,
};
use crate::identification::step_response::{
    step_response_settings_problem, SearchGrid, StepResponseSettings,
};

/// The lesson's settings: the template's windows and grids, with the measurement bound of 0.02 °C, the delay grid of 0 to 4 s
/// in steps of 0.2 s that the lesson sets, and the residual limit of the request (`modelAdequacy`), when one is given.
fn lesson_settings(residual_limit: Option<f64>) -> StepResponseSettings {
    StepResponseSettings {
        baseline_duration_seconds: 5.0,
        final_duration_seconds: 5.0,
        maximum_window_range: 0.1,
        maximum_window_slope_per_second: 0.01,
        measurement_bound: 0.02,
        onset_threshold: 0.15,
        onset_samples: 3,
        delay: SearchGrid {
            minimum: 0.0,
            maximum: 4.0,
            step: 0.2,
        },
        time_constant: SearchGrid {
            minimum: 0.1,
            maximum: 20.0,
            step: 0.1,
        },
        baseline_step: 0.01,
        gain_step: 0.001,
        timestamp_shift_seconds: 0.0,
        sensor_time_constant: None,
        model_adequacy: residual_limit.map(|limit| ModelAdequacy {
            maximum_absolute_residual: limit,
            maximum_final_error: 0.1,
        }),
    }
}

/// The probe's rise after `elapsed` seconds of the delayed heat, for a held step of `change` input units: the closed form of
/// the lesson's two-store response, `A·(1 − (5·e^(−t/5) − 2·e^(−t/2))/3)` with `A = 0.2 °C per % · change`.
fn probe_rise(change: f64, elapsed: f64) -> f64 {
    if elapsed < 0.0 {
        return 0.0;
    }
    let amplitude = 0.2 * change;
    amplitude * (1.0 - (5.0 * (-elapsed / 5.0).exp() - 2.0 * (-elapsed / 2.0).exp()) / 3.0)
}

/// The probe's record of a held step of `change` input units (75 % to 75 % + `change`) at 5.1 s, restored at 45.1 s: 452
/// samples every 0.1 s from 0 s, with the 35 °C baseline of the lesson. The lesson's saved recording is this closed form.
pub(crate) fn probe_record(change: f64) -> Vec<IdentificationSample> {
    (0..452_u32)
        .map(|index| {
            let time = f64::from(index) / 10.0;
            IdentificationSample {
                time_seconds: time,
                input: if (5.1..45.1).contains(&time) {
                    75.0 + change
                } else {
                    75.0
                },
                output: 35.0 + probe_rise(change, time - 5.1 - 1.0),
                quality: SampleQuality::Good,
                input_unit: Some("%".to_string()),
                output_unit: Some("degC".to_string()),
                clipped: None,
                saturated: None,
            }
        })
        .collect()
}

/// The lesson's review input: the 10-point probe record as the estimation recording, the given validation record, and the
/// settings with the given residual limit (none for the annex's default).
pub(crate) fn lesson_input(
    validation: Option<Vec<IdentificationSample>>,
    residual_limit: Option<f64>,
) -> IdentificationReviewInput {
    IdentificationReviewInput {
        recording: None,
        step_response_settings: Some(lesson_settings(residual_limit)),
        first_order_step_response_settings: None,
        maximum_normalized_error: None,
        estimation: probe_record(10.0),
        validation,
        estimation_recording_id: Some("estimation".to_string()),
        validation_recording_id: Some("validation".to_string()),
        excluded: Vec::new(),
        source: DataSource::Simulation,
        requested_kind: IdentificationModelSelection::Sopdt,
        declared_process: None,
        closed_loop: None,
    }
}

/// The preview (`validation_required`) of a review input with the given structure choice.
fn preview(
    input: &IdentificationReviewInput,
    structure: Option<StructureChoice>,
) -> ValidationReview {
    let review = prepare_identification_review_with_structure(input, structure)
        .expect("the comparison is previewed");
    let IdentificationReview::ValidationRequired(review) = review else {
        panic!("a preview waits for the independent check");
    };
    review
}

/// The independent check (`acceptance_required`) of a review input with the given structure choice.
fn checked(
    input: &IdentificationReviewInput,
    structure: Option<StructureChoice>,
) -> AcceptanceReview {
    let review = prepare_identification_review_with_structure(input, structure)
        .expect("the chosen structure is checked");
    let IdentificationReview::AcceptanceRequired(review) = review else {
        panic!("a checked structure awaits acceptance");
    };
    review
}

/// The refusal of a review input with the given structure choice.
fn refused(input: &IdentificationReviewInput, structure: Option<StructureChoice>) -> String {
    prepare_identification_review_with_structure(input, structure)
        .expect_err("the review is refused")
        .0
}

/// The structure comparison that a review's fit carries (`fit.modelStructure`).
fn comparison_of(fit: &ModelFitEvidence) -> &SensorLagComparison {
    fit.model_structure
        .as_deref()
        .expect("the comparison is attached to the fit")
}

#[test]
fn lesson_24_recommends_the_additional_lag_when_the_first_order_misses_the_residual_limit() {
    let review = preview(&lesson_input(None, Some(0.02)), None);
    let comparison = comparison_of(&review.fit);
    assert_eq!(
        comparison.recommended,
        StructureRecommendation::AdditionalLag
    );
    assert!(
        !comparison.candidate_a.adequate,
        "the first order misses the 0.02 limit"
    );
    assert!(
        comparison.candidate_b.adequate,
        "the additional lag meets it"
    );
    // The lesson reports 0.143 °C for the first order; the synthetic record gives the same figure within the search grid.
    assert!(
        (0.12..0.16).contains(&comparison.candidate_a.maximum_absolute_residual),
        "{}",
        comparison.candidate_a.maximum_absolute_residual
    );
    // The lesson reports 0.0024 °C for the additional lag, within its grid.
    assert!(comparison.candidate_b.maximum_absolute_residual < 0.01);
    // This record's improvement is close to the bound: about 0.020 by the least-squares first-order fit (the lesson's
    // README reports 0.023 from its own figures). The recommendation below depends on that difference, so a change in the
    // first-order fit moves it first.
    assert!(
        comparison.rms_improvement > 0.02,
        "the improvement is above the measurement bound"
    );
    assert_eq!(review.selected.kind, LowOrderModelKind::SecondOrderDeadTime);
    assert_eq!(comparison.choice, None, "the engineer has not chosen yet");
}

#[test]
fn with_the_default_residual_limit_the_first_order_model_is_adequate_and_recommended() {
    // The lesson: "With the default 0.2 °C limit, Control Studio suggests A".
    let review = preview(&lesson_input(None, None), None);
    let comparison = comparison_of(&review.fit);
    assert_eq!(comparison.requirements, DEFAULT_MODEL_ADEQUACY);
    assert!(comparison.candidate_a.adequate);
    assert_eq!(comparison.recommended, StructureRecommendation::FirstOrder);
    assert_eq!(review.selected.kind, LowOrderModelKind::FirstOrderDeadTime);
}

#[test]
fn a_preview_selects_the_structure_the_engineer_chose_and_keeps_both_models() {
    let review = preview(
        &lesson_input(None, Some(0.02)),
        Some(StructureChoice::AdditionalLag),
    );
    assert_eq!(review.selected.kind, LowOrderModelKind::SecondOrderDeadTime);
    assert_eq!(review.preview.models.len(), 2);
    assert_eq!(
        review.preview.recommended.kind,
        LowOrderModelKind::SecondOrderDeadTime
    );
    assert_eq!(
        comparison_of(&review.fit).choice,
        Some(StructureChoice::AdditionalLag)
    );
}

#[test]
fn a_first_order_choice_is_kept_when_the_first_order_model_is_adequate() {
    let review = preview(&lesson_input(None, None), Some(StructureChoice::FirstOrder));
    assert_eq!(review.selected.kind, LowOrderModelKind::FirstOrderDeadTime);
    assert_eq!(
        comparison_of(&review.fit).choice,
        Some(StructureChoice::FirstOrder)
    );
    let refusal = refused(
        &lesson_input(None, None),
        Some(StructureChoice::AdditionalLag),
    );
    assert_eq!(refusal, FIRST_ORDER_ADEQUATE);
}

#[test]
fn a_structure_other_than_the_recommended_one_cannot_be_chosen() {
    let refusal = refused(
        &lesson_input(None, Some(0.02)),
        Some(StructureChoice::FirstOrder),
    );
    assert_eq!(refusal, STRUCTURE_NOT_MET);
}

#[test]
fn a_structure_is_chosen_only_from_a_comparison() {
    let bounded = IdentificationReviewInput {
        requested_kind: IdentificationModelSelection::Fopdt,
        ..lesson_input(None, Some(0.02))
    };
    assert_eq!(
        refused(&bounded, Some(StructureChoice::FirstOrder)),
        CALCULATE_FIRST
    );
}

#[test]
fn the_chosen_additional_lag_is_checked_on_an_independent_recording() {
    let review = checked(
        &lesson_input(Some(probe_record(12.0)), Some(0.02)),
        Some(StructureChoice::AdditionalLag),
    );
    assert!(
        review.validation_qualification.suitable,
        "the frozen model meets the independent step"
    );
    assert_eq!(review.result.partition, Some(Partition::IndependentRuns));
    assert_eq!(review.selected.kind, LowOrderModelKind::SecondOrderDeadTime);
    let validation_error = review
        .selected
        .normalized_validation_error
        .expect("the frozen model is scored on the record");
    assert!(validation_error < 0.01, "{validation_error}");
    let residuals = review
        .selected
        .validation_residuals
        .clone()
        .expect("the validation residuals are kept");
    assert!(residuals.iter().all(|residual| residual.abs() <= 0.02));
    let applied = review
        .fit
        .step_response_measurement
        .as_ref()
        .map_or(0.0, |step| step.input_change);
    assert!(
        (applied - 12.0).abs() < 1e-9,
        "the validation step is the 12-point step"
    );
    assert_eq!(
        comparison_of(&review.fit).choice,
        Some(StructureChoice::AdditionalLag)
    );
}

#[test]
fn an_independent_check_waits_for_the_engineer_to_choose_a_structure() {
    let refusal = refused(&lesson_input(Some(probe_record(12.0)), Some(0.02)), None);
    assert_eq!(refusal, CHOOSE_BEFORE_VALIDATION);
}

#[test]
fn the_independent_check_needs_two_distinct_recording_identities() {
    let mut input = lesson_input(Some(probe_record(12.0)), Some(0.02));
    input.validation_recording_id = Some("estimation".to_string());
    assert_eq!(
        refused(&input, Some(StructureChoice::AdditionalLag)),
        DISTINCT_IDENTITIES_REFUSAL
    );
}

#[test]
fn the_independent_check_needs_a_different_applied_step() {
    let refusal = refused(
        &lesson_input(Some(probe_record(10.0)), Some(0.02)),
        Some(StructureChoice::AdditionalLag),
    );
    assert_eq!(refusal, DISTINCT_VALIDATION_STEP_REFUSAL);
}

#[test]
fn the_first_order_reference_must_share_the_comparison_windows_and_measurement_bound() {
    let mut reference = lesson_settings(Some(0.02));
    reference.measurement_bound = 0.05;
    let input = IdentificationReviewInput {
        first_order_step_response_settings: Some(reference),
        ..lesson_input(None, Some(0.02))
    };
    assert_eq!(refused(&input, None), REFERENCE_CHANGED);
}

#[test]
fn a_sensor_time_grid_must_be_above_zero_and_is_refused_with_the_settings() {
    let mut settings = lesson_settings(Some(0.02));
    settings.sensor_time_constant = Some(SearchGrid {
        minimum: 0.0,
        maximum: 10.0,
        step: 0.1,
    });
    let problem = step_response_settings_problem(&settings).expect("the grid is refused");
    assert_eq!(problem.field, "sensorTimeConstant.minimum");
    assert_eq!(problem.message, "Enter a finite value greater than zero.");
    let input = IdentificationReviewInput {
        step_response_settings: Some(settings),
        ..lesson_input(None, Some(0.02))
    };
    assert_eq!(refused(&input, None), INVALID_SETTINGS);
}

#[test]
fn a_sensor_time_grid_must_divide_its_span_exactly() {
    let mut settings = lesson_settings(Some(0.02));
    settings.sensor_time_constant = Some(SearchGrid {
        minimum: 0.1,
        maximum: 10.0,
        step: 0.4,
    });
    let problem = sensor_lag_settings_problem(&settings).expect("the grid is refused");
    assert_eq!(problem.field, "sensorTimeConstant.step");
    assert_eq!(
        problem.message,
        "Choose a bounded grid step that divides the sensor-time span exactly."
    );
}

#[test]
fn a_residual_limit_must_be_above_zero() {
    let settings = lesson_settings(Some(0.0));
    let problem = sensor_lag_settings_problem(&settings).expect("the limit is refused");
    assert_eq!(problem.field, "modelAdequacy.maximumAbsoluteResidual");
    assert_eq!(
        problem.message,
        "Enter a finite model-error limit greater than zero."
    );
}

#[test]
fn the_comparison_is_the_model_structure_of_the_presentation_and_reads_back() {
    let review = preview(&lesson_input(None, Some(0.02)), None);
    let presented = identified_model_presentation(&review.selected, &review.fit, "%", "degC");
    let value = serde_json::to_value(&presented).expect("the presentation serialises");
    assert_eq!(
        value["modelStructure"]["recommended"],
        json!("additional_lag")
    );
    assert_eq!(
        value["modelStructure"]["candidateA"]["adequate"],
        json!(false)
    );
    assert_eq!(
        value["modelStructure"]["candidateB"]["adequate"],
        json!(true)
    );
    let comparison = comparison_of(&review.fit).clone();
    let stored: SensorLagComparison =
        serde_json::from_value(value["modelStructure"].clone()).expect("the comparison reads back");
    assert_eq!(stored, comparison);
}
