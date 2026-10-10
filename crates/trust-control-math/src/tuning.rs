//! The tuning formulas of Control Studio's numerical annex: SIMC PI for a self-regulating model
//! (N5.2), with SIMC's half rule for a second-order model with real poles, and for an integrating model
//! (N5.3), the response choice that sets `λ` (N5.4), AMIGO PI (N5.5), and the proportional gain of an
//! integrating model with a dead time (N5.8).
//!
//! The formulas run on gain magnitudes (N5.1). The sign that the controller action requires (N6) is
//! applied by the caller. Each formula returns its magnitudes, or a typed refusal (N5.7). The texts of
//! the refusals belong to the caller. The two-point reduction of N5.15 is in [`two_point`].

#![allow(clippy::module_name_repetitions)] // `TuningRefusal` names the module's subject.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod two_point;

/// The response choice of N5.4: the closed-loop time constant `λ` is the Faster value times 1, 2 or 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ResponseChoice {
    /// `λ` is the Faster value.
    Faster,
    /// `λ` is twice the Faster value.
    Balanced,
    /// `λ` is four times the Faster value.
    MoreRobust,
}

impl ResponseChoice {
    /// The factor that the choice applies to the Faster value (N5.4): 1, 2 or 4.
    #[must_use]
    pub const fn factor(self) -> f64 {
        match self {
            Self::Faster => 1.0,
            Self::Balanced => 2.0,
            Self::MoreRobust => 4.0,
        }
    }
}

/// A stable, monotone first-order-plus-delay model `K·exp(−L·s)/(T·s + 1)` (N5.2): the static gain
/// `K` in output units per input unit, and the time constant `T` and dead time `L` in seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FirstOrderPlusDelay {
    /// The static gain `K`.
    pub gain: f64,
    /// The time constant `T` in seconds.
    pub time_constant_seconds: f64,
    /// The dead time `L` in seconds.
    pub delay_seconds: f64,
}

/// An integrating model `K′·exp(−L·s)/s` (N5.3): the rate gain `K′` in output units per input unit per
/// second, and the effective dead time `L` in seconds. `K′` is never used as a static gain.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IntegratingModel {
    /// The rate gain `K′`.
    pub rate_gain: f64,
    /// The effective dead time `L` in seconds.
    pub delay_seconds: f64,
}

/// A second-order model with a dead time, `K·exp(−θ·s)·a0/(a2·s² + a1·s + a0)` (N5.2): the static gain `K`, the
/// denominator's coefficients from the highest power down, and the dead time `θ` in seconds. The model
/// `K·exp(−θ·s)/((τ1·s + 1)(τ2·s + 1))` has `a2 = τ1·τ2`, `a1 = τ1 + τ2` and `a0 = 1`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecondOrderPlusDelay {
    /// The static gain `K`.
    pub gain: f64,
    /// The coefficient `a2` of `s²`.
    pub leading: f64,
    /// The coefficient `a1` of `s`.
    pub linear: f64,
    /// The constant coefficient `a0`.
    pub constant: f64,
    /// The dead time `θ` in seconds.
    pub delay_seconds: f64,
}

/// The ideal (ISA) PI gain magnitudes of N0.5 and N5.6: the proportional gain `Kp` and the integral
/// time `Ti` in seconds. The parallel integral gain is `Ki = Kp / Ti`, and the derivative gain is zero.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PiGains {
    /// The proportional gain `Kp`, as a magnitude.
    pub proportional_gain: f64,
    /// The integral time `Ti` in seconds.
    pub integral_time_seconds: f64,
}

impl PiGains {
    /// The parallel integral gain `Ki = Kp / Ti` (N5.2, N5.6).
    #[must_use]
    pub fn integral_gain(self) -> f64 {
        self.proportional_gain / self.integral_time_seconds
    }
}

/// Why a tuning formula creates no candidate: the refusals of N5.7 and the method's own domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TuningRefusal {
    /// The plant gain is zero (N5.7).
    #[error("the plant gain is zero")]
    ZeroGain,
    /// A parameter is not finite (N5.7).
    #[error("a parameter is not finite")]
    NonfiniteParameter,
    /// A required time constant is not positive (N5.7).
    #[error("a required time constant is not positive")]
    NonpositiveTimeConstant,
    /// The dead time is negative (N5.7).
    #[error("the dead time is negative")]
    InvalidDelay,
    /// The normalized error of the model is above its limit (N5.7, N3.2).
    #[error("the model's normalized error is above its limit")]
    ModelErrorAboveLimit,
    /// The model structure does not fit the method (N5.7).
    #[error("the model structure is not supported by this method")]
    UnsupportedStructure,
    /// The second-order model has complex poles, so SIMC's half rule does not reduce it (N5.2, N5.7).
    #[error("the second-order model has complex poles")]
    ComplexPoles,
    /// The second-order model is not stable: a coefficient of its denominator is not positive (N5.2, N5.7).
    #[error("the second-order model is not stable")]
    NotStable,
    /// An integrating model without a positive effective dead time has no response-time scale (N5.4).
    #[error(
        "an integrating model without a positive effective dead time has no response-time scale"
    )]
    NoResponseTimeScale,
    /// AMIGO needs a positive dead time (N5.5).
    #[error("AMIGO needs a positive dead time")]
    AmigoNeedsPositiveDelay,
    /// AMIGO's domain is `0 < L/T ≤ 1` (N5.5).
    #[error("the dead time over the time constant is above one, which is outside AMIGO's domain")]
    AmigoDelayRatioAboveOne,
    /// The formula gives a setting that is not finite and positive (N5.7).
    #[error("the formula does not give finite positive PI settings")]
    NonfiniteResult,
}

/// The model error check of N5.7: a normalized error (N3.2) above its limit creates no candidate.
///
/// # Errors
/// Returns [`TuningRefusal::NonfiniteParameter`] when the error or the limit is not finite, and
/// [`TuningRefusal::ModelErrorAboveLimit`] when the error is above `maximum`.
pub fn check_model_error(normalized_error: f64, maximum: f64) -> Result<(), TuningRefusal> {
    if !(normalized_error.is_finite() && maximum.is_finite()) {
        return Err(TuningRefusal::NonfiniteParameter);
    }
    if normalized_error > maximum {
        return Err(TuningRefusal::ModelErrorAboveLimit);
    }
    Ok(())
}

/// The Faster closed-loop time scale of a self-regulating model: `max(L, T/3)` (N5.4).
#[must_use]
pub fn self_regulating_faster_scale(time_constant_seconds: f64, delay_seconds: f64) -> f64 {
    delay_seconds.max(time_constant_seconds / 3.0)
}

/// The closed-loop time constant `λ` of a response choice: the Faster value times the choice's
/// factor (N5.4).
#[must_use]
pub fn closed_loop_time_constant(faster_seconds: f64, choice: ResponseChoice) -> f64 {
    faster_seconds * choice.factor()
}

/// SIMC PI for a stable, monotone first-order-plus-delay model (N5.2), with `λ` from the response
/// choice (N5.4): `Kp = T / (|K|(λ + L))`, `Ti = min(T, 4(λ + L))`, `Ki = Kp / Ti`, `Kd = 0`.
///
/// # Errors
/// Returns [`TuningRefusal::ZeroGain`] for a zero gain, [`TuningRefusal::NonfiniteParameter`] for a
/// parameter that is not finite, [`TuningRefusal::NonpositiveTimeConstant`] for `T ≤ 0`,
/// [`TuningRefusal::InvalidDelay`] for `L < 0`, and [`TuningRefusal::NonfiniteResult`] when the
/// settings are not finite and positive.
pub fn simc_pi_self_regulating(
    model: FirstOrderPlusDelay,
    choice: ResponseChoice,
) -> Result<PiGains, TuningRefusal> {
    let gain = magnitude_of_gain(model.gain)?;
    check_time_constant(model.time_constant_seconds)?;
    check_delay(model.delay_seconds)?;
    let faster = self_regulating_faster_scale(model.time_constant_seconds, model.delay_seconds);
    let horizon = closed_loop_time_constant(faster, choice) + model.delay_seconds;
    positive_pi(
        model.time_constant_seconds / (gain * horizon),
        model.time_constant_seconds.min(4.0 * horizon),
    )
}

/// The rounding allowance of the discriminant of [`half_rule`], relative to `σ²` where `σ = a1/a0`: a discriminant below
/// minus this is complex, and one within it is the rounding of a pair of equal time constants.
const DISCRIMINANT_ROUNDING: f64 = 8.0 * f64::EPSILON;

/// SIMC's half rule (N5.2, Skogestad 2003): a stable second-order model with real poles and a dead time, reduced to a
/// first-order-plus-delay model with `T = τ1 + τ2/2` and `L = θ + τ2/2`, where `τ1 ≥ τ2 > 0` are the time constants of its
/// poles. With the denominator normalized to a constant of one, `τ1 = (σ + √Δ)/2` and `τ2 = (σ − √Δ)/2`, where
/// `σ = a1/a0`, `Δ = σ² − 4·a2/a0`, and `τ1·τ2 = a2/a0`. The gain `K` is kept with its sign.
///
/// A discriminant below the rounding of its terms counts as zero, so that a pair of equal time constants is not refused
/// as complex poles.
///
/// # Errors
/// Returns [`TuningRefusal::ZeroGain`] for a zero gain, [`TuningRefusal::NonfiniteParameter`] for a value that is not
/// finite, [`TuningRefusal::InvalidDelay`] for a dead time below zero, [`TuningRefusal::NotStable`] when a coefficient of
/// the denominator is not positive, and [`TuningRefusal::ComplexPoles`] when the poles are complex.
pub fn half_rule(model: SecondOrderPlusDelay) -> Result<FirstOrderPlusDelay, TuningRefusal> {
    // The gain is checked as N5.7 states; its sign is kept for the caller (N6.4).
    magnitude_of_gain(model.gain)?;
    check_delay(model.delay_seconds)?;
    let coefficients = [model.leading, model.linear, model.constant];
    if !coefficients.iter().all(|value| value.is_finite()) {
        return Err(TuningRefusal::NonfiniteParameter);
    }
    if !coefficients.iter().all(|value| *value > 0.0) {
        return Err(TuningRefusal::NotStable);
    }
    let sum = model.linear / model.constant;
    let product = model.leading / model.constant;
    let discriminant = sum * sum - 4.0 * product;
    if discriminant < -DISCRIMINANT_ROUNDING * sum * sum {
        return Err(TuningRefusal::ComplexPoles);
    }
    let slower = f64::midpoint(sum, discriminant.max(0.0).sqrt());
    let faster = product / slower;
    Ok(FirstOrderPlusDelay {
        gain: model.gain,
        time_constant_seconds: slower + faster / 2.0,
        delay_seconds: model.delay_seconds + faster / 2.0,
    })
}

/// SIMC PI for an accepted integrating model (N5.3), with `λ` from the response choice (N5.4): the
/// Faster value is the effective dead time. `Kp = 1/(|K′|(λ + L))`, `Ti = 4(λ + L)`, `Ki = Kp / Ti`,
/// `Kd = 0`.
///
/// # Errors
/// Returns [`TuningRefusal::ZeroGain`], [`TuningRefusal::NonfiniteParameter`] and
/// [`TuningRefusal::InvalidDelay`] for the parameters, [`TuningRefusal::NoResponseTimeScale`] when the
/// effective dead time is not positive, and [`TuningRefusal::NonfiniteResult`] for a result that is not
/// finite and positive.
pub fn simc_pi_integrating(
    model: IntegratingModel,
    choice: ResponseChoice,
) -> Result<PiGains, TuningRefusal> {
    let rate = magnitude_of_gain(model.rate_gain)?;
    check_delay(model.delay_seconds)?;
    if model.delay_seconds <= 0.0 {
        return Err(TuningRefusal::NoResponseTimeScale);
    }
    let horizon = closed_loop_time_constant(model.delay_seconds, choice) + model.delay_seconds;
    positive_pi(1.0 / (rate * horizon), 4.0 * horizon)
}

/// The proportional gain of an accepted integrating model with a positive dead time (N5.8): the SIMC
/// proportional gain `Kp = 1/(|K′|(λ + L))` with the response choices of N5.4. The gain is a magnitude.
///
/// # Errors
/// The same refusals as [`simc_pi_integrating`].
pub fn integrating_proportional_gain(
    model: IntegratingModel,
    choice: ResponseChoice,
) -> Result<f64, TuningRefusal> {
    simc_pi_integrating(model, choice).map(|gains| gains.proportional_gain)
}

/// AMIGO PI for a stable, monotone first-order-plus-delay model with `0 < L/T ≤ 1` (N5.5):
/// `Kp = 0.15/|K| + (0.35 − L·T/(L + T)²)·T/(|K|·L)` and
/// `Ti = 0.35·L + 13·L·T²/(T² + 12·L·T + 7·L²)`, with `Ki = Kp / Ti` and `Kd = 0`. AMIGO has no
/// response-speed parameter, so no choice is taken. It is not applicable to integrating models.
///
/// # Errors
/// Returns the parameter refusals of [`simc_pi_self_regulating`], [`TuningRefusal::AmigoNeedsPositiveDelay`]
/// for `L = 0`, [`TuningRefusal::AmigoDelayRatioAboveOne`] for `L/T > 1`, and
/// [`TuningRefusal::NonfiniteResult`] for a result that is not finite and positive.
pub fn amigo_pi(model: FirstOrderPlusDelay) -> Result<PiGains, TuningRefusal> {
    let gain = magnitude_of_gain(model.gain)?;
    check_time_constant(model.time_constant_seconds)?;
    check_delay(model.delay_seconds)?;
    let time_constant = model.time_constant_seconds;
    let delay = model.delay_seconds;
    if delay <= 0.0 {
        return Err(TuningRefusal::AmigoNeedsPositiveDelay);
    }
    if delay / time_constant > 1.0 {
        return Err(TuningRefusal::AmigoDelayRatioAboveOne);
    }
    let sum = delay + time_constant;
    let proportional_gain =
        0.15 / gain + (0.35 - delay * time_constant / (sum * sum)) * time_constant / (gain * delay);
    let integral_time = 0.35 * delay
        + 13.0 * delay * time_constant * time_constant
            / (time_constant * time_constant + 12.0 * delay * time_constant + 7.0 * delay * delay);
    positive_pi(proportional_gain, integral_time)
}

/// The gain magnitude of a model, refused when it is not finite or is zero (N5.7).
fn magnitude_of_gain(gain: f64) -> Result<f64, TuningRefusal> {
    if !gain.is_finite() {
        return Err(TuningRefusal::NonfiniteParameter);
    }
    if gain == 0.0 {
        return Err(TuningRefusal::ZeroGain);
    }
    Ok(gain.abs())
}

/// A required time constant must be finite and positive (N5.7).
fn check_time_constant(time_constant_seconds: f64) -> Result<(), TuningRefusal> {
    if !time_constant_seconds.is_finite() {
        return Err(TuningRefusal::NonfiniteParameter);
    }
    if time_constant_seconds <= 0.0 {
        return Err(TuningRefusal::NonpositiveTimeConstant);
    }
    Ok(())
}

/// A dead time must be finite and not negative (N5.7).
fn check_delay(delay_seconds: f64) -> Result<(), TuningRefusal> {
    if !delay_seconds.is_finite() {
        return Err(TuningRefusal::NonfiniteParameter);
    }
    if delay_seconds < 0.0 {
        return Err(TuningRefusal::InvalidDelay);
    }
    Ok(())
}

/// The PI settings when both are finite and positive.
fn positive_pi(
    proportional_gain: f64,
    integral_time_seconds: f64,
) -> Result<PiGains, TuningRefusal> {
    if proportional_gain.is_finite()
        && proportional_gain > 0.0
        && integral_time_seconds.is_finite()
        && integral_time_seconds > 0.0
    {
        Ok(PiGains {
            proportional_gain,
            integral_time_seconds,
        })
    } else {
        Err(TuningRefusal::NonfiniteResult)
    }
}

#[cfg(test)]
mod tests;
