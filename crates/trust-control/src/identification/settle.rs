//! The steadiness and settling rules of specification 35, B3.4, measured on process time (A3.2): the length of the
//! baseline window, whether a baseline is steady, the noise band, and whether a response has settled. The rules read
//! the times and values they are given; none reads a clock (`settleDetection.ts`).

use trust_control_math::sampling::statistics::{mean, trend_slope, TimedValue};

/// A window, and a settle window, hold at least this many samples (B3.4).
pub const MINIMUM_WINDOW_SAMPLES: usize = 20;

/// The steady baseline window is never longer than 30 minutes (B3.4).
pub const BASELINE_WINDOW_CAP_SECONDS: f64 = 1_800.0;

/// The response has left the noise band beyond this many noise sigmas (B3.4).
const NOISE_BAND_SIGMAS: f64 = 3.0;

/// The baseline a settle test compares the response with: its mean, its noise and the spacing of its samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Baseline {
    /// The mean of the steady baseline.
    pub mean: f64,
    /// The noise `σ` of the steady baseline.
    pub noise: f64,
    /// The nominal spacing of the baseline's samples in seconds.
    pub spacing_seconds: f64,
}

/// The samples a window of `spacing` seconds holds, as seconds: twenty samples of that spacing.
#[allow(clippy::cast_precision_loss)] // The sample count is 20, a constant.
fn minimum_window_seconds(spacing_seconds: f64) -> f64 {
    MINIMUM_WINDOW_SAMPLES as f64 * spacing_seconds
}

/// The movement of the least-squares trend across the time span of the points: the magnitude of its slope times the
/// span between the first and the last point. Zero with fewer than two points.
#[must_use]
pub fn trend_movement(points: &[TimedValue]) -> f64 {
    match (points.first(), points.last()) {
        (Some(first), Some(last)) if points.len() >= 2 => {
            trend_slope(points).abs() * (last.seconds - first.seconds)
        }
        _ => 0.0,
    }
}

/// Whether a baseline is steady (B3.4): its trend moves it by no more than four times the noise, or by `10⁻⁵` of its
/// range when that is larger. A tolerance tied only to the range would let a short window call a fast-moving output
/// steady.
#[must_use]
pub fn is_steady(points: &[TimedValue], noise: f64, range: f64) -> bool {
    trend_movement(points) <= (4.0 * noise).max(1e-5 * range)
}

/// The baseline window (B3.4): the longest of twenty samples, 5 % of the maximum duration and, when a settling time is
/// declared, 10 % of it; never longer than [`BASELINE_WINDOW_CAP_SECONDS`].
#[must_use]
pub fn baseline_window_seconds(
    maximum_seconds: f64,
    spacing_seconds: f64,
    settling_seconds: Option<f64>,
) -> f64 {
    let declared = settling_seconds.map_or(0.0, |settling| 0.1 * settling);
    BASELINE_WINDOW_CAP_SECONDS.min(
        minimum_window_seconds(spacing_seconds)
            .max(0.05 * maximum_seconds)
            .max(declared),
    )
}

/// The settle window (B3.4): the longer of twenty samples and 10 % of the hold so far.
#[must_use]
pub fn settle_window_seconds(hold_seconds: f64, spacing_seconds: f64) -> f64 {
    minimum_window_seconds(spacing_seconds).max(0.1 * hold_seconds)
}

/// The points that lie within `seconds` of the last point, in time order (`trailingWindow`). Empty for no points.
#[must_use]
pub fn trailing_window(points: &[TimedValue], seconds: f64) -> &[TimedValue] {
    let Some(last) = points.last() else {
        return &[];
    };
    let start = last.seconds - seconds;
    let mut first = points.len() - 1;
    while first > 0 && points[first - 1].seconds >= start {
        first -= 1;
    }
    &points[first..]
}

/// The median spacing of successive points: the middle gap, or the mean of the two middle gaps for an even count
/// (`medianSpacing`). The nominal spacing when fewer than two points exist.
#[must_use]
pub fn median_spacing(points: &[TimedValue], nominal_seconds: f64) -> f64 {
    let mut gaps: Vec<f64> = points
        .windows(2)
        .map(|pair| pair[1].seconds - pair[0].seconds)
        .collect();
    if gaps.is_empty() {
        return nominal_seconds;
    }
    gaps.sort_by(f64::total_cmp);
    let middle = gaps.len() / 2;
    if gaps.len() % 2 == 1 {
        gaps[middle]
    } else {
        f64::midpoint(gaps[middle - 1], gaps[middle])
    }
}

/// Whether a single value has left the noise band (B3.4): it is more than three noise sigmas from the baseline mean.
#[must_use]
pub fn left_noise_band(value: f64, baseline_mean: f64, noise: f64) -> bool {
    (value - baseline_mean).abs() > NOISE_BAND_SIGMAS * noise
}

/// Whether a response has settled (B3.4): over the settle window it changes by less than the larger of twice the noise
/// and 2 % of its change so far. The caller has already seen the response leave the noise band.
#[must_use]
pub fn has_settled(points: &[TimedValue], baseline: &Baseline, step_seconds: f64) -> bool {
    let Some(last) = points.last() else {
        return false;
    };
    let recent = trailing_window(
        points,
        settle_window_seconds(last.seconds - step_seconds, baseline.spacing_seconds),
    );
    if recent.len() < MINIMUM_WINDOW_SAMPLES {
        return false;
    }
    let change = trend_movement(recent);
    let values: Vec<f64> = recent.iter().map(|point| point.value).collect();
    let so_far = (mean(&values) - baseline.mean).abs();
    change < (2.0 * baseline.noise).max(0.02 * so_far)
}

/// The ratio of process time to wall time over one observation (A3.2), or `None` until a second of wall time has
/// passed and the process has advanced.
#[must_use]
pub fn observed_time_scale(process_seconds: f64, wall_milliseconds: f64) -> Option<f64> {
    (wall_milliseconds >= 1_000.0 && process_seconds > 0.0)
        .then_some(process_seconds / (wall_milliseconds / 1_000.0))
}

#[cfg(test)]
#[path = "settle_tests.rs"]
mod tests;
