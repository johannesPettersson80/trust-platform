//! General feedback analysis (N4.4, N7.7 to N7.12): the open loop, the sensitivities, the closed-loop poles,
//! the Nyquist count and the classical margins of a selected-channel negative feedback loop. The loop closes
//! either with a second supplied model's channel, or with one declared controller law (a rational function of
//! its parameters, evaluated at the selected value). The mathematics is in `trust_control_math`; this module
//! decides which evidence applies and what each result says. The declared law's loop is in `feedback_declared`,
//! the Nyquist count and the margins in `feedback_margins`, and each channel's response on the grid in
//! `frequency_points`.

// The exact comparisons are deliberate: a sample time must match exactly, and a denominator is zero or not.
#![allow(clippy::float_cmp)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::classical::{transfer_value, Transfer};
use trust_control_math::feedback_poles::{
    algebraic_loop_is_singular, closed_loop_poles, Realization,
};
use trust_control_math::frequency::plotted_phase_degrees;
use trust_control_math::linalg::ComplexValue;
use trust_control_math::polynomial::convolve;

use crate::analysis::feedback_declared::analyze_declared;
use crate::analysis::feedback_margins::{
    closed_loop_result, contour_count_for, estimate_margins, report,
};
use crate::analysis::frequency::without_delay;
use crate::analysis::frequency_points::{channel_points, grid_of, position};
use crate::analysis::{limits, texts};
use crate::failure::{Cause, Failure, NextAction};
use crate::messages::FrequencyGrid;
use crate::model::{linear_transfer_model, GeneralModel, ModelChannel};
use crate::tuning::texts::engineering;

/// The channels of one loop end: the input and the output that the loop uses, by identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChannelPair {
    /// The identifier of the input channel.
    pub input_id: String,
    /// The identifier of the output channel.
    pub output_id: String,
}

/// A declared controller law: its numerator and denominator at the selected value of its parameter, and the
/// units it was declared with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredFeedbackController {
    /// The numerator at the selected value, from the highest power down.
    pub numerator: Vec<f64>,
    /// The denominator at the selected value, from the highest power down; its leading coefficient is not zero.
    pub denominator: Vec<f64>,
    /// The swept parameter of the declared analysis.
    pub parameter: String,
    /// The selected value of that parameter.
    pub selected_value: f64,
    /// The revision of the declared model.
    pub revision: String,
    /// The input unit of the law: the plant's output unit.
    pub input_unit: String,
    /// The output unit of the law: the plant's input unit.
    pub output_unit: String,
}

/// A selected-channel negative feedback loop: the plant, the controller and their channels. A declared law,
/// when present, replaces the controller model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FeedbackDefinition {
    /// The plant model.
    pub plant: GeneralModel,
    /// The controller model.
    pub controller: GeneralModel,
    /// The plant's channels in the loop.
    pub plant_channel: ChannelPair,
    /// The controller's channels in the loop.
    pub controller_channel: ChannelPair,
    /// A declared controller law that closes the loop instead of the controller model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_controller: Option<DeclaredFeedbackController>,
}

/// A point of a frequency response.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FeedbackPoint {
    /// The frequency in rad/s.
    pub frequency_rad_per_second: f64,
    /// The real part.
    pub real: f64,
    /// The imaginary part.
    pub imaginary: f64,
    /// The magnitude.
    pub magnitude: f64,
    /// The magnitude in dB, with a magnitude of zero read as the smallest positive number (N4.3).
    pub magnitude_db: f64,
    /// The plotted phase in radians: the same angle as the phase in degrees (N4.3).
    pub phase_radians: f64,
    /// The plotted phase in degrees (N4.3): unwrapped along the frequencies, with the loop's delay in closed form.
    pub phase_degrees: f64,
}

/// The sensitivities at one frequency, in dB.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SensitivityPoint {
    /// The frequency in rad/s.
    pub frequency_rad_per_second: f64,
    /// `|S|` in dB.
    pub sensitivity_magnitude_db: f64,
    /// `|T|` in dB.
    pub complementary_sensitivity_magnitude_db: f64,
    /// `|C·S|` in dB.
    pub control_sensitivity_magnitude_db: f64,
}

/// The figures of a Nyquist count that qualified.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NyquistSummary {
    /// The counter-clockwise encirclements of −1.
    pub encirclements_counterclockwise: i64,
    /// The open-loop poles with a positive real part.
    pub open_loop_right_half_plane_poles: usize,
    /// The closed-loop poles with a positive real part.
    pub closed_loop_right_half_plane_poles: i64,
    /// Whether the count establishes a stable closed loop.
    pub closed_loop_stable: bool,
    /// The smallest distance from the contour to −1.
    pub minimum_distance_to_minus_one: f64,
    /// The magnitude at the top of the sweep.
    pub endpoint_magnitude: f64,
}

/// The outcome of a Nyquist count: the figures, or the reason it was refused.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum NyquistReport {
    /// The count qualified.
    Qualified {
        /// The figures.
        summary: NyquistSummary,
    },
    /// The count was refused, for this reason.
    Rejected {
        /// The reason, as the Nyquist rules name it.
        reason: String,
    },
}

/// The closed-loop poles of the selection, or the reason they are not available.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum ClosedLoop {
    /// The poles were computed.
    Qualified {
        /// The poles, sorted by real part, then imaginary part.
        poles: Vec<ComplexValue>,
        /// Whether every pole is in the stable region.
        stable: bool,
        /// The scope of the poles.
        scope: String,
    },
    /// The poles are not available.
    Unavailable {
        /// The reason.
        reason: String,
    },
}

/// The classical margins of the loop, estimated over the frequency range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum Margins {
    /// The margins were estimated.
    Estimated {
        /// The first gain crossover in rad/s.
        gain_crossover_rad_per_second: f64,
        /// The phase margin in degrees.
        phase_margin_degrees: f64,
        /// The gain margin in dB, when a phase crossover was found.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gain_margin_db: Option<f64>,
        /// The scope of the estimate.
        scope: String,
    },
    /// The margins are not available.
    Unavailable {
        /// The reason.
        reason: String,
    },
}

/// The result of a general feedback analysis (N4.4, N7.7 to N7.11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FeedbackResult {
    /// The open loop `L = G·C` at each frequency.
    pub open_loop: Vec<FeedbackPoint>,
    /// The scope of the Nyquist evidence.
    pub nyquist_scope: String,
    /// The sensitivities, when the loop has a response at each frequency.
    pub sensitivity: Option<Vec<SensitivityPoint>>,
    /// The Nyquist count, when the loop is made of two transfer functions or a declared law.
    pub nyquist: Option<NyquistReport>,
    /// The closed-loop poles.
    pub closed_loop: ClosedLoop,
    /// The classical margins.
    pub margins: Margins,
    /// The frequency grid as the engine used it (N4.3).
    pub grid: FrequencyGrid,
    /// The plant's channels in the loop.
    pub plant_channel: ChannelPair,
    /// The controller's channels in the loop.
    pub controller_channel: ChannelPair,
    /// Where the controller comes from.
    pub controller_source: ControllerSource,
    /// The sentences of the result area, warnings first (B9.9).
    pub summary: Vec<String>,
    /// The sentences of the details disclosure.
    pub details: Vec<String>,
}

/// A general feedback analysis with its loop, as it is stored in a study.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedGeneralFeedback {
    /// The loop that was analysed.
    pub definition: FeedbackDefinition,
    /// The result.
    pub result: FeedbackResult,
}

/// Why a general feedback analysis is refused. The reasons are the codes of the analysis rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedbackRefusal {
    /// The models, channels or units do not form a valid loop.
    InvalidDefinition,
    /// The frequency grid is empty, too long, not positive or not increasing.
    InvalidFrequencyGrid,
    /// A sampled model's response is needed at or above its Nyquist frequency.
    AliasRisk,
    /// The algebraic loop `1 + Dp·Dc` is singular.
    SingularAlgebraicLoop,
    /// A response is not finite.
    NonfiniteResponse,
    /// Another rule refused the analysis, with its reason.
    Other(String),
}

impl FeedbackRefusal {
    /// The reason in the words of the analysis rules.
    #[must_use]
    pub fn reason(&self) -> String {
        match self {
            Self::InvalidDefinition => texts::FEEDBACK_INVALID.to_string(),
            Self::InvalidFrequencyGrid => texts::FEEDBACK_GRID.to_string(),
            Self::AliasRisk => texts::FEEDBACK_ALIAS.to_string(),
            Self::SingularAlgebraicLoop => texts::FEEDBACK_SINGULAR_LOOP.to_string(),
            Self::NonfiniteResponse => texts::FEEDBACK_NONFINITE.to_string(),
            Self::Other(reason) => reason.clone(),
        }
    }
}

/// Analyses the loop on a frequency grid (N4.4, N7.7 to N7.11).
///
/// # Errors
/// Returns the refusal of the first rule that the loop breaks: an invalid definition, a frequency grid that is
/// not valid, a singular algebraic loop, a response that is not finite, or a refused model response.
pub fn analyze_general_feedback(
    definition: &FeedbackDefinition,
    frequencies: &[f64],
) -> Result<FeedbackResult, FeedbackRefusal> {
    check_definition(definition)?;
    check_grid(frequencies)?;
    let plant_points = channel_points(&definition.plant, &definition.plant_channel, frequencies)?;
    if let Some(declared) = &definition.declared_controller {
        let mut result = analyze_declared(definition, declared, frequencies, &plant_points)?;
        put_delay_phase(&mut result.open_loop, definition, frequencies)?;
        return Ok(result);
    }
    let plant = &definition.plant;
    let controller = &definition.controller;
    let p = realization(plant, &definition.plant_channel)?;
    let c = realization(controller, &definition.controller_channel)?;
    let delay_sum = delay_of(plant) + delay_of(controller);
    if delay_sum == 0.0 && algebraic_loop_is_singular(p.d, c.d) {
        return Err(FeedbackRefusal::SingularAlgebraicLoop);
    }
    let controller_points =
        channel_points(controller, &definition.controller_channel, frequencies)?;
    let open_loop = multiply_points(&plant_points, &controller_points, frequencies)?;
    let sensitivity = sensitivity_points(&open_loop, &controller_points, frequencies);
    let nyquist = match (plant, controller) {
        (
            GeneralModel::TransferFunction {
                identity: plant_identity,
                transfer: plant_transfer,
            },
            GeneralModel::TransferFunction {
                identity: controller_identity,
                transfer: controller_transfer,
            },
        ) => Some(report(contour_count_for(
            &Transfer {
                numerator: convolve(&plant_transfer.numerator, &controller_transfer.numerator),
                denominator: convolve(
                    &plant_transfer.denominator,
                    &controller_transfer.denominator,
                ),
                delay_seconds: plant_identity.delay_seconds + controller_identity.delay_seconds,
            },
            frequencies,
        ))),
        _ => None,
    };
    let closed_loop = if delay_sum > 0.0 {
        ClosedLoop::Unavailable {
            reason: texts::FEEDBACK_DELAY_POLES.to_string(),
        }
    } else {
        let poles = closed_loop_poles(&p, &c)
            .map_err(|_| FeedbackRefusal::Other("numerical-rejection".to_string()))?;
        closed_loop_result(poles, plant, texts::FEEDBACK_CLOSED_LOOP_SCOPE.to_string())
    };
    let nyquist_scope = if nyquist.is_some() {
        texts::FEEDBACK_NYQUIST_TRANSFER_SCOPE
    } else {
        texts::FEEDBACK_NYQUIST_SAMPLED_SCOPE
    };
    let mut result = finish(FeedbackResult {
        open_loop: open_loop_points(&open_loop, frequencies),
        nyquist_scope: nyquist_scope.to_string(),
        sensitivity: Some(sensitivity),
        nyquist,
        closed_loop,
        margins: estimate_margins(definition, frequencies),
        grid: grid_of(frequencies),
        plant_channel: definition.plant_channel.clone(),
        controller_channel: definition.controller_channel.clone(),
        controller_source: ControllerSource::Model,
        summary: Vec::new(),
        details: Vec::new(),
    });
    put_delay_phase(&mut result.open_loop, definition, frequencies)?;
    Ok(result)
}

/// Checks a stored general feedback analysis against its loop: the same frequencies at every point, and no
/// closed-loop poles for a loop with a delay.
///
/// # Errors
/// Returns the refusal of [`analyze_general_feedback`]'s definition check, or [`FeedbackRefusal::Other`] with
/// the sentence of a stored result that does not match its frequencies.
pub fn check_saved_general_feedback(
    saved: &SavedGeneralFeedback,
    frequencies: &[f64],
) -> Result<(), FeedbackRefusal> {
    check_definition(&saved.definition)?;
    let result = &saved.result;
    let same_grid = |values: Vec<f64>| {
        values.len() == frequencies.len()
            && values
                .iter()
                .zip(frequencies)
                .all(|(left, right)| left == right)
    };
    let open_loop_ok = result.open_loop.len() == frequencies.len()
        && result
            .open_loop
            .iter()
            .zip(frequencies)
            .all(|(point, frequency)| {
                point.frequency_rad_per_second == *frequency
                    && [
                        point.real,
                        point.imaginary,
                        point.magnitude,
                        point.phase_radians,
                    ]
                    .iter()
                    .all(|v| v.is_finite())
            });
    let sensitivity_ok = result.sensitivity.as_ref().is_none_or(|points| {
        same_grid(
            points
                .iter()
                .map(|point| point.frequency_rad_per_second)
                .collect(),
        )
    });
    let closed_loop_ok = match &result.closed_loop {
        ClosedLoop::Qualified { .. } => {
            delay_of(&saved.definition.plant) + delay_of(&saved.definition.controller) == 0.0
        }
        ClosedLoop::Unavailable { .. } => true,
    };
    if open_loop_ok && sensitivity_ok && closed_loop_ok {
        Ok(())
    } else {
        Err(FeedbackRefusal::Other(
            "the stored analysis does not match its frequencies".to_string(),
        ))
    }
}

/// The definition rules: the channels exist, their units agree, both models are discrete with the same sample
/// time or both are continuous, and a declared law has a finite numerator and a nonzero leading denominator
/// coefficient with units that match the plant's.
fn check_definition(definition: &FeedbackDefinition) -> Result<(), FeedbackRefusal> {
    let plant = definition.plant.identity();
    let controller = definition.controller.identity();
    let find = |channels: &[ModelChannel], id: &str| {
        channels.iter().find(|channel| channel.id == id).cloned()
    };
    let plant_input = find(&plant.inputs, &definition.plant_channel.input_id);
    let plant_output = find(&plant.outputs, &definition.plant_channel.output_id);
    let controller_input = find(&controller.inputs, &definition.controller_channel.input_id);
    let controller_output = find(
        &controller.outputs,
        &definition.controller_channel.output_id,
    );
    let (Some(pi), Some(po), Some(ci), Some(co)) = (
        plant_input,
        plant_output,
        controller_input,
        controller_output,
    ) else {
        return Err(FeedbackRefusal::InvalidDefinition);
    };
    if pi.unit != co.unit || po.unit != ci.unit {
        return Err(FeedbackRefusal::InvalidDefinition);
    }
    match (&definition.plant, &definition.controller) {
        (
            GeneralModel::DiscreteStateSpace {
                sample_time_seconds: plant_time,
                ..
            },
            GeneralModel::DiscreteStateSpace {
                sample_time_seconds: controller_time,
                ..
            },
        ) => {
            if plant_time != controller_time {
                return Err(FeedbackRefusal::InvalidDefinition);
            }
        }
        (GeneralModel::DiscreteStateSpace { .. }, _)
        | (_, GeneralModel::DiscreteStateSpace { .. }) => {
            return Err(FeedbackRefusal::InvalidDefinition);
        }
        _ => {}
    }
    if let Some(declared) = &definition.declared_controller {
        let finite = declared
            .numerator
            .iter()
            .chain(&declared.denominator)
            .all(|value| value.is_finite());
        if declared.numerator.is_empty()
            || declared.denominator.is_empty()
            || !finite
            || declared.denominator[0] == 0.0
            || declared.input_unit != po.unit
            || declared.output_unit != pi.unit
        {
            return Err(FeedbackRefusal::InvalidDefinition);
        }
    }
    Ok(())
}

/// The frequency grid: between one and 512 points, all positive, finite and increasing (N4.3).
fn check_grid(frequencies: &[f64]) -> Result<(), FeedbackRefusal> {
    let increasing = frequencies.windows(2).all(|pair| pair[1] > pair[0]);
    if frequencies.is_empty()
        || frequencies.len() > limits::FREQUENCY_MAXIMUM_POINTS
        || !frequencies
            .iter()
            .all(|frequency| frequency.is_finite() && *frequency > 0.0)
        || !increasing
    {
        return Err(FeedbackRefusal::InvalidFrequencyGrid);
    }
    Ok(())
}

/// The realization of a loop end: the companion form of a transfer function, or the channel of a state-space
/// model (N7.11).
fn realization(
    model: &GeneralModel,
    channel: &ChannelPair,
) -> Result<Realization, FeedbackRefusal> {
    match model {
        GeneralModel::TransferFunction { transfer, .. } => {
            Realization::from_transfer(&transfer.numerator, &transfer.denominator)
                .map_err(|_| FeedbackRefusal::Other("invalid-polynomial".to_string()))
        }
        GeneralModel::ContinuousStateSpace {
            identity, system, ..
        }
        | GeneralModel::DiscreteStateSpace {
            identity, system, ..
        } => {
            let input = position(&identity.inputs, &channel.input_id)?;
            let output = position(&identity.outputs, &channel.output_id)?;
            Ok(Realization::from_channel(system, input, output))
        }
    }
}

/// The transfer function of a state-space model through the linear-model conversion of the model page.
pub(crate) fn linearised(model: &GeneralModel) -> Result<Transfer, FeedbackRefusal> {
    let value = serde_json::to_value(model).map_err(|_| FeedbackRefusal::InvalidDefinition)?;
    match linear_transfer_model(&value) {
        Ok(GeneralModel::TransferFunction { identity, transfer }) => Ok(Transfer {
            numerator: transfer.numerator,
            denominator: transfer.denominator,
            delay_seconds: identity.delay_seconds,
        }),
        Ok(_) => Err(FeedbackRefusal::Other(
            texts::DECLARED_PLANT_UNSUPPORTED.to_string(),
        )),
        Err(refusal) => Err(FeedbackRefusal::Other(refusal.to_string())),
    }
}

/// The product of two responses, point by point, with the finiteness rule of N4.3.
pub(crate) fn multiply_points(
    left: &[ComplexValue],
    right: &[ComplexValue],
    frequencies: &[f64],
) -> Result<Vec<ComplexValue>, FeedbackRefusal> {
    let product: Vec<ComplexValue> = left
        .iter()
        .zip(right)
        .map(|(a, b)| ComplexValue {
            real: a.real * b.real - a.imaginary * b.imaginary,
            imaginary: a.real * b.imaginary + a.imaginary * b.real,
        })
        .collect();
    let finite = product
        .iter()
        .all(|value| value.real.is_finite() && value.imaginary.is_finite());
    if product.len() != frequencies.len() || !finite {
        return Err(FeedbackRefusal::NonfiniteResponse);
    }
    Ok(product)
}

/// The points of the open loop with their magnitude and phase. The phase is the plotted phase of `open_loop` read as a
/// response without a delay (N4.3); `analyze_general_feedback` then puts the delay of a loop that has one in closed form
/// (`put_delay_phase`).
pub(crate) fn open_loop_points(
    open_loop: &[ComplexValue],
    frequencies: &[f64],
) -> Vec<FeedbackPoint> {
    let phases = plotted_phase_degrees(open_loop, frequencies, 0.0);
    open_loop
        .iter()
        .zip(frequencies)
        .zip(phases)
        .map(|((value, &frequency), phase_degrees)| FeedbackPoint {
            frequency_rad_per_second: frequency,
            real: value.real,
            imaginary: value.imaginary,
            magnitude: value.real.hypot(value.imaginary),
            magnitude_db: 20.0
                * value
                    .real
                    .hypot(value.imaginary)
                    .max(f64::from_bits(1))
                    .log10(),
            phase_radians: phase_degrees.to_radians(),
            phase_degrees,
        })
        .collect()
}

/// The plotted phase of a loop with a delay (N4.3): the unwrapped phase of the loop without its delays, the plant's channel
/// times the controller's channel or the declared law, plus the delay's phase `−ω·L` in closed form. A loop without a delay
/// keeps the phase of `open_loop_points`, which is its plotted phase already. The real and imaginary parts and the magnitudes
/// are not changed, and the margins never read this phase.
fn put_delay_phase(
    points: &mut [FeedbackPoint],
    definition: &FeedbackDefinition,
    frequencies: &[f64],
) -> Result<(), FeedbackRefusal> {
    let delay_seconds = loop_delay_seconds(definition);
    if delay_seconds <= 0.0 {
        return Ok(());
    }
    let plant = channel_points(
        &without_delay(&definition.plant),
        &definition.plant_channel,
        frequencies,
    )?;
    let controller: Vec<ComplexValue> = match &definition.declared_controller {
        Some(declared) => {
            let law = Transfer {
                numerator: declared.numerator.clone(),
                denominator: declared.denominator.clone(),
                delay_seconds: 0.0,
            };
            frequencies
                .iter()
                .map(|&frequency| transfer_value(&law, frequency))
                .collect()
        }
        None => channel_points(
            &without_delay(&definition.controller),
            &definition.controller_channel,
            frequencies,
        )?,
    };
    let delay_free = multiply_points(&plant, &controller, frequencies)?;
    let phases = plotted_phase_degrees(&delay_free, frequencies, delay_seconds);
    for (point, phase_degrees) in points.iter_mut().zip(phases) {
        point.phase_degrees = phase_degrees;
        point.phase_radians = phase_degrees.to_radians();
    }
    Ok(())
}

/// The known delay of a loop in seconds (N4.3, N4.4): the plant's delay plus the controller's, or the plant's alone when a
/// declared law, which has no delay, closes the loop.
fn loop_delay_seconds(definition: &FeedbackDefinition) -> f64 {
    let controller_delay = if definition.declared_controller.is_some() {
        0.0
    } else {
        delay_of(&definition.controller)
    };
    delay_of(&definition.plant) + controller_delay
}

/// The sensitivities `S = 1/(1 + L)`, `T = L·S` and `C·S`, in dB at each frequency of the grid (N7.7). A
/// value of zero is the smallest positive number, so that the logarithm stays finite.
pub(crate) fn sensitivity_points(
    open_loop: &[ComplexValue],
    controller: &[ComplexValue],
    frequencies: &[f64],
) -> Vec<SensitivityPoint> {
    let floor = f64::from_bits(1);
    open_loop
        .iter()
        .zip(controller)
        .zip(frequencies)
        .map(|((loop_value, controller_value), &frequency)| {
            let sensitivity = 1.0 / (1.0 + loop_value.real).hypot(loop_value.imaginary);
            let loop_magnitude = loop_value.real.hypot(loop_value.imaginary);
            let controller_magnitude = controller_value.real.hypot(controller_value.imaginary);
            SensitivityPoint {
                frequency_rad_per_second: frequency,
                sensitivity_magnitude_db: 20.0 * sensitivity.max(floor).log10(),
                complementary_sensitivity_magnitude_db: 20.0
                    * (loop_magnitude * sensitivity).max(floor).log10(),
                control_sensitivity_magnitude_db: 20.0
                    * (controller_magnitude * sensitivity).max(floor).log10(),
            }
        })
        .collect()
}

/// Where the controller of a feedback analysis comes from: the controller model of the request, or the loop's declared law.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ControllerSource {
    /// The controller model.
    Model,
    /// The declared controller law of the loop (N7.6).
    DeclaredLaw,
}

const REFUSED_DEFINITION: &str = "The plant and the controller do not form this loop: their channels, units or sample times disagree (N4.4).";
const REFUSED_GRID: &str =
    "The frequency grid must hold positive, increasing frequencies, at most 512 points (N4.3).";
const REFUSED_ALIAS: &str =
    "A sampled model's response is needed below its Nyquist frequency π/Ts (N4.3).";
const REFUSED_SINGULAR: &str =
    "The algebraic loop 1 + Dp·Dc is singular, so the loop has no unique response (N4.4).";
const REFUSED_NONFINITE: &str =
    "The loop's response is not finite at a frequency of the grid (N4.3).";

impl From<FeedbackRefusal> for Failure {
    /// The failure of a refused analysis (A11.1), with the next action that its refusal allows.
    fn from(refusal: FeedbackRefusal) -> Self {
        let (text, cause, next_action) = match &refusal {
            FeedbackRefusal::InvalidDefinition => (
                REFUSED_DEFINITION,
                Cause::ModelOrData,
                NextAction::OpenChannels,
            ),
            FeedbackRefusal::InvalidFrequencyGrid => {
                (REFUSED_GRID, Cause::Settings, NextAction::None)
            }
            FeedbackRefusal::AliasRisk => (REFUSED_ALIAS, Cause::Settings, NextAction::None),
            FeedbackRefusal::SingularAlgebraicLoop => {
                (REFUSED_SINGULAR, Cause::ModelOrData, NextAction::None)
            }
            FeedbackRefusal::NonfiniteResponse => {
                (REFUSED_NONFINITE, Cause::ModelOrData, NextAction::None)
            }
            FeedbackRefusal::Other(reason) => {
                (reason.as_str(), Cause::ModelOrData, NextAction::None)
            }
        };
        Self::new(text, cause, next_action)
    }
}

/// Completes a feedback result with its sentences: the result area's sentences, warnings first, and the details
/// disclosure's sentences (N7.8, N7.10, B10.2).
pub(crate) fn finish(result: FeedbackResult) -> FeedbackResult {
    let (summary, details) = feedback_words(&result);
    FeedbackResult {
        summary,
        details,
        ..result
    }
}

/// The sentences of a feedback result: the margins and the Nyquist count for the result area, and the scopes and the
/// reasons of the evidence for the details disclosure.
fn feedback_words(result: &FeedbackResult) -> (Vec<String>, Vec<String>) {
    let margins = match &result.margins {
        Margins::Estimated {
            gain_margin_db,
            phase_margin_degrees,
            ..
        } => format!(
            "Gain margin {}; phase margin {}°.",
            gain_margin_db.map_or_else(
                || "unavailable".to_string(),
                |db| format!("{} dB", engineering(db, None))
            ),
            engineering(*phase_margin_degrees, None)
        ),
        Margins::Unavailable { .. } => "Margins unavailable.".to_string(),
    };
    let mut summary = vec![margins];
    if let Some(nyquist) = &result.nyquist {
        summary.push(match nyquist {
            NyquistReport::Qualified { summary: figures } => {
                format!("Nyquist contour: {}", nyquist_count_text(figures))
            }
            NyquistReport::Rejected { reason } => format!("Nyquist contour unavailable: {reason}"),
        });
    }
    let margin_scope = match &result.margins {
        Margins::Estimated { scope, .. } => scope.clone(),
        Margins::Unavailable { reason } => reason.clone(),
    };
    let closed_loop_scope = match &result.closed_loop {
        ClosedLoop::Qualified { scope, .. } => scope.clone(),
        ClosedLoop::Unavailable { reason } => reason.clone(),
    };
    (
        summary,
        vec![
            result.nyquist_scope.clone(),
            margin_scope,
            closed_loop_scope,
        ],
    )
}

/// The criterion as the course states it: Z = P + N, with clockwise encirclements of −1 counted positive (N7.10).
fn nyquist_count_text(figures: &NyquistSummary) -> String {
    let clockwise = if figures.encirclements_counterclockwise == 0 {
        0
    } else {
        -figures.encirclements_counterclockwise
    };
    let stability = if figures.closed_loop_stable {
        "stable"
    } else {
        "unstable"
    };
    format!(
        "P = {} open-loop right-half-plane poles. N = {clockwise} clockwise encirclements of −1. \
         Z = P + N = {} closed-loop right-half-plane poles: the closed loop is {stability}.",
        figures.open_loop_right_half_plane_poles, figures.closed_loop_right_half_plane_poles
    )
}

/// The delay of a model in seconds.
fn delay_of(model: &GeneralModel) -> f64 {
    model.identity().delay_seconds
}

#[cfg(test)]
#[path = "general_feedback_tests.rs"]
mod tests;
