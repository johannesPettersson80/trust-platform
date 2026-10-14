//! Tests of the measured-disturbance feedforward (specification 35, C8), ported from `control-studio-feedforward.test.ts`: the two paths of
//! the heater (0.2 °C/% through 8 s, −0.1 °C/kW through 3 s), the feedforward `K_ff = −K_d/K_u = 0.5 %/kW` with `T_lead = 8 s` and
//! `T_lag = 3 s`, the largest deviations on the model, and a disturbance that reaches the measurement later than the output gets its
//! extra delay as lag. The test's loop runs at 100 ms, as its fixture does.
//!
//! The PI is declared balanced, the response the TypeScript tries first; see `fixtures::NoSupport` for why no prediction is made.

#![allow(clippy::float_cmp)] // The derivative gain is compared with zero exactly: a PI writes an explicit zero.

use trust_control_math::identify::gain::ControllerAction;

use super::*;
use crate::tuning::objective::TuningObjective;
use crate::tuning::shortfall::{GainLimits, OutputLimits};
use crate::tuning::structures::fixtures::{
    heater_model, requirements, slow_load_model, smith_heater_model, NoSupport,
};

fn facts() -> LoopFacts {
    LoopFacts {
        action: ControllerAction::Direct,
        task_period_ms: 100.0,
        gain_limits: GainLimits::default(),
        output_limits: OutputLimits {
            minimum: 0.0,
            maximum: 100.0,
        },
        process_unit: "degC".to_owned(),
        manipulated_unit: "%".to_owned(),
    }
}

fn channels() -> FeedforwardChannels {
    FeedforwardChannels {
        controller_input: 0,
        disturbance_input: 1,
        measurement: 0,
    }
}

/// The test's task: a 1 °C change, a 0.3 °C allowance at 40 s, a 0.1 °C band, and an 80 s test; balanced, as declared.
fn task() -> TuningRequirements {
    let mut task = requirements(1.0, 0.3, 0.1, 40.0, 80.0);
    task.tuning_objective = Some(TuningObjective::Balanced);
    task
}

fn design(model: &GeneralModel, with_lead_lag: bool) -> FeedforwardDesign {
    design_feedforward(
        model,
        &facts(),
        &channels(),
        Some(10.0),
        with_lead_lag,
        &task(),
        &NoSupport,
    )
    .expect("the heater's feedforward is designed")
}

#[test]
fn the_paths_and_the_three_feedforward_values_are_the_heaters_closed_forms() {
    let design = design(&heater_model(), true);
    assert!(
        (design.output_path.gain - 0.2).abs() < 0.002,
        "K_u {}",
        design.output_path.gain
    );
    assert!(
        (design.disturbance_path.gain + 0.1).abs() < 0.001,
        "K_d {}",
        design.disturbance_path.gain
    );
    assert!((design.gain - 0.5).abs() < 0.005, "K_ff {}", design.gain);
    let lead = design.lead_seconds.expect("the program has a lead");
    let lag = design.lag_seconds.expect("the program has a lag");
    assert!((lead - 8.0).abs() < 0.08, "T_lead {lead}");
    assert!((lag - 3.0).abs() < 0.03, "T_lag {lag}");
    assert_eq!(design.disturbance_step, 10.0);
    assert!(
        design.pi.proportional_gain > 0.0 && design.pi.integral_gain > 0.0,
        "{:?}",
        design.pi
    );
    assert_eq!(design.pi.derivative_gain, 0.0);
}

#[test]
fn the_feedforward_cancels_most_of_the_disturbance_on_the_model() {
    let design = design(&heater_model(), true);
    let deviation = design.largest_deviation;
    assert!(
        deviation.feedback > 0.0,
        "the disturbance reaches the water with the PI alone"
    );
    assert!(
        deviation.feedforward < deviation.feedback / 10.0,
        "the feedforward cancels most of the disturbance on the model: {deviation:?}"
    );
}

#[test]
fn without_a_lead_and_a_lag_the_feedforward_has_its_gain_alone() {
    let design = design(&heater_model(), false);
    assert_eq!(design.lead_seconds, None);
    assert_eq!(design.lag_seconds, None);
    assert!((design.gain - 0.5).abs() < 0.005);
}

#[test]
fn a_disturbance_slower_than_the_output_gets_its_extra_delay_as_lag() {
    // The load acts through two 1.5 s lags in series, so its path looks delayed; the heater's path has no delay.
    let design = design(&slow_load_model(), true);
    assert!(
        design.disturbance_path.delay_seconds > design.output_path.delay_seconds,
        "{:?}",
        design.disturbance_path
    );
    let lag = design.lag_seconds.expect("the program has a lag");
    let expected = design.disturbance_path.time_constant_seconds
        + design.disturbance_path.delay_seconds
        - design.output_path.delay_seconds;
    assert!(
        (lag - expected).abs() < 1e-9,
        "T_lag {lag} is τ_d + θ_d − θ_u = {expected}"
    );
}

#[test]
fn a_disturbance_without_an_identification_step_is_refused() {
    for step in [None, Some(0.0), Some(f64::NAN)] {
        let refusal = design_feedforward(
            &heater_model(),
            &facts(),
            &channels(),
            step,
            true,
            &task(),
            &NoSupport,
        )
        .expect_err("the disturbance needs its identification step");
        assert_eq!(refusal.0, texts::FEEDFORWARD_STEP);
    }
}

#[test]
fn a_feedforward_is_designed_on_two_inputs_and_one_output() {
    let mismatched = FeedforwardChannels {
        controller_input: 0,
        disturbance_input: 1,
        measurement: 1,
    };
    let refusal = design_feedforward(
        &heater_model(),
        &facts(),
        &mismatched,
        Some(10.0),
        true,
        &task(),
        &NoSupport,
    )
    .expect_err("the measurement is not an output of the model");
    assert_eq!(refusal.0, texts::FEEDFORWARD_CHANNELS);

    let refusal = design_feedforward(
        &smith_heater_model(),
        &facts(),
        &channels(),
        Some(10.0),
        true,
        &task(),
        &NoSupport,
    )
    .expect_err("a transfer function has one input");
    assert_eq!(refusal.0, texts::FEEDFORWARD_CHANNELS);
}
