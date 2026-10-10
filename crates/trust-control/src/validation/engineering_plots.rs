//! The engineering plot data of a model-predicted candidate (`buildEngineeringPlotEvidence` of
//! `engineeringPlots.ts`; B9.6 and N8 of the Control Studio specification). The data is computed once here, and
//! every page draws it: the response series by role and unit, the evaluation windows and bands of the step
//! (N8.3), the Bode, Nyquist and sensitivity data of the open loop, the root locus of a named gain sweep (N7.5),
//! the uncertainty envelope of the model's uncertainty, the margins with their crossovers, and the comparison
//! context that says which model, baseline and conditions the evidence belongs to (B9.6).
//!
//! The curves are read on the grid of the candidate's loop, so the margins and the curves agree. Phase is wrapped
//! on the plotted curves (N4.3); the margins are read on the unwrapped loop by the library. Only the drawing is in
//! TypeScript.

#![allow(clippy::module_name_repetitions)] // `EngineeringPlots` names the plots of this module; the precedent is `model.rs`.

use schemars::JsonSchema;
use serde::Serialize;
use trust_control_math::frequency::{phase_degrees, sensitivity, FrequencyModel};
use trust_control_math::identify::ParameterInterval;
use trust_control_math::linalg::ComplexValue;
use trust_control_math::model::TransferFunction;
use trust_control_math::polynomial::{add_aligned, convolve, roots};

use crate::identification::plant::{IdentifiedModel, UncertaintyMethod};
use crate::identification::refusal::Refusal;
use crate::limits::FIT_POINT_LIMIT;
use crate::tuning::candidate_margins::{
    controller_transfer, loop_margins, loop_value, magnitude, pid_loop, plant_transfer,
    CandidateMargins,
};
use crate::tuning::prediction::{
    predict_response, sampled_plant, simulate_loop, PredictedGains, PredictionConditions,
    ResponsePoint,
};
use crate::tuning::requirements::response_deadline_seconds;

/// The most uncertainty corners the envelope evaluates (`uncertaintyCornerModels`).
const MAXIMUM_CORNERS: usize = 16;
/// The most values a root-locus sweep may hold (`engineeringPlots.ts`).
const MAXIMUM_SWEEP_VALUES: usize = 1001;
/// The method label of a model that has an uncertainty but no method: never shown for the models here, which always
/// name their method, and kept for the words the legend shows when a method is missing.
const DECLARED_MODEL_SET: &str = "declared model set";
/// The label of an envelope that is not evaluated, because the model declares no uncertainty.
const NOT_EVALUATED: &str = "Not evaluated";
/// The method that the comparison context names (N8.6).
const COMPARISON_METHOD: &str = "unit-step-model-prediction-v1";
/// The sentence for a root-locus sweep that is not usable.
const SWEEP_REFUSED: &str =
    "Choose one gain, 1–1001 finite values, and a selected value in that sweep.";
/// The sentence for a root locus whose roots cannot be computed.
const ROOT_LOCUS_FAILED: &str = "The root locus could not be qualified.";
/// The approximation of a root locus, which states what the ideal controller leaves out.
const LOCUS_APPROXIMATION: &str = "Continuous ideal controller; sample/hold, derivative filtering and output constraints are not represented.";
/// The sentence for a sensitivity that is singular at a plotted frequency.
const SINGULAR_PLOT: &str = "The loop is singular at a plotted frequency, where 1 + L = 0.";

/// The magnitude of a value that is zero is shown as the smallest positive number, not as minus infinity (the
/// `Number.MIN_VALUE` of the TypeScript): the smallest positive subnormal.
const SMALLEST_MAGNITUDE: f64 = f64::from_bits(1);

/// The units of the response series (B9.6): the setpoint and the process value share one unit, and the
/// manipulated variable has its own.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SeriesUnits {
    /// The unit of the setpoint and of the process value.
    pub process_value: String,
    /// The unit of the manipulated variable.
    pub manipulated: String,
}

/// The requirement values of one setpoint step that the plots show as windows and bands (N8.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepRequirement {
    /// The allowed steady-state error, `maximumError`.
    pub maximum_error: f64,
    /// The settling band around the final value, `settlingBand`.
    pub settling_band: f64,
    /// The settling time, which sets the response deadline.
    pub settling_time_seconds: f64,
}

/// A closed interval of values on a plot.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
pub struct Band {
    /// The lowest value of the band.
    pub lower: f64,
    /// The highest value of the band.
    pub upper: f64,
}

/// The evaluation windows and bands of a setpoint step (N8.3): the run's window, the response deadline on the
/// sample grid, and the two bands around the target.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationMarks {
    /// The start of the run's window in seconds.
    pub window_start_seconds: f64,
    /// The end of the run's window in seconds: the horizon.
    pub window_end_seconds: f64,
    /// The target of the step: the operating output plus the setpoint change.
    pub target: f64,
    /// The settling deadline, rounded up to a whole sample period (N8.3).
    pub deadline_seconds: f64,
    /// The settling band around the target: the final value must stay inside it after the deadline.
    pub settling_band: Band,
    /// The allowed steady-state error around the target.
    pub error_band: Band,
}

/// One point of a frequency curve: its frequency, its magnitude in dB and its wrapped phase in degrees (N4.3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FrequencyPoint {
    /// The frequency in rad/s.
    pub frequency_rad_per_second: f64,
    /// The magnitude in dB.
    pub magnitude_db: f64,
    /// The phase in degrees, wrapped to the principal value.
    pub phase_degrees: f64,
}

/// One point of the Nyquist curve of the open loop.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NyquistPoint {
    /// The frequency in rad/s; the mirrored half has the frequency of its positive twin.
    pub frequency_rad_per_second: f64,
    /// The real part of the open loop.
    pub real: f64,
    /// The imaginary part of the open loop.
    pub imaginary: f64,
}

/// The sensitivity functions of the open loop at one frequency (N7.7), in dB.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SensitivityPoint {
    /// The frequency in rad/s.
    pub frequency_rad_per_second: f64,
    /// `|S| = |1/(1 + L)|`.
    pub sensitivity_magnitude_db: f64,
    /// `|T| = |L/(1 + L)|`.
    pub complementary_sensitivity_magnitude_db: f64,
    /// `|C·S|`, the controller's effort per unit of disturbance, with the measurement filter.
    pub control_sensitivity_magnitude_db: f64,
}

/// The parameter a root locus sweeps (N7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum RootLocusParameter {
    /// The proportional gain.
    ProportionalGain,
    /// The integral gain.
    IntegralGain,
    /// The derivative gain.
    DerivativeGain,
}

/// A named gain sweep of a root locus: the parameter, its values, and the value the candidate uses.
#[derive(Debug, Clone, PartialEq)]
pub struct RootLocusSweep {
    /// The swept parameter.
    pub parameter: RootLocusParameter,
    /// The values of the sweep, at most 1001, all finite.
    pub values: Vec<f64>,
    /// The value that the candidate uses; it must be one of the values.
    pub selected_value: f64,
}

/// The gains that a root locus keeps fixed.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FixedGains {
    /// The proportional gain, when it is fixed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proportional_gain: Option<f64>,
    /// The integral gain, when it is fixed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integral_gain: Option<f64>,
    /// The derivative gain, when it is fixed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivative_gain: Option<f64>,
}

/// The closed-loop poles at one value of the swept gain.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RootLocusBranch {
    /// The value of the swept gain.
    pub gain: f64,
    /// The roots of the characteristic polynomial at that gain.
    pub poles: Vec<ComplexValue>,
}

/// The root locus of a named gain sweep (N7.5), or the reason it cannot be qualified.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum RootLocus {
    /// The sweep was qualified: its branches and what it keeps fixed.
    #[serde(rename_all = "camelCase")]
    Qualified {
        /// The swept parameter.
        parameter: RootLocusParameter,
        /// The value the candidate uses.
        selected_value: f64,
        /// The gains that the sweep keeps fixed.
        fixed_parameters: FixedGains,
        /// What the ideal controller leaves out of the approximation.
        approximation: String,
        /// The poles at each value of the sweep.
        branches: Vec<RootLocusBranch>,
    },
    /// The sweep cannot be qualified, for the reason given.
    #[serde(rename_all = "camelCase")]
    Unavailable {
        /// The sentence that says why.
        reason: String,
    },
}

/// The status of an uncertainty envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UncertaintyStatus {
    /// The envelope was computed from the uncertainty corners of the model.
    Evaluated,
    /// The model declares no uncertainty, so there is no envelope.
    NotEvaluated,
}

/// One point of the uncertainty envelope: the nominal output and the lowest and highest corner outputs at the time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopePoint {
    /// The time of the point in seconds.
    pub time_seconds: f64,
    /// The lowest output among the nominal and the corners.
    pub lower: f64,
    /// The nominal output of the candidate.
    pub nominal: f64,
    /// The highest output among the nominal and the corners.
    pub upper: f64,
}

/// The uncertainty envelope of the candidate's response (N2.4): the nominal response and the outputs of the
/// model's uncertainty corners, with the words that name how the intervals were obtained.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UncertaintyEnvelope {
    /// Whether the envelope was evaluated.
    pub status: UncertaintyStatus,
    /// The words that the legend shows for the method (V-17).
    pub method: String,
    /// The envelope at the points of the response.
    pub points: Vec<EnvelopePoint>,
}

/// The context that a comparison's evidence belongs to (B9.6): the model it was computed on, the baseline and the
/// conditions, and the method. Two evidences may be combined only when these agree.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonContext {
    /// The model that the evidence was computed on.
    pub model_revision: String,
    /// The baseline settings and the conditions of the evidence.
    pub evidence_revision: String,
    /// The method of the evidence.
    pub method: String,
}

/// The candidate that the plots compare, with the context of the comparison.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// The candidate's settings.
    pub candidate: PredictedGains,
    /// The context that the candidate's evidence belongs to.
    pub context: ComparisonContext,
}

/// The class of evidence that the plots carry (N8.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
pub enum PlotEvidence {
    /// The response and the curves come from the model, not from a run on the plant.
    #[serde(rename = "Model-predicted")]
    ModelPredicted,
}

/// The model-predicted plot data of one candidate (`EngineeringPlotEvidence`).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EngineeringPlots {
    /// The class of the evidence.
    pub evidence: PlotEvidence,
    /// The units of the response series.
    pub units: SeriesUnits,
    /// The response series by role, at most `FIT_POINT_LIMIT` points, the first and the last included.
    pub response: Vec<ResponsePoint>,
    /// The evaluation windows and bands of the step, when the requirement is given.
    pub evaluation: Option<EvaluationMarks>,
    /// The plant's frequency response on the grid.
    pub plant_bode: Vec<FrequencyPoint>,
    /// The open loop's frequency response on the grid; the Nichols curve reads these points.
    pub open_loop_bode: Vec<FrequencyPoint>,
    /// The Nyquist curve of the open loop, mirrored below the real axis.
    pub nyquist: Vec<NyquistPoint>,
    /// The sensitivity functions on the grid.
    pub sensitivity: Vec<SensitivityPoint>,
    /// The margins and the peak sensitivity of the candidate.
    pub margins: CandidateMargins,
    /// The gain crossover, where `|L| = 1`, when the loop has one in the grid.
    pub gain_crossover: Option<FrequencyPoint>,
    /// The phase crossover, where the phase is −180°, when the grid shows one.
    pub phase_crossover: Option<FrequencyPoint>,
    /// The root locus of the named sweep.
    pub root_locus: RootLocus,
    /// The uncertainty envelope of the response.
    pub uncertainty: UncertaintyEnvelope,
    /// The candidate and the context of its comparison.
    pub comparison: Comparison,
}

/// The inputs of one set of plots: the selected model, the candidate and the current settings, the conditions, the
/// units, and the optional requirement windows and root-locus sweep.
#[derive(Debug, Clone, Copy)]
pub struct PlotInputs<'a> {
    /// The selected model that the predictions run on.
    pub model: &'a IdentifiedModel,
    /// The settings in service, when there are some: their response is the baseline.
    pub current: Option<&'a PredictedGains>,
    /// The candidate whose plots these are.
    pub candidate: &'a PredictedGains,
    /// The conditions of the response and the margins.
    pub conditions: &'a PredictionConditions,
    /// The units of the response series.
    pub units: &'a SeriesUnits,
    /// The requirements of the step, for its windows and bands, when they are known.
    pub requirement: Option<&'a StepRequirement>,
    /// The root-locus sweep; the candidate's proportional gain is swept when there is none.
    pub sweep: Option<&'a RootLocusSweep>,
}

/// The model-predicted plots of a candidate on the conditions (B9.6): one projection of the response and of the
/// curves, for every page.
///
/// # Errors
/// Returns the sentence of the plant, of the conditions, of the candidate's settings, of the margins, or of the
/// sensitivity at a plotted frequency.
pub fn build_engineering_plots(inputs: &PlotInputs<'_>) -> Result<EngineeringPlots, Refusal> {
    let conditions = inputs.conditions;
    let response = predict_response(inputs.model, inputs.candidate, inputs.current, conditions)?;
    let steps = response.points.len().saturating_sub(1);
    let response_points = decimate(&response.points, steps);
    let loop_ = pid_loop(inputs.model, inputs.candidate)?;
    let margins = loop_margins(&loop_.open_loop, &loop_.frequencies, loop_.far_frequency)?;
    let uncertainty =
        uncertainty_envelope(inputs.model, inputs.candidate, conditions, &response_points)?;
    Ok(EngineeringPlots {
        evidence: PlotEvidence::ModelPredicted,
        units: inputs.units.clone(),
        response: response_points,
        evaluation: inputs
            .requirement
            .map(|requirement| evaluation_marks(requirement, conditions)),
        plant_bode: frequency_points(&loop_.plant, &loop_.frequencies)?,
        open_loop_bode: frequency_points(&loop_.open_loop, &loop_.frequencies)?,
        nyquist: nyquist_points(&loop_.open_loop, &loop_.frequencies)?,
        sensitivity: sensitivity_points(&loop_.open_loop, &loop_.controller, &loop_.frequencies)?,
        gain_crossover: crossover_point(&loop_.open_loop, margins.gain_crossover_rad_per_second)?,
        phase_crossover: crossover_point(&loop_.open_loop, margins.phase_crossover_rad_per_second)?,
        margins,
        root_locus: root_locus(inputs.model, inputs.candidate, inputs.sweep),
        uncertainty,
        comparison: comparison(inputs),
    })
}

/// The points of the response that the plots keep: at most `FIT_POINT_LIMIT` evenly spaced periods, always with
/// the first sample and the last one (A4.5 and `engineeringPlots.ts`).
fn decimate(points: &[ResponsePoint], steps: usize) -> Vec<ResponsePoint> {
    let limit = usize::try_from(FIT_POINT_LIMIT).unwrap_or(usize::MAX);
    let stride = steps.div_ceil(limit).max(1);
    points
        .iter()
        .enumerate()
        .filter(|(index, _)| *index == 0 || *index % stride == 0 || *index == steps)
        .map(|(_, point)| point.clone())
        .collect()
}

/// The evaluation windows and bands of a step (N8.3): the window of the run from zero to the horizon, the settling
/// deadline on the sample grid, and the bands around the target.
fn evaluation_marks(
    requirement: &StepRequirement,
    conditions: &PredictionConditions,
) -> EvaluationMarks {
    let target = conditions.operating_output + conditions.reference_change;
    EvaluationMarks {
        window_start_seconds: 0.0,
        window_end_seconds: conditions.duration_seconds,
        target,
        deadline_seconds: response_deadline_seconds(
            requirement.settling_time_seconds,
            conditions.period_seconds * 1000.0,
        ),
        settling_band: Band {
            lower: target - requirement.settling_band,
            upper: target + requirement.settling_band,
        },
        error_band: Band {
            lower: target - requirement.maximum_error,
            upper: target + requirement.maximum_error,
        },
    }
}

/// The magnitude in dB of a value, with a zero shown as the smallest positive magnitude.
fn magnitude_db(value: ComplexValue) -> f64 {
    20.0 * magnitude(value).max(SMALLEST_MAGNITUDE).log10()
}

/// The magnitude and the wrapped phase of a model at each frequency (N4.3).
fn frequency_points(
    model: &FrequencyModel,
    frequencies: &[f64],
) -> Result<Vec<FrequencyPoint>, Refusal> {
    frequencies
        .iter()
        .map(|&frequency| -> Result<FrequencyPoint, Refusal> {
            let value = loop_value(model, frequency)?;
            Ok(FrequencyPoint {
                frequency_rad_per_second: frequency,
                magnitude_db: magnitude_db(value),
                phase_degrees: phase_degrees(value),
            })
        })
        .collect()
}

/// The Nyquist curve of the open loop: the positive frequencies, then their mirror image below the real axis.
fn nyquist_points(
    open_loop: &FrequencyModel,
    frequencies: &[f64],
) -> Result<Vec<NyquistPoint>, Refusal> {
    let positive = frequencies
        .iter()
        .map(|&frequency| -> Result<NyquistPoint, Refusal> {
            let value = loop_value(open_loop, frequency)?;
            Ok(NyquistPoint {
                frequency_rad_per_second: frequency,
                real: value.real,
                imaginary: value.imaginary,
            })
        })
        .collect::<Result<Vec<NyquistPoint>, Refusal>>()?;
    let mirrored = positive.iter().rev().map(|point| NyquistPoint {
        imaginary: -point.imaginary,
        ..*point
    });
    Ok(mirrored.chain(positive.iter().skip(1).copied()).collect())
}

/// The sensitivity functions of the open loop on the grid (N7.7): `S = 1/(1 + L)`, `T = L·S`, and `C·S`, where `C`
/// is the controller with its measurement filter.
fn sensitivity_points(
    open_loop: &FrequencyModel,
    controller: &FrequencyModel,
    frequencies: &[f64],
) -> Result<Vec<SensitivityPoint>, Refusal> {
    frequencies
        .iter()
        .map(|&frequency| -> Result<SensitivityPoint, Refusal> {
            let open_loop_value = loop_value(open_loop, frequency)?;
            let sensitivity_value =
                sensitivity(open_loop_value).map_err(|_| Refusal::new(SINGULAR_PLOT))?;
            let complementary = multiply(open_loop_value, sensitivity_value);
            let control = multiply(loop_value(controller, frequency)?, sensitivity_value);
            Ok(SensitivityPoint {
                frequency_rad_per_second: frequency,
                sensitivity_magnitude_db: magnitude_db(sensitivity_value),
                complementary_sensitivity_magnitude_db: magnitude_db(complementary),
                control_sensitivity_magnitude_db: magnitude_db(control),
            })
        })
        .collect()
}

/// The open loop's point at a crossover frequency: its magnitude (0 dB at the gain crossover) and its phase.
fn crossover_point(
    open_loop: &FrequencyModel,
    frequency: Option<f64>,
) -> Result<Option<FrequencyPoint>, Refusal> {
    let Some(frequency) = frequency else {
        return Ok(None);
    };
    let value = loop_value(open_loop, frequency)?;
    Ok(Some(FrequencyPoint {
        frequency_rad_per_second: frequency,
        magnitude_db: magnitude_db(value),
        phase_degrees: phase_degrees(value),
    }))
}

/// The product of two complex values.
fn multiply(left: ComplexValue, right: ComplexValue) -> ComplexValue {
    ComplexValue {
        real: left.real * right.real - left.imaginary * right.imaginary,
        imaginary: left.real * right.imaginary + left.imaginary * right.real,
    }
}

/// The root locus of the sweep (N7.5): for each value of the swept gain, the roots of `D + g·N` with the other
/// gains fixed. Without a sweep, the proportional gain is swept over zero, the candidate's gain, and 61 magnitudes
/// from `10⁻³` to `10³`, with the sign of the gain.
fn root_locus(
    model: &IdentifiedModel,
    candidate: &PredictedGains,
    requested: Option<&RootLocusSweep>,
) -> RootLocus {
    let sweep = requested
        .cloned()
        .unwrap_or_else(|| default_sweep(candidate));
    match sweep_branches(model, candidate, &sweep) {
        Ok(branches) => RootLocus::Qualified {
            parameter: sweep.parameter,
            selected_value: sweep.selected_value,
            fixed_parameters: fixed_gains(candidate, sweep.parameter),
            approximation: approximation(model),
            branches,
        },
        Err(reason) => RootLocus::Unavailable { reason },
    }
}

/// The default sweep of the proportional gain.
fn default_sweep(candidate: &PredictedGains) -> RootLocusSweep {
    let sign = if candidate.proportional_gain < 0.0 {
        -1.0
    } else {
        1.0
    };
    let mut values = vec![0.0, candidate.proportional_gain];
    values.extend((0..61_u32).map(|index| sign * 10f64.powf(-3.0 + f64::from(index) * 0.1)));
    values.sort_by(f64::total_cmp);
    values.dedup();
    RootLocusSweep {
        parameter: RootLocusParameter::ProportionalGain,
        values,
        selected_value: candidate.proportional_gain,
    }
}

/// The branches of a sweep: the roots of the characteristic polynomial at each value.
fn sweep_branches(
    model: &IdentifiedModel,
    candidate: &PredictedGains,
    sweep: &RootLocusSweep,
) -> Result<Vec<RootLocusBranch>, String> {
    let usable = !sweep.values.is_empty()
        && sweep.values.len() <= MAXIMUM_SWEEP_VALUES
        && sweep.values.iter().all(|value| value.is_finite())
        && sweep.values.contains(&sweep.selected_value);
    if !usable {
        return Err(SWEEP_REFUSED.to_string());
    }
    let plant = locus_plant(model);
    sweep
        .values
        .iter()
        .map(|&gain| -> Result<RootLocusBranch, String> {
            let mut gains = *candidate;
            gains.measurement_filter_seconds = 0.0;
            match sweep.parameter {
                RootLocusParameter::ProportionalGain => gains.proportional_gain = gain,
                RootLocusParameter::IntegralGain => gains.integral_gain = gain,
                RootLocusParameter::DerivativeGain => gains.derivative_gain = gain,
            }
            let controller = controller_transfer(&gains);
            let numerator = convolve(&plant.numerator, &controller.numerator);
            let denominator = convolve(&plant.denominator, &controller.denominator);
            let characteristic = add_aligned(&denominator, &numerator);
            let poles = roots(&characteristic).map_err(|_| ROOT_LOCUS_FAILED.to_string())?;
            Ok(RootLocusBranch { gain, poles })
        })
        .collect()
}

/// The plant of the root locus: the selected model's polynomial, with its delay replaced by a first-order Padé
/// approximation (`engineeringPlots.ts`).
fn locus_plant(model: &IdentifiedModel) -> TransferFunction {
    let plant = plant_transfer(model);
    let delay = model.dead_time_seconds.unwrap_or(0.0);
    if delay > 0.0 {
        TransferFunction {
            numerator: convolve(&plant.numerator, &[-delay / 2.0, 1.0]),
            denominator: convolve(&plant.denominator, &[delay / 2.0, 1.0]),
        }
    } else {
        plant
    }
}

/// The gains that a sweep keeps fixed: every gain of the candidate except the swept one.
fn fixed_gains(candidate: &PredictedGains, swept: RootLocusParameter) -> FixedGains {
    FixedGains {
        proportional_gain: (swept != RootLocusParameter::ProportionalGain)
            .then_some(candidate.proportional_gain),
        integral_gain: (swept != RootLocusParameter::IntegralGain)
            .then_some(candidate.integral_gain),
        derivative_gain: (swept != RootLocusParameter::DerivativeGain)
            .then_some(candidate.derivative_gain),
    }
}

/// What the root locus approximates: the ideal controller, and the delay's Padé term when the model has a delay.
fn approximation(model: &IdentifiedModel) -> String {
    let delay_note = if model.dead_time_seconds.unwrap_or(0.0) > 0.0 {
        " Delay uses first-order Padé approximation."
    } else {
        " No model delay."
    };
    format!("{LOCUS_APPROXIMATION}{delay_note}")
}

/// The uncertainty envelope of the response (N2.4): each uncertainty corner of the model is simulated with the same
/// candidate and conditions as the nominal response, and the envelope is the lowest and the highest output at each
/// point.
fn uncertainty_envelope(
    model: &IdentifiedModel,
    candidate: &PredictedGains,
    conditions: &PredictionConditions,
    nominal: &[ResponsePoint],
) -> Result<UncertaintyEnvelope, Refusal> {
    let corners = corner_models(model);
    if corners.is_empty() {
        return Ok(UncertaintyEnvelope {
            status: UncertaintyStatus::NotEvaluated,
            method: NOT_EVALUATED.to_string(),
            points: Vec::new(),
        });
    }
    let traces = corners
        .iter()
        .map(|corner| corner_trace(corner, candidate, conditions))
        .collect::<Result<Vec<Vec<f64>>, Refusal>>()?;
    let points = nominal
        .iter()
        .map(|point| {
            let step = period_index(point.time_seconds, conditions.period_seconds);
            let mut values = vec![point.candidate_process_value];
            values.extend(traces.iter().filter_map(|trace| trace.get(step).copied()));
            EnvelopePoint {
                time_seconds: point.time_seconds,
                lower: values.iter().copied().fold(f64::INFINITY, f64::min),
                nominal: point.candidate_process_value,
                upper: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            }
        })
        .collect();
    Ok(UncertaintyEnvelope {
        status: UncertaintyStatus::Evaluated,
        method: method_label(
            model
                .uncertainty
                .as_ref()
                .map(|uncertainty| uncertainty.method),
        )
        .to_string(),
        points,
    })
}

/// The process values of one corner from time zero: the operating output, then the output after each period.
fn corner_trace(
    corner: &IdentifiedModel,
    candidate: &PredictedGains,
    conditions: &PredictionConditions,
) -> Result<Vec<f64>, Refusal> {
    let plant = sampled_plant(corner, conditions)?;
    let samples = simulate_loop(&plant, candidate, conditions)?;
    let mut values = Vec::with_capacity(samples.len() + 1);
    values.push(conditions.operating_output);
    values.extend(samples.iter().map(|sample| sample.process_value));
    Ok(values)
}

/// The period index of a time: the time over the period, rounded to the nearest whole number.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // The times of the response are finite and not negative, and the index is used only to look a value up.
fn period_index(time_seconds: f64, period_seconds: f64) -> usize {
    (time_seconds / period_seconds).round() as usize
}

/// The uncertainty corners of a model: the models at the ends of each interval, combined, at most sixteen
/// (`uncertaintyCornerModels`). A model without uncertainty has none.
fn corner_models(model: &IdentifiedModel) -> Vec<IdentifiedModel> {
    let Some(uncertainty) = model.uncertainty.as_ref() else {
        return Vec::new();
    };
    let intervals: [(Option<ParameterInterval>, Setter); 4] = [
        (Some(uncertainty.gain), set_gain as Setter),
        (
            uncertainty.time_constant_seconds,
            set_time_constant as Setter,
        ),
        (
            uncertainty.second_time_constant_seconds,
            set_second_time_constant as Setter,
        ),
        (uncertainty.dead_time_seconds, set_dead_time as Setter),
    ];
    let mut variants = vec![model.clone()];
    for (interval, set) in intervals {
        let Some(interval) = interval else {
            continue;
        };
        variants = variants
            .iter()
            .flat_map(|variant| {
                [interval.minimum, interval.maximum]
                    .into_iter()
                    .map(move |value| {
                        let mut corner = variant.clone();
                        set(&mut corner, value);
                        corner
                    })
            })
            .collect();
    }
    variants.truncate(MAXIMUM_CORNERS);
    variants
}

/// Sets one parameter of an identified model.
type Setter = fn(&mut IdentifiedModel, f64);

/// Sets the gain of a corner model.
fn set_gain(corner: &mut IdentifiedModel, value: f64) {
    corner.gain = value;
}

/// Sets the time constant of a corner model.
fn set_time_constant(corner: &mut IdentifiedModel, value: f64) {
    corner.time_constant_seconds = Some(value);
}

/// Sets the second time constant of a corner model.
fn set_second_time_constant(corner: &mut IdentifiedModel, value: f64) {
    corner.second_time_constant_seconds = Some(value);
}

/// Sets the dead time of a corner model.
fn set_dead_time(corner: &mut IdentifiedModel, value: f64) {
    corner.dead_time_seconds = Some(value);
}

/// The words that the legend shows for an uncertainty method (V-17): no raw identifier reaches the plot.
#[must_use]
pub fn method_label(method: Option<UncertaintyMethod>) -> &'static str {
    match method {
        None => DECLARED_MODEL_SET,
        Some(UncertaintyMethod::SampleResolutionEnvelope) => {
            "sample-resolution envelope from the estimation record"
        }
        Some(UncertaintyMethod::GridResolutionAndValidationResidual) => {
            "grid-resolution and held-out-residual interval"
        }
        Some(UncertaintyMethod::MeasurementConsistentGrid) => {
            "box enclosing the measurement-consistent grid; parameter correlations omitted"
        }
    }
}

/// The comparison context of the plots: the model, the baseline and the conditions that the evidence belongs to.
fn comparison(inputs: &PlotInputs<'_>) -> Comparison {
    let key = EvidenceKey {
        baseline: inputs.current,
        conditions: inputs.conditions,
    };
    Comparison {
        candidate: *inputs.candidate,
        context: ComparisonContext {
            model_revision: serde_json::to_string(inputs.model).unwrap_or_default(),
            evidence_revision: serde_json::to_string(&key).unwrap_or_default(),
            method: COMPARISON_METHOD.to_string(),
        },
    }
}

/// What the evidence of a comparison depends on, besides the model.
#[derive(Serialize)]
struct EvidenceKey<'a> {
    baseline: Option<&'a PredictedGains>,
    conditions: &'a PredictionConditions,
}

#[cfg(test)]
#[path = "engineering_plots_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "engineering_plots_ported_tests.rs"]
mod ported_tests;

#[cfg(test)]
#[path = "engineering_plots_identify_tune_tests.rs"]
mod identify_tune_tests;
