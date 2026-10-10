#![allow(clippy::float_cmp)] // The held responses are compared with exact values on purpose.

use super::*;

#[test]
fn a_first_order_lag_responds_to_a_constant_held_input_as_one_minus_its_decay() {
    let times = [0.0, 1.0, 2.0, 3.0];
    let deviations = [1.0, 1.0, 1.0, 1.0];
    let response = held_unit_response(
        &times,
        &deviations,
        HeldShape::FirstOrder {
            time_constant_seconds: 1.0,
            delay_seconds: 0.0,
        },
    );
    let expected = [
        0.0,
        1.0 - (-1.0_f64).exp(),
        1.0 - (-2.0_f64).exp(),
        1.0 - (-3.0_f64).exp(),
    ];
    for (value, want) in response.iter().zip(expected) {
        assert!((value - want).abs() <= 1e-12, "{value} is not {want}");
    }
}

#[test]
fn the_delayed_input_enters_an_integrator_one_dead_time_later() {
    let times = [0.0, 1.0, 2.0, 3.0];
    let deviations = [1.0, 2.0, 3.0, 4.0];
    // Delayed by one second the input reads [0, 1, 2, 3]; its integral over the intervals is [0, 0, 1, 3].
    let response = held_unit_response(
        &times,
        &deviations,
        HeldShape::Integrating { delay_seconds: 1.0 },
    );
    assert_eq!(response, [0.0, 0.0, 1.0, 3.0]);
}

#[test]
fn the_least_squares_gain_recovers_a_scaled_response_exactly() {
    let response = [0.0, 0.5, 1.0, 1.0];
    let measured: Vec<f64> = response.iter().map(|value| 10.0 + 2.0 * value).collect();
    let fitted = least_squares_gain(&measured, 10.0, &response);
    assert!((fitted.gain - 2.0).abs() <= 1e-12);
    assert!(
        fitted.squared_error <= 1e-12,
        "the exact gain leaves no error"
    );
    assert!(
        least_squares_gain(&[1.0, 2.0], 0.0, &[0.0, 0.0])
            .gain
            .is_nan(),
        "a response without energy gives no gain"
    );
}

#[test]
fn the_reached_fraction_uses_the_last_five_percent_and_at_least_three_samples() {
    // Twenty samples at the baseline and the last three at 0.8 of a unit change: the three-sample tail gives 0.8.
    let mut measured = vec![0.0; 17];
    measured.extend([0.8, 0.8, 0.8]);
    assert!((reached_fraction(&measured, 0.0, 1.0) - 0.8).abs() <= 1e-12);
    // Ten samples with only the last one above the baseline: the tail is three samples, not the one that
    // 5 % of ten would give, so the fraction is one third.
    let mut short = vec![0.0; 9];
    short.push(1.0);
    assert!((reached_fraction(&short, 0.0, 1.0) - 1.0 / 3.0).abs() <= 1e-12);
}

#[test]
fn the_step_amplitude_is_the_mean_hold_input_less_the_baseline_mean() {
    // Specification N2.2: Δu = ū_hold − u₀. A hold whose input ramps to its final value of 2 has the mean of
    // (1.0 + 1.5 + 2.0 + 2.0 + 2.0) / 5 = 1.7 over the hold, so Δu = 1.7 − 0 = 1.7. The step sample alone would give 1.0.
    let hold = [1.0, 1.5, 2.0, 2.0, 2.0];
    assert!((step_amplitude(0.0, &hold) - 1.7).abs() <= 1e-12);
    assert!((step_amplitude(0.5, &[2.5, 2.5]) - 2.0).abs() <= 1e-12);
}
