//! The responses that identification compares with measured data (N2.2, N2.3): the unit-gain response of a held input
//! on the recorded sample times, the least-squares gain of a response, and the fraction of the final change a held
//! response has reached.

use super::ceil_share;
use crate::sampling::statistics::mean;

/// The dynamic shape of a low-order model: its time constants and dead time, without its gain. A parameter the
/// family does not have is `None`. The response reads a missing time constant as one and a missing dead time as
/// zero.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[allow(clippy::struct_field_names)] // The fields are the names of the model fit's fields.
pub struct LowOrderShape {
    /// The time constant `τ` in seconds, or the first time constant of a second-order form.
    pub time_constant_seconds: Option<f64>,
    /// The second time constant `τ₂` in seconds, for a second-order form.
    pub second_time_constant_seconds: Option<f64>,
    /// The dead time `L` in seconds.
    pub dead_time_seconds: Option<f64>,
}

/// The shape of a held-input model whose unit-gain response is simulated on the recorded sample times (N2.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeldShape {
    /// A static gain: the response follows the delayed input at once.
    StaticGain,
    /// An integrator with a dead time: the response rises at the delayed input's value.
    Integrating {
        /// The dead time `L` in seconds.
        delay_seconds: f64,
    },
    /// A first-order lag `1/(τs + 1)` with a dead time `L`.
    FirstOrder {
        /// The time constant `τ` in seconds.
        time_constant_seconds: f64,
        /// The dead time `L` in seconds.
        delay_seconds: f64,
    },
}

/// The unit-gain response to input deviations recorded at `times`, each held until the next sample (A4.3, N2.2).
///
/// The delayed input `uₖ` is the latest deviation at or before `tₖ − L` (zero before the first sample). A static
/// gain responds with `uₖ`. A first-order lag advances per interval with the exact discretization
/// `yₖ = aₖyₖ₋₁ + (1 − aₖ)uₖ₋₁` where `aₖ = e^(−Δtₖ/τ)`, and an integrator with `yₖ = yₖ₋₁ + Δtₖuₖ₋₁`. The first
/// sample responds with zero. `deviations` must hold one value per time.
#[must_use]
pub fn held_unit_response(times: &[f64], deviations: &[f64], shape: HeldShape) -> Vec<f64> {
    match shape {
        HeldShape::StaticGain => delayed_deviations(times, deviations, 0.0),
        HeldShape::Integrating { delay_seconds } => {
            let delayed = delayed_deviations(times, deviations, delay_seconds);
            let mut response = vec![0.0; times.len()];
            for (index, pair) in times.windows(2).enumerate() {
                let dt = pair[1] - pair[0];
                response[index + 1] = response[index] + dt * delayed[index];
            }
            response
        }
        HeldShape::FirstOrder {
            time_constant_seconds,
            delay_seconds,
        } => {
            let delayed = delayed_deviations(times, deviations, delay_seconds);
            let mut response = vec![0.0; times.len()];
            for (index, pair) in times.windows(2).enumerate() {
                let dt = pair[1] - pair[0];
                let decay = (-dt / time_constant_seconds).exp();
                response[index + 1] = decay * response[index] + (1.0 - decay) * delayed[index];
            }
            response
        }
    }
}

/// The tolerance, in seconds, with which a sample at the delayed time still counts as delayed (`TIME_TOLERANCE`).
const TIME_TOLERANCE: f64 = 1e-9;

/// The deviations delayed by `delay_seconds`, written into `values` with one entry per time: entry `k` is the latest
/// deviation whose time is at or before `times[k] − delay` (within the tolerance), and zero before the first sample. The
/// delayed input of the recorded simulation is this series (N1.2).
pub(super) fn delay_into(
    times: &[f64],
    deviations: &[f64],
    delay_seconds: f64,
    values: &mut [f64],
) {
    // The number of samples whose time is at or before the delayed time: the latest one is `available − 1`.
    let mut available = 0;
    for (index, &time) in times.iter().enumerate() {
        while available < times.len() && times[available] <= time - delay_seconds + TIME_TOLERANCE {
            available += 1;
        }
        values[index] = if available > 0 {
            at(deviations, available - 1)
        } else {
            0.0
        };
    }
}

/// The deviations delayed by `delay_seconds`, one per time (see [`delay_into`]).
fn delayed_deviations(times: &[f64], deviations: &[f64], delay_seconds: f64) -> Vec<f64> {
    let mut values = vec![0.0; times.len()];
    delay_into(times, deviations, delay_seconds, &mut values);
    values
}

/// The value at `index`, or `NaN` past the end (a missing value in arithmetic, as the `TypeScript` yields `undefined`).
fn at(values: &[f64], index: usize) -> f64 {
    values.get(index).copied().unwrap_or(f64::NAN)
}

/// A least-squares gain and the squared error it leaves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeastSquaresGain {
    /// The gain `K = Σ(yₖ − y₀)sₖ / Σsₖ²`; `NaN` when the response has no energy.
    pub gain: f64,
    /// The squared error `Σ(yₖ − y₀ − K·sₖ)²` the gain leaves.
    pub squared_error: f64,
}

/// The least-squares gain of a unit-gain response `sₖ` against the measured outputs `yₖ` above their baseline `y₀`
/// (N2.2), with the squared error it leaves. The gain is `NaN` when `Σsₖ²` is not above machine epsilon.
#[must_use]
pub fn least_squares_gain(measured: &[f64], baseline: f64, response: &[f64]) -> LeastSquaresGain {
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for (index, value) in measured.iter().enumerate() {
        let unit = at(response, index);
        numerator += (value - baseline) * unit;
        denominator += unit * unit;
    }
    let gain = if denominator > f64::EPSILON {
        numerator / denominator
    } else {
        f64::NAN
    };
    let squared_error = measured
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let residual = value - baseline - gain * at(response, index);
            residual * residual
        })
        .sum::<f64>();
    LeastSquaresGain {
        gain,
        squared_error,
    }
}

/// The fraction of its fitted final change that a held response has reached (N2.3, A10.3):
/// `f = (ȳ_end − y₀) / (K·Δu)`, where `ȳ_end` is the mean of the last 5 % of the samples, and at least three of them
/// so a single noisy sample does not decide the fraction. `final_change` is the fitted `K·Δu`.
#[must_use]
pub fn reached_fraction(measured: &[f64], baseline: f64, final_change: f64) -> f64 {
    let tail_length = ceil_share(measured.len(), 0.05).max(3);
    let tail = &measured[measured.len().saturating_sub(tail_length)..];
    (mean(tail) - baseline) / final_change
}

/// The step amplitude `Δu = ū_hold − u₀` (N2.2): the mean plant input over the hold, from the step to the end of the
/// hold, less the mean plant input over the steady baseline. Every fit and every fraction of a held step uses this one
/// amplitude. `hold_inputs` holds the plant input of each hold sample, the step sample first.
#[must_use]
pub fn step_amplitude(baseline_input: f64, hold_inputs: &[f64]) -> f64 {
    mean(hold_inputs) - baseline_input
}

#[cfg(test)]
mod tests;
