//! Closed-form tests of the N8.3 measures (`measure.rs`). Each expected value is worked out by hand from the formula in the
//! function's documentation, so the test checks the formula and not the implementation.

use super::*;

const TOLERANCE: f64 = 1e-12;

/// Asserts that `actual` lies within the test tolerance of `expected`.
fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= TOLERANCE,
        "{actual} is not {expected}"
    );
}

#[test]
fn max_abs_is_the_largest_magnitude_and_none_for_no_errors() {
    assert_close(max_abs(&[-3.0, 1.0, 2.0]).expect("errors"), 3.0);
    assert_close(max_abs(&[0.5, -0.5]).expect("errors"), 0.5);
    assert_eq!(max_abs(&[]), None);
}

#[test]
fn mean_abs_averages_the_magnitudes() {
    // |−3| + |1| + |2| = 6, and 6 / 3 = 2.
    assert_close(mean_abs(&[-3.0, 1.0, 2.0]).expect("errors"), 2.0);
    assert_eq!(mean_abs(&[]), None);
}

#[test]
fn rms_is_the_root_of_the_mean_square_computed_directly() {
    // (3² + 4²) / 2 = 12.5. The window of the TypeScript RMS test has errors 1, 3, 1, 3, so the RMS is √((1 + 9 + 1 + 9) / 4) = √5.
    assert_close(rms(&[3.0, -4.0]).expect("errors"), 12.5_f64.sqrt());
    assert_close(rms(&[1.0, 3.0, 1.0, 3.0]).expect("errors"), 5.0_f64.sqrt());
    assert_close(rms(&[2.0, -2.0, 2.0]).expect("errors"), 2.0);
    assert_eq!(rms(&[]), None);
}

#[test]
fn signed_mean_keeps_the_sign_and_cancels_opposite_errors() {
    // (1 − 3 + 2) / 3 = 0: the opposite errors cancel.
    assert_close(signed_mean(&[1.0, -3.0, 2.0]).expect("errors"), 0.0);
    assert_close(signed_mean(&[1.0, 2.0]).expect("errors"), 1.5);
    assert_close(signed_mean(&[-0.5]).expect("errors"), -0.5);
    assert_eq!(signed_mean(&[]), None);
}

#[test]
fn overshoot_is_the_largest_excursion_beyond_the_target_in_the_direction_of_the_step() {
    // A rise to 1.0: the largest excursion beyond 1.0 is 1.2 − 1.0 = 0.2.
    assert_close(overshoot(1.0, &[0.0, 1.2, 1.05], 1.0), 0.2);
    // A fall to 1.0 (direction −1): the excursion is −(0.8 − 1.0) = 0.2, below the target.
    assert_close(overshoot(-1.0, &[2.0, 0.8, 1.0], 1.0), 0.2);
    // No value passes the target: zero, never a negative number.
    assert_close(overshoot(1.0, &[0.5, 0.9], 1.0), 0.0);
    assert_close(overshoot(1.0, &[], 1.0), 0.0);
}

#[test]
fn adjacent_rate_is_the_largest_movement_per_second_in_the_chosen_direction() {
    let times = [0.0, 1.0, 3.0];
    // Either: |2 − 0| / 1 = 2 and |1 − 2| / 2 = 0.5, so the largest is 2.
    assert_close(
        adjacent_rate(&times, &[0.0, 2.0, 1.0], Movement::Either),
        2.0,
    );
    // Up on values 2, 0, 1: (0 − 2) / 1 = −2 is a fall and does not count, so only (1 − 0) / 2 = 0.5 remains.
    assert_close(adjacent_rate(&times, &[2.0, 0.0, 1.0], Movement::Up), 0.5);
    // Down on the same values: (2 − 0) / 1 = 2 is the largest fall.
    assert_close(adjacent_rate(&times, &[2.0, 0.0, 1.0], Movement::Down), 2.0);
    // A single sample has no interval.
    assert_close(adjacent_rate(&[0.0], &[1.0], Movement::Either), 0.0);
}

#[test]
fn saturation_time_holds_each_sample_over_the_following_interval() {
    // Samples at 0, 1, 2 and 4 s. The first value (−1.5) is at or below −1 and is held from 0 to 1 s (1 s). The third
    // value (1.2) is at or above 1 and is held from 2 to 4 s (2 s). The middle value is inside the band, and the last
    // sample holds no interval.
    assert_close(
        saturation_time(&[0.0, 1.0, 2.0, 4.0], &[-1.5, 0.5, 1.2, 0.0], -1.0, 1.0),
        3.0,
    );
    // A value exactly at a bound is saturated.
    assert_close(saturation_time(&[0.0, 1.0], &[1.0, 0.0], -1.0, 1.0), 1.0);
    // The last sample alone holds nothing.
    assert_close(saturation_time(&[0.0, 1.0], &[0.0, 1.0], -1.0, 1.0), 0.0);
}

#[test]
fn switching_count_counts_the_changes_strictly_above_the_threshold() {
    // The changes are 0.3 (counted), 0 (not), 0.4 (counted) and 0.6 (counted): three switches.
    assert_close(switching_count(&[0.0, 0.3, 0.3, -0.1, 0.5], 0.25), 3.0);
    // A change exactly equal to the threshold is not a switch.
    assert_close(switching_count(&[0.0, 0.25], 0.25), 0.0);
    assert_close(switching_count(&[], 0.25), 0.0);
}

#[test]
fn movement_total_sums_the_absolute_changes() {
    // |1 − 0| + |−1 − 1| + |0.5 − (−1)| = 1 + 2 + 1.5 = 4.5.
    assert_close(movement_total(&[0.0, 1.0, -1.0, 0.5]), 4.5);
    assert_close(movement_total(&[1.0]), 0.0);
    assert_close(movement_total(&[]), 0.0);
}

#[test]
fn bound_violation_is_the_largest_excursion_outside_the_band_and_zero_inside() {
    // 1.3 is 0.3 above 1, and −0.2 is 0.2 below 0. The largest excursion is 0.3.
    assert_close(bound_violation(&[0.5, -0.2, 1.3], 0.0, 1.0), 0.3);
    // Values on the bounds are inside them.
    assert_close(bound_violation(&[0.0, 0.5, 1.0], 0.0, 1.0), 0.0);
    assert_close(bound_violation(&[], 0.0, 1.0), 0.0);
}

#[test]
fn level_distance_is_the_largest_distance_to_the_nearest_level() {
    // 0.1 and 0.9 are 0.1 from their nearest levels; 1.6 is 0.6 from the level 1, which is its nearest.
    assert_close(level_distance(&[0.1, 0.9, 1.6], &[0.0, 1.0]), 0.6);
    assert_close(level_distance(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
    assert_close(level_distance(&[], &[0.0, 1.0]), 0.0);
}

#[test]
fn dwell_intervals_measure_the_completed_stays_in_the_named_state() {
    // Samples at 0, 1, 2, 3.5 and 5 s alternate around the threshold 0.5. The high stays are 1 s (1 s to 2 s) and 1.5 s
    // (3.5 s to 5 s). The low stay from 2 s to 3.5 s is 1.5 s. The first sample starts the state it holds, so the low
    // stay from 0 s to 1 s has no start time and is not counted.
    let times = [0.0, 1.0, 2.0, 3.5, 5.0];
    let values = [0.0, 1.0, 0.0, 1.0, 0.0];
    let high = dwell_intervals(&times, &values, 0.5, true);
    assert_eq!(high.len(), 2);
    assert_close(high[0], 1.0);
    assert_close(high[1], 1.5);
    let low = dwell_intervals(&times, &values, 0.5, false);
    assert_eq!(low.len(), 1);
    assert_close(low[0], 1.5);
    assert_eq!(
        dwell_intervals(&[0.0, 1.0, 2.0], &[1.0, 1.0, 1.0], 0.5, false),
        [] as [f64; 0]
    );
    assert_eq!(dwell_intervals(&[], &[], 0.5, true), [] as [f64; 0]);
}

#[test]
fn first_in_band_finds_the_first_value_within_the_band_inclusively() {
    // 1.25 is exactly 0.25 from the target 1.0, which is inside a band of 0.25.
    assert_eq!(first_in_band(&[3.0, 1.25, 1.0, 1.0], 1.0, 0.25), Some(1));
    assert_eq!(first_in_band(&[3.0, 1.3], 1.0, 0.25), None);
    assert_eq!(first_in_band(&[], 1.0, 0.25), None);
}

#[test]
fn all_in_band_from_requires_every_later_value_to_lie_inside_the_band() {
    assert!(all_in_band_from(&[1.5, 1.0, 0.98], 1.0, 0.03, 1));
    assert!(!all_in_band_from(&[1.5, 1.0, 0.98], 1.0, 0.03, 0));
    assert!(all_in_band_from(&[1.5, 1.0, 0.98], 1.0, 0.03, 2));
}

#[test]
fn final_in_band_start_is_the_first_index_of_the_last_run_inside_the_band() {
    // 1.4 is the last value outside the band of 0.05, so the final run starts at index 3.
    assert_eq!(
        final_in_band_start(&[1.5, 1.0, 1.4, 1.0, 0.99], 1.0, 0.05),
        Some(3)
    );
    // Every value is inside the band, so the run starts at the first index.
    assert_eq!(final_in_band_start(&[1.0, 1.02, 0.99], 1.0, 0.05), Some(0));
    // The last value is outside the band, so no run ends the window inside it.
    assert_eq!(final_in_band_start(&[1.0, 1.5], 1.0, 0.05), None);
    assert_eq!(final_in_band_start(&[], 1.0, 0.05), None);
}
