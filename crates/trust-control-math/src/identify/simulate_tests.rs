#![allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)] // The times are small sample counts scaled by exact step sizes.

use super::*;

/// The shape of a single time constant.
fn lag_shape(time_constant_seconds: f64) -> LowOrderShape {
    LowOrderShape {
        time_constant_seconds: Some(time_constant_seconds),
        second_time_constant_seconds: None,
        dead_time_seconds: None,
    }
}

#[test]
fn a_first_order_lag_responds_to_a_constant_held_input_as_one_minus_its_exponential() {
    // N1.2: the held input is discretized exactly, so for a constant deviation 1 the response at tₖ is 1 − e^(−tₖ/τ) with
    // the first sample at zero. Here τ = 3 s and the samples are 0.37 s apart.
    let times: Vec<f64> = (0..20_u32).map(|index| f64::from(index) * 0.37).collect();
    let deviations = vec![1.0; times.len()];
    let response = unit_response(
        LowOrderModelKind::FirstOrder,
        lag_shape(3.0),
        &times,
        &deviations,
    );
    for (time, value) in times.iter().zip(&response) {
        let expected = 1.0 - (-time / 3.0).exp();
        assert!(
            (value - expected).abs() <= 1e-12,
            "at {time} s: {value} against {expected}"
        );
    }
}

#[test]
fn two_lags_in_cascade_match_their_closed_form_step_response() {
    // N2.1 item 5: the step response of 1/((τ₁s + 1)(τ₂s + 1)) is 1 − (τ₁e^(−t/τ₁) − τ₂e^(−t/τ₂))/(τ₁ − τ₂), and for equal
    // lags 1 − (1 + t/τ)e^(−t/τ). The exact two-state update of N1.2 reproduces both at every sample of a held unit step.
    let times: Vec<f64> = (0..200_u32).map(|index| f64::from(index) * 0.37).collect();
    let deviations = vec![1.0; times.len()];
    let shape = LowOrderShape {
        time_constant_seconds: Some(4.0),
        second_time_constant_seconds: Some(1.5),
        dead_time_seconds: None,
    };
    let distinct = unit_response(LowOrderModelKind::SecondOrder, shape, &times, &deviations);
    for (time, value) in times.iter().zip(&distinct) {
        let expected = 1.0 - (4.0 * (-time / 4.0).exp() - 1.5 * (-time / 1.5).exp()) / (4.0 - 1.5);
        assert!(
            (value - expected).abs() <= 1e-12,
            "distinct lags at {time} s: {value} against {expected}"
        );
    }
    let equal_shape = LowOrderShape {
        time_constant_seconds: Some(2.0),
        second_time_constant_seconds: Some(2.0),
        dead_time_seconds: None,
    };
    let equal = unit_response(
        LowOrderModelKind::SecondOrder,
        equal_shape,
        &times,
        &deviations,
    );
    for (time, value) in times.iter().zip(&equal) {
        let normalized = time / 2.0;
        let expected = 1.0 - (1.0 + normalized) * (-normalized).exp();
        assert!(
            (value - expected).abs() <= 1e-12,
            "equal lags at {time} s: {value} against {expected}"
        );
    }
}

#[test]
fn a_delayed_input_is_the_latest_recorded_input_at_or_before_the_delayed_time() {
    // N1.2: the delayed input at tₖ − L is the latest recorded input at or before that time. With samples every 0.1 s and
    // L = 0.25 s, sample k reads sample k − 3 (at 0.1·(k − 3) ≤ 0.1·k − 0.25 < 0.1·(k − 2)), and the first three samples have
    // no recorded input yet, so they read zero. A static gain passes that delayed input straight through.
    let times: Vec<f64> = (0..10_u32).map(|index| f64::from(index) * 0.1).collect();
    let deviations: Vec<f64> = (0..10_i32).map(f64::from).collect();
    let shape = LowOrderShape {
        time_constant_seconds: None,
        second_time_constant_seconds: None,
        dead_time_seconds: Some(0.25),
    };
    let response = unit_response(LowOrderModelKind::StaticGain, shape, &times, &deviations);
    assert_eq!(
        response,
        vec![0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
    );
}

#[test]
fn an_integrator_adds_the_delayed_input_times_each_interval() {
    // N2.1 item 4: with a held input 2 over intervals of 0.5 s, the integral grows by one per sample, from zero at the first.
    let times: Vec<f64> = (0..5_u32).map(|index| f64::from(index) * 0.5).collect();
    let deviations = vec![2.0; times.len()];
    let response = unit_response(
        LowOrderModelKind::Integrating,
        LowOrderShape::default(),
        &times,
        &deviations,
    );
    assert_eq!(response, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn uneven_intervals_are_discretized_exactly_for_a_held_input() {
    // A4.3 and N1.2: the intervals are 0.1 s and 0.2 s, and the input 1 is held over both. A lag of τ = 2 s responds at 0.3 s
    // with 1 − e^(−0.3/2), exactly as one interval of 0.3 s would, because the exponential factors multiply.
    let times = [0.0, 0.1, 0.3];
    let deviations = [1.0, 1.0, 1.0];
    let response = unit_response(
        LowOrderModelKind::FirstOrder,
        lag_shape(2.0),
        &times,
        &deviations,
    );
    assert!((response[2] - (1.0 - (-0.3_f64 / 2.0).exp())).abs() <= 1e-12);
    assert_eq!(response[0], 0.0, "the first sample responds with zero");
}
