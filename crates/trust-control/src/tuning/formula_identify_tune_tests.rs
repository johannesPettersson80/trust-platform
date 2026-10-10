// Ported from editors/vscode/src/test/suite/control-studio-identify-tune.test.ts: the refusals of the formula methods on an
// identified model (`tuneController`), and the derivative term of a PI result. The numbers of the TypeScript tests for
// SIMC, AMIGO and the integrating tank are already asserted in `formula::tests`; the zero derivative term is asserted by
// the last test of this file.

use super::*;
use trust_control_math::identify::LowOrderModelKind;

/// An identified model of the given form, stable, with its normalized error at zero.
fn identified(
    kind: LowOrderModelKind,
    gain: f64,
    time_constant_seconds: Option<f64>,
    dead_time_seconds: Option<f64>,
) -> IdentifiedModel {
    IdentifiedModel {
        kind,
        gain,
        time_constant_seconds,
        second_time_constant_seconds: None,
        dead_time_seconds,
        normalized_error: 0.0,
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    }
}

/// The first-order model with a dead time of the `TypeScript` test: gain 1, time constant 1 s, and the given dead time.
fn first_order_dead_time(dead_time_seconds: f64) -> IdentifiedModel {
    identified(
        LowOrderModelKind::FirstOrderDeadTime,
        1.0,
        Some(1.0),
        Some(dead_time_seconds),
    )
}

// control-studio-identify-tune: "maps only SIMC response objectives and rejects models outside each published domain"
#[test]
fn an_unstable_model_or_a_model_error_above_the_limit_creates_no_candidate() {
    let unstable = IdentifiedModel {
        stable: false,
        ..first_order_dead_time(0.5)
    };
    let refusal = tune_identified(
        &unstable,
        TuningMethod::Simc,
        TuningObjective::Balanced,
        ControllerAction::Direct,
        "",
    )
    .expect_err("an unstable model is refused");
    assert_eq!(
        refusal.0,
        "A stable model is required for frequency results."
    );

    let inaccurate = IdentifiedModel {
        normalized_error: 0.3,
        ..first_order_dead_time(0.5)
    };
    let refusal = tune_identified(
        &inaccurate,
        TuningMethod::Simc,
        TuningObjective::Balanced,
        ControllerAction::Direct,
        "",
    )
    .expect_err("a normalized error above 0.25 is refused");
    assert_eq!(
        refusal.0,
        "SIMC PI tuning requires accepted model error no greater than 0.25."
    );
}

// control-studio-identify-tune: "maps only SIMC response objectives and rejects models outside each published domain"
#[test]
fn amigo_needs_a_resolved_dead_time_and_a_delay_ratio_of_at_most_one() {
    let refusal = tune_identified(
        &first_order_dead_time(0.0),
        TuningMethod::Amigo,
        TuningObjective::Balanced,
        ControllerAction::Direct,
        "",
    )
    .expect_err("AMIGO needs a positive dead time");
    assert_eq!(refusal.0, "AMIGO requires resolved positive dead time.");

    let refusal = tune_identified(
        &first_order_dead_time(2.0),
        TuningMethod::Amigo,
        TuningObjective::Balanced,
        ControllerAction::Direct,
        "",
    )
    .expect_err("a dead time of 2 s over a time constant of 1 s is outside AMIGO's domain");
    assert_eq!(
        refusal.0,
        "AMIGO delay ratio is outside the qualified delay domain L/T <= 1."
    );
}

// control-studio-identify-tune: "qualifies SIMC PI for an integrating tank without treating rate gain as static gain"
#[test]
fn an_integrating_tank_needs_a_resolved_delay_for_simc_and_has_no_amigo_form() {
    let without_delay = identified(LowOrderModelKind::Integrating, 0.05, None, Some(0.0));
    let refusal = tune_identified(
        &without_delay,
        TuningMethod::Simc,
        TuningObjective::Balanced,
        ControllerAction::Direct,
        "",
    )
    .expect_err("SIMC needs a resolved positive effective delay");
    assert_eq!(
        refusal.0,
        "SIMC integrating PI requires resolved positive effective delay."
    );

    let tank = identified(LowOrderModelKind::Integrating, 0.05, None, Some(1.0));
    let refusal = tune_identified(
        &tank,
        TuningMethod::Amigo,
        TuningObjective::Balanced,
        ControllerAction::Direct,
        "",
    )
    .expect_err("AMIGO has no integrating form");
    assert_eq!(refusal.0, "AMIGO is unavailable for integrating models.");
}

// control-studio-identify-tune: "describes formula assumptions, units, inputs, results, and unavailable reasons"
#[test]
fn a_pi_result_has_no_derivative_gain_and_an_integral_gain_of_kp_over_ti() {
    let model = identified(
        LowOrderModelKind::FirstOrderDeadTime,
        1.5,
        Some(4.0),
        Some(0.5),
    );
    let tuning = tune_identified(
        &model,
        TuningMethod::Simc,
        TuningObjective::Balanced,
        ControllerAction::Direct,
        "",
    )
    .expect("SIMC accepts the model");
    let gains = tuning.gains();
    // Compared bit for bit, as the TypeScript test's strictEqual compares it (N5.6 gives Kd = 0).
    assert_eq!(
        gains.derivative_gain.to_bits(),
        0.0_f64.to_bits(),
        "the derivative gain of a PI result is exactly zero"
    );
    let integral = tuning.proportional_gain / tuning.integral_time_seconds;
    assert!(
        (gains.integral_gain - integral).abs() <= 1e-12,
        "{}",
        gains.integral_gain
    );
}
