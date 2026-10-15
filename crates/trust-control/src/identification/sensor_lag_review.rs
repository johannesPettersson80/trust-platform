//! The sensor-lag comparison of a held step (`prepareSensorLagComparison` and `chooseSensorLagModel` in
//! `sensorLagComparison.ts`; specification 35, B3.5 and B5.2; numerical annex N2.10 and N3.4). The first-order reference
//! is the bounded step review of the estimation recording, the additional lag is fitted on that record by the sensor-lag
//! search, and the two structures are compared. The structure the engineer chose (only the recommended one can be chosen)
//! is then checked on the independent recording, by the held-step rules of N3.4 and N3.5. The comparison stays provisional
//! until that check passes, and the additional lag's physical origin stays unresolved (N2.10). The comparison is attached to
//! the fit of the review that chose the structure (`fit.modelStructure`).

use trust_control_math::identify::LowOrderModelKind;

use crate::identification::bounded_step_review::{
    distinct_recordings, exceeds_bound, prepare_bounded_step_review, score_frozen_step,
    BEST_FIT_BOUND_REFUSAL, DISTINCT_IDENTITIES_REFUSAL, DISTINCT_VALIDATION_STEP_REFUSAL,
    FROZEN_MODEL_BOUND_REFUSAL, RESOLVE_ESTIMATION_REFUSAL, TOLERANCE,
};
use crate::identification::data::{qualify_identification_data, IdentificationSample};
use crate::identification::evidence::{model_fit_evidence, ModelFitEvidence, ModelFitPoint};
use crate::identification::plant::{
    IdentificationResult, IdentifiedModel, IdentifiedUncertainty, Partition, UncertaintyMethod,
};
use crate::identification::presentation::identified_model_equation;
use crate::identification::refusal::Refusal;
use crate::identification::review::{
    AcceptanceReview, IdentificationModelSelection, IdentificationReview,
    IdentificationReviewInput, ValidationReview,
};
use crate::identification::sensor_lag::{fit_sensor_lag, SensorLagRanges, SensorLagStepAnalysis};
use crate::identification::sensor_lag_comparison::{
    assess_structure, choose_structure, recommend_structure, rms_improvement, SensorLagComparison,
    StructureAssessment, StructureCandidate, StructureChoice, StructureRecommendation,
    DEFAULT_MODEL_ADEQUACY,
};
use crate::identification::sensor_lag_texts::{
    CHOOSE_BEFORE_VALIDATION, NEEDS_PREVIEW, REFERENCE_CHANGED,
};
use crate::identification::step_response::{
    fit_record_span, measure_step_response, valid_step_response_settings, StepResponseAnalysis,
    StepResponseSettings, INVALID_SETTINGS,
};

/// Compares the first-order reference with the additional lag on the estimation recording (`prepareSensorLagComparison`,
/// N2.10), and checks the structure the engineer chose on an independent recording when one is given (N3.4, N3.5).
///
/// The sensor-time grid and the residual requirements are the settings' own (`sensorTimeConstant`, `modelAdequacy`). The
/// comparison always runs on the estimation recording, so a validation recording never refuses it. Without a validation
/// recording the review is a preview (`validation_required`): it selects the structure the engineer chose, or else the
/// recommended one, and a `neither` recommendation selects the first-order model. With a validation recording, the chosen
/// structure is checked on it: the first-order model by the bounded step rules, and the additional lag by the same rules on
/// its frozen parameters. The comparison is attached to the fit of the review (`fit.modelStructure`).
///
/// # Errors
/// Refuses, in this order: missing or invalid step-response settings; a first-order reference whose windows or measurement
/// assumptions differ from the comparison's; the refusals of the bounded step review of the reference and of the sensor-lag
/// search; a chosen structure that is not the recommended one; a validation recording without a chosen structure; and the
/// refusals of the check.
pub fn prepare_sensor_lag_comparison(
    input: &IdentificationReviewInput,
    structure: Option<StructureChoice>,
) -> Result<IdentificationReview, Refusal> {
    let settings = input
        .step_response_settings
        .as_ref()
        .ok_or_else(|| Refusal::new(INVALID_SETTINGS))?;
    if !valid_step_response_settings(settings) {
        return Err(Refusal::new(INVALID_SETTINGS));
    }
    let first_order = input
        .first_order_step_response_settings
        .as_ref()
        .unwrap_or(settings);
    refuse_changed_reference(settings, first_order)?;
    let reference = estimation_reference(input, first_order)?;
    let candidate = second_order_candidate(&reference, settings)?;
    let comparison = compare(&reference, &candidate, settings, structure)?;
    let recommended_structure = default_structure(comparison.recommended);
    let chosen = comparison.choice.unwrap_or(recommended_structure);
    let review = match input.validation.as_deref() {
        None => preview_review(reference, &candidate, recommended_structure, chosen),
        Some(validation) => {
            if comparison.choice.is_none() {
                return Err(Refusal::new(CHOOSE_BEFORE_VALIDATION));
            }
            check_chosen(
                input,
                settings,
                first_order,
                &reference,
                &candidate,
                chosen,
                validation,
            )?
        }
    };
    Ok(with_structure(review, comparison))
}

/// The first-order reference must have been measured with the comparison's windows and measurement assumptions
/// (`prepareSensorLagComparison`): the same baseline and final durations, measurement bound, window range and slope, and
/// sample time offset.
#[allow(clippy::float_cmp)] // The settings are compared exactly, as the `TypeScript` compares them.
fn refuse_changed_reference(
    settings: &StepResponseSettings,
    first_order: &StepResponseSettings,
) -> Result<(), Refusal> {
    let unchanged = settings.baseline_duration_seconds == first_order.baseline_duration_seconds
        && settings.final_duration_seconds == first_order.final_duration_seconds
        && settings.measurement_bound == first_order.measurement_bound
        && settings.maximum_window_range == first_order.maximum_window_range
        && settings.maximum_window_slope_per_second == first_order.maximum_window_slope_per_second
        && settings.timestamp_shift_seconds == first_order.timestamp_shift_seconds;
    if unchanged {
        Ok(())
    } else {
        Err(Refusal::new(REFERENCE_CHANGED))
    }
}

/// The first-order reference of the estimation recording: the bounded step review of the recording with the first-order
/// settings, taken without a validation recording (N2.10: the comparison runs on the estimation record).
fn estimation_reference(
    input: &IdentificationReviewInput,
    first_order: &StepResponseSettings,
) -> Result<ValidationReview, Refusal> {
    let estimation = IdentificationReviewInput {
        requested_kind: IdentificationModelSelection::Fopdt,
        step_response_settings: Some(first_order.clone()),
        validation: None,
        validation_recording_id: None,
        ..input.clone()
    };
    match prepare_bounded_step_review(&estimation)? {
        IdentificationReview::ValidationRequired(review) => Ok(review),
        IdentificationReview::AcceptanceRequired(_) => Err(Refusal::new(NEEDS_PREVIEW)),
    }
}

/// The additional-lag candidate on the estimation record (`fitSensorLagStep`, N2.10): the sensor-lag search over the held
/// step of the reference, with the comparison's own grids and measurement settings, its model with the normalized error of
/// N3.2, and its fit evidence on the estimation record.
struct SecondOrderCandidate {
    /// The sensor-lag analysis of the search.
    analysis: SensorLagStepAnalysis,
    /// The second-order model with dead time.
    model: IdentifiedModel,
    /// The fit evidence of the model on the estimation record.
    fit: ModelFitEvidence,
}

/// Fits the additional lag on the estimation record of the reference.
///
/// # Errors
/// Refuses with the sentences of the sensor-lag search when its grids or work exceed their limits or no candidate lies within
/// the qualified windows, and with the refusals of the record's span and fit evidence.
fn second_order_candidate(
    reference: &ValidationReview,
    settings: &StepResponseSettings,
) -> Result<SecondOrderCandidate, Refusal> {
    let Some(step) = reference.step_response.as_ref() else {
        return Err(Refusal::new(NEEDS_PREVIEW));
    };
    // The windows and measurement assumptions of the reference are the comparison's (`refuse_changed_reference`), so the
    // search reads the comparison's own settings over the reference's measurement.
    let search_reference = StepResponseAnalysis {
        settings: settings.clone(),
        ..step.clone()
    };
    let analysis = fit_sensor_lag(
        &reference.estimation,
        &search_reference,
        settings.sensor_time_constant.clone(),
    )?;
    let rms = (analysis.best.squared_error / count_f64(analysis.points.len())).sqrt();
    let span = fit_record_span(&reference.estimation, settings)?;
    let model = second_order_model(&analysis, rms / span);
    let fit = second_order_fit(&model, &reference.estimation, &analysis, rms)?;
    Ok(SecondOrderCandidate {
        analysis,
        model,
        fit,
    })
}

/// The second-order model with dead time of the sensor-lag search: its gain, its two time constants and its dead time, its
/// normalized error, the residuals of the held samples, the measurement-consistent grid as its uncertainty, and the
/// search-boundary finding (N2.10, N3.2).
fn second_order_model(analysis: &SensorLagStepAnalysis, normalized_error: f64) -> IdentifiedModel {
    IdentifiedModel {
        kind: LowOrderModelKind::SecondOrderDeadTime,
        gain: analysis.best.gain,
        time_constant_seconds: Some(analysis.best.time_constant_seconds),
        second_time_constant_seconds: Some(analysis.best.second_time_constant_seconds),
        dead_time_seconds: Some(analysis.best.delay_seconds),
        normalized_error: Some(normalized_error),
        normalized_estimation_error: Some(normalized_error),
        normalized_validation_error: None,
        estimation_residuals: Some(analysis.points.iter().map(|point| point.residual).collect()),
        validation_residuals: None,
        uncertainty: second_order_uncertainty(&analysis.consistency),
        stable: true,
        on_search_boundary: analysis.boundary_limited,
    }
}

/// The measurement-consistent grid of the sensor-lag search as the model's uncertainty (N2.4), present only when the set
/// holds a combination.
fn second_order_uncertainty(consistency: &SensorLagRanges) -> Option<IdentifiedUncertainty> {
    let gain = consistency.gain.filter(|_| consistency.count > 0)?;
    Some(IdentifiedUncertainty {
        method: UncertaintyMethod::MeasurementConsistentGrid,
        gain,
        time_constant_seconds: consistency.time_constant_seconds,
        second_time_constant_seconds: consistency.second_time_constant_seconds,
        dead_time_seconds: consistency.delay_seconds,
    })
}

/// The fit evidence of the second-order model on the estimation record: the samples from the start of the baseline window to
/// the end of the hold, with the baseline as the prediction before the step and the search's fitted points after it, the
/// root-mean-square residual of the hold, and the search's warnings.
///
/// # Errors
/// Refuses with the refusals of the model's fit evidence.
fn second_order_fit(
    model: &IdentifiedModel,
    estimation: &[IdentificationSample],
    analysis: &SensorLagStepAnalysis,
    rms: f64,
) -> Result<ModelFitEvidence, Refusal> {
    let window = analysis.baseline.start_seconds..analysis.hold_end_seconds;
    let measured: Vec<IdentificationSample> = estimation
        .iter()
        .filter(|sample| window.contains(&sample.time_seconds))
        .cloned()
        .collect();
    let mut fit = model_fit_evidence(model, &measured)?;
    let baseline = analysis.best.baseline;
    let before_step = measured
        .iter()
        .filter(|sample| sample.time_seconds < analysis.step_time_seconds)
        .map(|sample| ModelFitPoint {
            time_seconds: sample.time_seconds,
            measured: sample.output,
            predicted: baseline,
            residual: sample.output - baseline,
        });
    fit.points = before_step.chain(analysis.points.iter().cloned()).collect();
    fit.root_mean_square_error = rms;
    fit.warnings.clone_from(&analysis.warnings);
    Ok(fit)
}

/// The comparison of the two structures on the estimation record (`prepareSensorLagComparison`): each candidate's residual
/// assessment, the root-mean-square improvement of the additional lag, the recommendation, and the structure the engineer
/// chose, which must be the recommended one (`chooseSensorLagModel`).
///
/// # Errors
/// Refuses with the sentence of [`choose_structure`] when the chosen structure is not the recommended one.
fn compare(
    reference: &ValidationReview,
    candidate: &SecondOrderCandidate,
    settings: &StepResponseSettings,
    structure: Option<StructureChoice>,
) -> Result<SensorLagComparison, Refusal> {
    let Some(step) = reference.step_response.as_ref() else {
        return Err(Refusal::new(NEEDS_PREVIEW));
    };
    let requirements = settings.model_adequacy.unwrap_or(DEFAULT_MODEL_ADEQUACY);
    let first = assess_structure(&reference.fit.points, &step.final_window, requirements);
    let second = assess_structure(
        &candidate.fit.points,
        &candidate.analysis.final_window,
        requirements,
    );
    let improvement = rms_improvement(&first, &second);
    let recommended = recommend_structure(
        &first,
        &second,
        improvement,
        candidate.analysis.settings.measurement_bound,
    );
    let choice = structure
        .map(|chosen| choose_structure(Some(recommended), chosen))
        .transpose()?;
    Ok(SensorLagComparison {
        candidate_a: structure_candidate(
            first,
            step.clone(),
            reference.selected.clone(),
            reference.fit.clone(),
        ),
        candidate_b: structure_candidate(
            second,
            candidate.analysis.clone(),
            candidate.model.clone(),
            candidate.fit.clone(),
        ),
        requirements,
        rms_improvement: improvement,
        recommended,
        choice,
    })
}

/// One structure of the comparison: its assessment, its analysis, its model and its fit, with the model's equation.
fn structure_candidate<A>(
    assessment: StructureAssessment,
    analysis: A,
    model: IdentifiedModel,
    fit: ModelFitEvidence,
) -> StructureCandidate<A> {
    let equation = identified_model_equation(&model);
    StructureCandidate {
        maximum_absolute_residual: assessment.maximum_absolute_residual,
        final_error: assessment.final_error,
        root_mean_square_error: assessment.root_mean_square_error,
        adequate: assessment.adequate,
        analysis,
        model,
        fit,
        equation,
    }
}

/// The structure a review selects when the engineer has not chosen one: the additional lag when it is recommended, and
/// otherwise the first-order model (`useExtended` in `sensorLagComparison.ts`).
fn default_structure(recommended: StructureRecommendation) -> StructureChoice {
    match recommended {
        StructureRecommendation::AdditionalLag => StructureChoice::AdditionalLag,
        StructureRecommendation::FirstOrder | StructureRecommendation::Neither => {
            StructureChoice::FirstOrder
        }
    }
}

/// The preview of the comparison (`validation_required`): it selects the chosen structure, or else the recommended one, with
/// that structure's fit, and it keeps the recommended model beside the first-order model. The comparison is attached by
/// [`with_structure`].
fn preview_review(
    reference: ValidationReview,
    candidate: &SecondOrderCandidate,
    recommended_structure: StructureChoice,
    chosen: StructureChoice,
) -> IdentificationReview {
    let selected = model_of(chosen, &reference, candidate);
    let fit = fit_of(chosen, &reference, candidate);
    let preview = IdentificationResult {
        recommended: model_of(recommended_structure, &reference, candidate),
        models: vec![reference.selected.clone(), candidate.model.clone()],
        ..reference.preview
    };
    IdentificationReview::ValidationRequired(ValidationReview {
        step_response: reference.step_response,
        estimation_qualification: reference.estimation_qualification,
        preview,
        selected,
        fit,
        estimation: reference.estimation,
    })
}

/// The model a structure names: the first-order reference's model, or the additional lag's second-order model.
fn model_of(
    structure: StructureChoice,
    reference: &ValidationReview,
    candidate: &SecondOrderCandidate,
) -> IdentifiedModel {
    match structure {
        StructureChoice::FirstOrder => reference.selected.clone(),
        StructureChoice::AdditionalLag => candidate.model.clone(),
    }
}

/// The fit evidence a structure names: the reference's first-order fit, or the additional lag's fit on the estimation record.
fn fit_of(
    structure: StructureChoice,
    reference: &ValidationReview,
    candidate: &SecondOrderCandidate,
) -> ModelFitEvidence {
    match structure {
        StructureChoice::FirstOrder => reference.fit.clone(),
        StructureChoice::AdditionalLag => candidate.fit.clone(),
    }
}

/// The independent check of the chosen structure (N2.10, N3.4, N3.5): the first-order model is checked by the bounded step
/// rules, and the additional lag by the same rules on its frozen parameters.
fn check_chosen(
    input: &IdentificationReviewInput,
    settings: &StepResponseSettings,
    first_order: &StepResponseSettings,
    reference: &ValidationReview,
    candidate: &SecondOrderCandidate,
    chosen: StructureChoice,
    validation: &[IdentificationSample],
) -> Result<IdentificationReview, Refusal> {
    match chosen {
        StructureChoice::FirstOrder => prepare_bounded_step_review(&IdentificationReviewInput {
            requested_kind: IdentificationModelSelection::Fopdt,
            step_response_settings: Some(first_order.clone()),
            ..input.clone()
        }),
        StructureChoice::AdditionalLag => {
            check_second_order(input, settings, reference, candidate, validation)
        }
    }
}

/// The independent check of the additional lag (N3.4, N3.5): the frozen second-order model is scored by free-running
/// prediction on the validation recording, each residual is compared with the measurement bound, and the normalized
/// validation error is taken over the validation record's own span (N3.2). The estimation fit must first have a
/// measurement-consistent set, no search-boundary finding, no timestamp shift, and no residual beyond the bound.
///
/// # Errors
/// Refuses with the sentences of the bounded step review's checks (the recording identities, the estimation fit's consistent
/// set, boundary and bound, and the validation step's amplitude), and with the refusals of the measurement and fit evidence.
fn check_second_order(
    input: &IdentificationReviewInput,
    settings: &StepResponseSettings,
    reference: &ValidationReview,
    candidate: &SecondOrderCandidate,
    validation: &[IdentificationSample],
) -> Result<IdentificationReview, Refusal> {
    if !distinct_recordings(input) {
        return Err(Refusal::new(DISTINCT_IDENTITIES_REFUSAL));
    }
    let analysis = &candidate.analysis;
    if analysis.consistency.count == 0
        || analysis.boundary_limited
        || settings.timestamp_shift_seconds != 0.0
    {
        return Err(Refusal::new(RESOLVE_ESTIMATION_REFUSAL));
    }
    let bound = settings.measurement_bound;
    if exceeds_bound(analysis.points.iter().map(|point| point.residual), bound) {
        return Err(Refusal::new(BEST_FIT_BOUND_REFUSAL));
    }
    let validation_step = measure_step_response(validation, settings)?;
    if (validation_step.input_change - analysis.input_change).abs() <= TOLERANCE {
        return Err(Refusal::new(DISTINCT_VALIDATION_STEP_REFUSAL));
    }
    let mut fit = score_frozen_step(&candidate.model, validation, &validation_step)?;
    let mut qualification = qualify_identification_data(validation, &[], true);
    qualification.step_response = Some(validation_step.clone());
    if exceeds_bound(fit.points.iter().map(|point| point.residual), bound) {
        qualification.suitable = false;
        qualification.suitable_for_design = false;
        qualification.reason = Some(FROZEN_MODEL_BOUND_REFUSAL.to_string());
    }
    let validation_span = fit_record_span(validation, settings)?;
    let mut model = candidate.model.clone();
    model.normalized_validation_error = Some(fit.root_mean_square_error / validation_span);
    model.validation_residuals = Some(fit.points.iter().map(|point| point.residual).collect());
    fit.step_response.clone_from(&reference.step_response);
    fit.step_response_measurement = Some(validation_step);
    fit.warnings.clone_from(&analysis.warnings);
    let result = IdentificationResult {
        source: reference.preview.source,
        recommended: model.clone(),
        models: vec![model.clone()],
        partition: Some(Partition::IndependentRuns),
        refusal: None,
        integrator_refusal: None,
    };
    Ok(IdentificationReview::AcceptanceRequired(AcceptanceReview {
        step_response: reference.step_response.clone(),
        estimation_qualification: reference.estimation_qualification.clone(),
        validation_qualification: qualification,
        result,
        selected: model,
        fit,
        estimation: reference.estimation.clone(),
        validation: validation.to_vec(),
    }))
}

/// Attaches the comparison to the fit of the review whose structure it chose (`fit.modelStructure`).
fn with_structure(
    review: IdentificationReview,
    comparison: SensorLagComparison,
) -> IdentificationReview {
    let comparison = Box::new(comparison);
    match review {
        IdentificationReview::ValidationRequired(mut review) => {
            review.fit.model_structure = Some(comparison);
            IdentificationReview::ValidationRequired(review)
        }
        IdentificationReview::AcceptanceRequired(mut review) => {
            review.fit.model_structure = Some(comparison);
            IdentificationReview::AcceptanceRequired(review)
        }
    }
}

/// A count as a float. A count of samples is far below 2⁵³, so the conversion is exact.
#[allow(clippy::cast_precision_loss)]
fn count_f64(count: usize) -> f64 {
    count as f64
}

#[cfg(test)]
#[path = "sensor_lag_review_tests.rs"]
pub(crate) mod tests;
