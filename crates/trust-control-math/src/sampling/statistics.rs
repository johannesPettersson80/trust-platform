//! Statistics over a record: the mean, the noise of first differences, the least-squares trend of timed
//! values, the lag-one correlation of a residual series and the RMS noise of a steady baseline. Each function
//! states its formula. A record too short for a formula gives the value stated for it, never a panic.

use super::as_f64;

/// One measured value at a process time: the time in seconds and the value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimedValue {
    /// The process time in seconds.
    pub seconds: f64,
    /// The measured value at that time.
    pub value: f64,
}

/// The arithmetic mean `Σ vᵢ / n`, summed from the first value. `NaN` for no values.
#[must_use]
pub fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / as_f64(values.len())
}

/// The noise `σ` of a record: the root of half the mean squared first difference,
/// `σ = √(Σ(vᵢ − vᵢ₋₁)² / (n − 1) / 2)` (B3.4). Zero with fewer than two values.
#[must_use]
pub fn noise_sigma(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let squares: f64 = values
        .windows(2)
        .map(|pair| {
            let step = pair[1] - pair[0];
            step * step
        })
        .sum();
    (squares / as_f64(values.len() - 1) / 2.0).sqrt()
}

/// The RMS noise of a steady baseline (N1.3): `σ` of its first differences, so a slow drift does not count
/// as noise. Zero with fewer than three values.
#[must_use]
pub fn baseline_noise_rms(outputs: &[f64]) -> f64 {
    if outputs.len() < 3 {
        return 0.0;
    }
    noise_sigma(outputs)
}

/// The least-squares slope of the values against their times, in value per second (B3.4):
/// `Σ(tᵢ − t̄)(vᵢ − v̄) / Σ(tᵢ − t̄)²`. Zero with fewer than two points or no spread in time.
#[must_use]
pub fn trend_slope(points: &[TimedValue]) -> f64 {
    if points.len() < 2 {
        return 0.0;
    }
    let seconds: Vec<f64> = points.iter().map(|point| point.seconds).collect();
    let values: Vec<f64> = points.iter().map(|point| point.value).collect();
    let mean_seconds = mean(&seconds);
    let mean_value = mean(&values);
    let mut covariance = 0.0;
    let mut variance = 0.0;
    for point in points {
        let deviation = point.seconds - mean_seconds;
        covariance += deviation * (point.value - mean_value);
        variance += deviation * deviation;
    }
    if variance > 0.0 {
        covariance / variance
    } else {
        0.0
    }
}

/// The lag-one correlation of a series `x`: the correlation of each value with the next,
/// `Σ(xᵢ − x̄)(xᵢ₊₁ − x̄′) / √(Σ(xᵢ − x̄)² · Σ(xᵢ₊₁ − x̄′)²)`, where `x̄` is the mean of the first `n − 1`
/// values and `x̄′` the mean of the last `n − 1`. Zero with fewer than three values, a value that is not finite,
/// or no spread.
#[must_use]
pub fn lag_one_correlation(values: &[f64]) -> f64 {
    if values.len() < 3 || !values.iter().all(|value| value.is_finite()) {
        return 0.0;
    }
    let left = &values[..values.len() - 1];
    let right = &values[1..];
    let left_mean = mean(left);
    let right_mean = mean(right);
    let numerator: f64 = left
        .iter()
        .zip(right)
        .map(|(left_value, right_value)| (left_value - left_mean) * (right_value - right_mean))
        .sum();
    let left_energy: f64 = left
        .iter()
        .map(|value| {
            let deviation = value - left_mean;
            deviation * deviation
        })
        .sum();
    let right_energy: f64 = right
        .iter()
        .map(|value| {
            let deviation = value - right_mean;
            deviation * deviation
        })
        .sum();
    let denominator = (left_energy * right_energy).sqrt();
    if denominator > 0.0 {
        numerator / denominator
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests;
