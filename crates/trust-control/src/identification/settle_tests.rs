#![allow(clippy::float_cmp)] // Zero and unit results of the rules are compared exactly on purpose.

use super::*;

fn timed(seconds: f64, value: f64) -> TimedValue {
    TimedValue { seconds, value }
}

#[test]
fn the_baseline_window_is_the_longest_of_twenty_samples_five_percent_and_a_declared_settling_time()
{
    assert!(
        (baseline_window_seconds(6_000.0, 1.0, None) - 300.0).abs() <= 1e-9,
        "5 % of 100 minutes"
    );
    assert!(
        (baseline_window_seconds(6_000.0, 1.0, Some(4_000.0)) - 400.0).abs() <= 1e-9,
        "10 % of a declared 4000 s settling time is longer"
    );
    assert!(
        (baseline_window_seconds(60_000.0, 1.0, Some(100_000.0)) - 1_800.0).abs() <= 1e-9,
        "capped at 30 minutes"
    );
    assert!(
        (baseline_window_seconds(10.0, 1.0, None) - 20.0).abs() <= 1e-9,
        "twenty samples of a 1 s spacing are longer than 5 % of 10 s"
    );
    // The room heating's model check (10 October): a 41,000 s settling time gives a 205 s record period (A4.2), so twenty
    // samples span 4,100 s. The 30-minute cap would leave nine samples, and the baseline could never be judged steady.
    assert!(
        (baseline_window_seconds(27_360.0, 205.0, Some(41_000.0)) - 4_100.0).abs() <= 1e-9,
        "twenty samples of a long record period are kept beyond the cap"
    );
}

#[test]
fn a_settle_window_is_the_longer_of_twenty_samples_and_a_tenth_of_the_hold() {
    assert_eq!(settle_window_seconds(100.0, 1.0), 20.0);
    assert_eq!(settle_window_seconds(1_000.0, 1.0), 100.0);
}

#[test]
fn a_settled_response_is_recognised_and_a_ramp_that_never_settles_is_not() {
    let exponential: Vec<TimedValue> = (0..=500)
        .map(|second| {
            let seconds = f64::from(second);
            timed(seconds, 2.0 * (1.0 - (-seconds / 50.0).exp()))
        })
        .collect();
    let baseline = Baseline {
        mean: 0.0,
        noise: 0.01,
        spacing_seconds: 1.0,
    };
    assert!(has_settled(&exponential, &baseline, 0.0));
    let ramp: Vec<TimedValue> = (0..=500)
        .map(|second| timed(f64::from(second), 0.01 * f64::from(second)))
        .collect();
    assert!(!has_settled(&ramp, &baseline, 0.0));
    assert!(
        !has_settled(&[], &baseline, 0.0),
        "no points have not settled"
    );
}

#[test]
fn a_flat_baseline_is_steady_and_a_fast_trend_is_not() {
    let flat: Vec<TimedValue> = (0..60)
        .map(|second| timed(f64::from(second), 35.0))
        .collect();
    assert!(is_steady(&flat, 0.01, 100.0));
    // A trend of 0.01 per second over 59 s moves the value 0.59, more than four times a 0.01 noise (0.04).
    let drifting: Vec<TimedValue> = (0..60)
        .map(|second| timed(f64::from(second), 35.0 + 0.01 * f64::from(second)))
        .collect();
    assert!((trend_movement(&drifting) - 0.59).abs() <= 1e-9);
    assert!(!is_steady(&drifting, 0.01, 100.0));
}

#[test]
fn the_trailing_window_holds_the_points_within_its_span_of_the_last_one() {
    let points: Vec<TimedValue> = (0..10)
        .map(|second| timed(f64::from(second), 0.0))
        .collect();
    let window = trailing_window(&points, 3.0);
    assert_eq!(window.len(), 4, "the points at 6, 7, 8 and 9 s");
    assert_eq!(window[0].seconds, 6.0);
    assert_eq!(trailing_window(&[], 3.0), [] as [TimedValue; 0]);
}

#[test]
fn the_median_spacing_takes_the_middle_gap_or_averages_the_two_middle_gaps() {
    // Gaps 1, 2, 1 sort to 1, 1, 2: the middle gap is 1.
    let odd = [
        timed(0.0, 0.0),
        timed(1.0, 0.0),
        timed(3.0, 0.0),
        timed(4.0, 0.0),
    ];
    assert_eq!(median_spacing(&odd, 9.0), 1.0);
    // Gaps 1, 2, 3, 4 sort to the middle pair 2 and 3: their mean is 2.5.
    let even = [
        timed(0.0, 0.0),
        timed(1.0, 0.0),
        timed(3.0, 0.0),
        timed(6.0, 0.0),
        timed(10.0, 0.0),
    ];
    assert_eq!(median_spacing(&even, 9.0), 2.5);
    assert_eq!(
        median_spacing(&[timed(0.0, 0.0)], 9.0),
        9.0,
        "fewer than two points: the nominal spacing"
    );
}

#[test]
fn the_noise_band_is_three_noise_sigmas_about_the_baseline_mean() {
    // Three sigmas of 0.05 is 0.15: 0.2 is outside the band, 0.1 is inside it.
    assert!(left_noise_band(10.2, 10.0, 0.05));
    assert!(!left_noise_band(10.1, 10.0, 0.05));
}

#[test]
fn the_time_scale_needs_a_second_of_wall_time_and_some_process_time() {
    assert_eq!(observed_time_scale(10.0, 500.0), None);
    assert_eq!(observed_time_scale(0.0, 2_000.0), None);
    assert_eq!(observed_time_scale(10.0, 2_000.0), Some(5.0));
}
