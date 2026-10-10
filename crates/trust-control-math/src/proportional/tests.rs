use super::*;

fn inputs(plant_gain: f64, proportional_gain: f64) -> PredictionInputs {
    PredictionInputs {
        plant_gain,
        proportional_gain,
        operating_input: 0.0,
        input_disturbance: 0.0,
        reference_change: 1.0,
        settling_time_seconds: 2.0,
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn integrator_prediction_matches_the_manual_example_of_the_test_suite() {
    // k = 2 (θ/u = 2/s), Kp = 2, u0 = 0.4, d = 0, ΔR = 1, Tset = 2 s: the pole is −4, the constant error
    // bound is 0.4/2 = 0.2, and the envelope at Tset is (1 + 0.2)·e^(−8).
    let prediction = integrating_prediction(PredictionInputs {
        operating_input: 0.4,
        ..inputs(2.0, 2.0)
    })
    .expect("prediction");
    assert_close(prediction.closed_loop_pole, -4.0, 1e-12);
    assert_close(
        prediction.steady_error_bound.expect("error bound"),
        0.2,
        1e-12,
    );
    assert_close(
        prediction.settling_residual_bound.expect("residual bound"),
        1.2 * (-8.0_f64).exp(),
        1e-12,
    );
}

#[test]
fn integrator_prediction_separates_stable_boundary_and_unstable_gains() {
    // Kp = 0 leaves the pole at zero, and Kp = −1 gives a positive pole. Neither has an asymptotic bound.
    let boundary = integrating_prediction(inputs(2.0, 0.0)).expect("prediction");
    assert_close(boundary.closed_loop_pole, 0.0, 1e-12);
    assert_eq!(boundary.steady_error_bound, None);
    assert_eq!(boundary.settling_residual_bound, None);
    let unstable = integrating_prediction(inputs(2.0, -1.0)).expect("prediction");
    assert_close(unstable.closed_loop_pole, 2.0, 1e-12);
    assert_eq!(unstable.settling_residual_bound, None);
}

#[test]
fn an_input_disturbance_enters_the_load_as_the_larger_of_the_two_operating_offsets() {
    // load = max(|u0|, |u0 − d|) = max(0.4, 0.6) = 0.6, so E∞ = 0.6/2 = 0.3.
    let prediction = integrating_prediction(PredictionInputs {
        operating_input: 0.4,
        input_disturbance: 1.0,
        ..inputs(2.0, 2.0)
    })
    .expect("prediction");
    assert_close(
        prediction.steady_error_bound.expect("error bound"),
        0.3,
        1e-12,
    );
}

#[test]
fn first_order_prediction_matches_the_manual_p_test_values() {
    // K = 1, τ = 1, u0 = 0, d = 0, ΔR = 1, Tset = 2 s.
    // Kp = 0.5: q = 0.5, the pole is −1.5, E∞ = 1/1.5 = 2/3, and the transient bound is max(1, 1/3)·e^(−3).
    let stable = first_order_prediction(inputs(1.0, 0.5), 1.0).expect("prediction");
    assert_close(stable.closed_loop_pole, -1.5, 1e-12);
    assert_close(
        stable.steady_error_bound.expect("error bound"),
        2.0 / 3.0,
        1e-12,
    );
    assert_close(
        stable.settling_residual_bound.expect("residual bound"),
        (-3.0_f64).exp(),
        1e-12,
    );
    // Kp = −0.75: q = −0.75, the pole is −0.25, E∞ = 1/0.25 = 4, and the transient bound is 3·e^(−0.5).
    let negative_q = first_order_prediction(inputs(1.0, -0.75), 1.0).expect("prediction");
    assert_close(negative_q.closed_loop_pole, -0.25, 1e-12);
    assert_close(
        negative_q.steady_error_bound.expect("error bound"),
        4.0,
        1e-12,
    );
    assert_close(
        negative_q.settling_residual_bound.expect("residual bound"),
        3.0 * (-0.5_f64).exp(),
        1e-12,
    );
    // Kp = −1 makes the closed-loop factor zero: the pole is zero and no asymptotic bound exists.
    let boundary = first_order_prediction(inputs(1.0, -1.0), 1.0).expect("prediction");
    assert_close(boundary.closed_loop_pole, 0.0, 1e-12);
    assert_eq!(boundary.steady_error_bound, None);
    assert_eq!(boundary.settling_residual_bound, None);
}

#[test]
fn first_order_prediction_refuses_a_nonpositive_time_constant() {
    assert_eq!(
        first_order_prediction(inputs(1.0, 0.5), 0.0),
        Err(ProportionalRefusal::InvalidInput)
    );
}

#[test]
fn compatible_integrating_gain_is_the_smallest_gain_that_meets_both_allowances() {
    // k = 2, u0 = 0.4, d = 0, ΔR = 1, E_max = 0.1, band 0.02, Tset = 2 s, |Kp| in [0.1, 10].
    // λ ≥ |k|·load/E_max = 2·0.4/0.1 = 8, and λ ≥ ln((1 + 0.1)/0.02)/2 ≈ 2.004. So λ = 8 and |Kp| = 8/2 = 4.
    // Then E∞ = 0.4/4 = 0.1 meets the allowance.
    let task = IntegratingTask {
        rate_gain: 2.0,
        operating_input: 0.4,
        input_disturbance: 0.0,
        reference_change: 1.0,
        maximum_steady_error: 0.1,
        settling_band: 0.02,
        settling_time_seconds: 2.0,
        minimum_gain: 0.1,
        maximum_gain: 10.0,
    };
    let gain = compatible_integrating_gain(task).expect("gain");
    assert_close(gain, 4.0, 1e-12);
    assert_close(0.4 / gain, 0.1, 1e-12);
    // With a gain bound of 3, the largest λ is 6, below the 8 that the error allowance needs.
    let bounded = IntegratingTask {
        maximum_gain: 3.0,
        ..task
    };
    assert_eq!(
        compatible_integrating_gain(bounded),
        Err(ProportionalRefusal::NoCompatibleGain)
    );
    let zero_allowance = IntegratingTask {
        maximum_steady_error: 0.0,
        ..task
    };
    assert_eq!(
        compatible_integrating_gain(zero_allowance),
        Err(ProportionalRefusal::InvalidInput)
    );
}

#[test]
fn proportional_steady_error_follows_n5_9_and_refuses_a_nonpositive_denominator() {
    // |ΔR + K·u0| / (1 + K·Kp): K = 1, Kp = 3, u0 = 0, ΔR = 1 gives 1/4.
    assert_close(
        proportional_steady_error(1.0, 3.0, 0.0, 1.0).expect("error"),
        0.25,
        1e-12,
    );
    // With u0 = 0.5 the offset is 1.5 and the error is 1.5/4.
    assert_close(
        proportional_steady_error(1.0, 3.0, 0.5, 1.0).expect("error"),
        0.375,
        1e-12,
    );
    // K·Kp = −1 gives a zero denominator, and K·Kp = −2 a negative one: both are refused.
    assert_eq!(
        proportional_steady_error(-1.0, 1.0, 0.0, 1.0),
        Err(ProportionalRefusal::NonpositiveDcDenominator)
    );
    assert_eq!(
        proportional_steady_error(-1.0, 2.0, 0.0, 1.0),
        Err(ProportionalRefusal::NonpositiveDcDenominator)
    );
}

#[test]
fn minimum_proportional_magnitude_inverts_the_steady_error_bound() {
    // offset 1 and allowance 0.25 need (1/0.25 − 1)/|K| = 3 for |K| = 1, and 1.5 for |K| = 2.
    assert_close(
        minimum_proportional_magnitude(1.0, 0.25, 1.0).expect("gain"),
        3.0,
        1e-12,
    );
    assert_close(
        minimum_proportional_magnitude(1.0, 0.25, 2.0).expect("gain"),
        1.5,
        1e-12,
    );
    // An offset within the allowance needs no gain.
    assert_close(
        minimum_proportional_magnitude(0.1, 0.25, 1.0).expect("gain"),
        0.0,
        1e-12,
    );
    assert_eq!(
        minimum_proportional_magnitude(1.0, 0.0, 1.0),
        Err(ProportionalRefusal::InvalidInput)
    );
}
