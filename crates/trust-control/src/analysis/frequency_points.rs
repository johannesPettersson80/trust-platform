//! The response of a model's channel on a frequency grid (N4.3): the transfer function's bode data, or the state-space
//! response of the channel. The `frequency` and the `feedback` tools share it. The grid of a request is checked in the
//! tools, with the rules of N4.3 (`frequency::checked_grid`, `feedback_tool::feedback_grid`).

use std::f64::consts::PI;

use trust_control_math::classical::{analyze_transfer, Transfer};
use trust_control_math::frequency::{response_at, FrequencyModel};
use trust_control_math::linalg::ComplexValue;

use crate::analysis::general_feedback::{ChannelPair, FeedbackRefusal};
use crate::analysis::texts;
use crate::messages::FrequencyGrid;
use crate::model::{GeneralModel, ModelChannel};

/// The grid as the engine used it, from the frequencies it evaluated: the ends in rad/s and the number of points (N4.3).
#[must_use]
pub fn grid_of(frequencies: &[f64]) -> FrequencyGrid {
    FrequencyGrid {
        minimum_frequency: frequencies.first().copied().unwrap_or(0.0),
        maximum_frequency: frequencies.last().copied().unwrap_or(0.0),
        points: u32::try_from(frequencies.len()).unwrap_or(u32::MAX),
    }
}

/// The response of one channel of a model on the grid (N4.3): the transfer function's bode data, or the
/// state-space response of the channel. A sampled model must stay below its Nyquist frequency.
///
/// # Errors
/// The channel is not one of the model's, a frequency of a sampled model is at or above its Nyquist frequency, or the
/// response is singular at a frequency.
pub fn channel_points(
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
pub(crate) fn position(channels: &[ModelChannel], id: &str) -> Result<usize, FeedbackRefusal> {
    channels
        .iter()
        .position(|channel| channel.id == id)
        .ok_or(FeedbackRefusal::InvalidDefinition)
}
