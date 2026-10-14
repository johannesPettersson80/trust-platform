//! Tests of the sampled closed loops (specification 35, C6 and C8). Each expected value is a closed form of the sampled recursion.

use super::*;

/// The discrete plant `x(k+1) = 0.5·x(k) + 0.5·u(k)`, `y = x`, with no feedthrough: its steady gain is one.
fn half_lag() -> StateSpace {
    StateSpace {
        a: vec![vec![0.5]],
        b: vec![vec![0.5]],
        c: vec![vec![1.0]],
        d: vec![vec![0.0]],
    }
}

#[test]
fn the_open_loop_unit_step_of_a_discrete_lag_is_one_minus_half_to_the_k() {
    let response = unit_step_response(&half_lag(), 1.0, 0.0, 10.0, 0, None);
    assert_eq!(response.times.len(), 11, "samples 0 through 10");
    for (step, value) in response.values.iter().enumerate() {
        let expected = 1.0 - 0.5_f64.powi(i32::try_from(step).expect("a small step"));
        assert!((value - expected).abs() < 1e-12, "sample {step}: {value}");
    }
    assert!((response.times[10] - 10.0).abs() < 1e-12);
}

#[test]
fn a_delay_of_one_sample_holds_the_step_back_by_one_sample() {
    // The command of sample k reaches the plant at sample k + 1, so y(0) = 0 and y(k) = 1 − 0.5^(k−1) for k ≥ 1.
    let response = unit_step_response(&half_lag(), 1.0, 1.0, 10.0, 0, None);
    assert!(
        response.values[0].abs() < 1e-12,
        "the first sample has no input yet"
    );
    for step in 1..=10 {
        let expected = 1.0 - 0.5_f64.powi(i32::try_from(step - 1).expect("a small step"));
        assert!(
            (response.values[step] - expected).abs() < 1e-12,
            "sample {step}: {}",
            response.values[step]
        );
    }
}

#[test]
fn a_pi_around_the_measurement_removes_the_offset_of_the_setpoint_step() {
    // With Kp = 0.2 and Ki = 0.05 per unit sample, the closed loop's deviations evolve by [[0.4, 0.5], [−0.05, 1]], whose
    // eigenvalues are 0.955 and 0.445: the response settles on the setpoint, 0.955^200 ≈ 1.1·10⁻⁴ after 200 samples.
    let law = SamplePi {
        proportional_gain: 0.2,
        integral_gain: 0.05,
    };
    let response = unit_step_response(&half_lag(), 1.0, 0.0, 200.0, 0, Some((0, law)));
    let last = *response.values.last().expect("samples");
    assert!(
        (last - 1.0).abs() < 1e-3,
        "the output settles on the setpoint: {last}"
    );
}

/// The plant `x(k+1) = 0.5·x(k) + 0.5·u(k) + 0.5·d(k)`, `y = x`, with inputs `[u, d]`: the disturbance has the same path as the
/// controller output, so a feedforward of −1 cancels it exactly.
fn disturbed_lag() -> StateSpace {
    StateSpace {
        a: vec![vec![0.5]],
        b: vec![vec![0.5, 0.5]],
        c: vec![vec![1.0]],
        d: vec![vec![0.0, 0.0]],
    }
}

fn disturbance_case(system: &StateSpace) -> DisturbanceCase<'_> {
    DisturbanceCase {
        system,
        sample_time_seconds: 1.0,
        delay_seconds: 0.0,
        controller_input: 0,
        disturbance_input: 1,
        measurement: 0,
        law: SamplePi {
            proportional_gain: 1.0,
            integral_gain: 0.0,
        },
        period_seconds: 1.0,
        disturbance_step: 1.0,
        duration_seconds: 10.0,
    }
}

#[test]
fn a_feedback_alone_lets_the_disturbance_reach_the_measurement() {
    // y(1) = 0.5·d = 0.5 before the PI has acted, so the largest deviation is at least 0.5.
    let system = disturbed_lag();
    let largest = largest_disturbance_deviation(&disturbance_case(&system), None);
    assert!(largest >= 0.5 - 1e-12, "largest deviation {largest}");
}

#[test]
fn a_static_feedforward_of_minus_one_cancels_the_disturbance_exactly() {
    // K_ff = −K_d/K_u = −1: the command −d cancels the disturbance's 0.5·d in the state equation at once.
    let system = disturbed_lag();
    let feedforward = Feedforward {
        gain: -1.0,
        lead_lag: None,
    };
    let largest = largest_disturbance_deviation(&disturbance_case(&system), Some(feedforward));
    assert!(largest.abs() < 1e-12, "largest deviation {largest}");
}
