use super::*;

fn fopdt(gain: f64, time_constant_seconds: f64, delay_seconds: f64) -> FirstOrderPlusDelay {
    FirstOrderPlusDelay {
        gain,
        time_constant_seconds,
        delay_seconds,
    }
}

fn tank(rate_gain: f64, delay_seconds: f64) -> IntegratingModel {
    IntegratingModel {
        rate_gain,
        delay_seconds,
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn simc_balanced_symmetric_model_gives_one_third_and_unit_integral_time() {
    // K = 1, T = 1, L = 1: the Faster value is max(1, 1/3) = 1, so balanced λ = 2 and
    // Kp = T/(K(λ + L)) = 1/3 and Ti = min(T, 4(λ + L)) = 1. The published oracle is Kp = 1/3, Ti = 1.
    let gains =
        simc_pi_self_regulating(fopdt(1.0, 1.0, 1.0), ResponseChoice::Balanced).expect("PI");
    assert_close(gains.proportional_gain, 1.0 / 3.0, 1e-12);
    assert_close(gains.integral_time_seconds, 1.0, 1e-12);
    assert_close(gains.integral_gain(), 1.0 / 3.0, 1e-12);
}

#[test]
fn amigo_symmetric_model_gives_quarter_gain_and_unit_integral_time() {
    // K = 1, T = 1, L = 1: Kp = 0.15 + (0.35 − 1/4)·1/1 = 0.25 and Ti = 0.35 + 13/20 = 1.
    let gains = amigo_pi(fopdt(1.0, 1.0, 1.0)).expect("PI");
    assert_close(gains.proportional_gain, 0.25, 1e-12);
    assert_close(gains.integral_time_seconds, 1.0, 1e-12);
    assert_close(gains.integral_gain(), 0.25, 1e-12);
}

#[test]
fn amigo_asymmetric_model_matches_the_published_numbers() {
    // K = 2, T = 10, L = 1: Kp = 0.075 + (0.35 − 10/121)·5 = 1.4117768595, and
    // Ti = 0.35 + 1300/227 = 6.0768722467. The response choice does not change AMIGO.
    let gains = amigo_pi(fopdt(2.0, 10.0, 1.0)).expect("PI");
    assert_close(gains.proportional_gain, 1.411_776_859_5, 1e-9);
    assert_close(gains.integral_time_seconds, 6.076_872_246_7, 1e-9);
}

#[test]
fn simc_response_choices_order_the_gain_and_keep_ti_at_the_model_time_constant() {
    // K = 1, T = 1, L = 0.5: Faster λ = max(0.5, 1/3) = 0.5, so Kp = 1/(λ + L) gives 1, 2/3 and 0.4
    // for Faster, Balanced and More robust. Ti = min(T, 4(λ + L)) = 1 in every case, because λ ≥ T/3.
    let model = fopdt(1.0, 1.0, 0.5);
    let fast = simc_pi_self_regulating(model, ResponseChoice::Faster).expect("PI");
    let balanced = simc_pi_self_regulating(model, ResponseChoice::Balanced).expect("PI");
    let robust = simc_pi_self_regulating(model, ResponseChoice::MoreRobust).expect("PI");
    assert_close(fast.proportional_gain, 1.0, 1e-12);
    assert_close(balanced.proportional_gain, 2.0 / 3.0, 1e-12);
    assert_close(robust.proportional_gain, 0.4, 1e-12);
    for gains in [fast, balanced, robust] {
        assert_close(gains.integral_time_seconds, 1.0, 1e-12);
    }
}

#[test]
fn simc_integrating_tank_matches_the_test_suite_and_keeps_rate_gain_out_of_the_static_gain() {
    // K′ = 0.05 per second with L = 1 s: λ = L, 2L and 4L give Kp = 1/(K′(λ + L)) = 10, 20/3 and 4,
    // with Ti = 4(λ + L) = 8, 12 and 20 seconds.
    let model = tank(0.05, 1.0);
    let fast = simc_pi_integrating(model, ResponseChoice::Faster).expect("PI");
    let balanced = simc_pi_integrating(model, ResponseChoice::Balanced).expect("PI");
    let robust = simc_pi_integrating(model, ResponseChoice::MoreRobust).expect("PI");
    assert_close(fast.proportional_gain, 10.0, 1e-9);
    assert_close(fast.integral_time_seconds, 8.0, 1e-9);
    assert_close(balanced.proportional_gain, 20.0 / 3.0, 1e-9);
    assert_close(balanced.integral_time_seconds, 12.0, 1e-9);
    assert_close(robust.proportional_gain, 4.0, 1e-9);
    assert_close(robust.integral_time_seconds, 20.0, 1e-9);
    assert_close(
        integrating_proportional_gain(model, ResponseChoice::Balanced).expect("P gain"),
        20.0 / 3.0,
        1e-9,
    );
}

#[test]
fn negative_gains_are_tuned_on_their_magnitude() {
    // N5.1: the formulas run on |K|, and the caller gives the sign of the action (N6).
    let positive =
        simc_pi_self_regulating(fopdt(1.0, 1.0, 1.0), ResponseChoice::Balanced).expect("PI");
    let negative =
        simc_pi_self_regulating(fopdt(-1.0, 1.0, 1.0), ResponseChoice::Balanced).expect("PI");
    assert_close(
        negative.proportional_gain,
        positive.proportional_gain,
        1e-12,
    );
    assert_close(
        negative.integral_time_seconds,
        positive.integral_time_seconds,
        1e-12,
    );
}

#[test]
fn the_response_time_scale_and_the_closed_loop_constant_follow_n5_4() {
    // Faster = max(L, T/3): with T = 3 and L = 0.5 it is max(0.5, 1) = 1; the choices give 1, 2 and 4.
    assert_close(self_regulating_faster_scale(3.0, 0.5), 1.0, 1e-12);
    assert_close(self_regulating_faster_scale(3.0, 2.0), 2.0, 1e-12);
    assert_close(
        closed_loop_time_constant(1.0, ResponseChoice::Faster),
        1.0,
        1e-12,
    );
    assert_close(
        closed_loop_time_constant(1.0, ResponseChoice::Balanced),
        2.0,
        1e-12,
    );
    assert_close(
        closed_loop_time_constant(1.0, ResponseChoice::MoreRobust),
        4.0,
        1e-12,
    );
}

#[test]
fn each_method_domain_refuses_with_its_typed_reason() {
    assert_eq!(
        simc_pi_self_regulating(fopdt(0.0, 1.0, 1.0), ResponseChoice::Balanced),
        Err(TuningRefusal::ZeroGain)
    );
    assert_eq!(
        simc_pi_self_regulating(fopdt(f64::NAN, 1.0, 1.0), ResponseChoice::Balanced),
        Err(TuningRefusal::NonfiniteParameter)
    );
    assert_eq!(
        simc_pi_self_regulating(fopdt(1.0, 0.0, 1.0), ResponseChoice::Balanced),
        Err(TuningRefusal::NonpositiveTimeConstant)
    );
    assert_eq!(
        simc_pi_self_regulating(fopdt(1.0, 1.0, -0.5), ResponseChoice::Balanced),
        Err(TuningRefusal::InvalidDelay)
    );
    assert_eq!(
        simc_pi_integrating(tank(0.05, 0.0), ResponseChoice::Balanced),
        Err(TuningRefusal::NoResponseTimeScale)
    );
    assert_eq!(
        amigo_pi(fopdt(1.0, 1.0, 0.0)),
        Err(TuningRefusal::AmigoNeedsPositiveDelay)
    );
    assert_eq!(
        amigo_pi(fopdt(1.0, 1.0, 2.0)),
        Err(TuningRefusal::AmigoDelayRatioAboveOne)
    );
}

#[test]
fn the_model_error_limit_refuses_above_its_maximum_only() {
    assert_eq!(check_model_error(0.2, 0.25), Ok(()));
    assert_eq!(check_model_error(0.25, 0.25), Ok(()));
    assert_eq!(
        check_model_error(0.3, 0.25),
        Err(TuningRefusal::ModelErrorAboveLimit)
    );
    assert_eq!(
        check_model_error(f64::NAN, 0.25),
        Err(TuningRefusal::NonfiniteParameter)
    );
}

fn second_order(
    gain: f64,
    leading: f64,
    linear: f64,
    constant: f64,
    delay_seconds: f64,
) -> SecondOrderPlusDelay {
    SecondOrderPlusDelay {
        gain,
        leading,
        linear,
        constant,
        delay_seconds,
    }
}

#[test]
fn the_half_rule_of_tau_10_and_4_with_dead_time_1_gives_t_12_and_l_3() {
    // K = 2, τ1 = 10, τ2 = 4, θ = 1: a2 = 40, a1 = 14, a0 = 1. Then T = τ1 + τ2/2 = 12 and L = θ + τ2/2 = 3.
    let reduced = half_rule(second_order(2.0, 40.0, 14.0, 1.0, 1.0)).expect("the half rule");
    assert_close(reduced.gain, 2.0, 1e-12);
    assert_close(reduced.time_constant_seconds, 12.0, 1e-12);
    assert_close(reduced.delay_seconds, 3.0, 1e-12);
}

#[test]
fn the_simc_pi_of_the_reduced_model_follows_n5_2_for_each_response_choice() {
    // The FOPDT K = 2, T = 12, L = 3 has the Faster value max(L, T/3) = max(3, 4) = 4, so λ = 4, 8 and 16 and λ + L = 7,
    // 11 and 19. Kp = T/(K(λ + L)) = 6/7, 6/11 and 6/19, and Ti = min(T, 4(λ + L)) = 12 in every case.
    let reduced = half_rule(second_order(2.0, 40.0, 14.0, 1.0, 1.0)).expect("the half rule");
    let expected = [
        (ResponseChoice::Faster, 6.0 / 7.0),
        (ResponseChoice::Balanced, 6.0 / 11.0),
        (ResponseChoice::MoreRobust, 6.0 / 19.0),
    ];
    for (choice, proportional_gain) in expected {
        let gains = simc_pi_self_regulating(reduced, choice).expect("PI");
        assert_close(gains.proportional_gain, proportional_gain, 1e-12);
        assert_close(gains.integral_time_seconds, 12.0, 1e-12);
    }
}

#[test]
fn equal_time_constants_are_real_poles_and_reduce_to_t_three_halves_and_l_one_half() {
    // τ1 = τ2 = 1: a2 = 1, a1 = 2, a0 = 1 has the double pole −1, so T = 1 + 1/2 and L = 0 + 1/2.
    let reduced = half_rule(second_order(1.0, 1.0, 2.0, 1.0, 0.0)).expect("the half rule");
    assert_close(reduced.time_constant_seconds, 1.5, 1e-12);
    assert_close(reduced.delay_seconds, 0.5, 1e-12);
}

#[test]
fn a_discriminant_within_rounding_of_zero_is_not_complex() {
    // a1² − 4·a2·a0 = −8ε is the rounding of a double pole, so the poles are real and equal (T = 3/2).
    let reduced = half_rule(second_order(1.0, 1.0 + 2.0 * f64::EPSILON, 2.0, 1.0, 0.0))
        .expect("a double pole is real");
    assert_close(reduced.time_constant_seconds, 1.5, 1e-12);
}

#[test]
fn complex_poles_are_refused_with_their_typed_reason() {
    // s² + s + 1 has the poles (−1 ± j√3)/2: a1² − 4·a2·a0 = −3.
    assert_eq!(
        half_rule(second_order(1.0, 1.0, 1.0, 1.0, 0.0)),
        Err(TuningRefusal::ComplexPoles)
    );
    // The discriminant −4·10⁻³ is far beyond the rounding allowance.
    assert_eq!(
        half_rule(second_order(1.0, 1.001, 2.0, 1.0, 0.0)),
        Err(TuningRefusal::ComplexPoles)
    );
}

#[test]
fn a_second_order_model_whose_coefficients_are_not_positive_is_not_stable() {
    assert_eq!(
        half_rule(second_order(1.0, 40.0, -14.0, 1.0, 1.0)),
        Err(TuningRefusal::NotStable)
    );
    assert_eq!(
        half_rule(second_order(1.0, 40.0, 14.0, 0.0, 1.0)),
        Err(TuningRefusal::NotStable)
    );
}

#[test]
fn the_half_rule_keeps_the_sign_of_the_gain_and_refuses_the_gain_and_delay_domains() {
    let reduced = half_rule(second_order(-2.0, 40.0, 14.0, 1.0, 1.0)).expect("the half rule");
    assert_close(reduced.gain, -2.0, 1e-12);
    assert_eq!(
        half_rule(second_order(0.0, 40.0, 14.0, 1.0, 1.0)),
        Err(TuningRefusal::ZeroGain)
    );
    assert_eq!(
        half_rule(second_order(2.0, 40.0, 14.0, 1.0, -1.0)),
        Err(TuningRefusal::InvalidDelay)
    );
}
