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
