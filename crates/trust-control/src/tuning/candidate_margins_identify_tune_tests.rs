// Ported from editors/vscode/src/test/suite/control-studio-identify-tune.test.ts: the margins of a formula candidate on its
// model (`tuneController`), and the frequency response of an integrating plant (`frequencyResponse`). The TypeScript tests
// use an identified model; here the same rules run on a declared model of the same form.

use super::*;
use crate::tuning::formula::tune_identified;
use crate::tuning::objective::{TuningMethod, TuningObjective};
use trust_control_math::identify::gain::ControllerAction;

/// A first-order model with a dead time: the gain, the time constant in seconds and the dead time in seconds.
fn first_order_dead_time(
    gain: f64,
    time_constant_seconds: f64,
    dead_time_seconds: f64,
) -> IdentifiedModel {
    IdentifiedModel {
        kind: LowOrderModelKind::FirstOrderDeadTime,
        gain,
        time_constant_seconds: Some(time_constant_seconds),
        second_time_constant_seconds: None,
        dead_time_seconds: Some(dead_time_seconds),
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

/// Whether a margin is positive: a finite positive value, or an unbounded one, which the `TypeScript` test reads as infinity.
fn positive(margin: MarginValue) -> bool {
    match margin {
        MarginValue::Finite(value) => value > 0.0,
        MarginValue::Unbounded => true,
        MarginValue::NotObservedInRange => false,
    }
}

// control-studio-identify-tune: "generates unapplied SIMC and AMIGO PI candidates with model-backed margins"
#[test]
fn a_simc_pi_has_positive_margins_on_its_model_and_an_amigo_pi_is_defined() {
    let model = first_order_dead_time(1.5, 4.0, 0.5);
    let simc = tune_identified(
        &model,
        TuningMethod::Simc,
        TuningObjective::Balanced,
        ControllerAction::Direct,
        "",
    )
    .expect("SIMC accepts the model");
    let amigo = tune_identified(
        &model,
        TuningMethod::Amigo,
        TuningObjective::Balanced,
        ControllerAction::Direct,
        "",
    )
    .expect("AMIGO accepts the model");
    assert!(simc.proportional_gain > 0.0 && simc.integral_time_seconds > 0.0);
    assert!(amigo.proportional_gain > 0.0 && amigo.integral_time_seconds > 0.0);

    let gains = PredictedGains::parallel(simc.proportional_gain, simc.gains().integral_gain, 0.0);
    let loop_ = pid_loop(&model, &gains).expect("the loop of the candidate is valid");
    assert!(loop_.frequencies.len() >= 40, "{}", loop_.frequencies.len());
    let margins = pid_margins(&model, &gains).expect("the margins of the candidate are valid");
    assert!(
        positive(margins.gain_margin_db),
        "{:?}",
        margins.gain_margin_db
    );
    assert!(
        positive(margins.phase_margin_degrees),
        "{:?}",
        margins.phase_margin_degrees
    );
}

// control-studio-identify-tune: "qualifies SIMC PI for an integrating tank without treating rate gain as static gain"
#[test]
fn the_integrating_plant_gain_falls_by_more_than_100_decibels_over_six_decades() {
    // The rate gain 0.05 with a 1 s dead time: |G(jω)| = 0.05/ω, so between 1e-3 and 1e3 rad/s the
    // magnitude falls by 120 dB, and the TypeScript test asks for more than 100 dB.
    let tank = IdentifiedModel {
        kind: LowOrderModelKind::Integrating,
        gain: 0.05,
        time_constant_seconds: None,
        second_time_constant_seconds: None,
        dead_time_seconds: Some(1.0),
        normalized_error: 0.04,
        normalized_estimation_error: None,
        normalized_validation_error: None,
        estimation_residuals: None,
        validation_residuals: None,
        uncertainty: None,
        stable: true,
        on_search_boundary: false,
    };
    let plant = FrequencyModel::Transfer {
        transfer: plant_transfer(&tank),
        timing: Timing::Continuous,
        delay_seconds: 1.0,
    };
    let low = magnitude(loop_value(&plant, 1e-3).expect("the plant responds at 1e-3 rad/s"));
    let high = magnitude(loop_value(&plant, 1e3).expect("the plant responds at 1e3 rad/s"));
    let span_db = 20.0 * (low / high).log10();
    assert!(span_db > 100.0, "{span_db}");
}
