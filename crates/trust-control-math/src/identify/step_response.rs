//! The arithmetic of a held-step identification (N2.2, N2.4–N2.6, N3.2): the statistics of a baseline or final
//! window, the search lattices and their size limits, the gains two windows allow, the normalized error, the
//! detectable onset, the first-order grid search and the parameter set that agrees with a measurement bound
//! (`stepResponseIdentification.ts`). The refusal sentences belong to the product: a refusal here is a typed error.

// `StepResponseWindow` keeps the name of the step-response record.
#![allow(clippy::module_name_repetitions)]

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::response::{held_unit_response, least_squares_gain, HeldShape};
use super::{count_f64, ParameterInterval};
use crate::sampling::statistics::mean;
use crate::sampling::{largest, smallest};

/// The most intervals a search lattice may span (`MAX_GRID_POINTS`): a lattice holds at most this many points plus
/// one.
pub const MAXIMUM_GRID_POINTS: usize = 1_000_000;

/// The most simulated responses a first-order grid search may evaluate, counted as delays × time constants × held
/// samples (`MAX_GRID_EVALUATIONS`).
pub const MAXIMUM_GRID_EVALUATIONS: usize = 100_000_000;

/// The tolerance of the step-response arithmetic, in seconds for times and in output units for values (`EPSILON`).
const EPSILON: f64 = 1e-9;

/// Why the statistics of a window cannot be taken (N2.6): a window error tells missing coverage from too few
/// samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WindowError {
    /// The window holds fewer than two samples.
    #[error("the window holds fewer than two samples")]
    TooFewSamples,
    /// The first sample comes a whole sample time or more after the window's start, or the last one more than a
    /// sample time before its end: an expected sampling instant is missing.
    #[error("the window lacks a sample near its start or its end")]
    Coverage,
}

/// The statistics of the measured output over a baseline or final window (N2.6, B3.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StepResponseWindow {
    /// The requested start of the window in seconds.
    pub start_seconds: f64,
    /// The requested end of the window in seconds; the window excludes it.
    pub end_seconds: f64,
    /// The number of samples in the window.
    pub count: usize,
    /// The mean measured output over the window.
    pub mean: f64,
    /// The smallest measured output in the window.
    pub minimum: f64,
    /// The largest measured output in the window.
    pub maximum: f64,
    /// The largest less the smallest measured output.
    pub range: f64,
    /// The change from the first to the last measured output over the time between them, per second; zero when the
    /// two samples share a time.
    pub slope_per_second: f64,
}

/// The statistics of the measured output over the window `[start, end)` (N2.6; `window` in
/// `stepResponseIdentification.ts`).
///
/// A sample belongs to the window when `start − ε ≤ tₖ < end − ε`, with `ε = 10⁻⁹` s. The window must hold at least
/// two samples, and its coverage must include every expected sampling instant: with the sample time `Tₛ`, the first
/// sample must come before `start + Tₛ − ε` and the last at or after `end − Tₛ − ε`. A start between sampling
/// instants is therefore permitted. The statistics are the mean, the minimum, the maximum and the range of the
/// outputs, and the slope `(y_last − y_first)/(t_last − t_first)`, which is zero when the two times are equal.
/// `times` and `outputs` hold one value per sample.
///
/// # Errors
/// Returns [`WindowError::TooFewSamples`] when fewer than two samples lie in the window, and
/// [`WindowError::Coverage`] when its first sample is too late or its last one too early.
pub fn window_statistics(
    times: &[f64],
    outputs: &[f64],
    start_seconds: f64,
    end_seconds: f64,
    sample_time_seconds: f64,
) -> Result<StepResponseWindow, WindowError> {
    let (window_times, values): (Vec<f64>, Vec<f64>) = times
        .iter()
        .zip(outputs)
        .filter(|&(&time, _)| time >= start_seconds - EPSILON && time < end_seconds - EPSILON)
        .map(|(&time, &output)| (time, output))
        .unzip();
    let (&[first_time, .., last_time], &[first_value, .., last_value]) =
        (window_times.as_slice(), values.as_slice())
    else {
        return Err(WindowError::TooFewSamples);
    };
    if first_time >= start_seconds + sample_time_seconds - EPSILON
        || last_time < end_seconds - sample_time_seconds - EPSILON
    {
        return Err(WindowError::Coverage);
    }
    let minimum = smallest(&values);
    let maximum = largest(&values, f64::NEG_INFINITY);
    let duration = last_time - first_time;
    let slope_per_second = if duration > 0.0 {
        (last_value - first_value) / duration
    } else {
        0.0
    };
    Ok(StepResponseWindow {
        start_seconds,
        end_seconds,
        count: values.len(),
        mean: mean(&values),
        minimum,
        maximum,
        range: maximum - minimum,
        slope_per_second,
    })
}

/// Why a search grid is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GridError {
    /// A lattice has no finite size or more than [`MAXIMUM_GRID_POINTS`] intervals, or a search would evaluate more
    /// than [`MAXIMUM_GRID_EVALUATIONS`] responses.
    #[error("the search grid is too large")]
    TooLarge,
}

/// The number of whole steps from `minimum` to `maximum` (`gridPointCount` in `stepResponseIdentification.ts`):
/// with `r = (maximum − minimum)/step`, the count is `max(0, ⌊r + 10⁻⁹⌋)` when `r` is finite and at least `−10⁻⁹`,
/// and `None` otherwise. A count beyond `usize::MAX` saturates there, which is above every grid limit.
#[must_use]
pub fn grid_point_count(minimum: f64, maximum: f64, step: f64) -> Option<usize> {
    let raw = (maximum - minimum) / step;
    (raw.is_finite() && raw >= -EPSILON).then(|| whole_count((raw + EPSILON).floor().max(0.0)))
}

/// The search lattice `minimum + i·step` for `i = 0, …, n`, where `n` is the [`grid_point_count`] (`lattice` in
/// `stepResponseIdentification.ts`; the grids of N2.2 and N2.4).
///
/// # Errors
/// Returns [`GridError::TooLarge`] when the count is undefined or more than [`MAXIMUM_GRID_POINTS`].
pub fn lattice(minimum: f64, maximum: f64, step: f64) -> Result<Vec<f64>, GridError> {
    match grid_point_count(minimum, maximum, step) {
        Some(count) if count <= MAXIMUM_GRID_POINTS => Ok((0..=count)
            .map(|index| minimum + count_f64(index) * step)
            .collect()),
        _ => Err(GridError::TooLarge),
    }
}

/// The gains that a baseline and a final window allow within the measurement bound `b` (N2.4; `gainBounds` in
/// `stepResponseIdentification.ts`): the interval between `(ȳ_final − b − (ȳ_base + b))/Δu` and
/// `(ȳ_final + b − (ȳ_base − b))/Δu`, whichever is smaller first. Both ends are `NaN` when either value is, as
/// `Math.min` and `Math.max` give them.
#[must_use]
pub fn gain_bounds(
    baseline: &StepResponseWindow,
    final_window: &StepResponseWindow,
    input_change: f64,
    measurement_bound: f64,
) -> ParameterInterval {
    let narrow = (final_window.mean - measurement_bound - (baseline.mean + measurement_bound))
        / input_change;
    let wide = (final_window.mean + measurement_bound - (baseline.mean - measurement_bound))
        / input_change;
    ParameterInterval {
        minimum: propagating_min(narrow, wide),
        maximum: propagating_max(narrow, wide),
    }
}

/// The normalized error of N3.2 (`normalizedRms` in `stepResponseIdentification.ts`):
/// `√(mean of eₖ²) / span`, where `span` is the plain span of the measured output, with no floor. `NaN` for no residuals.
/// A flat output has a zero span, and its caller refuses it before the division.
#[must_use]
pub fn normalized_rms(residuals: &[f64], span: f64) -> f64 {
    let squares: Vec<f64> = residuals.iter().map(|value| value * value).collect();
    mean(&squares).sqrt() / span
}

/// The time of the detectable onset (N2.5; `detectOnset` in `stepResponseIdentification.ts`): the time `tᵢ` of the
/// first index `i`, from `0` to `n − m` inclusive, whose `m` consecutive outputs all lie strictly beyond the
/// threshold `h`, that is `d·(yₖ − h) > 0` for `k = i, …, i + m − 1`. The direction `d` is `+1` for a rising
/// response and `−1` for a falling one; a direction of zero never crosses. `None` when no run qualifies, so an
/// isolated crossing does not establish onset. The caller subtracts the step time. `times` and `outputs` hold one
/// value per sample.
#[must_use]
pub fn onset_time(
    times: &[f64],
    outputs: &[f64],
    threshold: f64,
    required: usize,
    direction: f64,
) -> Option<f64> {
    let count = times.len().min(outputs.len());
    if required > count {
        return None;
    }
    (0..=count - required)
        .find(|&index| {
            outputs[index..index + required]
                .iter()
                .all(|output| direction * (output - threshold) > 0.0)
        })
        .and_then(|index| times.get(index).copied())
}

/// A held step on its recorded instants (N2.2): the times, the plant input deviations, the held samples and the
/// measured baseline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeldStep<'a> {
    /// The times of every sample of the record, in seconds.
    pub times: &'a [f64],
    /// The plant input deviation `dₖ = uₖ − u₀` from the input before the step, one per time.
    pub deviations: &'a [f64],
    /// The indices into `times` of the held samples, from the step to the end of the hold.
    pub hold: &'a [usize],
    /// The measured outputs of the held samples, one per entry of `hold`.
    pub hold_outputs: &'a [f64],
    /// The measured baseline mean `y₀` over the steady baseline window; not a fitted quantity.
    pub baseline: f64,
}

/// The first-order-with-delay candidate of a grid search with the smallest squared error (N2.2).
#[derive(Debug, Clone, PartialEq)]
pub struct FirstOrderFit {
    /// The least-squares gain `K`.
    pub gain: f64,
    /// The grid delay `L` in seconds; the response was simulated with this delay less the timestamp shift.
    pub delay_seconds: f64,
    /// The grid time constant `T` in seconds.
    pub time_constant_seconds: f64,
    /// The squared error `E = Σ(yₖ − y₀ − K·sₖ)²` over the held samples.
    pub squared_error: f64,
    /// The unit-gain response `sₖ` on every time of the record, so the fitted output is `y₀ + K·sₖ`.
    pub response: Vec<f64>,
}

/// The first-order-with-delay grid search of a held step (N2.2; the search loop of `analyzeStepResponse` in
/// `stepResponseIdentification.ts`).
///
/// For each delay `L` (the outer loop) and time constant `T` (the inner loop), the unit-gain response `sₖ` of
/// `1/(T·s + 1)` with the dead time `L − δ`, where `δ` is the timestamp shift, is simulated on every time of the record
/// from the held input deviations ([`held_unit_response`]). Its gain is the least-squares value over the held
/// samples, `K = Σ(yₖ − y₀)sₖ / Σsₖ²`, with the error `E = Σ(yₖ − y₀ − K·sₖ)²`. A candidate is kept when its gain is
/// finite and its error is strictly below the best so far, so the first of equal errors wins. `Ok(None)` when no
/// candidate has a finite gain.
///
/// # Errors
/// Returns [`GridError::TooLarge`], before any response is simulated, when delays × time constants ×
/// `max(1, held samples)` is more than [`MAXIMUM_GRID_EVALUATIONS`].
pub fn best_first_order_fit(
    held: &HeldStep<'_>,
    delays: &[f64],
    time_constants: &[f64],
    timestamp_shift_seconds: f64,
) -> Result<Option<FirstOrderFit>, GridError> {
    let evaluations = delays
        .len()
        .saturating_mul(time_constants.len())
        .saturating_mul(held.hold.len().max(1));
    if evaluations > MAXIMUM_GRID_EVALUATIONS {
        return Err(GridError::TooLarge);
    }
    let mut best: Option<FirstOrderFit> = None;
    for &delay_seconds in delays {
        for &time_constant_seconds in time_constants {
            let response = first_order_response(
                held,
                delay_seconds,
                time_constant_seconds,
                timestamp_shift_seconds,
            );
            let held_response: Vec<f64> = held
                .hold
                .iter()
                .map(|&index| value_at(&response, index))
                .collect();
            let fitted = least_squares_gain(held.hold_outputs, held.baseline, &held_response);
            let improves = best
                .as_ref()
                .is_none_or(|current| fitted.squared_error < current.squared_error);
            if fitted.gain.is_finite() && improves {
                best = Some(FirstOrderFit {
                    gain: fitted.gain,
                    delay_seconds,
                    time_constant_seconds,
                    squared_error: fitted.squared_error,
                    response,
                });
            }
        }
    }
    Ok(best)
}

/// The grid combinations that keep every held sample within the measurement bound (N2.4), with the marginal range of
/// each parameter over them. An empty set has a count of zero and no ranges: it is reported as empty and never
/// replaced by intervals derived from residuals. The ranges are grid-resolved, not statistical confidence.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsistentSet {
    /// The number of consistent combinations of baseline, gain, delay and time constant.
    pub count: usize,
    /// The smallest and largest consistent gain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain: Option<ParameterInterval>,
    /// The smallest and largest consistent delay in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_seconds: Option<ParameterInterval>,
    /// The smallest and largest consistent time constant in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_constant_seconds: Option<ParameterInterval>,
    /// The smallest and largest consistent baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<ParameterInterval>,
}

/// The inputs of a measurement-consistent parameter search (N2.4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConsistencySearch<'a> {
    /// The held step, whose baseline `y₀` centres the baseline lattice.
    pub held: HeldStep<'a>,
    /// The timestamp shift `δ` in seconds that each grid delay is reduced by, as in [`best_first_order_fit`].
    pub timestamp_shift_seconds: f64,
    /// The step amplitude `Δu`.
    pub input_change: f64,
    /// The absolute measurement bound `b`.
    pub measurement_bound: f64,
    /// The spacing `Δc` of the baseline lattice `y₀ − b + j·Δc`.
    pub baseline_step: f64,
    /// The gain grid.
    pub gains: &'a [f64],
    /// The delay grid in seconds.
    pub delays: &'a [f64],
    /// The time-constant grid in seconds.
    pub time_constants: &'a [f64],
    /// The mean of a steady final window, which the steady value `c + K·Δu` must meet within the bound; `None` when
    /// no steady final window is declared.
    pub final_target: Option<f64>,
}

/// The grid-resolved parameter set of N2.4 (`consistentSet` in `stepResponseIdentification.ts`).
///
/// For each delay `L`, time constant `T` and gain `K` of the grids, in that loop order, the unit response at each held
/// sample is `rₖ = sₖ/Δu`, with `sₖ` simulated as in [`best_first_order_fit`]. The baselines `c` that keep every held
/// sample within the bound `b` form the interval `[lower, upper]`. It starts as `[y₀ − b, y₀ + b]`, intersected with
/// `[f − b − K·Δu, f + b − K·Δu]` when a final target `f` is given. Each held sample in turn intersects it with
/// `[yₖ − b − K·Δu·rₖ, yₖ + b − K·Δu·rₖ]`, and the samples stop once `lower > upper + 10⁻⁹`. Each baseline of the
/// lattice `y₀ − b + j·Δc` with `⌈(lower − (y₀ − b) − 10⁻⁹)/Δc⌉ ≤ j ≤ ⌊(upper − (y₀ − b) + 10⁻⁹)/Δc⌋` is one
/// consistent combination. The ranges are the smallest and largest gain, delay, time constant and baseline of the
/// combinations, folded as `Math.min` and `Math.max` fold them. A combination whose lattice bounds are not numbers
/// adds nothing, and the count saturates at `usize::MAX`.
#[must_use]
pub fn consistent_parameter_set(search: &ConsistencySearch<'_>) -> ConsistentSet {
    let lowest_baseline = search.held.baseline - search.measurement_bound;
    let mut count: usize = 0;
    let mut gain = EMPTY_INTERVAL;
    let mut delay = EMPTY_INTERVAL;
    let mut time_constant = EMPTY_INTERVAL;
    let mut baseline = EMPTY_INTERVAL;
    for &delay_seconds in search.delays {
        for &time_constant_seconds in search.time_constants {
            let full = first_order_response(
                &search.held,
                delay_seconds,
                time_constant_seconds,
                search.timestamp_shift_seconds,
            );
            let responses: Vec<f64> = search
                .held
                .hold
                .iter()
                .map(|&index| value_at(&full, index) / search.input_change)
                .collect();
            for &candidate_gain in search.gains {
                let Some((first, last)) = baseline_indices(search, &responses, candidate_gain)
                else {
                    continue;
                };
                count = count.saturating_add(whole_count(last - first + 1.0));
                gain = widened(gain, candidate_gain, candidate_gain);
                delay = widened(delay, delay_seconds, delay_seconds);
                time_constant =
                    widened(time_constant, time_constant_seconds, time_constant_seconds);
                baseline = widened(
                    baseline,
                    lowest_baseline + first * search.baseline_step,
                    lowest_baseline + last * search.baseline_step,
                );
            }
        }
    }
    if count == 0 {
        return ConsistentSet::default();
    }
    ConsistentSet {
        count,
        gain: Some(gain),
        delay_seconds: Some(delay),
        time_constant_seconds: Some(time_constant),
        baseline: Some(baseline),
    }
}

/// The first and last baseline lattice index `j` that keep every held sample within the bound for one gain, or `None`
/// when no index does (the gain loop of `consistentSet`). `responses` holds the unit response `rₖ` of each held
/// sample.
fn baseline_indices(
    search: &ConsistencySearch<'_>,
    responses: &[f64],
    gain: f64,
) -> Option<(f64, f64)> {
    let held = &search.held;
    let bound = search.measurement_bound;
    let change = gain * search.input_change;
    let mut lower = propagating_max(
        held.baseline - bound,
        search
            .final_target
            .map_or(f64::NEG_INFINITY, |target| target - bound - change),
    );
    let mut upper = propagating_min(
        held.baseline + bound,
        search
            .final_target
            .map_or(f64::INFINITY, |target| target + bound - change),
    );
    for (index, output) in held.hold_outputs.iter().enumerate() {
        let response = value_at(responses, index);
        lower = propagating_max(lower, output - bound - change * response);
        upper = propagating_min(upper, output + bound - change * response);
        if lower > upper + EPSILON {
            break;
        }
    }
    let lowest = held.baseline - bound;
    let first = ((lower - lowest - EPSILON) / search.baseline_step).ceil();
    let last = ((upper - lowest + EPSILON) / search.baseline_step).floor();
    if first.is_nan() || last.is_nan() || first > last {
        None
    } else {
        Some((first, last))
    }
}

/// The unit-gain first-order response of the held step on every time of the record, with the dead time `L − δ`
/// (`responseFor` in `analyzeStepResponse`).
fn first_order_response(
    held: &HeldStep<'_>,
    delay_seconds: f64,
    time_constant_seconds: f64,
    timestamp_shift_seconds: f64,
) -> Vec<f64> {
    held_unit_response(
        held.times,
        held.deviations,
        HeldShape::FirstOrder {
            time_constant_seconds,
            delay_seconds: delay_seconds - timestamp_shift_seconds,
        },
    )
}

/// The interval that no value has widened yet: the folds of `Math.min` and `Math.max` start from the infinities.
const EMPTY_INTERVAL: ParameterInterval = ParameterInterval {
    minimum: f64::INFINITY,
    maximum: f64::NEG_INFINITY,
};

/// The interval widened so its minimum includes `low` and its maximum includes `high`.
fn widened(interval: ParameterInterval, low: f64, high: f64) -> ParameterInterval {
    ParameterInterval {
        minimum: propagating_min(interval.minimum, low),
        maximum: propagating_max(interval.maximum, high),
    }
}

/// A whole, non-negative float as a count. A value beyond `usize::MAX` saturates there, which keeps it above every
/// grid limit.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn whole_count(value: f64) -> usize {
    value as usize
}

/// The value at `index`, or `NaN` past the end, as the `TypeScript` reads a missing entry in arithmetic.
pub(super) fn value_at(values: &[f64], index: usize) -> f64 {
    values.get(index).copied().unwrap_or(f64::NAN)
}

/// The larger of two values as `Math.max` gives it: `NaN` when either is `NaN`.
pub(super) fn propagating_max(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else {
        left.max(right)
    }
}

/// The smaller of two values as `Math.min` gives it: `NaN` when either is `NaN`.
pub(super) fn propagating_min(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else {
        left.min(right)
    }
}

#[cfg(test)]
mod tests;
