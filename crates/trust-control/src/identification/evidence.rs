//! The fit evidence of an identified model (specification 35, B3.5 and B3.8; A10.3): the measured response beside the
//! model's prediction, the residuals and their root mean square, the operating range the samples cover, and the
//! warnings the engineer reads before the model is used (`modelFitEvidence` in `identification.ts`).

#![allow(clippy::module_name_repetitions)] // `ModelFitEvidence` is the name the `TypeScript` record has.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::identify::response::{reached_fraction, step_amplitude, LowOrderShape};
use trust_control_math::identify::simulate::unit_response;
use trust_control_math::identify::LowOrderModelKind;
use trust_control_math::sampling::statistics::mean;
use trust_control_math::sampling::{largest, smallest};

use crate::experiment::number_text::{js_number, js_round};
use crate::identification::data::{
    first_output_unit, held_step_samples, identification_operating_point, inputs_of,
    median_interval, outputs_of, IdentificationSample, OperatingPoint,
};
use crate::identification::plant::IdentifiedModel;
use crate::identification::refusal::Refusal;
use crate::identification::sensor_lag_comparison::SensorLagComparison;
use crate::identification::step_response::{StepResponseAnalysis, StepResponseMeasurement};

/// The normalized prediction error above which a model is flagged as too inaccurate for controller design.
const HIGH_PREDICTION_ERROR: f64 = 0.2;

/// The fitted final change at or below which the fraction reached is not defined.
const MINIMUM_FINAL_CHANGE: f64 = 1e-12;

/// The relative difference below which two consecutive inputs count as equal (`inputChangeCount`).
const INPUT_CHANGE_TOLERANCE: f64 = 1e-9;

/// One point of the fit evidence (`ModelFitPoint`): the time, the measured output, the model's prediction and the
/// residual `measured − predicted`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelFitPoint {
    /// The time of the sample, in s.
    pub time_seconds: f64,
    /// The measured output.
    pub measured: f64,
    /// The output the model predicts.
    pub predicted: f64,
    /// The measured output minus the prediction.
    pub residual: f64,
}

/// The range of the input and of the output that the fitted samples cover (`operatingRange`): the model is evidence
/// within this range only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OperatingRange {
    /// The smallest input.
    pub input_minimum: f64,
    /// The largest input.
    pub input_maximum: f64,
    /// The smallest output.
    pub output_minimum: f64,
    /// The largest output.
    pub output_maximum: f64,
}

/// The fit evidence of an identified model (B3.5, B3.8; `ModelFitEvidence`): the measured response with the
/// prediction, the residual root mean square, the delay, the operating range and the warnings. A bounded step review
/// adds its step-response analysis and the measurement of the plotted record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelFitEvidence {
    /// The measured and predicted output at every sample.
    pub points: Vec<ModelFitPoint>,
    /// The root mean square of the residuals.
    pub root_mean_square_error: f64,
    /// The first output unit a sample states, or `unit`.
    pub output_unit: String,
    /// Whether the model is stable.
    pub stable: bool,
    /// The fraction of its fitted final change a held step reached (A10.3); absent where it is not defined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_change_fraction: Option<f64>,
    /// The model's dead time, in s; zero without one.
    pub delay_seconds: f64,
    /// The range the samples cover.
    pub operating_range: OperatingRange,
    /// The warnings the engineer reads before using the model.
    pub warnings: Vec<String>,
    /// The step-response analysis of a bounded step review: the estimation grid the model was fitted on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_response: Option<StepResponseAnalysis>,
    /// The step-response measurement of the plotted record, when it is another record than the estimation's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_response_measurement: Option<StepResponseMeasurement>,
    /// The model-structure comparison the model was chosen by, when a held step was compared (`modelStructure`; N2.10).
    /// Boxed, because the comparison holds the fit evidence of its two candidates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_structure: Option<Box<SensorLagComparison>>,
}

/// The fitted response of a held step (A10.3, N2.3, N2.6; `heldStepEvidence`): its size, how long it was held, and
/// the fraction of its fitted final change it reached. The fit evidence reports the fraction; the physical refusals of
/// `plant` judge the size against the noise and the fraction against 63 %.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeldStepEvidence {
    /// The size of the fitted response: `|K·Δu|`, and `|K·Δu·H|` for an integrator held for `H`.
    pub(crate) response_size: f64,
    /// The time `H` from the step to the last held sample, in s.
    pub(crate) hold_seconds: f64,
    /// The fraction of its fitted final change the response reached; `None` for an integrator, and for a final change
    /// of `10⁻¹²` or less.
    pub(crate) fraction: Option<f64>,
}

/// The fit evidence of a model on the samples it is checked against (B3.5, B3.8; `modelFitEvidence`).
///
/// The prediction is the model's free-running response to the recorded input (N3.1): `y0 + K · sₖ`, where `sₖ` is the
/// unit-gain response to the input deviations `uₖ − u₀` on the recorded sample times, from the operating point (N1.2). The
/// residual root mean square is taken over every sample. The warnings come in this order: an unstable model, a normalized
/// error above 0.2, the fraction of its fitted final change a held step reached (A10.3), and a dead time shorter than the
/// median sample interval. The step-response analysis and measurement are left to the caller that made them.
///
/// # Errors
/// Refuses samples without a finite measured operating point ([`identification_operating_point`]).
pub fn model_fit_evidence(
    model: &IdentifiedModel,
    samples: &[IdentificationSample],
) -> Result<ModelFitEvidence, Refusal> {
    let final_change_fraction =
        held_step_evidence(model, samples).and_then(|evidence| evidence.fraction);
    fit_evidence(model, samples, final_change_fraction)
}

/// The fit evidence of a pulse that has returned to its baseline (N2.3): a pulse has no final change, so its fraction
/// is not defined, and neither a fraction nor a fraction warning is reported.
///
/// # Errors
/// Refuses as [`model_fit_evidence`] does.
pub(crate) fn pulse_fit_evidence(
    model: &IdentifiedModel,
    samples: &[IdentificationSample],
) -> Result<ModelFitEvidence, Refusal> {
    fit_evidence(model, samples, None)
}

/// The fit evidence of a model on the samples, with the fraction of its final change that the caller reads (N2.3).
fn fit_evidence(
    model: &IdentifiedModel,
    samples: &[IdentificationSample],
    final_change_fraction: Option<f64>,
) -> Result<ModelFitEvidence, Refusal> {
    let operating = identification_operating_point(samples)?;
    let points = predicted_points(model, samples, &operating);
    let squared_residuals: Vec<f64> = points
        .iter()
        .map(|point| point.residual * point.residual)
        .collect();
    let root_mean_square_error = mean(&squared_residuals).sqrt();
    Ok(ModelFitEvidence {
        points,
        root_mean_square_error,
        output_unit: first_output_unit(samples),
        stable: model.stable,
        final_change_fraction,
        delay_seconds: model.dead_time_seconds.unwrap_or(0.0),
        operating_range: operating_range(samples),
        warnings: fit_warnings(model, samples, final_change_fraction),
        step_response: None,
        step_response_measurement: None,
        model_structure: None,
    })
}

/// The measured and predicted output at every sample: `y0 + K·sₖ`, with `sₖ` the model's unit-gain response to the recorded
/// input deviations from the operating point (N3.1, N1.2).
fn predicted_points(
    model: &IdentifiedModel,
    samples: &[IdentificationSample],
    operating: &OperatingPoint,
) -> Vec<ModelFitPoint> {
    let times: Vec<f64> = samples.iter().map(|sample| sample.time_seconds).collect();
    let deviations: Vec<f64> = samples
        .iter()
        .map(|sample| sample.input - operating.input0)
        .collect();
    let shape = LowOrderShape {
        time_constant_seconds: model.time_constant_seconds,
        second_time_constant_seconds: model.second_time_constant_seconds,
        dead_time_seconds: model.dead_time_seconds,
    };
    let response = unit_response(model.kind, shape, &times, &deviations);
    samples
        .iter()
        .zip(response)
        .map(|(sample, unit)| {
            let predicted = operating.output0 + model.gain * unit;
            ModelFitPoint {
                time_seconds: sample.time_seconds,
                measured: sample.output,
                predicted,
                residual: sample.output - predicted,
            }
        })
        .collect()
}

/// The span of the output a model predicts on the samples, `y0 + K·sₖ` (N3.1): the largest less the smallest predicted
/// value. It is the response size that the noise check of N2.6 reads for a fit that is not a held step, as an
/// excitation fit reads it. `None` when the samples have no operating point (see [`identification_operating_point`]).
#[must_use]
pub(crate) fn predicted_span(
    model: &IdentifiedModel,
    samples: &[IdentificationSample],
) -> Option<f64> {
    let operating = identification_operating_point(samples).ok()?;
    let predicted: Vec<f64> = predicted_points(model, samples, &operating)
        .iter()
        .map(|point| point.predicted)
        .collect();
    Some(largest(&predicted, f64::NEG_INFINITY) - smallest(&predicted))
}

/// The fitted response of a held step on a record (A10.3, N2.3; `heldStepEvidence`), read over the held part of the
/// record (B3.4 step 5). The final change is `K·Δu`, with `Δu = ū_hold − u₀` the mean held input less the baseline mean
/// (N2.2); the fraction reached is `(ȳ_end − y0) / (K·Δu)`, with the library's tail mean `ȳ_end` (N2.3). `None` for a
/// record that is not a single held step (see [`is_single_held_step`]), where neither the size nor the fraction is
/// defined, and for a held part without a finite measured step, where the `TypeScript` throws the operating point's
/// refusal: the held part of a record that has an operating point always has one.
pub(crate) fn held_step_evidence(
    model: &IdentifiedModel,
    samples: &[IdentificationSample],
) -> Option<HeldStepEvidence> {
    if !is_single_held_step(samples) {
        return None;
    }
    let held = held_step_samples(samples);
    let operating = identification_operating_point(&held).ok()?;
    let step = &held[operating.step_index];
    let hold_seconds = held.last()?.time_seconds - step.time_seconds;
    let step_input = step_amplitude(operating.input0, &inputs_of(&held[operating.step_index..]));
    let final_change = model.gain * step_input;
    if model.kind == LowOrderModelKind::Integrating {
        return Some(HeldStepEvidence {
            response_size: (final_change * hold_seconds).abs(),
            hold_seconds,
            fraction: None,
        });
    }
    let fraction = (final_change.abs() > MINIMUM_FINAL_CHANGE).then(|| {
        reached_fraction(
            &outputs_of(&held[operating.step_index..]),
            operating.output0,
            final_change,
        )
    });
    Some(HeldStepEvidence {
        response_size: final_change.abs(),
        hold_seconds,
        fraction,
    })
}

/// The dominant time constant of a fitted model (N2.3, A10.3): the largest of its time constants, which is the largest
/// `1/|Re p|` over its poles. A model without a time constant reads it as one, as the first-order forms do.
#[must_use]
pub(crate) fn dominant_time_constant(model: &IdentifiedModel) -> f64 {
    let first = model.time_constant_seconds.unwrap_or(1.0);
    model
        .second_time_constant_seconds
        .map_or(first, |second| first.max(second))
}

/// Whether a record is a single held step (N2.3): its held part, from the step to the restoration (`held_step_samples`),
/// moves in one direction only, so a ramp that reaches its final value over several samples (an actuator rate limit)
/// is a held step, and the whole record reverses direction at most once, the restoration. A PRBS, a multisine or a relay
/// reverses many times and is not one. A record that has returned to its baseline reverses once, so the rule reads it
/// as the held step B3.4 defines; an experiment that is not a held step must be identified as such by its caller.
fn is_single_held_step(samples: &[IdentificationSample]) -> bool {
    // The signs are `±1`, so two adjacent changes reverse direction exactly when their product is negative.
    let reversals = change_signs(samples)
        .windows(2)
        .filter(|pair| pair[0] * pair[1] < 0.0)
        .count();
    if reversals > 1 {
        return false;
    }
    change_signs(&held_step_samples(samples))
        .windows(2)
        .all(|pair| pair[0] * pair[1] > 0.0)
}

/// The direction of each input change above the tolerance of `10⁻⁹` of the input's magnitude (or of one for a smaller
/// magnitude), in time order: `+1` for a rise and `−1` for a fall.
fn change_signs(samples: &[IdentificationSample]) -> Vec<f64> {
    samples
        .windows(2)
        .filter_map(|pair| {
            let input = pair[1].input;
            let change = input - pair[0].input;
            (change.abs() > INPUT_CHANGE_TOLERANCE * input.abs().max(1.0)).then(|| change.signum())
        })
        .collect()
}

/// The warnings of the fit evidence, in the order the `TypeScript` gives them.
fn fit_warnings(
    model: &IdentifiedModel,
    samples: &[IdentificationSample],
    final_change_fraction: Option<f64>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    if !model.stable {
        warnings.push("The fitted model is not stable.".to_string());
    }
    if model
        .normalized_error
        .is_some_and(|error| error > HIGH_PREDICTION_ERROR)
    {
        warnings.push("Prediction error is high for controller design.".to_string());
    }
    if let Some(fraction) = final_change_fraction {
        let percent = js_number(js_round(fraction * 100.0));
        warnings.push(format!(
            "The response reached {percent} % of its fitted final change."
        ));
    }
    let dead_time = model.dead_time_seconds.unwrap_or(0.0);
    if dead_time > 0.0 && dead_time < median_interval(samples) {
        warnings.push(
            "Dead time is shorter than one sample interval and is weakly resolved.".to_string(),
        );
    }
    warnings
}

/// The range of the input and of the output over the samples.
fn operating_range(samples: &[IdentificationSample]) -> OperatingRange {
    let inputs = inputs_of(samples);
    let outputs = outputs_of(samples);
    OperatingRange {
        input_minimum: smallest(&inputs),
        input_maximum: largest(&inputs, f64::NEG_INFINITY),
        output_minimum: smallest(&outputs),
        output_maximum: largest(&outputs, f64::NEG_INFINITY),
    }
}

#[cfg(test)]
#[path = "evidence_tests.rs"]
pub(crate) mod tests;
