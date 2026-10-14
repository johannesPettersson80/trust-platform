//! Tests of the sensor-lag fit (N2.2, N2.4, N3.2): the closed-form second-order step, the exact recovery of a clean held
//! step on the grid, the exhaustive check of the measurement-consistent set, the empty set of an inconsistent record, the
//! sign of a falling response, and the refusals of oversized searches. The TypeScript cases are the math parts of
//! `control-studio-sensor-lag-comparison.test.ts`: "matches exhaustive grid fitting and every measurement-consistent
//! combination", "fits a falling temperature response with signed gain", "retains an empty measurement-consistent set",
//! "rejects malformed or excessive sensor grids", and "fits the repeated-pole limit".

#![allow(clippy::cast_precision_loss)] // Sample counts and indices are far below 2⁵³.
#![allow(clippy::float_cmp)] // The zero response before a dead time is exact.

use super::{
    fit_sensor_lag_step, second_order_dead_time_step, HeldSample, SearchRange, SensorLagError,
    SensorLagSearch, StepReference,
};
use crate::identify::step_response::{gain_bounds, lattice, window_statistics, StepResponseWindow};
use crate::identify::ParameterInterval;

/// The input change of the test records: the plant input moves from 75 to 85 at time zero.
const INPUT_CHANGE: f64 = 10.0;
/// The steady output before the step.
const BASELINE: f64 = 35.0;

/// A held step: its samples, the baseline window before the step, and the two windows of the fit.
struct Record {
    hold: Vec<HeldSample>,
    pre_step: Vec<f64>,
    baseline: StepResponseWindow,
    final_window: StepResponseWindow,
}

impl Record {
    fn reference(&self) -> StepReference<'_> {
        StepReference {
            baseline: &self.baseline,
            final_window: &self.final_window,
            input_change: INPUT_CHANGE,
            step_time_seconds: 0.0,
        }
    }
}

/// A record sampled every 0.2 s: 25 baseline samples from −5 s, and 225 held samples from the step to 45 s. The held
/// output is `35 + Δu·K·S(t − L)` for a second-order response with time constants `τ`, `τ₂` and dead time `L`, plus `noise(k)`.
fn record(
    gain: f64,
    time_constant: f64,
    second: f64,
    delay: f64,
    noise: impl Fn(usize) -> f64,
) -> Record {
    let pre_times: Vec<f64> = (0..25).map(|k| -5.0 + 0.2 * f64::from(k)).collect();
    let pre_step: Vec<f64> = (0..25).map(|k| BASELINE + noise(k)).collect();
    let hold_times: Vec<f64> = (0..225).map(|k| 0.2 * f64::from(k)).collect();
    let hold: Vec<HeldSample> = hold_times
        .iter()
        .enumerate()
        .map(|(k, &time_seconds)| HeldSample {
            time_seconds,
            output: BASELINE
                + INPUT_CHANGE
                    * gain
                    * second_order_dead_time_step(time_seconds, time_constant, second, delay)
                + noise(25 + k),
        })
        .collect();
    let all_times: Vec<f64> = pre_times.iter().chain(&hold_times).copied().collect();
    let all_outputs: Vec<f64> = pre_step
        .iter()
        .copied()
        .chain(hold.iter().map(|sample| sample.output))
        .collect();
    let baseline =
        window_statistics(&all_times, &all_outputs, -5.0, 0.0, 0.2).expect("a baseline window");
    let final_window =
        window_statistics(&all_times, &all_outputs, 40.0, 45.0, 0.2).expect("a final window");
    Record {
        hold,
        pre_step,
        baseline,
        final_window,
    }
}

fn clean(gain: f64, time_constant: f64, second: f64, delay: f64) -> Record {
    record(gain, time_constant, second, delay, |_| 0.0)
}

/// The grids of the TypeScript fixture (`settings` of the sensor-lag test), with its measurement bound of 0.05.
fn fixture_search() -> SensorLagSearch {
    SensorLagSearch {
        delay: SearchRange {
            minimum: 0.0,
            maximum: 3.0,
            step: 0.5,
        },
        time_constant: SearchRange {
            minimum: 1.0,
            maximum: 9.0,
            step: 0.5,
        },
        sensor_time_constant: SearchRange {
            minimum: 0.5,
            maximum: 6.0,
            step: 0.5,
        },
        measurement_bound: 0.05,
        baseline_step: 0.01,
        gain_step: 0.001,
        timestamp_shift_seconds: 0.0,
    }
}

#[test]
fn the_second_order_step_has_its_closed_form_and_limits() {
    // τ₁ = 1, τ₂ = 2 at t = 1: S = 1 + e^(−1) − 2e^(−1/2).
    let expected = 1.0 + (-1.0_f64).exp() - 2.0 * (-0.5_f64).exp();
    assert!((second_order_dead_time_step(1.0, 1.0, 2.0, 0.0) - expected).abs() < 1e-12);
    // Equal time constants take the limit 1 − (1 + t/τ)e^(−t/τ); a near-equal pair agrees with it.
    let equal = 1.0 - 2.0 * (-1.0_f64).exp();
    assert!((second_order_dead_time_step(2.0, 2.0, 2.0, 0.0) - equal).abs() < 1e-12);
    assert!((second_order_dead_time_step(2.0, 2.0, 2.0 + 1e-7, 0.0) - equal).abs() < 1e-6);
    // Zero before the dead time, one far after the step, and zero at the step itself without a dead time.
    assert_eq!(second_order_dead_time_step(1.9, 5.0, 2.0, 2.0), 0.0);
    assert!((second_order_dead_time_step(1e4, 5.0, 2.0, 0.0) - 1.0).abs() < 1e-12);
    assert!(second_order_dead_time_step(0.0, 5.0, 2.0, 0.0).abs() < 1e-12);
}

#[test]
fn a_clean_held_step_is_recovered_on_the_grid() {
    // The generating shape lies on the grids: L = 1 (0 + 2·0.5), τ = 5 (1 + 8·0.5) and τ₂ = 2 (0.5 + 3·0.5). Swapping the
    // two time constants gives the same response, so the fit may report either assignment.
    let data = clean(0.2, 5.0, 2.0, 1.0);
    let fit = fit_sensor_lag_step(
        &data.hold,
        &data.pre_step,
        &data.reference(),
        &fixture_search(),
    )
    .expect("a clean record is fitted");
    let best = fit.best;
    assert!((best.delay_seconds - 1.0).abs() < 1e-9);
    let (first, second) = (
        best.time_constant_seconds,
        best.second_time_constant_seconds,
    );
    let swapped = (first - 2.0).abs() < 1e-9 && (second - 5.0).abs() < 1e-9;
    assert!(
        swapped || ((first - 5.0).abs() < 1e-9 && (second - 2.0).abs() < 1e-9),
        "{best:?}"
    );
    assert!((best.gain - 0.2).abs() < 1e-3, "{best:?}");
    assert!((best.baseline - BASELINE).abs() < 0.01, "{best:?}");
    assert!(best.squared_error < 0.05, "{best:?}");
    assert_eq!(fit.predicted.len(), data.hold.len());
    assert!(!fit.amplitude_on_boundary && !fit.shape_on_boundary);
    // The truth is measurement consistent: its delay, a time constant and the gain lie inside the consistent intervals.
    let consistency = &fit.consistency;
    assert!(consistency.count > 0);
    assert!(contains(consistency.delay_seconds, 1.0));
    assert!(contains(consistency.time_constant_seconds, 5.0));
    assert!(contains(consistency.second_time_constant_seconds, 2.0));
    assert!(contains(consistency.gain, 0.2));
}

#[test]
fn a_falling_response_is_fitted_with_a_negative_gain() {
    // TypeScript: "fits a falling temperature response with signed gain on the same positive lag grid" (gain −0.2).
    let data = clean(-0.2, 5.0, 2.0, 1.0);
    let fit = fit_sensor_lag_step(
        &data.hold,
        &data.pre_step,
        &data.reference(),
        &fixture_search(),
    )
    .expect("a falling record is fitted");
    assert!((fit.best.gain + 0.2).abs() < 2e-3, "{:?}", fit.best);
}

#[test]
#[allow(clippy::too_many_lines)] // One enumeration of every combination, checked against the search in one place.
fn the_consistent_set_matches_an_exhaustive_enumeration() {
    // TypeScript: "matches exhaustive grid fitting and every measurement-consistent combination". The grids are small
    // enough to enumerate every combination of shape, gain and baseline, and the enumeration is the check.
    let data = record(0.2, 5.0, 2.0, 1.0, |k| 0.03 * (1.7 * k as f64).sin());
    let search = SensorLagSearch {
        delay: SearchRange {
            minimum: 0.5,
            maximum: 1.5,
            step: 0.5,
        },
        time_constant: SearchRange {
            minimum: 4.0,
            maximum: 6.0,
            step: 1.0,
        },
        sensor_time_constant: SearchRange {
            minimum: 1.0,
            maximum: 3.0,
            step: 1.0,
        },
        baseline_step: 0.05,
        gain_step: 0.005,
        ..fixture_search()
    };
    let fit = fit_sensor_lag_step(&data.hold, &data.pre_step, &data.reference(), &search)
        .expect("the small grids are fitted");

    let bound = search.measurement_bound;
    let bounds = gain_bounds(&data.baseline, &data.final_window, INPUT_CHANGE, bound);
    let gains = lattice(bounds.minimum, bounds.maximum, search.gain_step).expect("gains");
    let baselines = lattice(
        data.baseline.mean - bound,
        data.baseline.mean + bound,
        search.baseline_step,
    )
    .expect("baselines");
    let delays = lattice(0.5, 1.5, 0.5).expect("delays");
    let time_constants = lattice(4.0, 6.0, 1.0).expect("time constants");
    let sensors = lattice(1.0, 3.0, 1.0).expect("sensors");
    let mut count = 0;
    let mut best = f64::INFINITY;
    let mut spans: [Option<(f64, f64)>; 5] = [None; 5];
    for &delay in &delays {
        for &time_constant in &time_constants {
            for &sensor in &sensors {
                let phi: Vec<f64> = data
                    .hold
                    .iter()
                    .map(|sample| {
                        INPUT_CHANGE
                            * second_order_dead_time_step(
                                sample.time_seconds,
                                time_constant,
                                sensor,
                                delay,
                            )
                    })
                    .collect();
                for &gain in &gains {
                    let mut consistent_here = false;
                    for &baseline in &baselines {
                        let final_ok = (data.final_window.mean - baseline - gain * INPUT_CHANGE)
                            .abs()
                            <= bound + 1e-9;
                        if final_ok {
                            let squares: f64 = data
                                .hold
                                .iter()
                                .zip(&phi)
                                .map(|(sample, value)| {
                                    (sample.output - baseline - gain * value).powi(2)
                                })
                                .sum();
                            best = best.min(squares);
                        }
                        let pre_ok = data
                            .pre_step
                            .iter()
                            .all(|output| (output - baseline).abs() <= bound + 1e-9);
                        let hold_ok = data.hold.iter().zip(&phi).all(|(sample, value)| {
                            (sample.output - baseline - gain * value).abs() <= bound + 1e-9
                        });
                        if final_ok && pre_ok && hold_ok {
                            count += 1;
                            consistent_here = true;
                            span(&mut spans[4], baseline);
                        }
                    }
                    if consistent_here {
                        span(&mut spans[0], gain);
                        span(&mut spans[1], delay);
                        span(&mut spans[2], time_constant);
                        span(&mut spans[3], sensor);
                    }
                }
            }
        }
    }
    assert!(count > 0, "the enumeration must find a consistent point");
    assert_eq!(fit.consistency.count, count);
    assert!(
        (fit.best.squared_error - best).abs() < 1e-9,
        "{} vs {best}",
        fit.best.squared_error
    );
    let fitted = [
        fit.consistency.gain,
        fit.consistency.delay_seconds,
        fit.consistency.time_constant_seconds,
        fit.consistency.second_time_constant_seconds,
        fit.consistency.baseline,
    ];
    for (interval, expected) in fitted.into_iter().zip(spans) {
        let interval = interval.expect("the consistent set holds this parameter");
        let (low, high) = expected.expect("the enumeration holds this parameter");
        assert!(
            (interval.minimum - low).abs() < 1e-9,
            "{interval:?} vs {low}"
        );
        assert!(
            (interval.maximum - high).abs() < 1e-9,
            "{interval:?} vs {high}"
        );
    }
}

#[test]
fn an_inconsistent_record_keeps_an_empty_consistent_set() {
    // TypeScript: "retains an empty measurement-consistent set even when the model is adequate for a looser task".
    let data = record(0.2, 5.0, 2.0, 1.0, |k| if k == 60 { 0.15 } else { 0.0 });
    let search = SensorLagSearch {
        measurement_bound: 0.005,
        baseline_step: 0.001,
        gain_step: 0.0001,
        ..fixture_search()
    };
    let fit = fit_sensor_lag_step(&data.hold, &data.pre_step, &data.reference(), &search)
        .expect("the best candidate is still found");
    assert_eq!(fit.consistency.count, 0);
    assert_eq!(fit.consistency.gain, None);
}

#[test]
fn oversized_searches_and_grids_are_refused() {
    let data = clean(0.2, 5.0, 2.0, 1.0);
    let too_many_shapes = SensorLagSearch {
        delay: SearchRange {
            minimum: 0.0,
            maximum: 1.0e6,
            step: 1.0e-3,
        },
        ..fixture_search()
    };
    assert_eq!(
        fit_sensor_lag_step(
            &data.hold,
            &data.pre_step,
            &data.reference(),
            &too_many_shapes
        ),
        Err(SensorLagError::SearchTooLarge)
    );
    let fine_gains = SensorLagSearch {
        gain_step: 1.0e-12,
        ..fixture_search()
    };
    assert_eq!(
        fit_sensor_lag_step(&data.hold, &data.pre_step, &data.reference(), &fine_gains),
        Err(SensorLagError::GridTooLarge)
    );
}

/// Whether an interval exists and contains a value.
fn contains(interval: Option<ParameterInterval>, value: f64) -> bool {
    interval.is_some_and(|known| known.minimum - 1e-9 <= value && value <= known.maximum + 1e-9)
}

/// Widens a `(minimum, maximum)` span to include a value.
fn span(slot: &mut Option<(f64, f64)>, value: f64) {
    *slot = Some(match *slot {
        None => (value, value),
        Some((low, high)) => (low.min(value), high.max(value)),
    });
}
