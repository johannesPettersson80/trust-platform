// The ported assertions of control-studio-manual-proportional-design.test.ts that the inline tests of `proportional.rs` do not
// state exactly: the bounds sentence of an integrating gain outside its bounds, and an unstable entered gain that is kept with
// its reason. The other assertions are in `tests` of `proportional.rs` and in `trust_control_math::proportional`.

use super::*;

/// The requirements of the manual proportional test of the `TypeScript` suite: a unit change, a 0.1 error allowance, a 0.02
/// band, a 2 s settling time, and gain bounds of ±2.
fn manual_requirements() -> ProportionalRequirements {
    ProportionalRequirements {
        reference_change: 1.0,
        constant_input_disturbance: 0.0,
        maximum_steady_error: 0.1,
        settling_band: 0.02,
        settling_time_seconds: 2.0,
        gain_bounds: GainBounds {
            minimum: -2.0,
            maximum: 2.0,
        },
    }
}

// control-studio-manual-proportional-design: "preserves pinned integrating gains and distinguishes stable, boundary and unstable predictions"
#[test]
fn a_pinned_integrating_gain_outside_its_bounds_is_refused_with_the_bounds_sentence() {
    // G = 2/s with the operating input 0.4: the gain 3 is outside ±2, so it is refused before any prediction.
    let plant = IntegratingModel {
        rate_gain: 2.0,
        delay_seconds: 0.0,
    };
    let refusal = integrating_manual(plant, 3.0, 0.4, &manual_requirements())
        .expect_err("3 is outside the bounds ±2");
    assert_eq!(
        refusal.0,
        "Manual proportional gain is outside the declared bounds."
    );
}

// control-studio-manual-proportional-design: "keeps an unstable entered gain exploratory without inventing asymptotic bounds"
#[test]
fn an_unstable_entered_gain_is_kept_as_entered_and_its_reason_says_no_bound_exists() {
    // K = 1 and τ = 1 with Kp = −1: the closed-loop factor is zero, so the pole is zero and the entered gain is kept.
    let model = FirstOrderPlusDelay {
        gain: 1.0,
        time_constant_seconds: 1.0,
        delay_seconds: 0.0,
    };
    let design = first_order_manual(model, -1.0, 0.0, &manual_requirements())
        .expect("an in-range gain is kept");
    let kept = design.proportional_gain;
    assert!(
        (kept + 1.0).abs() <= 1e-12,
        "the entered gain -1 is kept, got {kept}"
    );
    let reason = design
        .prediction
        .reason
        .expect("a pole that is not negative names the missing bound");
    assert!(
        reason.to_lowercase().contains("no asymptotic"),
        "the reason says that no asymptotic bound exists: {reason}"
    );
}

// The ported assertions of control-studio-simple-controller-design.test.ts (N5.9, specification 35 B4.4): the gain of a delay-free
// first-order plant and what refuses it. The engine reads a model as its transfer function, so a supplied and a fitted model of the same
// numbers are one plant here, and the TypeScript's formula lines and `actualSTVerify` flag are not part of the design.

/// The requirements of the simple design suite: a unit change, a 0.1 steady-error allowance, a 0.02 band, a 2 s settling time and the
/// given gain bounds.
fn simple_requirements(bounds: GainBounds) -> ProportionalRequirements {
    ProportionalRequirements {
        reference_change: 1.0,
        constant_input_disturbance: 0.0,
        maximum_steady_error: 0.1,
        settling_band: 0.02,
        settling_time_seconds: 2.0,
        gain_bounds: bounds,
    }
}

// control-studio-simple-controller-design: "derives the same provisional P prediction from supplied and fitted first-order models"
#[test]
fn a_delay_free_first_order_gain_meets_both_allowances_and_its_prediction_confirms_it() {
    // K = 1 and τ = 1: q = K·Kp = 1/0.1 − 1 = 9 meets the steady-error allowance, and the pole −(1 + q)/τ = −10 settles far inside the band.
    let plant = FirstOrderPlusDelay {
        gain: 1.0,
        time_constant_seconds: 1.0,
        delay_seconds: 0.0,
    };
    let design = first_order_design(
        plant,
        0.0,
        &simple_requirements(GainBounds {
            minimum: -20.0,
            maximum: 20.0,
        }),
    )
    .expect("a gain within the bounds meets the requirements");
    assert!(
        (design.proportional_gain - 9.0).abs() <= 1e-9,
        "{}",
        design.proportional_gain
    );
    let prediction = &design.prediction;
    assert!(
        (prediction.closed_loop_pole + 10.0).abs() <= 1e-9,
        "{}",
        prediction.closed_loop_pole
    );
    let steady = prediction
        .steady_error_bound
        .expect("a stable loop has a steady-error bound");
    assert!(steady <= 0.1 + 1e-12, "{steady}");
    let settling = prediction
        .settling_residual_bound
        .expect("a stable loop has a settling residual");
    assert!(settling <= 0.02 + 1e-12, "{settling}");
    assert_eq!(prediction.reason, None);
}

// control-studio-simple-controller-design: "rejects unavailable bounded and delayed design paths without weakening the requirement"
#[test]
fn gain_bounds_that_cannot_meet_the_requirements_and_a_dead_time_are_refused_not_relaxed() {
    let plant = FirstOrderPlusDelay {
        gain: 1.0,
        time_constant_seconds: 1.0,
        delay_seconds: 0.0,
    };
    // q = 9 needs a gain of 9, and the bounds end at 8.
    let narrow = first_order_design(
        plant,
        0.0,
        &simple_requirements(GainBounds {
            minimum: 0.0,
            maximum: 8.0,
        }),
    )
    .expect_err("the bounds cannot satisfy the requirements");
    assert_eq!(narrow.0, texts::SIMPLE_P_BOUNDS);
    let delayed = first_order_design(
        FirstOrderPlusDelay {
            delay_seconds: 0.1,
            ..plant
        },
        0.0,
        &simple_requirements(GainBounds {
            minimum: -20.0,
            maximum: 20.0,
        }),
    )
    .expect_err("the simple design has no transport-delay adapter");
    assert_eq!(delayed.0, texts::SIMPLE_P_NO_DELAY);
}
