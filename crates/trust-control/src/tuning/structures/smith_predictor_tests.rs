//! Tests of the Smith predictor (specification 35, C9), ported from `control-studio-smith-predictor.test.ts`: the model values are the
//! two-point reduction of the model (`K = 0.2`, `T = 5`, `θ = 1`); the PI settles the loop through the predictor within the error allowance
//! after the settling time; and a settling time no longer than the delay is refused. The test's loop runs at 100 ms, as its fixture does.
//!
//! The PI is declared balanced, the response the TypeScript tries first; see `fixtures::NoSupport` for why no prediction is made.

#![allow(clippy::float_cmp)] // The derivative gain is compared with zero exactly: a PI writes an explicit zero.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Delays are whole samples: a rounded, non-negative ratio.

use std::collections::VecDeque;

use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::identify::ParameterInterval;

use super::*;
use crate::tuning::objective::TuningObjective;
use crate::tuning::prediction::RecordShaping;
use crate::tuning::shortfall::{GainLimits, OutputLimits};
use crate::tuning::structures::fixtures::{requirements, smith_heater_model, NoSupport};

fn facts() -> LoopFacts {
    LoopFacts {
        action: ControllerAction::Direct,
        task_period_ms: 100.0,
        gain_limits: GainLimits::default(),
        output_limits: OutputLimits {
            minimum: 0.0,
            maximum: 100.0,
        },
        rate_limits: ParameterInterval {
            minimum: -1e6,
            maximum: 1e6,
        },
        shaping: RecordShaping::default(),
        process_unit: "degC".to_owned(),
        manipulated_unit: "%".to_owned(),
    }
}

/// The test's task: a 0.5 °C change, a 0.1 °C allowance, a 0.05 °C band, a 10 s settling time and a 30 s test; balanced, as declared.
fn task(settling_time_seconds: f64) -> TuningRequirements {
    let mut task = requirements(0.5, 0.1, 0.05, settling_time_seconds, 30.0);
    task.tuning_objective = Some(TuningObjective::Balanced);
    task
}

fn design(settling_time_seconds: f64) -> Result<SmithDesign, Refusal> {
    design_smith_predictor(
        &smith_heater_model(),
        &facts(),
        &task(settling_time_seconds),
        &NoSupport,
    )
}

#[test]
fn the_model_values_are_the_two_point_reduction_of_the_model() {
    let design = design(10.0).expect("the Smith predictor is designed");
    assert!(
        (design.model_gain - 0.2).abs() < 0.002,
        "K {}",
        design.model_gain
    );
    assert!(
        (design.model_time_constant_seconds - 5.0).abs() < 0.05,
        "T {}",
        design.model_time_constant_seconds
    );
    assert!(
        (design.model_delay_seconds - 1.0).abs() < 0.02,
        "θ {}",
        design.model_delay_seconds
    );
}

#[test]
fn the_pi_settles_the_loop_through_the_predictor_within_the_error_allowance() {
    let design = design(10.0).expect("the Smith predictor is designed");
    let (kp, ki) = (design.pi.proportional_gain, design.pi.integral_gain);
    assert!(kp > 0.0 && ki > 0.0, "{:?}", design.pi);
    assert_eq!(design.pi.derivative_gain, 0.0);

    // The plant with its true delay of 1 s, and the predictor's correction with the designed model: the step must be within
    // 0.1 °C of its target from 10 s on.
    let step = 0.01_f64;
    let delay_steps = (1.0 / step).round() as usize;
    let model_delay_steps = (design.model_delay_seconds / step).round() as usize;
    let mut plant_history: VecDeque<f64> = VecDeque::from(vec![0.0; delay_steps]);
    let mut output_history: VecDeque<f64> = VecDeque::from(vec![0.0; model_delay_steps]);
    let (mut y, mut correction, mut integral) = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut error = 0.0_f64;
    for index in 0..3_000_u32 {
        let time = f64::from(index) * step;
        let e = 0.5 - (y + correction);
        let u = kp * e + integral;
        integral += ki * e * step;
        plant_history.push_back(u);
        let applied = plant_history.pop_front().unwrap_or(0.0);
        output_history.push_back(u);
        let earlier = output_history.pop_front().unwrap_or(0.0);
        y += step * (0.2 * applied - y) / 5.0;
        correction += step * (design.model_gain * (u - earlier) - correction)
            / design.model_time_constant_seconds;
        if time >= 10.0 {
            error = error.max((y - 0.5).abs());
        }
    }
    assert!(
        error <= 0.1,
        "{error} °C from the target after 10 s; 0.1 °C is allowed"
    );
}

#[test]
fn a_settling_time_no_longer_than_the_models_delay_is_refused() {
    let refusal = design(0.8).expect_err("no controller acts sooner than the delay");
    assert!(
        refusal
            .0
            .starts_with("The settling time must be longer than the model's delay of "),
        "{}",
        refusal.0
    );
    assert!(
        refusal.0.ends_with(" s: no controller acts sooner."),
        "{}",
        refusal.0
    );
}
