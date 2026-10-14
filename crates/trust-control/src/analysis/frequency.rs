//! The Bode frequency response of a model (specification 35, B9.9; numerical annex N4.2 and N4.3; draft §2.3 `frequency`). The
//! response of every input-output pair on a logarithmic grid, the model's poles and zeros, and their stability (N4.2). The phase is
//! the principal value, as N4.3 requires for a plotted phase. The response of a channel is `channel_points`, shared with `feedback`.

use std::f64::consts::PI;

use trust_control_math::classical::roots_of_real_polynomial;
use trust_control_math::frequency::logarithmic_grid;
use trust_control_math::linalg::{eigenvalues, ComplexValue, NumericalTolerance};
use trust_control_math::model::Timing;

use crate::analysis::budget::Budget;
use crate::analysis::frequency_points::channel_points;
use crate::analysis::general_feedback::ChannelPair;
use crate::analysis::tool::{
    channel_count, channel_ref, default_grid, input_failure, GRID_POINTS,
    MAXIMUM_CHANNEL_FREQUENCY_POINTS,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::messages::{
    AnalysisInputs, FrequencyAnswer, FrequencyChannel, FrequencyGrid, FrequencyPoint,
};
use crate::model::GeneralModel;

/// The scope of the poles (N4.2 and draft §2.3): the list holds the model's poles, not a pure delay's, and it says nothing of a
/// closed loop.
pub const POLE_SCOPE: &str = "Model poles only; pure delay is not represented in the pole list. This is not closed-loop stability.";

/// The smallest positive number, which the page floors a magnitude at before its logarithm (`Number.MIN_VALUE`).
const MINIMUM_MAGNITUDE: f64 = f64::from_bits(1);

/// The tolerance of the eigenvalue calculation of a state matrix (the page's `tolerance`, N4.2).
const TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-12,
    relative: 1e-10,
    condition_limit: 1e12,
};

/// The grid of `frequency` as it is used: the requested grid, or the default for the model (C23). Each check names its input
/// (A7, N4.3): the points from 2 to 512, finite and positive increasing frequencies, the Nyquist limit of a discrete model, and
/// the channel-frequency points.
///
/// # Errors
/// Returns the input failure of the first rule the grid breaks, with its next action none.
pub fn checked_grid(
    model: &GeneralModel,
    given: Option<FrequencyGrid>,
) -> Result<FrequencyGrid, Failure> {
    let grid = given.unwrap_or_else(|| default_grid(model));
    if !GRID_POINTS.contains(&grid.points) {
        return Err(input_failure("The grid's points must be from 2 to 512."));
    }
    let ordered = grid.minimum_frequency.is_finite()
        && grid.maximum_frequency.is_finite()
        && grid.minimum_frequency > 0.0
        && grid.maximum_frequency > grid.minimum_frequency;
    if !ordered {
        return Err(input_failure(
            "The grid's frequencies must be finite and positive, with the maximum above the minimum.",
        ));
    }
    if let Timing::Discrete {
        sample_time_seconds,
    } = model.timing()
    {
        if grid.maximum_frequency >= PI / sample_time_seconds {
            return Err(input_failure(
                "The grid's maximum must be below π/Ts, the Nyquist frequency of this discrete model (N4.3).",
            ));
        }
    }
    let total = u64::from(grid.points)
        .saturating_mul(u64::try_from(channel_count(model)).unwrap_or(u64::MAX));
    if total > MAXIMUM_CHANNEL_FREQUENCY_POINTS {
        return Err(input_failure(
            "The grid's points times the model's channels must be at most 16,384 (N4.3).",
        ));
    }
    Ok(grid)
}

/// The poles and zeros of a model (N4.2): a transfer function's denominator and numerator roots, or the eigenvalues of a
/// state-space model's state matrix, which has no zeros here.
///
/// # Errors
/// Returns the failure of a root or eigenvalue calculation that does not converge.
pub fn model_poles_and_zeros(
    model: &GeneralModel,
) -> Result<(Vec<ComplexValue>, Vec<ComplexValue>), Failure> {
    match model {
        GeneralModel::TransferFunction { transfer, .. } => {
            let poles = if transfer.denominator.len() <= 1 {
                Vec::new()
            } else {
                roots_of_real_polynomial(&transfer.denominator).ok_or_else(roots_failure)?
            };
            let zeros = if transfer.numerator.len() <= 1 {
                Vec::new()
            } else {
                roots_of_real_polynomial(&transfer.numerator).ok_or_else(roots_failure)?
            };
            Ok((poles, zeros))
        }
        GeneralModel::ContinuousStateSpace { system, .. }
        | GeneralModel::DiscreteStateSpace { system, .. } => {
            let solution = eigenvalues(&system.a, &TOLERANCE).map_err(|_| {
                Failure::new(
                    "The poles of this model could not be calculated from its state matrix.",
                    Cause::ModelOrData,
                    NextAction::None,
                )
            })?;
            Ok((solution.values, Vec::new()))
        }
    }
}

/// Whether every pole is stable (N4.2): a continuous pole has a negative real part, a discrete pole a magnitude below one. A
/// pole on the boundary is marginal, so it is not stable.
#[must_use]
pub fn is_stable(model: &GeneralModel, poles: &[ComplexValue]) -> bool {
    let discrete = matches!(model, GeneralModel::DiscreteStateSpace { .. });
    poles.iter().all(|pole| {
        if discrete {
            pole.real.hypot(pole.imaginary) < 1.0
        } else {
            pole.real < 0.0
        }
    })
}

/// The answer of `frequency` (B9.9; N4.2, N4.3): the grid as used, the poles and zeros, the stability and the response of every
/// input-output pair, the inputs running fastest. `budget` is checked before the grid, the poles and each channel (A7).
///
/// # Errors
/// Returns the grid's input failure, or the failure of a calculation the model refuses: a response that is singular at a grid
/// frequency, a root or eigenvalue calculation that does not converge, or an exceeded budget or a cancellation.
pub fn frequency_answer(
    model: &GeneralModel,
    inputs: &AnalysisInputs,
    budget: &Budget<'_>,
) -> Result<FrequencyAnswer, Failure> {
    let grid = checked_grid(model, inputs.grid)?;
    budget.step()?;
    let points = usize::try_from(grid.points).unwrap_or(0);
    let frequencies = logarithmic_grid(
        grid.minimum_frequency,
        grid.maximum_frequency,
        points,
    )
    .map_err(|_| {
        input_failure("The grid's frequencies must be finite and positive, with the maximum above the minimum.")
    })?;
    budget.step()?;
    let (poles, zeros) = model_poles_and_zeros(model)?;
    let stable = is_stable(model, &poles);
    let identity = model.identity();
    let mut channels = Vec::new();
    for output in &identity.outputs {
        for input in &identity.inputs {
            budget.step()?;
            let pair = ChannelPair {
                input_id: input.id.clone(),
                output_id: output.id.clone(),
            };
            let values = channel_points(model, &pair, &frequencies).map_err(|refusal| {
                Failure::new(&refusal.reason(), Cause::ModelOrData, NextAction::None)
            })?;
            channels.push(FrequencyChannel {
                input: channel_ref(&input.id, &input.unit),
                output: channel_ref(&output.id, &output.unit),
                points: frequencies
                    .iter()
                    .zip(&values)
                    .map(|(frequency, value)| point(*frequency, *value))
                    .collect(),
            });
        }
    }
    let summary = channels
        .iter()
        .map(|channel| format!("Channel {} → {}.", channel.input.id, channel.output.id))
        .collect();
    Ok(FrequencyAnswer {
        grid,
        poles,
        zeros,
        stable,
        pole_scope: POLE_SCOPE.to_owned(),
        channels,
        summary,
        details: vec![format!("Pole scope: {POLE_SCOPE}")],
    })
}

/// One point of a response: the complex value, its magnitude in dB and its principal phase (N4.3).
fn point(frequency_rad_per_second: f64, value: ComplexValue) -> FrequencyPoint {
    let magnitude = value.real.hypot(value.imaginary);
    let phase = value.imaginary.atan2(value.real);
    FrequencyPoint {
        frequency_rad_per_second,
        real: value.real,
        imaginary: value.imaginary,
        magnitude,
        magnitude_db: 20.0 * magnitude.max(MINIMUM_MAGNITUDE).log10(),
        phase_radians: phase,
        phase_degrees: phase.to_degrees(),
    }
}

fn roots_failure() -> Failure {
    Failure::new(
        "The poles of this model could not be calculated.",
        Cause::ModelOrData,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "frequency_tests.rs"]
mod tests;
