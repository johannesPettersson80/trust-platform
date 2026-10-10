//! The numeric measures of a requirement over its recorded window (numerics N8.3; specification 35, B5.4). Each function
//! takes the samples of one channel over the window and returns the number that its formula gives, and each states that
//! formula. Which channel is measured, which window and limit apply, and what counts as Pass are rules of `trust-control`;
//! this module only computes. Inputs are finite numbers. Where a formula has no sample to act on, the function says so
//! (`None`, or zero for a sum), and a caller treats a missing value as not finite.

use crate::sampling::largest;
use crate::sampling::statistics::mean;

/// The direction of a movement between two adjacent samples `v_k` and `v_{k+1}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Movement {
    /// The rise `v_{k+1} − v_k`.
    Up,
    /// The fall `v_k − v_{k+1}`.
    Down,
    /// The absolute change `|v_{k+1} − v_k|`.
    Either,
}

/// The largest absolute error `max_k |e_k|`; `None` when there are no errors.
#[must_use]
pub fn max_abs(errors: &[f64]) -> Option<f64> {
    if errors.is_empty() {
        return None;
    }
    let magnitudes: Vec<f64> = errors.iter().copied().map(f64::abs).collect();
    Some(largest(&magnitudes, f64::NEG_INFINITY))
}

/// The mean absolute error `mean_k |e_k|`; `None` when there are no errors.
#[must_use]
pub fn mean_abs(errors: &[f64]) -> Option<f64> {
    let magnitudes: Vec<f64> = errors.iter().copied().map(f64::abs).collect();
    mean_of(&magnitudes)
}

/// The root mean square error `sqrt(mean_k e_k²)`, computed directly from the squared errors; `None` when there are no
/// errors. The error is unweighted: every sample counts once (N8.3).
#[must_use]
pub fn rms(errors: &[f64]) -> Option<f64> {
    let squares: Vec<f64> = errors.iter().map(|error| error * error).collect();
    mean_of(&squares).map(f64::sqrt)
}

/// The signed mean error `mean_k e_k` (N8.3 "Mean error"); `None` when there are no errors. Positive and negative errors
/// cancel, so the result shows a bias rather than a magnitude.
#[must_use]
pub fn signed_mean(errors: &[f64]) -> Option<f64> {
    mean_of(errors)
}

/// The mean of `values`, or `None` when there are none.
fn mean_of(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| mean(values))
}

/// The overshoot beyond a target, `max(0, max_k direction · (v_k − target))`: how far the values pass the target in the
/// direction of the step (N8.3). `direction` is the sign of the declared step, `1.0` for a rise and `-1.0` for a fall.
/// The result is zero when no value lies beyond the target, and for no values.
#[must_use]
pub fn overshoot(direction: f64, values: &[f64], target: f64) -> f64 {
    let beyond: Vec<f64> = values
        .iter()
        .map(|value| direction * (value - target))
        .collect();
    largest(&beyond, 0.0)
}

/// The largest adjacent-sample rate `max_k movement(v_{k+1} − v_k) / (t_{k+1} − t_k)`, or zero when no rate is positive.
/// `Up` counts only rises, `Down` only falls and `Either` the absolute change. Zero with fewer than two samples.
#[must_use]
pub fn adjacent_rate(times: &[f64], values: &[f64], movement: Movement) -> f64 {
    let rates: Vec<f64> = times
        .windows(2)
        .zip(values.windows(2))
        .map(|(time_pair, value_pair)| {
            let change = match movement {
                Movement::Up => value_pair[1] - value_pair[0],
                Movement::Down => value_pair[0] - value_pair[1],
                Movement::Either => (value_pair[1] - value_pair[0]).abs(),
            };
            change / (time_pair[1] - time_pair[0])
        })
        .collect();
    largest(&rates, 0.0)
}

/// The saturation time `Σ_k (t_{k+1} − t_k)` over the intervals whose held value `v_k` is at or below `minimum` or at or
/// above `maximum` (N8.3: the signal is held left over each adjacent interval). The last sample holds no interval, and
/// the result is zero with fewer than two samples.
#[must_use]
pub fn saturation_time(times: &[f64], values: &[f64], minimum: f64, maximum: f64) -> f64 {
    values
        .iter()
        .zip(times.windows(2))
        .filter(|&(value, _)| *value <= minimum || *value >= maximum)
        .map(|(_, time_pair)| time_pair[1] - time_pair[0])
        .sum()
}

/// The number of adjacent changes `|v_k − v_{k−1}|` that strictly exceed `threshold` (N8.3 switching count), as a count.
#[must_use]
pub fn switching_count(values: &[f64], threshold: f64) -> f64 {
    values
        .windows(2)
        .filter(|pair| (pair[1] - pair[0]).abs() > threshold)
        .fold(0.0, |count, _| count + 1.0)
}

/// The total movement `Σ_k |v_k − v_{k−1}|` of the values; zero with fewer than two values.
#[must_use]
pub fn movement_total(values: &[f64]) -> f64 {
    values
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .sum()
}

/// The largest violation of a band, `max(0, max_k max(minimum − v_k, v_k − maximum))`: zero when every value lies within
/// `[minimum, maximum]` (N8.3 signal and state bounds).
#[must_use]
pub fn bound_violation(values: &[f64], minimum: f64, maximum: f64) -> f64 {
    let violations: Vec<f64> = values
        .iter()
        .map(|value| (minimum - value).max(value - maximum))
        .collect();
    largest(&violations, 0.0)
}

/// The largest distance from a value to its nearest level, `max(0, max_k min_l |v_k − l|)`: zero when every value lies on
/// a level. With no levels the distance is infinite.
#[must_use]
pub fn level_distance(values: &[f64], levels: &[f64]) -> f64 {
    let distances: Vec<f64> = values
        .iter()
        .map(|&value| nearest_distance(value, levels))
        .collect();
    largest(&distances, 0.0)
}

/// The distance from `value` to the nearest of `levels`; positive infinity for no levels.
fn nearest_distance(value: f64, levels: &[f64]) -> f64 {
    levels
        .iter()
        .map(|level| (value - level).abs())
        .fold(f64::INFINITY, f64::min)
}

/// The completed dwell intervals of one state: the time from each switch into the other state to the switch that ends
/// it. A state is `v ≥ threshold` when `high` is true and `v < threshold` otherwise. The first sample starts the first
/// state, which has no start time and is not counted. The intervals are returned in time order.
#[must_use]
pub fn dwell_intervals(times: &[f64], values: &[f64], threshold: f64, high: bool) -> Vec<f64> {
    let mut intervals = Vec::new();
    let Some(&first) = values.first() else {
        return intervals;
    };
    let mut currently_high = first >= threshold;
    let mut transition_start: Option<f64> = None;
    for (&time, &value) in times.iter().zip(values).skip(1) {
        let next_high = value >= threshold;
        if next_high != currently_high {
            if let Some(start) = transition_start.filter(|_| currently_high == high) {
                intervals.push(time - start);
            }
            transition_start = Some(time);
            currently_high = next_high;
        }
    }
    intervals
}

/// Whether `value` lies within `band` of `target`: `|v − target| ≤ band`.
fn within_band(value: f64, target: f64, band: f64) -> bool {
    (value - target).abs() <= band
}

/// The index of the first value within `band` of `target`, or `None` when no value lies in the band.
#[must_use]
pub fn first_in_band(values: &[f64], target: f64, band: f64) -> Option<usize> {
    values
        .iter()
        .position(|&value| within_band(value, target, band))
}

/// Whether every value from index `from` on lies within `band` of `target`. The result is true when no value follows
/// `from`.
#[must_use]
pub fn all_in_band_from(values: &[f64], target: f64, band: f64, from: usize) -> bool {
    values
        .iter()
        .skip(from)
        .all(|&value| within_band(value, target, band))
}

/// The index that starts the final run of values inside the band: the first index from which every later value lies
/// within `band` of `target`. The result is `None` when the last value lies outside the band, or when there are no
/// values.
#[must_use]
pub fn final_in_band_start(values: &[f64], target: f64, band: f64) -> Option<usize> {
    let start = values
        .iter()
        .rposition(|&value| !within_band(value, target, band))
        .map_or(0, |last_outside| last_outside + 1);
    (start < values.len()).then_some(start)
}

#[cfg(test)]
#[path = "measure_tests.rs"]
mod tests;
