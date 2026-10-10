//! The free-running prediction of a low-order form under the recorded input (N1.2, N2.2, N3.1): the input is held between
//! samples, each sample interval is discretized exactly, and a delayed input is the latest recorded input at or before the
//! delayed time. The grid fit of `fit`, the scoring of a frozen form on another record and the evidence of a fitted model all
//! use these functions, so one prediction serves every path.

use super::response::{delay_into, LowOrderShape};
use super::LowOrderModelKind;

/// The relative difference of two time constants below which the second-order cascade uses its equal-lag limit (N2.1).
const EQUAL_LAG_TOLERANCE: f64 = 1e-9;

/// The decay factors `aₖ = e^(−Δtₖ/τ)` of every sample interval `Δtₖ = tₖ₊₁ − tₖ` of the record, for the time constant `τ`
/// (N1.2). A record of `n` samples has `n − 1` factors.
#[must_use]
pub(super) fn decay_factors(times: &[f64], time_constant_seconds: f64) -> Vec<f64> {
    times
        .windows(2)
        .map(|pair| (-(pair[1] - pair[0]) / time_constant_seconds).exp())
        .collect()
}

/// A time constant with its decay factors on the intervals of a record.
#[derive(Debug, Clone, Copy)]
pub(super) struct Lag<'a> {
    /// The time constant `τ` in seconds.
    pub(super) time_constant_seconds: f64,
    /// The decay factors of the record's intervals, from [`decay_factors`] for this time constant.
    pub(super) decay: &'a [f64],
}

/// The dynamics of a form, without its gain and dead time: a static gain, an integrator, a first-order lag, or the cascade
/// of two first-order lags (N2.1 items 1, 2, 4 and 5 and their dead-time forms).
#[derive(Debug, Clone, Copy)]
pub(super) enum Dynamics<'a> {
    /// `G(s) = 1`: the response follows the delayed input at once.
    Static,
    /// `G(s) = 1/s`: the response integrates the delayed input.
    Integrating,
    /// `G(s) = 1/(τs + 1)`.
    FirstOrder(Lag<'a>),
    /// `G(s) = 1/((τ₁s + 1)(τ₂s + 1))`, the two lags in cascade.
    SecondOrder {
        /// The lag that receives the input.
        first: Lag<'a>,
        /// The lag that receives the first lag's output.
        second: Lag<'a>,
    },
}

/// Writes the unit-gain response of `dynamics` to the delayed input `delayed` into `out`, one value per sample (N1.2).
///
/// The first sample responds with zero for a lag or an integrator, since the record starts at its steady baseline. A
/// static gain responds with the delayed input itself. A held input `uₖ` acts over `[tₖ, tₖ₊₁)`:
///
/// - integrator: `yₖ₊₁ = yₖ + Δtₖ·uₖ`;
/// - first-order lag: `yₖ₊₁ = aₖ·yₖ + (1 − aₖ)·uₖ`, with `aₖ = e^(−Δtₖ/τ)`;
/// - cascade: with `x` the first lag's output, `xₖ₊₁ = a₁ₖ·xₖ + (1 − a₁ₖ)·uₖ` and
///   `yₖ₊₁ = c·xₖ + a₂ₖ·yₖ + g·uₖ`, where `c = τ₁(a₁ₖ − a₂ₖ)/(τ₁ − τ₂)` and
///   `g = (τ₁(1 − a₁ₖ) − τ₂(1 − a₂ₖ))/(τ₁ − τ₂)`. Equal lags use the limit `c = (Δtₖ/τ)·a₁ₖ` and
///   `g = 1 − (1 + Δtₖ/τ)·a₁ₖ`.
///
/// Each formula is the exact solution of the two-state system over one held interval. `times`, `delayed` and `out` hold one
/// value per sample, and the decay factors of each lag hold one per interval.
pub(super) fn respond(dynamics: Dynamics<'_>, times: &[f64], delayed: &[f64], out: &mut [f64]) {
    match dynamics {
        Dynamics::Static => out.copy_from_slice(delayed),
        Dynamics::Integrating => integrate(times, delayed, out),
        Dynamics::FirstOrder(lag) => lag_response(lag.decay, delayed, out),
        Dynamics::SecondOrder { first, second } => {
            cascade_response(times, first, second, delayed, out);
        }
    }
}

/// The unit-gain response of an integrator to the delayed input over the record (N2.1 item 4).
fn integrate(times: &[f64], delayed: &[f64], out: &mut [f64]) {
    let mut state = 0.0;
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = state;
        if let (Some(&next), Some(&now), Some(&input)) =
            (times.get(index + 1), times.get(index), delayed.get(index))
        {
            state += (next - now) * input;
        }
    }
}

/// The unit-gain response of a first-order lag to the delayed input, with the exact decay of each interval (N1.2).
fn lag_response(decay: &[f64], delayed: &[f64], out: &mut [f64]) {
    let mut state = 0.0;
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = state;
        if let (Some(&factor), Some(&input)) = (decay.get(index), delayed.get(index)) {
            state = factor * state + (1.0 - factor) * input;
        }
    }
}

/// The unit-gain response of two first-order lags in cascade to the delayed input, with the exact two-state update of each
/// interval (N2.1 item 5, N1.2). The formulas are those of [`respond`].
fn cascade_response(
    times: &[f64],
    first: Lag<'_>,
    second: Lag<'_>,
    delayed: &[f64],
    out: &mut [f64],
) {
    let (first_tau, second_tau) = (first.time_constant_seconds, second.time_constant_seconds);
    let equal = (first_tau - second_tau).abs() <= EQUAL_LAG_TOLERANCE * first_tau.max(second_tau);
    let mut first_state = 0.0;
    let mut state = 0.0;
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = state;
        let (Some(&a), Some(&b), Some(&input), Some(&now), Some(&next)) = (
            first.decay.get(index),
            second.decay.get(index),
            delayed.get(index),
            times.get(index),
            times.get(index + 1),
        ) else {
            continue;
        };
        let (coupling, gain) = if equal {
            let ratio = (next - now) / first_tau;
            (ratio * a, 1.0 - (1.0 + ratio) * a)
        } else {
            let difference = first_tau - second_tau;
            (
                first_tau * (a - b) / difference,
                (first_tau * (1.0 - a) - second_tau * (1.0 - b)) / difference,
            )
        };
        state = coupling * first_state + b * state + gain * input;
        first_state = a * first_state + (1.0 - a) * input;
    }
}

/// The unit-gain response of a form with the given shape to the recorded plant input deviations `dₖ = uₖ − u₀` on the
/// recorded sample times (N2.2, N3.1). The shape's dead time delays the input (zero when missing); a missing time constant
/// reads as one and a missing second one as the first (N1.2).
#[must_use]
pub fn unit_response(
    kind: LowOrderModelKind,
    shape: LowOrderShape,
    times: &[f64],
    deviations: &[f64],
) -> Vec<f64> {
    let mut delayed = vec![0.0; times.len()];
    delay_into(
        times,
        deviations,
        shape.dead_time_seconds.unwrap_or(0.0),
        &mut delayed,
    );
    let first_tau = shape.time_constant_seconds.unwrap_or(1.0);
    let second_tau = shape.second_time_constant_seconds.unwrap_or(first_tau);
    let first_decay = decay_factors(times, first_tau);
    let second_decay = decay_factors(times, second_tau);
    let dynamics = match kind {
        LowOrderModelKind::StaticGain => Dynamics::Static,
        LowOrderModelKind::Integrating => Dynamics::Integrating,
        LowOrderModelKind::FirstOrder | LowOrderModelKind::FirstOrderDeadTime => {
            Dynamics::FirstOrder(Lag {
                time_constant_seconds: first_tau,
                decay: &first_decay,
            })
        }
        LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime => {
            Dynamics::SecondOrder {
                first: Lag {
                    time_constant_seconds: first_tau,
                    decay: &first_decay,
                },
                second: Lag {
                    time_constant_seconds: second_tau,
                    decay: &second_decay,
                },
            }
        }
    };
    let mut response = vec![0.0; times.len()];
    respond(dynamics, times, &delayed, &mut response);
    response
}

#[cfg(test)]
#[path = "simulate_tests.rs"]
mod tests;
