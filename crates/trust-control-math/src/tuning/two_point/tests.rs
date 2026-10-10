use super::*;

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn a_linear_ramp_reduces_exactly_by_its_interpolated_fractions() {
    // y = t over 0…10 s: the final change is 10, t28 = 0.283·10 = 2.83 s and t63 = 0.632·10 = 6.32 s.
    // τ = 1.5·(6.32 − 2.83) = 5.235 s and L = 6.32 − 5.235 = 1.085 s. Linear interpolation is exact here.
    let times: Vec<f64> = (0..=10).map(f64::from).collect();
    let responses = times.clone();
    let reduction = two_point_reduction(&times, &responses).expect("reduction");
    assert_close(reduction.gain, 10.0, 1e-12);
    assert_close(reduction.time_constant_seconds, 5.235, 1e-9);
    assert_close(reduction.delay_seconds, 1.085, 1e-9);
}

#[test]
fn a_first_order_lag_has_no_delay_and_takes_its_time_constant_from_t63() {
    // y = 1 − e^(−t) has t63 = −ln(0.368) ≈ 0.99968 s. The delay candidate would be negative (about
    // −0.0008 s), so the reduction uses L = 0 and τ = t63, as N5.15 states.
    let step = 1e-3;
    let times: Vec<f64> = (0..=15_000).map(|index| f64::from(index) * step).collect();
    let responses: Vec<f64> = times.iter().map(|&time| 1.0 - (-time).exp()).collect();
    let reduction = two_point_reduction(&times, &responses).expect("reduction");
    assert_close(reduction.gain, 1.0, 1e-6);
    assert_close(reduction.delay_seconds, 0.0, 1e-15);
    assert_close(
        reduction.time_constant_seconds,
        -(1.0 - 0.632_f64).ln(),
        2e-3,
    );
}

#[test]
fn a_delayed_first_order_step_is_recovered_by_the_closed_form_reduction() {
    // y = 1 − e^(−(t − L)/τ) for t ≥ L, with L = 0.5 s and τ = 2 s. The closed-form crossing times are
    // t28 = L − τ·ln(1 − 0.283) and t63 = L − τ·ln(1 − 0.632). The reduction of those exact times is
    // τ' = 1.5·(t63 − t28) and L' = t63 − τ', which is what the rule must return.
    let (delay, tau) = (0.5_f64, 2.0_f64);
    let step = 1e-3;
    let times: Vec<f64> = (0..=40_000).map(|index| f64::from(index) * step).collect();
    let responses: Vec<f64> = times
        .iter()
        .map(|&time| {
            if time < delay {
                0.0
            } else {
                1.0 - (-(time - delay) / tau).exp()
            }
        })
        .collect();
    let time_low = delay - tau * (1.0 - 0.283_f64).ln();
    let time_high = delay - tau * (1.0 - 0.632_f64).ln();
    let expected_tau = 1.5 * (time_high - time_low);
    let expected_delay = time_high - expected_tau;
    let reduction = two_point_reduction(&times, &responses).expect("reduction");
    assert_close(reduction.gain, 1.0, 1e-6);
    assert_close(reduction.time_constant_seconds, expected_tau, 1e-4);
    assert_close(reduction.delay_seconds, expected_delay, 1e-4);
    // The reduction recovers the true delay to within the method's own bias, about 2 ms here.
    assert_close(reduction.delay_seconds, delay, 3e-3);
}

#[test]
fn unusable_responses_are_refused_with_their_reason() {
    assert_eq!(
        two_point_reduction(&[0.0, 1.0], &[1.0]),
        Err(ReductionRefusal::Shape)
    );
    assert_eq!(
        two_point_reduction(&[0.0, 0.0, 1.0], &[0.0, 0.5, 1.0]),
        Err(ReductionRefusal::InvalidSamples)
    );
    assert_eq!(
        two_point_reduction(&[0.0, 1.0, 2.0], &[0.0, 0.0, 0.0]),
        Err(ReductionRefusal::NoSteadyChange)
    );
    // The response is already at 50 % at its first sample, so the 28.3 % fraction has no crossing to read.
    assert_eq!(
        two_point_reduction(&[0.0, 1.0, 2.0], &[0.5, 0.5, 0.5]),
        Err(ReductionRefusal::NotReached)
    );
}
