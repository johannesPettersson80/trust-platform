#![allow(clippy::float_cmp)] // Zero results of the short-record rules are compared exactly on purpose.

use super::*;

fn timed(seconds: f64, value: f64) -> TimedValue {
    TimedValue { seconds, value }
}

#[test]
fn the_mean_and_the_noise_sigma_are_hand_computed() {
    assert!((mean(&[1.0, 2.0, 6.0]) - 3.0).abs() <= 1e-12);
    // Differences 1, −1, 1 give Σd² = 3 over 3 differences: σ = √(3/3/2) = √0.5, as the TypeScript test states.
    assert!((noise_sigma(&[0.0, 1.0, 0.0, 1.0]) - 0.5_f64.sqrt()).abs() <= 1e-12);
    assert_eq!(noise_sigma(&[1.0]), 0.0, "fewer than two values");
}

#[test]
fn the_baseline_noise_needs_three_values() {
    assert_eq!(
        baseline_noise_rms(&[0.0, 1.0]),
        0.0,
        "two values give no baseline noise"
    );
    // The same three differences as the noise sigma above.
    assert!((baseline_noise_rms(&[0.0, 1.0, 0.0, 1.0]) - 0.5_f64.sqrt()).abs() <= 1e-12);
}

#[test]
fn the_trend_slope_is_the_least_squares_slope() {
    // Values 2 + 3t at t = 0, 1, 2, 3 lie on a line of slope 3.
    let line: Vec<TimedValue> = (0..4)
        .map(|index| timed(f64::from(index), 2.0 + 3.0 * f64::from(index)))
        .collect();
    assert!((trend_slope(&line) - 3.0).abs() <= 1e-12);
    assert_eq!(trend_slope(&line[..1]), 0.0, "one point has no trend");
    let same_time = [timed(1.0, 1.0), timed(1.0, 5.0)];
    assert_eq!(
        trend_slope(&same_time),
        0.0,
        "no spread in time has no slope"
    );
}

#[test]
fn the_lag_one_correlation_is_the_correlation_of_each_value_with_the_next() {
    // 1, −1, 1, −1, 1: every value is opposite to the next, so the correlation is −1.
    assert!((lag_one_correlation(&[1.0, -1.0, 1.0, -1.0, 1.0]) + 1.0).abs() <= 1e-12);
    // A ramp: each value rises with the next, so the correlation is +1.
    assert!((lag_one_correlation(&[1.0, 2.0, 3.0, 4.0, 5.0]) - 1.0).abs() <= 1e-12);
    assert_eq!(lag_one_correlation(&[1.0, 1.0, 1.0]), 0.0, "no spread");
    assert_eq!(
        lag_one_correlation(&[1.0, f64::NAN, 1.0]),
        0.0,
        "a value that is not finite"
    );
    assert_eq!(
        lag_one_correlation(&[1.0, 2.0]),
        0.0,
        "fewer than three values"
    );
}
