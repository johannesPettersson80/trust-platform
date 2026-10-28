// The ported numerical assertions of control-studio-manual-proportional-design.test.ts: an overflowing closed-loop gain and an
// underflowed pole are numerical uncertainty, not an unstable pole (specification control-studio-workflows/contract-reference
// section 9.2). The sentences of the product are in `trust_control::tuning::proportional`.

use super::*;

// control-studio-manual-proportional-design: "rejects numerical overflow instead of classifying it as an unstable pole"
#[test]
fn an_overflowing_closed_loop_gain_is_numerical_uncertainty_not_an_unstable_pole() {
    // K = f64::MAX and Kp = 2: q = K·Kp overflows to infinity, so the closed-loop factor and the pole are not finite.
    let result = first_order_prediction(
        PredictionInputs {
            plant_gain: f64::MAX,
            proportional_gain: 2.0,
            operating_input: 0.0,
            input_disturbance: 0.0,
            reference_change: 1.0,
            settling_time_seconds: 2.0,
        },
        1.0,
    );
    assert_eq!(result, Err(ProportionalRefusal::NumericalUncertainty));
}

// control-studio-manual-proportional-design: "rejects an underflowed pole when the finite closed-loop factor is nonzero"
#[test]
fn an_underflowed_pole_with_a_nonzero_closed_loop_factor_is_numerical_uncertainty() {
    // K = 1, τ = f64::MAX and Kp = −0.9999999999999998: the factor 1 + q is 2.2e−16, and −factor/τ underflows to zero.
    let result = first_order_prediction(
        PredictionInputs {
            plant_gain: 1.0,
            proportional_gain: -0.999_999_999_999_999_8,
            operating_input: 0.0,
            input_disturbance: 0.0,
            reference_change: 1.0,
            settling_time_seconds: 2.0,
        },
        f64::MAX,
    );
    assert_eq!(result, Err(ProportionalRefusal::NumericalUncertainty));
}

// control-studio-manual-proportional-design: "rejects numerical overflow instead of classifying it as an unstable pole"
#[test]
fn an_overflowing_operating_effect_on_a_zero_pole_is_numerical_uncertainty() {
    // K = 2 and Kp = −0.5: q = −1, so the closed-loop factor and the pole are zero. K·u0 = 2·1e308 overflows, so the worst
    // final error is infinite, and the TypeScript refuses that before it classifies the pole.
    let result = first_order_prediction(
        PredictionInputs {
            plant_gain: 2.0,
            proportional_gain: -0.5,
            operating_input: 1.0e308,
            input_disturbance: 0.0,
            reference_change: 1.0,
            settling_time_seconds: 2.0,
        },
        1.0,
    );
    assert_eq!(result, Err(ProportionalRefusal::NumericalUncertainty));
}
