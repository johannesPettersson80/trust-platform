#![allow(clippy::float_cmp)] // Copied grid values and exact results are compared exactly.

use super::*;

/// The held-step record of `control-studio-step-response-numerics.test.ts` (`samples`).
struct Samples {
    times: Vec<f64>,
    inputs: Vec<f64>,
    outputs: Vec<f64>,
}

/// Builds the `TypeScript` record: 41 samples every 0.1 s, the input stepping from 75 to 85 at sample 10 and back at
/// sample 40, and from sample 10 the output `35 + 2·(1 − e^(−max(0, t − 1 − 0.1)/0.2))`, a first-order response of
/// gain 0.2 with a delay of 0.1 s; `overrides` replaces outputs by sample index.
fn samples(overrides: &[(u32, f64)]) -> Samples {
    let mut times = Vec::new();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for index in 0..=40_u32 {
        let time_seconds = f64::from(index) * 0.1;
        let elapsed = (time_seconds - 1.0 - 0.1).max(0.0);
        let modelled = if index < 10 {
            35.0
        } else {
            35.0 + 2.0 * (1.0 - (-elapsed / 0.2).exp())
        };
        times.push(time_seconds);
        inputs.push(if (10..40).contains(&index) {
            85.0
        } else {
            75.0
        });
        outputs.push(
            overrides
                .iter()
                .find_map(|&(at, value)| (at == index).then_some(value))
                .unwrap_or(modelled),
        );
    }
    Samples {
        times,
        inputs,
        outputs,
    }
}

/// The indices of the held samples, from the step at 1 s to the restoration at 4 s (`1 ≤ t < 4`).
fn hold_indices(times: &[f64]) -> Vec<usize> {
    times
        .iter()
        .enumerate()
        .filter(|&(_, time)| (1.0..4.0).contains(time))
        .map(|(index, _)| index)
        .collect()
}

/// The times and values of the held samples.
fn held_part(times: &[f64], values: &[f64]) -> (Vec<f64>, Vec<f64>) {
    hold_indices(times)
        .iter()
        .map(|&index| (times[index], values[index]))
        .unzip()
}

/// The consistent set of a record as `analyzeStepResponse` searches it with the test's settings: the baseline window
/// [0, 1) s, the steady final window [3, 4) s as the target and the source of the gain bounds, the gain step 0.001,
/// the baseline step 0.01, the time constants 0.1 to 0.4 s by 0.1 s, and the given delays.
fn record_consistency(record: &Samples, measurement_bound: f64, delays: &[f64]) -> ConsistentSet {
    let baseline = window_statistics(&record.times, &record.outputs, 0.0, 1.0, 0.1)
        .expect("the baseline window is covered");
    let final_window = window_statistics(&record.times, &record.outputs, 3.0, 4.0, 0.1)
        .expect("the final window is covered");
    let gain_interval = gain_bounds(&baseline, &final_window, 10.0, measurement_bound);
    let gains = lattice(gain_interval.minimum, gain_interval.maximum, 0.001)
        .expect("the gain lattice is within its limit");
    let time_constants = lattice(0.1, 0.4, 0.1).expect("the time-constant lattice");
    let hold = hold_indices(&record.times);
    let hold_outputs: Vec<f64> = hold.iter().map(|&index| record.outputs[index]).collect();
    let deviations: Vec<f64> = record.inputs.iter().map(|input| input - 75.0).collect();
    consistent_parameter_set(&ConsistencySearch {
        held: HeldStep {
            times: &record.times,
            deviations: &deviations,
            hold: &hold,
            hold_outputs: &hold_outputs,
            baseline: baseline.mean,
        },
        timestamp_shift_seconds: 0.0,
        input_change: 10.0,
        measurement_bound,
        baseline_step: 0.01,
        gains: &gains,
        delays,
        time_constants: &time_constants,
        final_target: Some(final_window.mean),
    })
}

/// The brute-force count of the `TypeScript` oracle: the gain from `(ȳ_f − y₀ − 2b)/Δu` to `(ȳ_f − y₀ + 2b)/Δu` by
/// 0.001, the baseline `c` from `y₀ − b` to `y₀ + b` by 0.01, the delay from 0 to 0.2 s by 0.1 s and the time constant
/// from 0.1 to 0.4 s by 0.1 s, each stepped by accumulation, with `b = 0.05` and `Δu = 10`. A combination counts when
/// its steady value lies within `b + 10⁻⁸` of the final mean and its continuous response
/// `c + K·Δu·(1 − e^(−(t − 1 − L)/T))` lies within `b + 10⁻⁸` of every held sample.
fn oracle_count(points: &[(f64, f64)], baseline: f64, final_mean: f64) -> usize {
    let (bound, input_change) = (0.05, 10.0);
    let gain_maximum = (final_mean - baseline + 2.0 * bound) / input_change;
    let mut count = 0;
    let mut gain = (final_mean - baseline - 2.0 * bound) / input_change;
    while gain <= gain_maximum + 1e-9 {
        let mut candidate = baseline - bound;
        while candidate <= baseline + bound + 1e-9 {
            let mut delay = 0.0;
            while delay <= 0.2 + 1e-9 {
                let mut tau = 0.1;
                while tau <= 0.4 + 1e-9 {
                    let steady = candidate + gain * input_change;
                    let consistent = steady >= final_mean - bound - 1e-8
                        && steady <= final_mean + bound + 1e-8
                        && points.iter().all(|&(time, measured)| {
                            let elapsed = time - 1.0 - delay;
                            let predicted = if elapsed < 0.0 {
                                candidate
                            } else {
                                candidate + gain * input_change * (1.0 - (-elapsed / tau).exp())
                            };
                            (measured - predicted).abs() <= bound + 1e-8
                        });
                    if consistent {
                        count += 1;
                    }
                    tau += 0.1;
                }
                delay += 0.1;
            }
            candidate += 0.01;
        }
        gain += 0.001;
    }
    count
}

#[test]
fn a_steady_baseline_is_flat_and_a_drifting_one_reports_its_range_and_slope() {
    // `qualifies a complete held step and rejects drift`, on the window numbers: the drifting baseline rises by 0.02
    // per sample, a range of 0.18 and a slope of 0.2 per second, beyond the test's limits of 0.1 and 0.01 per second.
    let steady = samples(&[]);
    let window =
        window_statistics(&steady.times, &steady.outputs, 0.0, 1.0, 0.1).expect("covered window");
    assert_eq!(
        (
            window.count,
            window.mean,
            window.range,
            window.slope_per_second
        ),
        (10, 35.0, 0.0, 0.0)
    );
    let drifting = samples(&[
        (0, 35.0),
        (1, 35.02),
        (2, 35.04),
        (3, 35.06),
        (4, 35.08),
        (5, 35.1),
        (6, 35.12),
        (7, 35.14),
        (8, 35.16),
        (9, 35.18),
    ]);
    let window = window_statistics(&drifting.times, &drifting.outputs, 0.0, 1.0, 0.1)
        .expect("covered window");
    assert!(
        (window.range - 0.18).abs() <= 1e-9,
        "range {}",
        window.range
    );
    assert!(
        (window.slope_per_second - 0.2).abs() <= 1e-9,
        "slope {}",
        window.slope_per_second
    );
    assert!((window.mean - 35.09).abs() <= 1e-9, "mean {}", window.mean);
}

#[test]
fn the_onset_is_the_first_of_three_samples_above_the_threshold() {
    // `detects three consecutive samples above the absolute onset threshold`: 35.1 stays below 35.15, so the run
    // starts at sample 12, 0.2 s after the step at 1 s.
    let record = samples(&[(11, 35.1), (12, 35.2), (13, 35.3)]);
    let (times, outputs) = held_part(&record.times, &record.outputs);
    let onset = onset_time(&times, &outputs, 35.15, 3, 1.0).expect("a sustained onset");
    assert!((onset - 1.0 - 0.2).abs() < 1e-9, "onset {onset}");
}

#[test]
fn an_isolated_excursion_does_not_start_the_onset_in_either_direction() {
    // `detects sustained onset for input sign … and output sign …`, on the onset alone: the excursion at sample 11 is
    // followed by a sample back inside the threshold, so the onset is the run from sample 13, 0.3 s after the step.
    // The output's direction decides the side of the threshold; the input's sign does not enter the onset.
    let record = samples(&[(11, 35.2), (12, 35.1)]);
    for output_sign in [-1.0, 1.0] {
        let mirrored: Vec<f64> = record
            .outputs
            .iter()
            .map(|output| 35.0 + output_sign * (output - 35.0))
            .collect();
        let (times, outputs) = held_part(&record.times, &mirrored);
        let onset = onset_time(&times, &outputs, 35.0 + output_sign * 0.15, 3, output_sign)
            .expect("a sustained onset");
        assert!(
            (onset - 1.0 - 0.3).abs() < 1e-9,
            "output sign {output_sign}: onset {onset}"
        );
    }
}

#[test]
fn an_unchanged_output_has_no_onset() {
    // `leaves onset unavailable when the output does not change`: the direction of a zero change is zero, and no
    // output lies beyond the threshold in that direction.
    let record = samples(&[]);
    let flat = vec![35.0; record.times.len()];
    let (times, outputs) = held_part(&record.times, &flat);
    assert_eq!(onset_time(&times, &outputs, 35.15, 3, 0.0), None);
}

#[test]
fn the_consistent_set_matches_the_brute_force_oracle() {
    // `matches a small explicit brute-force consistency oracle`, on the library's search.
    let record = samples(&[]);
    let delays = lattice(0.0, 0.2, 0.1).expect("the delay lattice");
    let set = record_consistency(&record, 0.05, &delays);
    let baseline =
        window_statistics(&record.times, &record.outputs, 0.0, 1.0, 0.1).expect("covered window");
    let final_window =
        window_statistics(&record.times, &record.outputs, 3.0, 4.0, 0.1).expect("covered window");
    let (times, outputs) = held_part(&record.times, &record.outputs);
    let points: Vec<(f64, f64)> = times.into_iter().zip(outputs).collect();
    let oracle = oracle_count(&points, baseline.mean, final_window.mean);
    assert!(
        oracle > 0,
        "the independent oracle must exercise a nonempty consistent set"
    );
    assert_eq!(set.count, oracle);
}

#[test]
fn an_impossible_measurement_bound_gives_an_empty_set_without_intervals() {
    // `returns an empty consistent set without inventing intervals`: a bound of 10⁻⁶ admits no combination.
    let delays = lattice(0.0, 0.2, 0.1).expect("the delay lattice");
    let set = record_consistency(&samples(&[]), 1e-6, &delays);
    assert_eq!(set.count, 0);
    assert_eq!(set.gain, None);
    assert_eq!(set, ConsistentSet::default());
}

#[test]
fn a_falling_response_has_negative_consistent_gains() {
    // `supports signed gain lattices`: the mirrored output 70 − y falls, so its consistent gains are negative.
    let mut reversed = samples(&[]);
    reversed.outputs = reversed
        .outputs
        .iter()
        .map(|output| 70.0 - output)
        .collect();
    let delays = lattice(0.1, 0.1, 0.1).expect("the delay lattice");
    let set = record_consistency(&reversed, 0.05, &delays);
    assert!(set.count > 0);
    assert!(set.gain.expect("a consistent gain").minimum < 0.0);
}

#[test]
fn an_immediate_response_is_fitted_with_zero_delay() {
    // `leaves zero delay as a one-sided admissible bound`, on the fit: with the delay grid {0} the best fit has zero
    // delay, and it recovers the gain 0.2 and the time constant 0.2 s of the immediate response.
    let mut immediate = samples(&[]);
    immediate.outputs = immediate
        .times
        .iter()
        .enumerate()
        .map(|(index, time)| {
            if (10..40).contains(&index) {
                35.0 + 2.0 * (1.0 - (-(time - 1.0) / 0.2).exp())
            } else {
                35.0
            }
        })
        .collect();
    let hold = hold_indices(&immediate.times);
    let hold_outputs: Vec<f64> = hold.iter().map(|&index| immediate.outputs[index]).collect();
    let deviations: Vec<f64> = immediate.inputs.iter().map(|input| input - 75.0).collect();
    let held_step = HeldStep {
        times: &immediate.times,
        deviations: &deviations,
        hold: &hold,
        hold_outputs: &hold_outputs,
        baseline: 35.0,
    };
    let time_constants = lattice(0.1, 0.4, 0.1).expect("the time-constant lattice");
    let fit = best_first_order_fit(&held_step, &[0.0], &time_constants, 0.0)
        .expect("the grid is within its limit")
        .expect("a finite gain");
    assert_eq!(fit.delay_seconds, 0.0);
    assert!((fit.time_constant_seconds - 0.2).abs() <= 1e-12);
    assert!((fit.gain - 0.2).abs() <= 1e-9, "gain {}", fit.gain);
}

#[test]
fn a_window_without_its_first_expected_sample_lacks_coverage() {
    // `requires the first baseline sample even when all remaining timestamps are uniform`, on the window: without the
    // sample at 0 s the baseline window [0, 1) s starts a whole sample time late.
    let record = samples(&[]);
    assert_eq!(
        window_statistics(&record.times[1..], &record.outputs[1..], 0.0, 1.0, 0.1),
        Err(WindowError::Coverage)
    );
    // A requested start between sampling instants does not imply a missing sample: [0.05, 1) s holds 0.1 s to 0.9 s.
    let window =
        window_statistics(&record.times, &record.outputs, 0.05, 1.0, 0.1).expect("covered window");
    assert_eq!(window.count, 9);
}

#[test]
fn a_window_with_fewer_than_two_samples_is_refused() {
    // `rejects windows too short to contain two samples`: a baseline of 0.1 s before the step and a final window of
    // 0.1 s before the restoration each hold one sample.
    let record = samples(&[]);
    assert_eq!(
        window_statistics(&record.times, &record.outputs, 0.9, 1.0, 0.1),
        Err(WindowError::TooFewSamples)
    );
    assert_eq!(
        window_statistics(&record.times, &record.outputs, 3.9, 4.0, 0.1),
        Err(WindowError::TooFewSamples)
    );
}

#[test]
fn window_statistics_are_hand_computed() {
    // Outputs 3, 2 and 5 at 1, 2 and 3 s lie in [1, 4): mean 10/3, range 3, slope (5 − 3)/(3 − 1) = 1.
    let times = [0.0, 1.0, 2.0, 3.0, 4.0];
    let outputs = [1.0, 3.0, 2.0, 5.0, 4.0];
    let window = window_statistics(&times, &outputs, 1.0, 4.0, 1.0).expect("covered window");
    assert_eq!(
        (window.start_seconds, window.end_seconds, window.count),
        (1.0, 4.0, 3)
    );
    assert!((window.mean - 10.0 / 3.0).abs() <= 1e-12);
    assert_eq!(
        (
            window.minimum,
            window.maximum,
            window.range,
            window.slope_per_second
        ),
        (2.0, 5.0, 3.0, 1.0)
    );
    // [1, 6) misses the instant at 5 s: its last sample, at 4 s, is more than a sample time before the end.
    assert_eq!(
        window_statistics(&times, &outputs, 1.0, 6.0, 1.0),
        Err(WindowError::Coverage)
    );
    // Two samples at one time have no slope.
    let window =
        window_statistics(&[1.0, 1.0], &[2.0, 4.0], 0.5, 1.5, 1.0).expect("covered window");
    assert_eq!(
        (window.mean, window.range, window.slope_per_second),
        (3.0, 2.0, 0.0)
    );
}

#[test]
fn the_grid_point_count_tolerates_rounding_and_refuses_reversed_or_unbounded_grids() {
    assert_eq!(grid_point_count(0.0, 0.2, 0.1), Some(2));
    // (0.4 − 0.1)/0.1 is 3.0000000000000004 in binary arithmetic; the count is still 3.
    assert_eq!(grid_point_count(0.1, 0.4, 0.1), Some(3));
    assert_eq!(
        grid_point_count(0.0, 1.0, 0.3),
        Some(3),
        "a partial last step"
    );
    assert_eq!(grid_point_count(5.0, 5.0, 1.0), Some(0));
    assert_eq!(
        grid_point_count(1.0, 1.0 - 1e-10, 1.0),
        Some(0),
        "short by less than 10⁻⁹ steps"
    );
    assert_eq!(grid_point_count(1.0, 0.0, 0.1), None, "a reversed grid");
    assert_eq!(grid_point_count(0.0, f64::INFINITY, 1.0), None);
    assert_eq!(grid_point_count(0.0, 1.0, 0.0), None, "a zero step");
    assert_eq!(grid_point_count(0.0, 0.0, 0.0), None, "0/0");
    assert_eq!(
        grid_point_count(0.0, 1e30, 1.0),
        Some(usize::MAX),
        "a huge count saturates"
    );
}

#[test]
fn a_lattice_steps_from_its_minimum_and_refuses_oversized_grids() {
    assert_eq!(lattice(0.0, 0.2, 0.1), Ok(vec![0.0, 0.1, 0.2]));
    let time_constants = lattice(0.1, 0.4, 0.1).expect("four points");
    assert_eq!(time_constants.len(), 4);
    for (value, want) in time_constants.iter().zip([0.1, 0.2, 0.3, 0.4]) {
        assert!((value - want).abs() <= 1e-12, "{value} is not {want}");
    }
    // (−0.1 + 0.3)/0.1 is 1.9999999999999998 in binary arithmetic; the lattice still has three points.
    let signed = lattice(-0.3, -0.1, 0.1).expect("three points");
    assert_eq!(signed.len(), 3);
    assert!((signed[2] + 0.1).abs() <= 1e-12);
    assert_eq!(lattice(5.0, 5.0, 1.0), Ok(vec![5.0]));
    let largest_lattice = lattice(0.0, 1e6, 1.0).expect("the largest lattice");
    assert_eq!(largest_lattice.len(), MAXIMUM_GRID_POINTS + 1);
    assert_eq!(lattice(0.0, 1e6 + 1.0, 1.0), Err(GridError::TooLarge));
    assert_eq!(
        lattice(1.0, 0.0, 0.1),
        Err(GridError::TooLarge),
        "a reversed grid"
    );
}

/// A window whose outputs all equal `level`.
fn flat_window(level: f64) -> StepResponseWindow {
    StepResponseWindow {
        start_seconds: 0.0,
        end_seconds: 1.0,
        count: 2,
        mean: level,
        minimum: level,
        maximum: level,
        range: 0.0,
        slope_per_second: 0.0,
    }
}

#[test]
fn the_gain_bounds_widen_the_window_difference_by_the_bound_on_each_side() {
    // (30 − 0.5 − 10.5)/4 = 4.75 and (30.5 − 9.5)/4 = 5.25.
    let bounds = gain_bounds(&flat_window(10.0), &flat_window(30.0), 4.0, 0.5);
    assert_eq!((bounds.minimum, bounds.maximum), (4.75, 5.25));
    // A falling input change swaps the ends.
    let bounds = gain_bounds(&flat_window(10.0), &flat_window(30.0), -4.0, 0.5);
    assert_eq!((bounds.minimum, bounds.maximum), (-5.25, -4.75));
    // An infinite baseline and bound give the ends −∞ and ∞ − (∞ − ∞) = NaN; one NaN end makes both NaN.
    let bounds = gain_bounds(
        &flat_window(f64::INFINITY),
        &flat_window(0.0),
        1.0,
        f64::INFINITY,
    );
    assert!(bounds.minimum.is_nan() && bounds.maximum.is_nan());
}

#[test]
fn the_normalized_rms_divides_the_rms_residual_by_the_span() {
    // √((9 + 16)/2) over the span 2.
    assert!((normalized_rms(&[3.0, 4.0], 2.0) - 12.5_f64.sqrt() / 2.0).abs() <= 1e-12);
    // The plain span has no floor (N3.2): a span of 10⁻¹² gives an error of one for a residual of 10⁻¹². A floor of 10⁻⁹
    // would give 10⁻³ here.
    assert!((normalized_rms(&[1e-12], 1e-12) - 1.0).abs() <= 1e-9);
    assert!(normalized_rms(&[], 1.0).is_nan());
}

#[test]
fn the_onset_is_the_first_full_run_beyond_the_threshold() {
    let times = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
    // Rising: of the crossings at 1 s and 3 s, only the run from 3 s holds two samples above 1.
    assert_eq!(
        onset_time(&times, &[0.0, 2.0, 0.0, 2.0, 2.0, 2.0], 1.0, 2, 1.0),
        Some(3.0)
    );
    // Falling: the mirror image below −1.
    assert_eq!(
        onset_time(&times, &[0.0, -2.0, 0.0, -2.0, -2.0, -2.0], -1.0, 2, -1.0),
        Some(3.0)
    );
    // The last run that can qualify starts at n − m.
    assert_eq!(
        onset_time(&times[..4], &[0.0, 0.0, 2.0, 2.0], 1.0, 2, 1.0),
        Some(2.0)
    );
    // An isolated crossing, fewer samples than the run, and a direction of zero establish no onset.
    assert_eq!(
        onset_time(&times[..4], &[0.0, 2.0, 0.0, 2.0], 1.0, 2, 1.0),
        None
    );
    assert_eq!(onset_time(&times[..2], &[2.0, 2.0], 1.0, 3, 1.0), None);
    assert_eq!(onset_time(&times[..2], &[2.0, 2.0], 1.0, 1, 0.0), None);
}

/// The hand-computed held step: unit input deviations from the sample at 1 s, held at 1, 2 and 3 s.
const HAND_TIMES: [f64; 4] = [0.0, 1.0, 2.0, 3.0];
const HAND_DEVIATIONS: [f64; 4] = [0.0, 1.0, 1.0, 1.0];
const HAND_HOLD: [usize; 3] = [1, 2, 3];

#[test]
fn the_first_order_search_recovers_the_exact_candidate() {
    // A unit deviation held from 1 s through a lag of 1 s responds 0, 1 − e⁻¹ and 1 − e⁻² at 1, 2 and 3 s. Outputs of
    // twice that are fitted exactly by the gain 2 with no delay, among the delays 0 and 1 s and the time constants
    // 0.5, 1 and 2 s.
    let after_one = 1.0 - (-1.0_f64).exp();
    let after_two = 1.0 - (-2.0_f64).exp();
    let hold_outputs = [0.0, 2.0 * after_one, 2.0 * after_two];
    let held = HeldStep {
        times: &HAND_TIMES,
        deviations: &HAND_DEVIATIONS,
        hold: &HAND_HOLD,
        hold_outputs: &hold_outputs,
        baseline: 0.0,
    };
    let fit = best_first_order_fit(&held, &[0.0, 1.0], &[0.5, 1.0, 2.0], 0.0)
        .expect("a small grid")
        .expect("a finite gain");
    assert_eq!((fit.delay_seconds, fit.time_constant_seconds), (0.0, 1.0));
    assert!((fit.gain - 2.0).abs() <= 1e-12, "gain {}", fit.gain);
    assert!(fit.squared_error <= 1e-24, "error {}", fit.squared_error);
    for (value, want) in fit.response.iter().zip([0.0, 0.0, after_one, after_two]) {
        assert!((value - want).abs() <= 1e-12, "{value} is not {want}");
    }
    // A timestamp shift of 0.5 s simulates the grid delay 0.5 s with no delay; the grid delay is reported.
    let shifted = best_first_order_fit(&held, &[0.5], &[1.0], 0.5)
        .expect("a small grid")
        .expect("a finite gain");
    assert_eq!(shifted.delay_seconds, 0.5);
    assert!((shifted.gain - 2.0).abs() <= 1e-12, "gain {}", shifted.gain);
}

#[test]
fn the_first_of_equal_errors_is_kept() {
    // On samples one second apart, delays of 0.5 s and 0.9 s both delay the input by one sample, so their responses
    // and errors are equal: the first delay is kept.
    let held = HeldStep {
        times: &HAND_TIMES,
        deviations: &HAND_DEVIATIONS,
        hold: &HAND_HOLD,
        hold_outputs: &[0.0, 0.0, 1.0],
        baseline: 0.0,
    };
    let fit = best_first_order_fit(&held, &[0.5, 0.9], &[1.0], 0.0)
        .expect("a small grid")
        .expect("a finite gain");
    assert_eq!(fit.delay_seconds, 0.5);
}

#[test]
fn an_oversized_search_is_refused_before_it_runs_and_a_flat_response_has_no_fit() {
    // 10 001 delays × 10 000 time constants × max(1, no held samples) is more than 10⁸ evaluations.
    let delays = vec![0.0; 10_001];
    let time_constants = vec![1.0; 10_000];
    let empty = HeldStep {
        times: &[],
        deviations: &[],
        hold: &[],
        hold_outputs: &[],
        baseline: 0.0,
    };
    assert_eq!(
        best_first_order_fit(&empty, &delays, &time_constants, 0.0),
        Err(GridError::TooLarge)
    );
    // Without an input deviation the response has no energy, so no gain is finite.
    let flat = HeldStep {
        times: &HAND_TIMES,
        deviations: &[0.0; 4],
        hold: &HAND_HOLD,
        hold_outputs: &[1.0, 2.0, 3.0],
        baseline: 0.0,
    };
    assert_eq!(best_first_order_fit(&flat, &[0.0], &[1.0], 0.0), Ok(None));
}

/// A search over one held sample whose lag of 10⁻³ s settles within its one-second interval, so its unit response is
/// exactly one: the deviation 2 at both instants, the output 12.2, the baseline 10, the bound 0.5 and the baseline
/// lattice `9.5 + 0.1·j`.
fn settled_search(gains: &[f64], final_target: Option<f64>) -> ConsistencySearch<'_> {
    ConsistencySearch {
        held: HeldStep {
            times: &[0.0, 1.0],
            deviations: &[2.0, 2.0],
            hold: &[1],
            hold_outputs: &[12.2],
            baseline: 10.0,
        },
        timestamp_shift_seconds: 0.0,
        input_change: 2.0,
        measurement_bound: 0.5,
        baseline_step: 0.1,
        gains,
        delays: &[0.0],
        time_constants: &[1e-3],
        final_target,
    }
}

#[test]
fn the_consistent_set_counts_the_baselines_each_gain_allows() {
    // The gain K leaves the sample at 12.2 − 2K, so a baseline c is consistent when |12.2 − 2K − c| ≤ 0.5 and
    // |c − 10| ≤ 0.5. Gain 0.9 allows 9.9 to 10.5 (7 lattice points), gain 1 allows 9.7 to 10.5 (9), gain 1.1 allows
    // 9.5 to 10.5 (11) and gain 2 allows none.
    let set = consistent_parameter_set(&settled_search(&[0.9, 1.0, 1.1, 2.0], None));
    assert_eq!(set.count, 27);
    assert_eq!(
        set.gain,
        Some(ParameterInterval {
            minimum: 0.9,
            maximum: 1.1
        })
    );
    assert_eq!(
        set.delay_seconds,
        Some(ParameterInterval {
            minimum: 0.0,
            maximum: 0.0
        })
    );
    assert_eq!(
        set.time_constant_seconds,
        Some(ParameterInterval {
            minimum: 1e-3,
            maximum: 1e-3
        })
    );
    assert_eq!(
        set.baseline,
        Some(ParameterInterval {
            minimum: 9.5,
            maximum: 10.5
        })
    );
}

#[test]
fn a_steady_final_window_bounds_the_steady_value() {
    // With the final mean 12, the steady value c + 2K must also lie within 0.5 of 12. Gain 1.1 then allows only 9.5
    // to 10.3 (9 lattice points), and the count falls from 27 to 25.
    let set = consistent_parameter_set(&settled_search(&[0.9, 1.0, 1.1, 2.0], Some(12.0)));
    assert_eq!(set.count, 25);
    assert_eq!(
        set.gain,
        Some(ParameterInterval {
            minimum: 0.9,
            maximum: 1.1
        })
    );
}

#[test]
fn a_combination_whose_bounds_are_not_numbers_adds_nothing() {
    // An input change of zero makes the unit response 2/0 and every bound 0·∞ = NaN: the combination adds no
    // candidate, where the `TypeScript` would count NaN.
    let mut search = settled_search(&[1.0], None);
    search.input_change = 0.0;
    assert_eq!(consistent_parameter_set(&search), ConsistentSet::default());
}
