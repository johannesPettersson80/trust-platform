//! The margins of a linear candidate on the selected model (B4.6, N7.7 to N7.9 of the Control Studio numerical
//! annex; C0 "Margins of linear candidates"). The crossovers and the closed-loop poles are the library's
//! (`trust_control_math::margins`); this module builds the loop of each law, reads it on the grid that the plots
//! use, and states why a margin is not available.
//!
//! The loop of a parallel law is `L = G·C` with `C = (kd·s² + kp·s + ki)/(s·(Tf·s + 1))`: the controller acts on
//! the filtered measurement, and the plant carries the model's delay (C0). A sampled state-feedback law is read on
//! its sampled model, broken at the plant input (N7.9). A switching or other nonlinear law has no margins.

#![allow(clippy::module_name_repetitions)] // `CandidateMargins` and `PidLoop` name the margins they belong to; the precedent is `model.rs`.

use std::f64::consts::{PI, TAU};

use schemars::JsonSchema;
use serde::Serialize;
use trust_control_math::discretize::{discretize_state_space, DiscretizationMethod};
use trust_control_math::frequency::{
    logarithmic_grid, response_at, sensitivity, FrequencyError, FrequencyModel,
};
use trust_control_math::identify::LowOrderModelKind;
use trust_control_math::linalg::ComplexValue;
use trust_control_math::margins::{analyze_loop, MarginError, MarginValue};
use trust_control_math::model::{StateSpace, Timing, TransferFunction};
use trust_control_math::polynomial::{convolve, trim_leading_zeros};

use crate::identification::plant::IdentifiedModel;
use crate::identification::refusal::Refusal;
use crate::tuning::prediction::{validate_plant, PredictedGains};
use crate::tuning::structure::is_exactly_zero;

/// The most points a continuous loop's logarithmic grid holds (`engineeringPlots.ts`).
const MAXIMUM_PLOT_POINTS: usize = 2_000;
/// The fewest points a continuous loop's logarithmic grid holds (`engineeringPlots.ts`).
const MINIMUM_PLOT_POINTS: usize = 100;
/// The most points a grid refined for a pure delay may hold; beyond it the margin is unavailable (N7.8).
const MAXIMUM_REFINED_POINTS: usize = 16_384;
/// The points of a sampled state-feedback loop's grid, from `1e-5` of the Nyquist frequency up to `0.999` of it (N7.9).
const STATE_FEEDBACK_POINTS: usize = 2_000;
/// The number of doublings and halvings that find the ends of a continuous loop's grid (`loopMagnitude`).
const GRID_EXPANSIONS: usize = 16;
/// The factor above the grid where a continuous loop's sensitivity is also read: a strictly proper loop's `|S|` tends to one there.
const FAR_FREQUENCY_FACTOR: f64 = 1.0e6;

/// The sentence for a filter time that is not finite or is negative.
const FILTER: &str = "The measurement filter time must be finite and not negative.";
/// The sentence for a frequency response that is refused.
const FREQUENCY: &str = "The loop's frequency response is refused at a grid point.";
/// The sentence for a frequency grid that is not valid.
const GRID: &str = "The frequency grid for the margins is not valid.";
/// The sentence for more than one gain crossover in the grid.
const AMBIGUOUS: &str =
    "More than one gain crossover lies in the grid, so the gain crossover is ambiguous.";
/// The sentence for a grid too coarse for a pure delay that cannot be refined within the limit.
const DELAY_TOO_FAST: &str = "The grid is too coarse for the pure delay (N7.8), so the margin estimate is unavailable until the range is narrowed.";
/// The sentence for a loop of unit magnitude at every frequency.
const DEGENERATE: &str =
    "The loop gain has magnitude one at every frequency, so its crossovers are undefined.";
/// The sentence for closed-loop poles that cannot be computed.
const POLES: &str = "The closed-loop poles could not be computed.";
/// The sentence for a delay that is not a whole number of sampling periods.
const NON_INTEGER_DELAY: &str =
    "The delay is not a whole number of sampling periods, so the sampled loop cannot carry it.";
/// The sentence for a loop that is singular at a grid point (`1 + L = 0`).
const SINGULAR: &str = "The loop is singular at a frequency on the grid, where 1 + L = 0.";
/// The sentence for feedback gains that do not match the model's states.
const GAIN_MISMATCH: &str = "The feedback gains do not match the model's states.";
/// The sentence for a model that cannot be sampled at the task period.
const SAMPLING: &str = "The model could not be sampled at the task period.";
/// The sentence for a switching or other nonlinear law, which shows no margins (N7.7).
const NONLINEAR: &str = "A switching or other nonlinear law shows no margins (N7.7).";

/// The open loop of a parallel law on the selected model (C0, N7.7): the plant, the controller with its
/// measurement filter, the loop with the model's delay, the grid that it is read on, and the frequency above the
/// grid where its sensitivity is also read. The plots read the same loop, so the margins and the curves agree.
#[derive(Debug, Clone, PartialEq)]
pub struct PidLoop {
    /// The plant with its delay: `G(s)` of the selected model.
    pub plant: FrequencyModel,
    /// The controller with the measurement filter: `C(s)` of the candidate.
    pub controller: FrequencyModel,
    /// The open loop `L = G·C`, with the plant's delay.
    pub open_loop: FrequencyModel,
    /// The frequencies, in rad/s and increasing, where the loop is read.
    pub frequencies: Vec<f64>,
    /// The frequency above the grid where the sensitivity is also read, or `None` for a sampled loop.
    pub far_frequency: Option<f64>,
}

/// The margins of a loop on its grid (B4.6, N7.7): the gain and phase margins with their crossovers, and the
/// peak sensitivity `Ms`. A margin the loop does not have is `Unbounded`; a margin that the grid does not show is
/// `NotObservedInRange` and never infinite (N7.8). When the crossover margins cannot be computed at all, their
/// values are `NotObservedInRange` and `unavailable` gives the sentence that says why.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CandidateMargins {
    /// The gain margin in dB (N7.7).
    pub gain_margin_db: MarginValue,
    /// The phase margin in degrees (N7.7).
    pub phase_margin_degrees: MarginValue,
    /// The gain-crossover frequency in rad/s, where `|L| = 1`, when the loop has one in the grid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_crossover_rad_per_second: Option<f64>,
    /// The phase-crossover frequency in rad/s, where the phase of `L` is −180°, when the grid shows one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase_crossover_rad_per_second: Option<f64>,
    /// The peak sensitivity `Ms = max |1/(1 + L)|`, over the grid and at the frequency above it (N7.7).
    pub peak_sensitivity: f64,
    /// The frequency in rad/s where `Ms` is attained.
    pub peak_sensitivity_rad_per_second: f64,
    /// The sentence that says why the crossover margins are not available, when they are not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
}

/// The open loop of a parallel law (C0, N7.7) on the selected model: the plant with its delay, the controller
/// with its measurement filter, their product, and the grid it is read on.
///
/// # Errors
/// Returns the sentence of the plant when the model is not a usable plant, the sentence of the filter time when it
/// is not finite or is negative, or the sentence of the grid when it cannot be formed.
pub fn pid_loop(model: &IdentifiedModel, gains: &PredictedGains) -> Result<PidLoop, Refusal> {
    validate_plant(model)?;
    if !(gains.measurement_filter_seconds.is_finite() && gains.measurement_filter_seconds >= 0.0) {
        return Err(Refusal::new(FILTER));
    }
    let delay = model.dead_time_seconds.unwrap_or(0.0);
    let plant = plant_transfer(model);
    let controller = controller_transfer(gains);
    let open_loop_transfer = TransferFunction {
        numerator: convolve(&plant.numerator, &controller.numerator),
        denominator: convolve(&plant.denominator, &controller.denominator),
    };
    let open_loop = FrequencyModel::Transfer {
        transfer: open_loop_transfer,
        timing: Timing::Continuous,
        delay_seconds: delay,
    };
    let (minimum, maximum) = grid_bounds(&open_loop, base_time(model), delay)?;
    let frequencies = continuous_grid(minimum, maximum, delay)?;
    Ok(PidLoop {
        plant: FrequencyModel::Transfer {
            transfer: plant,
            timing: Timing::Continuous,
            delay_seconds: delay,
        },
        controller: FrequencyModel::Transfer {
            transfer: controller,
            timing: Timing::Continuous,
            delay_seconds: 0.0,
        },
        open_loop,
        frequencies,
        far_frequency: Some(maximum * FAR_FREQUENCY_FACTOR),
    })
}

/// The margins of a parallel law on the selected model (B4.6): the margins of the loop of [`pid_loop`].
///
/// # Errors
/// Returns the sentence of [`pid_loop`], or the sentence of the sensitivity when a grid point is singular.
pub fn pid_margins(
    model: &IdentifiedModel,
    gains: &PredictedGains,
) -> Result<CandidateMargins, Refusal> {
    let loop_ = pid_loop(model, gains)?;
    loop_margins(&loop_.open_loop, &loop_.frequencies, loop_.far_frequency)
}

/// The margins of a sampled state-feedback law at the plant input (N5.14, N7.9): the loop `L(z) = K·(zI − A)⁻¹·B`
/// of the model sampled at the task period with the exact zero-order hold (N4.7(a)), read from zero up to `0.999`
/// of the Nyquist frequency `π/T`.
///
/// # Errors
/// Returns the sentence of the sampling when the model cannot be sampled, of the gains when their count is not the
/// number of states or they are not finite, or of the margins of the loop.
pub fn state_feedback_margins(
    plant: &StateSpace,
    period_seconds: f64,
    delay_seconds: f64,
    gains: &[f64],
) -> Result<CandidateMargins, Refusal> {
    let sampled = discretize_state_space(plant, period_seconds, DiscretizationMethod::ExactZoh)
        .map_err(|_| Refusal::new(SAMPLING))?;
    if gains.len() != sampled.system.states() || gains.iter().any(|gain| !gain.is_finite()) {
        return Err(Refusal::new(GAIN_MISMATCH));
    }
    let loop_system = StateSpace {
        a: sampled.system.a,
        b: sampled.system.b,
        c: vec![gains.to_vec()],
        d: vec![vec![0.0]],
    };
    let open_loop = FrequencyModel::StateSpace {
        system: loop_system,
        timing: Timing::Discrete {
            sample_time_seconds: period_seconds,
        },
        delay_seconds,
    };
    let nyquist = PI / period_seconds;
    let frequencies = logarithmic_grid(nyquist * 1e-5, nyquist * 0.999, STATE_FEEDBACK_POINTS)
        .map_err(|_| Refusal::new(GRID))?;
    loop_margins(&open_loop, &frequencies, None)
}

/// The margins and the peak sensitivity of a loop read on its grid (N7.7 and N7.8). The peak sensitivity is taken
/// over the grid and at `far_frequency` when there is one. The crossover margins are the library's; when the
/// library refuses them, they are reported as not available, with the reason.
///
/// # Errors
/// Returns the sentence of the sensitivity when a grid point is singular.
pub fn loop_margins(
    open_loop: &FrequencyModel,
    frequencies: &[f64],
    far_frequency: Option<f64>,
) -> Result<CandidateMargins, Refusal> {
    let (peak_sensitivity, peak_frequency) =
        peak_sensitivity(open_loop, frequencies, far_frequency)?;
    let mut margins = CandidateMargins {
        gain_margin_db: MarginValue::NotObservedInRange,
        phase_margin_degrees: MarginValue::NotObservedInRange,
        gain_crossover_rad_per_second: None,
        phase_crossover_rad_per_second: None,
        peak_sensitivity,
        peak_sensitivity_rad_per_second: peak_frequency,
        unavailable: None,
    };
    match analyze_loop(open_loop, frequencies) {
        Ok(report) => {
            margins.gain_margin_db = report.gain_margin_db;
            margins.phase_margin_degrees = report.phase_margin_degrees;
            margins.gain_crossover_rad_per_second = report.gain_crossover_rad_per_second;
            margins.phase_crossover_rad_per_second = report.phase_crossover_rad_per_second;
        }
        Err(error) => margins.unavailable = Some(margin_sentence(error).to_string()),
    }
    Ok(margins)
}

/// The refusal of a law that is not linear on the selected model (a switching law, or any other nonlinear law): it has
/// no margins, and this is the sentence that says why (N7.7).
#[must_use]
pub fn nonlinear_law_refusal() -> Refusal {
    Refusal::new(NONLINEAR)
}

/// The value of a single-input, single-output loop at a frequency.
///
/// # Errors
/// Returns the sentence of the frequency response when the value is refused.
pub fn loop_value(model: &FrequencyModel, frequency: f64) -> Result<ComplexValue, Refusal> {
    response_at(model, frequency)
        .map_err(frequency_sentence)?
        .first()
        .and_then(|row| row.first())
        .copied()
        .ok_or_else(|| Refusal::new(FREQUENCY))
}

/// The magnitude of a complex value.
#[must_use]
pub fn magnitude(value: ComplexValue) -> f64 {
    value.real.hypot(value.imaginary)
}

/// The largest `|S| = |1/(1 + L)|` over the grid and at the far frequency, and the frequency where it is attained.
fn peak_sensitivity(
    open_loop: &FrequencyModel,
    frequencies: &[f64],
    far_frequency: Option<f64>,
) -> Result<(f64, f64), Refusal> {
    let mut peak = (0.0_f64, 0.0_f64);
    for &frequency in frequencies.iter().chain(far_frequency.iter()) {
        let value =
            sensitivity(loop_value(open_loop, frequency)?).map_err(|_| Refusal::new(SINGULAR))?;
        let size = magnitude(value);
        if size > peak.0 {
            peak = (size, frequency);
        }
    }
    Ok(peak)
}

/// The numerator and denominator of the plant of a model, with the form's integrator and lags (C0, N0.4): a static
/// gain, an integrator, a first-order lag or a second-order lag, each with the static gain `K`.
#[must_use]
pub fn plant_transfer(model: &IdentifiedModel) -> TransferFunction {
    let integrator = if model.kind == LowOrderModelKind::Integrating {
        vec![1.0, 0.0]
    } else {
        vec![1.0]
    };
    let denominator = [
        model.time_constant_seconds,
        model.second_time_constant_seconds,
    ]
    .into_iter()
    .flatten()
    .fold(integrator, |product, time| convolve(&product, &[time, 1.0]));
    TransferFunction {
        numerator: vec![model.gain],
        denominator,
    }
}

/// The controller with its measurement filter: `(kd·s² + kp·s + ki)/(s·(Tf·s + 1))`, or `(kd·s + kp)/(Tf·s + 1)` when
/// the integral gain is exactly zero, as the positional form has it. The filter is omitted when its time is zero.
#[must_use]
pub fn controller_transfer(gains: &PredictedGains) -> TransferFunction {
    let positional = is_exactly_zero(gains.integral_gain);
    let numerator = if positional {
        trim_leading_zeros(&[gains.derivative_gain, gains.proportional_gain])
    } else {
        trim_leading_zeros(&[
            gains.derivative_gain,
            gains.proportional_gain,
            gains.integral_gain,
        ])
    };
    let integrator = if positional {
        vec![1.0]
    } else {
        vec![1.0, 0.0]
    };
    let denominator = if gains.measurement_filter_seconds > 0.0 {
        convolve(&integrator, &[gains.measurement_filter_seconds, 1.0])
    } else {
        integrator
    };
    TransferFunction {
        numerator,
        denominator,
    }
}

/// The time that scales the frequency grid: the first time constant, else the second, else one second, and never
/// below a microsecond (`engineeringPlots.ts`).
fn base_time(model: &IdentifiedModel) -> f64 {
    model
        .time_constant_seconds
        .or(model.second_time_constant_seconds)
        .unwrap_or(1.0)
        .max(1e-6)
}

/// The bounds of a continuous loop's grid, widened as `engineeringPlots.ts` widens them: the minimum starts at
/// `0.01/T` and the maximum at `max(10/T, 2π/L)`. The maximum doubles while the loop gain is above one, and the
/// minimum halves while it is below one, at most sixteen times each.
fn grid_bounds(
    open_loop: &FrequencyModel,
    base_time: f64,
    delay: f64,
) -> Result<(f64, f64), Refusal> {
    let mut minimum = 1e-2 / base_time;
    let mut maximum = (10.0 / base_time).max(if delay > 0.0 { TAU / delay } else { 0.0 });
    for _ in 0..GRID_EXPANSIONS {
        if magnitude(loop_value(open_loop, maximum)?) > 1.0 {
            maximum *= 2.0;
        } else {
            break;
        }
    }
    for _ in 0..GRID_EXPANSIONS {
        if magnitude(loop_value(open_loop, minimum)?) < 1.0 {
            minimum /= 2.0;
        } else {
            break;
        }
    }
    Ok((minimum, maximum))
}

/// The grid of a continuous loop: the logarithmic points of `engineeringPlots.ts`, then, where a pure delay would
/// advance the phase by more than a quarter of `π/2` between neighbouring points, a uniform grid at that spacing
/// from the first wide spacing up to the maximum (N7.8). A grid that would need more points than the limit is
/// refused, and the margin is then unavailable until the range is narrowed.
fn continuous_grid(minimum: f64, maximum: f64, delay: f64) -> Result<Vec<f64>, Refusal> {
    let logarithmic = logarithmic_grid(minimum, maximum, plot_point_count(minimum, maximum))
        .map_err(|_| Refusal::new(GRID))?;
    if delay > 0.0 {
        refine_for_delay(logarithmic, delay)
    } else {
        Ok(logarithmic)
    }
}

/// The number of points of a continuous loop's logarithmic grid: forty to the decade, at least 100 and at most 2000.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)] // The count is clamped to [100, 2000], which the float holds exactly, before the conversion.
fn plot_point_count(minimum: f64, maximum: f64) -> usize {
    let decades = (maximum / minimum).log10();
    ((decades * 40.0).ceil() + 1.0).clamp(MINIMUM_PLOT_POINTS as f64, MAXIMUM_PLOT_POINTS as f64)
        as usize
}

/// Refines a logarithmic grid so that no spacing exceeds a quarter of the limit `π/(2L)` of the pure delay (N7.8).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)] // The count is checked against the limit before the conversion.
fn refine_for_delay(grid: Vec<f64>, delay: f64) -> Result<Vec<f64>, Refusal> {
    let step = PI / (4.0 * delay);
    let wide_spacing = grid.windows(2).position(|pair| pair[1] - pair[0] > step);
    let Some(index) = wide_spacing else {
        return Ok(grid);
    };
    let (Some(&start), Some(&end)) = (grid.get(index), grid.last()) else {
        return Err(Refusal::new(GRID));
    };
    let count = ((end - start) / step).ceil();
    if !(count.is_finite() && count <= MAXIMUM_REFINED_POINTS as f64) {
        return Err(Refusal::new(DELAY_TOO_FAST));
    }
    let mut refined = grid[..=index].to_vec();
    for offset in 1..count as usize {
        let offset = f64::from(u32::try_from(offset).unwrap_or(u32::MAX));
        refined.push(start + offset * step);
    }
    refined.push(end);
    Ok(refined)
}

/// The sentence of a margin error of the library.
fn margin_sentence(error: MarginError) -> Refusal {
    Refusal::new(match error {
        MarginError::Frequency(FrequencyError::NonIntegerDiscreteDelay) => NON_INTEGER_DELAY,
        MarginError::Frequency(_) => FREQUENCY,
        MarginError::InvalidGrid => GRID,
        MarginError::AmbiguousGainCrossover => AMBIGUOUS,
        MarginError::DelayPhaseTooFast => DELAY_TOO_FAST,
        MarginError::DegenerateLoop => DEGENERATE,
        MarginError::PoleSolverFailed => POLES,
    })
}

/// The sentence of a frequency-response error.
fn frequency_sentence(error: FrequencyError) -> Refusal {
    match error {
        FrequencyError::NonIntegerDiscreteDelay => Refusal::new(NON_INTEGER_DELAY),
        FrequencyError::Singular => Refusal::new(SINGULAR),
        _ => Refusal::new(FREQUENCY),
    }
}

#[cfg(test)]
#[path = "candidate_margins_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "candidate_margins_identify_tune_tests.rs"]
mod identify_tune_tests;
