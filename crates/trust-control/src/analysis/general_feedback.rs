//! General feedback analysis (N4.4, N7.7 to N7.12): the open loop, the sensitivities, the closed-loop poles,
//! the Nyquist count and the classical margins of a selected-channel negative feedback loop. The loop closes
//! either with a second supplied model's channel, or with one declared controller law (a rational function of
//! its parameters, evaluated at the selected value). The mathematics is in `trust_control_math`; this module
//! decides which evidence applies and what each result says.

// The exact comparisons are deliberate: a sample time must match exactly, and a denominator is zero or not.
#![allow(clippy::float_cmp)]

use std::f64::consts::PI;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::classical::{
    analyze_transfer, classical_stability_margins, roots_of_real_polynomial, ClassicalError,
    GainMargin, Sweep, Transfer,
};
use trust_control_math::feedback_poles::{
    algebraic_loop_is_singular, closed_loop_poles, Realization,
};
use trust_control_math::frequency::{response_at, FrequencyModel};
use trust_control_math::linalg::ComplexValue;
use trust_control_math::model::TransferFunction;
use trust_control_math::nyquist::{nyquist_stability, Nyquist};
use trust_control_math::polynomial::{add_aligned, convolve};

use crate::analysis::declared::DeclaredControlAnalysis;
use crate::analysis::{limits, texts};
use crate::model::{linear_transfer_model, GeneralModel, ModelChannel, ModelIdentity};

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
    /// The phase in radians.
    pub phase_radians: f64,
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
        return analyze_declared(definition, declared, frequencies, &plant_points);
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
    Ok(FeedbackResult {
        open_loop: open_loop_points(&open_loop, frequencies),
        nyquist_scope: nyquist_scope.to_string(),
        sensitivity: Some(sensitivity),
        nyquist,
        closed_loop,
        margins: estimate_margins(definition, frequencies),
    })
}

/// The loop of a declared controller law: the plant's channel times the law at each frequency, the closed-loop
/// poles of the declared characteristic polynomial, and the Nyquist count of the declared loop.
fn analyze_declared(
    definition: &FeedbackDefinition,
    declared: &DeclaredFeedbackController,
    frequencies: &[f64],
    plant_points: &[ComplexValue],
) -> Result<FeedbackResult, FeedbackRefusal> {
    let plant = match &definition.plant {
        GeneralModel::ContinuousStateSpace { .. } => linearised(&definition.plant)?,
        GeneralModel::TransferFunction { identity, transfer } => Transfer {
            numerator: transfer.numerator.clone(),
            denominator: transfer.denominator.clone(),
            delay_seconds: identity.delay_seconds,
        },
        GeneralModel::DiscreteStateSpace { .. } => {
            return Err(FeedbackRefusal::Other(
                texts::DECLARED_PLANT_UNSUPPORTED.to_string(),
            ));
        }
    };
    let law = Transfer {
        numerator: declared.numerator.clone(),
        denominator: declared.denominator.clone(),
        delay_seconds: 0.0,
    };
    let response = analyze_transfer(&law, frequencies)
        .map_err(|error| FeedbackRefusal::Other(texts::classical_reason(error).to_string()))?;
    if plant_points.len() != response.bode.len() {
        return Err(FeedbackRefusal::Other(
            texts::DECLARED_GRID_MISMATCH.to_string(),
        ));
    }
    let controller_points: Vec<ComplexValue> = response
        .bode
        .iter()
        .map(|point| {
            let magnitude = 10f64.powf(point.magnitude_db / 20.0);
            let phase = point.phase_degrees * PI / 180.0;
            ComplexValue {
                real: magnitude * phase.cos(),
                imaginary: magnitude * phase.sin(),
            }
        })
        .collect();
    let open_loop = multiply_points(plant_points, &controller_points, frequencies)?;
    let sensitivity = sensitivity_points(&open_loop, &controller_points, frequencies);
    let characteristic = add_aligned(
        &convolve(&plant.denominator, &law.denominator),
        &convolve(&plant.numerator, &law.numerator),
    );
    let closed_loop = if plant.delay_seconds > 0.0 {
        ClosedLoop::Unavailable {
            reason: texts::FEEDBACK_DELAY_POLES.to_string(),
        }
    } else {
        let poles = roots_of_real_polynomial(&characteristic).ok_or_else(|| {
            FeedbackRefusal::Other(texts::DECLARED_CHARACTERISTIC_UNSOLVED.to_string())
        })?;
        let scope = format!(
            "{}{}{}",
            texts::FEEDBACK_DECLARED_POLES_SCOPE,
            if matches!(definition.plant, GeneralModel::ContinuousStateSpace { .. }) {
                texts::FEEDBACK_DECLARED_STATE_SUFFIX
            } else {
                ""
            },
            texts::FEEDBACK_DECLARED_POLES_TAIL
        );
        closed_loop_result(poles, &definition.plant, scope)
    };
    let nyquist = report(contour_count_for(
        &Transfer {
            numerator: convolve(&plant.numerator, &law.numerator),
            denominator: convolve(&plant.denominator, &law.denominator),
            delay_seconds: plant.delay_seconds,
        },
        frequencies,
    ));
    let nyquist_scope = match &nyquist {
        NyquistReport::Qualified { .. } => texts::FEEDBACK_NYQUIST_DECLARED_SCOPE.to_string(),
        NyquistReport::Rejected { reason } => {
            format!("The shared Nyquist stability owner rejected the requested contour: {reason}")
        }
    };
    Ok(FeedbackResult {
        open_loop: open_loop_points(&open_loop, frequencies),
        nyquist_scope,
        sensitivity: Some(sensitivity),
        nyquist: Some(nyquist),
        closed_loop,
        margins: estimate_margins(definition, frequencies),
    })
}

/// The declared controller law as a feedback definition on the plant's channels, or `None` when the declared
/// law does not fit the plant: a discrete or non-SISO plant, unit mismatches, or a law that does not evaluate
/// (N7.6). The controller model is a placeholder that carries the channels and the units.
#[must_use]
pub fn declared_feedback_definition(
    analysis: &DeclaredControlAnalysis,
    plant: &GeneralModel,
) -> Option<FeedbackDefinition> {
    crate::analysis::declared::check_declared(analysis).ok()?;
    if plant.identity().inputs.len() != 1 || plant.identity().outputs.len() != 1 {
        return None;
    }
    let (input, output) = (&plant.identity().inputs[0], &plant.identity().outputs[0]);
    match plant {
        GeneralModel::DiscreteStateSpace { .. } => return None,
        GeneralModel::ContinuousStateSpace { .. } => {
            linearised(plant).ok()?;
        }
        GeneralModel::TransferFunction { .. } => {}
    }
    if analysis.plant.input_unit != input.unit || analysis.plant.output_unit != output.unit {
        return None;
    }
    let sweep = &analysis.root_locus;
    let controller = &analysis.controller;
    let evaluate = |coefficients: &[crate::analysis::declared::DeclaredCoefficient]| -> Vec<f64> {
        crate::analysis::declared::substitute(
            coefficients,
            &sweep.parameter,
            sweep.selected_value,
            controller,
        )
    };
    let numerator = evaluate(&controller.numerator);
    let denominator = evaluate(&controller.denominator);
    if !numerator.iter().all(|value| value.is_finite())
        || !denominator.iter().all(|value| value.is_finite())
        || denominator[0] == 0.0
    {
        return None;
    }
    let revision = analysis.plant.revision.clone();
    let placeholder_identity = ModelIdentity {
        revision: format!("{revision}:declared-controller-analysis"),
        source: analysis.plant.source,
        validation: analysis.plant.validation,
        evidence: vec![format!("declared-controller-law:{revision}")],
        inputs: vec![ModelChannel {
            id: format!("{}:feedback", output.id),
            unit: output.unit.clone(),
            operating_range: output.operating_range,
        }],
        outputs: vec![ModelChannel {
            id: format!("{}:command", input.id),
            unit: input.unit.clone(),
            operating_range: input.operating_range,
        }],
        delay_seconds: 0.0,
        operating_point: None,
    };
    let controller_model = GeneralModel::TransferFunction {
        identity: placeholder_identity,
        transfer: TransferFunction {
            numerator: vec![1.0],
            denominator: vec![1.0],
        },
    };
    let controller_channel = ChannelPair {
        input_id: format!("{}:feedback", output.id),
        output_id: format!("{}:command", input.id),
    };
    Some(FeedbackDefinition {
        plant: plant.clone(),
        controller: controller_model,
        plant_channel: ChannelPair {
            input_id: input.id.clone(),
            output_id: output.id.clone(),
        },
        controller_channel,
        declared_controller: Some(DeclaredFeedbackController {
            numerator,
            denominator,
            parameter: sweep.parameter.clone(),
            selected_value: sweep.selected_value,
            revision,
            input_unit: output.unit.clone(),
            output_unit: input.unit.clone(),
        }),
    })
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

/// The frequency grid: between one and 4096 points, all positive, finite and increasing.
fn check_grid(frequencies: &[f64]) -> Result<(), FeedbackRefusal> {
    let increasing = frequencies.windows(2).all(|pair| pair[1] > pair[0]);
    if frequencies.is_empty()
        || frequencies.len() > limits::FEEDBACK_MAXIMUM_FREQUENCIES
        || !frequencies
            .iter()
            .all(|frequency| frequency.is_finite() && *frequency > 0.0)
        || !increasing
    {
        return Err(FeedbackRefusal::InvalidFrequencyGrid);
    }
    Ok(())
}

/// The response of one channel of a model on the grid (N4.3): the transfer function's bode data, or the
/// state-space response of the channel. A sampled model must stay below its Nyquist frequency.
fn channel_points(
    model: &GeneralModel,
    channel: &ChannelPair,
    frequencies: &[f64],
) -> Result<Vec<ComplexValue>, FeedbackRefusal> {
    if let GeneralModel::DiscreteStateSpace {
        sample_time_seconds,
        ..
    } = model
    {
        if frequencies
            .iter()
            .any(|frequency| *frequency >= PI / sample_time_seconds)
        {
            return Err(FeedbackRefusal::AliasRisk);
        }
    }
    match model {
        GeneralModel::TransferFunction { identity, transfer } => {
            let transfer = Transfer {
                numerator: transfer.numerator.clone(),
                denominator: transfer.denominator.clone(),
                delay_seconds: identity.delay_seconds,
            };
            let analysis = analyze_transfer(&transfer, frequencies).map_err(|error| {
                FeedbackRefusal::Other(texts::classical_reason(error).to_string())
            })?;
            Ok(analysis
                .bode
                .iter()
                .map(|point| {
                    let magnitude = 10f64.powf(point.magnitude_db / 20.0);
                    let phase = point.phase_degrees * PI / 180.0;
                    ComplexValue {
                        real: magnitude * phase.cos(),
                        imaginary: magnitude * phase.sin(),
                    }
                })
                .collect())
        }
        GeneralModel::ContinuousStateSpace {
            identity, system, ..
        }
        | GeneralModel::DiscreteStateSpace {
            identity, system, ..
        } => {
            let input = position(&identity.inputs, &channel.input_id)?;
            let output = position(&identity.outputs, &channel.output_id)?;
            let frequency_model = FrequencyModel::StateSpace {
                system: system.clone(),
                timing: model.timing(),
                delay_seconds: identity.delay_seconds,
            };
            frequencies
                .iter()
                .map(|&frequency| {
                    response_at(&frequency_model, frequency)
                        .map_err(|_| {
                            FeedbackRefusal::Other(
                                "singular-or-ill-conditioned-frequency".to_string(),
                            )
                        })
                        .and_then(|rows| {
                            rows.get(output)
                                .and_then(|row| row.get(input))
                                .copied()
                                .ok_or_else(|| {
                                    FeedbackRefusal::Other(
                                        "singular-or-ill-conditioned-frequency".to_string(),
                                    )
                                })
                        })
                })
                .collect()
        }
    }
}

/// The index of a channel by identifier.
fn position(channels: &[ModelChannel], id: &str) -> Result<usize, FeedbackRefusal> {
    channels
        .iter()
        .position(|channel| channel.id == id)
        .ok_or(FeedbackRefusal::InvalidDefinition)
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
fn linearised(model: &GeneralModel) -> Result<Transfer, FeedbackRefusal> {
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
fn multiply_points(
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

/// The points of the open loop with their magnitude and phase.
fn open_loop_points(open_loop: &[ComplexValue], frequencies: &[f64]) -> Vec<FeedbackPoint> {
    open_loop
        .iter()
        .zip(frequencies)
        .map(|(value, &frequency)| FeedbackPoint {
            frequency_rad_per_second: frequency,
            real: value.real,
            imaginary: value.imaginary,
            magnitude: value.real.hypot(value.imaginary),
            phase_radians: value.imaginary.atan2(value.real),
        })
        .collect()
}

/// The sensitivities `S = 1/(1 + L)`, `T = L·S` and `C·S`, in dB at each frequency of the grid (N7.7). A
/// value of zero is the smallest positive number, so that the logarithm stays finite.
fn sensitivity_points(
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

/// The Nyquist count of a transfer function on the grid, between its first and last frequencies (N7.10).
fn contour_count_for(transfer: &Transfer, frequencies: &[f64]) -> Result<Nyquist, ClassicalError> {
    let first = frequencies[0];
    let last = frequencies[frequencies.len() - 1];
    let sweep = Sweep {
        minimum_rad_per_second: first,
        maximum_rad_per_second: last,
        points_per_decade: nyquist_points((last / first).log10()),
    };
    nyquist_stability(transfer, sweep)
}

/// The points per decade of a Nyquist contour: `⌊511 / decades⌋`, clamped to the allowed range.
fn nyquist_points(decades: f64) -> u32 {
    let budget = (limits::MARGIN_POINT_BUDGET / decades).floor();
    let clamped = budget.clamp(
        f64::from(limits::NYQUIST_MINIMUM_POINTS_PER_DECADE),
        f64::from(limits::NYQUIST_MAXIMUM_POINTS_PER_DECADE),
    );
    // The clamp keeps the value between ten and one hundred, so the conversion is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let points = clamped as u32;
    points
}

/// The report of a Nyquist count.
fn report(outcome: Result<Nyquist, ClassicalError>) -> NyquistReport {
    match outcome {
        Ok(nyquist) => NyquistReport::Qualified {
            summary: NyquistSummary {
                encirclements_counterclockwise: nyquist.encirclements_counterclockwise,
                open_loop_right_half_plane_poles: nyquist.open_loop_right_half_plane_poles,
                closed_loop_right_half_plane_poles: nyquist.closed_loop_right_half_plane_poles,
                closed_loop_stable: nyquist.closed_loop_stable,
                minimum_distance_to_minus_one: nyquist.minimum_distance_to_minus_one,
                endpoint_magnitude: nyquist.endpoint_magnitude,
            },
        },
        Err(error) => NyquistReport::Rejected {
            reason: texts::classical_reason(error).to_string(),
        },
    }
}

/// The closed-loop result of a set of poles: sorted, and stable when every pole lies in the stable region
/// (N7.11).
fn closed_loop_result(
    mut poles: Vec<ComplexValue>,
    plant: &GeneralModel,
    scope: String,
) -> ClosedLoop {
    poles.sort_by(|left, right| {
        left.real
            .partial_cmp(&right.real)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                left.imaginary
                    .partial_cmp(&right.imaginary)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    let stable = poles.iter().all(|pole| match plant {
        GeneralModel::DiscreteStateSpace { .. } => pole.real.hypot(pole.imaginary) < 1.0 - 1e-10,
        _ => pole.real < -1e-10,
    });
    ClosedLoop::Qualified {
        poles,
        stable,
        scope,
    }
}

/// The classical margins of the loop, estimated on the grid (N7.8): the first crossovers, with the range and
/// the pure delay checked first.
fn estimate_margins(definition: &FeedbackDefinition, frequencies: &[f64]) -> Margins {
    let unavailable = |reason: &str| Margins::Unavailable {
        reason: reason.to_string(),
    };
    let GeneralModel::TransferFunction { identity, transfer } = &definition.plant else {
        return unavailable(texts::MARGIN_NEEDS_TRANSFER);
    };
    let controller_transfer = match &definition.controller {
        GeneralModel::TransferFunction { identity, transfer } => Some((identity, transfer)),
        _ => None,
    };
    let (numerator, denominator, controller_delay) =
        match (&definition.declared_controller, controller_transfer) {
            (Some(declared), _) => (
                declared.numerator.clone(),
                declared.denominator.clone(),
                0.0,
            ),
            (None, Some((controller_identity, controller_transfer))) => (
                controller_transfer.numerator.clone(),
                controller_transfer.denominator.clone(),
                controller_identity.delay_seconds,
            ),
            (None, None) => return unavailable(texts::MARGIN_NEEDS_TRANSFER),
        };
    let delay = identity.delay_seconds + controller_delay;
    let first = frequencies[0];
    let last = frequencies[frequencies.len() - 1];
    let decades = (last / first).log10();
    let points = (limits::MARGIN_POINT_BUDGET / decades).floor().min(100.0);
    if !decades.is_finite() || decades <= 0.0 || points < 10.0 {
        return unavailable(texts::MARGIN_RANGE_UNSUPPORTED);
    }
    let largest_interval = last * (1.0 - 10f64.powf(-1.0 / points));
    if largest_interval * delay > PI / 2.0 {
        return unavailable(texts::MARGIN_DELAY_TOO_FAST);
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let points_per_decade = points as u32;
    let loop_transfer = Transfer {
        numerator: convolve(&transfer.numerator, &numerator),
        denominator: convolve(&transfer.denominator, &denominator),
        delay_seconds: delay,
    };
    let sweep = Sweep {
        minimum_rad_per_second: first,
        maximum_rad_per_second: last,
        points_per_decade,
    };
    match classical_stability_margins(&loop_transfer, sweep) {
        Ok(margins)
            if margins.gain_crossover_rad_per_second.is_finite()
                && margins.phase_margin_degrees.is_finite() =>
        {
            Margins::Estimated {
                gain_crossover_rad_per_second: margins.gain_crossover_rad_per_second,
                phase_margin_degrees: margins.phase_margin_degrees,
                gain_margin_db: match margins.gain_margin {
                    GainMargin::Decibels(value) if value.is_finite() => Some(value),
                    _ => None,
                },
                scope: texts::MARGIN_SCOPE.to_string(),
            }
        }
        _ => unavailable(texts::MARGIN_NO_CROSSOVER),
    }
}

/// The delay of a model in seconds.
fn delay_of(model: &GeneralModel) -> f64 {
    model.identity().delay_seconds
}

#[cfg(test)]
#[path = "general_feedback_tests.rs"]
mod tests;
