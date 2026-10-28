//! The two-point first-order-plus-delay reduction of a settled step response (N5.15; the same rule as
//! C7 step 2 of the Control Studio specification). Its times are read by linear interpolation between
//! samples.

#![allow(clippy::module_name_repetitions)] // `TwoPointReduction` names the module's subject.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The fraction of the final change at which `t28` is read (N5.15: 28.3 %).
const LOW_FRACTION: f64 = 0.283;
/// The fraction of the final change at which `t63` is read (N5.15: 63.2 %).
const HIGH_FRACTION: f64 = 0.632;
/// A final change at or below this magnitude has no steady-state gain to reduce.
const NO_CHANGE_TOLERANCE: f64 = 1e-12;

/// The first-order-plus-delay model `K·exp(−L·s)/(τ·s + 1)` of a step response (N5.15).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TwoPointReduction {
    /// The gain `K`: the final change per unit input step.
    pub gain: f64,
    /// The time constant `τ = 1.5·(t63 − t28)` in seconds, or `t63` when the delay would be negative.
    pub time_constant_seconds: f64,
    /// The dead time `L = t63 − τ` in seconds, or zero when that would be negative.
    pub delay_seconds: f64,
}

/// Why a step response cannot be reduced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ReductionRefusal {
    /// The times and the responses differ in length, or there are fewer than two samples.
    #[error("the times and responses differ in length or have fewer than two samples")]
    Shape,
    /// A time or a response is not finite, or the times do not increase.
    #[error("a time or response is not finite, or the times do not increase")]
    InvalidSamples,
    /// The response has no steady-state change to reduce.
    #[error("the step response has no steady-state change")]
    NoSteadyChange,
    /// The response does not reach a required fraction of its final change after its first sample.
    #[error("the step response does not reach the required fraction of its final change")]
    NotReached,
}

/// The two-point reduction (N5.15). With `t28` and `t63` the times at which the response first reaches
/// 28.3 % and 63.2 % of its final change, `τ = 1.5·(t63 − t28)` and `L = t63 − τ`. When `L < 0`, the
/// reduction uses `L = 0` and `τ = t63`. The gain is the final change, `K = y(end)`, for a unit step.
///
/// # Errors
/// Returns [`ReductionRefusal::Shape`] or [`ReductionRefusal::InvalidSamples`] for unusable samples,
/// [`ReductionRefusal::NoSteadyChange`] when the final change is zero, and
/// [`ReductionRefusal::NotReached`] when a fraction is not reached after the first sample.
pub fn two_point_reduction(
    times: &[f64],
    responses: &[f64],
) -> Result<TwoPointReduction, ReductionRefusal> {
    validate(times, responses)?;
    let final_change = *responses.last().ok_or(ReductionRefusal::Shape)?;
    if final_change.abs() <= NO_CHANGE_TOLERANCE {
        return Err(ReductionRefusal::NoSteadyChange);
    }
    let time_low = crossing_time(times, responses, LOW_FRACTION * final_change)?;
    let time_high = crossing_time(times, responses, HIGH_FRACTION * final_change)?;
    let time_constant = 1.5 * (time_high - time_low);
    let delay = time_high - time_constant;
    if delay < 0.0 {
        Ok(TwoPointReduction {
            gain: final_change,
            time_constant_seconds: time_high,
            delay_seconds: 0.0,
        })
    } else {
        Ok(TwoPointReduction {
            gain: final_change,
            time_constant_seconds: time_constant,
            delay_seconds: delay,
        })
    }
}

/// Checks that the samples have one time and one response each, at least two, all finite, and with
/// increasing times.
fn validate(times: &[f64], responses: &[f64]) -> Result<(), ReductionRefusal> {
    if times.len() != responses.len() || times.len() < 2 {
        return Err(ReductionRefusal::Shape);
    }
    if !times.iter().chain(responses).all(|value| value.is_finite()) {
        return Err(ReductionRefusal::InvalidSamples);
    }
    if times.windows(2).any(|pair| pair[1] <= pair[0]) {
        return Err(ReductionRefusal::InvalidSamples);
    }
    Ok(())
}

/// The time at which the response first reaches `target` in magnitude, interpolated linearly between
/// the two samples that bracket it. A response already at the target at its first sample has no
/// crossing to read.
fn crossing_time(times: &[f64], responses: &[f64], target: f64) -> Result<f64, ReductionRefusal> {
    let index = responses
        .iter()
        .position(|value| value.abs() >= target.abs())
        .ok_or(ReductionRefusal::NotReached)?;
    if index == 0 {
        return Err(ReductionRefusal::NotReached);
    }
    let (time_before, time_after) = (times[index - 1], times[index]);
    let (value_before, value_after) = (responses[index - 1], responses[index]);
    Ok(time_before
        + (time_after - time_before) * (target - value_before) / (value_after - value_before))
}

#[cfg(test)]
mod tests;
