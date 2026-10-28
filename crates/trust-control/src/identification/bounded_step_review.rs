//! The bounded step review (`boundedStepReview.ts`): the explicit step method inside Identify's estimation and
//! validation stages (numerical annex N2.4, N3.4 and N3.5; specification 35, B3.5 and B5.2). The estimation stage
//! fits the held step with `analyze_step_response` and keeps its measurement-consistent set as the model's
//! uncertainty. The validation stage freezes that model and scores it on a distinct recording of a changed step
//! amplitude against the same measurement bound; the model is never refitted on the validation data.

// `prepare_bounded_step_review` keeps the name of the `TypeScript` rule.
#![allow(clippy::module_name_repetitions)]

use trust_control_math::identify::step_response::ConsistentSet;
use trust_control_math::identify::LowOrderModelKind;

use crate::identification::data::{
    qualify_identification_data, DataSource, IdentificationSample, QualifiedIdentificationData,
};
use crate::identification::evidence::{model_fit_evidence, ModelFitEvidence, ModelFitPoint};
use crate::identification::plant::{
    IdentificationResult, IdentifiedModel, IdentifiedUncertainty, Partition, ResultSource,
    UncertaintyMethod,
};
use crate::identification::refusal::Refusal;
use crate::identification::review::{
    AcceptanceReview, IdentificationModelSelection, IdentificationReview,
    IdentificationReviewInput, ValidationReview,
};
use crate::identification::step_response::{
    analyze_step_response, fit_record_span, measure_step_response, StepResponseAnalysis,
    StepResponseMeasurement, StepResponsePoint, StepResponseSettings,
    DEFAULT_MAXIMUM_NORMALIZED_ERROR, INVALID_SETTINGS,
};

/// The tolerance of the review's comparisons of responses, residuals and step amplitudes.
pub(crate) const TOLERANCE: f64 = 1e-9;

/// The refusal of a validation whose two recording identities are missing, empty or equal (N3.4).
pub(crate) const DISTINCT_IDENTITIES_REFUSAL: &str =
    "Bounded step validation requires two distinct immutable recording identities.";

/// The refusal of an estimation fit whose consistent set is empty, whose search reaches a boundary, or whose timestamps
/// are shifted: it cannot be accepted, and its record is retained.
pub(crate) const RESOLVE_ESTIMATION_REFUSAL: &str = "Resolve the estimation grid, search boundary and nominal timestamp assumptions before accepting a model. The estimation record is retained.";

/// The refusal of an estimation fit with a residual beyond the measurement bound (N2.4).
pub(crate) const BEST_FIT_BOUND_REFUSAL: &str = "The best estimation fit exceeds the measurement bound; retain the consistent set and refine the analysis before acceptance.";

/// The refusal of a validation step whose applied amplitude equals the estimation step's (N3.4).
pub(crate) const DISTINCT_VALIDATION_STEP_REFUSAL: &str =
    "Use a distinct validation step with a different applied amplitude.";

/// The reason a validation recording gives when its frozen model fails the measurement bound (N3.5).
pub(crate) const FROZEN_MODEL_BOUND_REFUSAL: &str = "The frozen estimation model fails the independent measurement bound. Retain both recordings and revise the model or stated assumptions.";

/// The estimation stage of a bounded step review: the estimation recording and its qualification, its step
/// analysis, the model the analysis selects, and the source of the evidence.
struct Estimation {
    /// The estimation samples, copied from the review input.
    samples: Vec<IdentificationSample>,
    /// The qualification of the estimation recording, with its step measurement.
    qualification: QualifiedIdentificationData,
    /// The step analysis of the estimation recording.
    analysis: StepResponseAnalysis,
    /// The first-order-with-delay model of the best grid fit.
    selected: IdentifiedModel,
    /// Whether the evidence comes from a simulated or a physical plant.
    source: ResultSource,
}

/// Reviews a held step with the bounded step method (`prepareBoundedStepReview`).
///
/// The estimation recording is fitted with [`analyze_step_response`] at the input's normalized error limit (0.15 when
/// it names none). The best grid fit becomes a first-order-with-delay model whose normalized error is the residual
/// root mean square over the output span of the fit record (N3.2), and whose uncertainty is the measurement-consistent
/// grid when that set holds a combination (N2.4). Without validation samples the review needs validation: it shows the
/// estimation fit from the start of the baseline window to the restoration, with the baseline mean as the prediction
/// before the step. With validation samples the frozen model is scored on the validation step from the start of its
/// baseline window to its restoration: it is simulated from the validation's steady baseline under the recorded plant
/// input, on the recorded sample times (N3.1), and each residual is compared with the same measurement bound (N3.4,
/// N3.5). A residual beyond the bound marks the validation recording unsuitable with its reason; the review still reports
/// the scored evidence.
///
/// # Errors
/// Refuses, in this order: a requested model other than FOPDT or any excluded interval; missing step-response
/// settings; the refusals of [`analyze_step_response`] on the estimation recording; a measured change of `10⁻⁹` or
/// less; a recording without fit evidence ([`model_fit_evidence`]); and, with validation samples, recording identities
/// that are missing or equal, an estimation with an empty consistent set, a search-boundary fit or a timestamp shift,
/// an estimation residual beyond the measurement bound, the refusals of [`measure_step_response`] on the validation
/// recording, a validation step of the same amplitude, a validation recording without fit evidence, and a validation
/// recording whose fit record is flat.
pub fn prepare_bounded_step_review(
    input: &IdentificationReviewInput,
) -> Result<IdentificationReview, Refusal> {
    if !matches!(input.requested_kind, IdentificationModelSelection::Fopdt)
        || !input.excluded.is_empty()
    {
        return Err(Refusal::new(
            "Bounded step fitting needs FOPDT and the complete recording.",
        ));
    }
    // The `TypeScript` reads the settings unchecked; its analysis then refuses the missing settings as invalid.
    let Some(settings) = input.step_response_settings.as_ref() else {
        return Err(Refusal::new(INVALID_SETTINGS));
    };
    let (estimated, fit) = estimate(input, settings)?;
    let Some(validation) = input.validation.clone() else {
        return Ok(IdentificationReview::ValidationRequired(ValidationReview {
            step_response: Some(estimated.analysis),
            estimation_qualification: estimated.qualification,
            preview: single_model_result(
                &estimated.selected,
                estimated.source,
                Partition::SingleRun,
            ),
            selected: estimated.selected,
            fit,
            estimation: estimated.samples,
        }));
    };
    validate(input, settings, estimated, validation)
}

/// The estimation stage: the analysis of the estimation step, the model it selects, the qualification of the
/// recording with its step measurement, and the estimation fit evidence.
fn estimate(
    input: &IdentificationReviewInput,
    settings: &StepResponseSettings,
) -> Result<(Estimation, ModelFitEvidence), Refusal> {
    let samples = input.estimation.clone();
    let maximum_normalized_error = input
        .maximum_normalized_error
        .unwrap_or(DEFAULT_MAXIMUM_NORMALIZED_ERROR);
    let analysis = analyze_step_response(&samples, settings, maximum_normalized_error)?;
    let rms = (analysis.best.squared_error / count_f64(analysis.points.len())).sqrt();
    let response = (analysis.final_window.mean - analysis.baseline.mean).abs();
    if response.is_nan() || response <= TOLERANCE {
        return Err(Refusal::new(
            "The measured step has no identifiable output response.",
        ));
    }
    // The normalized error divides by the span of the fit record (N3.2), not by the response size.
    let span = fit_record_span(&samples, settings)?;
    let selected = estimated_model(&analysis, rms / span);
    let source = match input.source {
        DataSource::Physical => ResultSource::FromPhysical,
        DataSource::Simulation => ResultSource::FromSimulation,
    };
    let mut qualification = qualify_identification_data(&samples, &[], false);
    qualification.step_response = Some(measure_step_response(&samples, settings)?);
    let fit = estimation_fit(&selected, &samples, &analysis, rms)?;
    let estimated = Estimation {
        samples,
        qualification,
        analysis,
        selected,
        source,
    };
    Ok((estimated, fit))
}

/// The validation stage: the frozen estimation model scored on a distinct validation step (N3.4, N3.5).
fn validate(
    input: &IdentificationReviewInput,
    settings: &StepResponseSettings,
    estimated: Estimation,
    validation: Vec<IdentificationSample>,
) -> Result<IdentificationReview, Refusal> {
    let Estimation {
        samples,
        qualification,
        analysis,
        mut selected,
        source,
    } = estimated;
    if !distinct_recordings(input) {
        return Err(Refusal::new(DISTINCT_IDENTITIES_REFUSAL));
    }
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
    let validation_step = measure_step_response(&validation, settings)?;
    if (validation_step.input_change - analysis.input_change).abs() <= TOLERANCE {
        return Err(Refusal::new(DISTINCT_VALIDATION_STEP_REFUSAL));
    }
    let mut fit = score_frozen_step(&selected, &validation, &validation_step)?;
    let mut validation_qualification = qualify_identification_data(&validation, &[], true);
    validation_qualification.step_response = Some(validation_step.clone());
    if exceeds_bound(fit.points.iter().map(|point| point.residual), bound) {
        validation_qualification.suitable = false;
        validation_qualification.suitable_for_design = false;
        validation_qualification.reason = Some(FROZEN_MODEL_BOUND_REFUSAL.to_string());
    }
    // The validation's normalized error divides by the span of its own fit record, as the estimation's does (N3.2).
    let validation_span = fit_record_span(&validation, settings)?;
    selected.normalized_validation_error = Some(fit.root_mean_square_error / validation_span);
    selected.validation_residuals = Some(fit.points.iter().map(|point| point.residual).collect());
    fit.step_response = Some(analysis.clone());
    fit.step_response_measurement = Some(validation_step);
    fit.warnings.clone_from(&analysis.warnings);
    Ok(IdentificationReview::AcceptanceRequired(AcceptanceReview {
        step_response: Some(analysis),
        estimation_qualification: qualification,
        validation_qualification,
        result: single_model_result(&selected, source, Partition::IndependentRuns),
        selected,
        fit,
        estimation: samples,
        validation,
    }))
}

/// The first-order-with-delay model of the best grid fit: its gain, time constant and dead time, the normalized error,
/// the estimation residuals, the measurement-consistent grid as its uncertainty, and the search-boundary finding of the
/// analysis (N2.4).
fn estimated_model(analysis: &StepResponseAnalysis, normalized_error: f64) -> IdentifiedModel {
    IdentifiedModel {
        kind: LowOrderModelKind::FirstOrderDeadTime,
        gain: analysis.best.gain,
        time_constant_seconds: Some(analysis.best.time_constant_seconds),
        second_time_constant_seconds: None,
        dead_time_seconds: Some(analysis.best.delay_seconds),
        normalized_error: Some(normalized_error),
        normalized_estimation_error: Some(normalized_error),
        normalized_validation_error: None,
        estimation_residuals: Some(analysis.points.iter().map(|point| point.residual).collect()),
        validation_residuals: None,
        uncertainty: consistent_uncertainty(&analysis.consistency),
        stable: true,
        on_search_boundary: analysis.boundary_limited,
    }
}

/// The measurement-consistent grid as a model's uncertainty (N2.4): its gain, time-constant and delay ranges, present
/// only when the set holds a combination.
fn consistent_uncertainty(consistency: &ConsistentSet) -> Option<IdentifiedUncertainty> {
    let gain = consistency.gain.filter(|_| consistency.count > 0)?;
    Some(IdentifiedUncertainty {
        method: UncertaintyMethod::MeasurementConsistentGrid,
        gain,
        time_constant_seconds: consistency.time_constant_seconds,
        second_time_constant_seconds: None,
        dead_time_seconds: consistency.delay_seconds,
    })
}

/// The result of one bounded step model: the model is both the recommended model and the only one.
fn single_model_result(
    selected: &IdentifiedModel,
    source: ResultSource,
    partition: Partition,
) -> IdentificationResult {
    IdentificationResult {
        source,
        recommended: selected.clone(),
        models: vec![selected.clone()],
        partition: Some(partition),
        refusal: None,
        integrator_refusal: None,
    }
}

/// The fit evidence of the estimation recording from the start of the baseline window to the restoration: the
/// baseline mean as the prediction before the step, then the analysis's fitted points, with the residual root mean
/// square of the fit and the analysis's warnings.
fn estimation_fit(
    selected: &IdentifiedModel,
    estimation: &[IdentificationSample],
    analysis: &StepResponseAnalysis,
    root_mean_square_error: f64,
) -> Result<ModelFitEvidence, Refusal> {
    let window = analysis.baseline.start_seconds..analysis.hold_end_seconds;
    let measured: Vec<IdentificationSample> = estimation
        .iter()
        .filter(|sample| window.contains(&sample.time_seconds))
        .cloned()
        .collect();
    let mut fit = model_fit_evidence(selected, &measured)?;
    let baseline = analysis.baseline.mean;
    let before_step = measured
        .iter()
        .filter(|sample| sample.time_seconds < analysis.step_time_seconds)
        .map(|sample| ModelFitPoint {
            time_seconds: sample.time_seconds,
            measured: sample.output,
            predicted: baseline,
            residual: sample.output - baseline,
        });
    fit.points = before_step
        .chain(analysis.points.iter().map(fit_point))
        .collect();
    fit.root_mean_square_error = root_mean_square_error;
    fit.step_response = Some(analysis.clone());
    fit.warnings.clone_from(&analysis.warnings);
    Ok(fit)
}

/// A fitted point of the step analysis as a point of the fit evidence.
fn fit_point(point: &StepResponsePoint) -> ModelFitPoint {
    ModelFitPoint {
        time_seconds: point.time_seconds,
        measured: point.measured,
        predicted: point.predicted,
        residual: point.residual,
    }
}

/// The frozen model scored on a measured step (`scoreFrozenStep`; N3.1, N3.2): the fit evidence of the samples from the
/// start of the baseline window to the restoration. The frozen model is simulated from the validation's steady baseline
/// under the recorded plant input on the recorded sample times (N1.2), and the residuals and their root mean square are
/// taken over the response, from the step on.
pub(crate) fn score_frozen_step(
    model: &IdentifiedModel,
    samples: &[IdentificationSample],
    measured: &StepResponseMeasurement,
) -> Result<ModelFitEvidence, Refusal> {
    let window = measured.baseline.start_seconds..measured.hold_end_seconds;
    let held: Vec<IdentificationSample> = samples
        .iter()
        .filter(|sample| window.contains(&sample.time_seconds))
        .cloned()
        .collect();
    let mut evidence = model_fit_evidence(model, &held)?;
    evidence
        .points
        .retain(|point| point.time_seconds >= measured.step_time_seconds);
    let squares: f64 = evidence
        .points
        .iter()
        .map(|point| point.residual * point.residual)
        .sum();
    evidence.root_mean_square_error = (squares / count_f64(evidence.points.len())).sqrt();
    Ok(evidence)
}

/// Whether the review input names two distinct, non-empty recording identities (N3.4).
pub(crate) fn distinct_recordings(input: &IdentificationReviewInput) -> bool {
    let (Some(estimation), Some(validation)) = (
        input.estimation_recording_id.as_deref(),
        input.validation_recording_id.as_deref(),
    ) else {
        return false;
    };
    !estimation.is_empty() && !validation.is_empty() && estimation != validation
}

/// Whether a residual lies beyond the measurement bound by more than the tolerance (N2.4).
pub(crate) fn exceeds_bound(residuals: impl IntoIterator<Item = f64>, bound: f64) -> bool {
    residuals
        .into_iter()
        .any(|residual| residual.abs() > bound + TOLERANCE)
}

/// A count as a float. The counts here are sample counts, far below 2^53.
#[allow(clippy::cast_precision_loss)]
fn count_f64(count: usize) -> f64 {
    count as f64
}

#[cfg(test)]
#[path = "bounded_step_review_tests.rs"]
mod tests;
