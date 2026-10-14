//! Tests of the cascade design (specification 35, C6), ported from `control-studio-cascade-together.test.ts`: the inner loop designed first
//! and the outer through the closed inner loop meet the outer requirements on the model, and the inner loop integrates faster than the
//! outer one. The test's loop runs at a task period of 100 ms, as its fixture does.
//!
//! The support makes no prediction (see `fixtures::NoSupport`), so each PI is the declared response's. The TypeScript falls back to the
//! balanced response when no objective is declared, so these tests declare balanced: it is the response the TypeScript tries first.

#![allow(clippy::float_cmp)] // The derivative gains are compared with zero exactly: a PI writes an explicit zero.

use trust_control_math::identify::gain::ControllerAction;

use super::*;
use crate::tuning::objective::TuningObjective;
use crate::tuning::structures::fixtures::{
    requirements, smith_heater_model, vessel_model, NoSupport,
};

fn cascade_loop() -> CascadeLoop {
    CascadeLoop {
        action: ControllerAction::Direct,
        task_period_ms: 100.0,
        gain_limits: GainLimits::default(),
        plant_input_unit: "%".to_owned(),
        plant_input_limits: OutputLimits {
            minimum: 0.0,
            maximum: 100.0,
        },
    }
}

fn measurement(model_output: usize) -> CascadeMeasurement {
    CascadeMeasurement {
        model_output,
        unit: "degC".to_owned(),
        range: OutputLimits {
            minimum: 0.0,
            maximum: 90.0,
        },
    }
}

/// The test's task: a 2 °C change, a 0.5 °C error allowance at 60 s, a 0.2 °C band, and a 120 s test; balanced, as declared.
fn task() -> CascadeRequirements {
    let mut task = requirements(2.0, 0.5, 0.2, 60.0, 120.0);
    task.tuning_objective = Some(TuningObjective::Balanced);
    CascadeRequirements {
        task,
        inner: None,
        outer: None,
    }
}

#[test]
fn the_inner_loop_first_and_the_outer_through_it_meet_the_outer_requirement_on_the_model() {
    let design = design_cascade(
        &vessel_model(),
        &cascade_loop(),
        &measurement(0),
        &measurement(1),
        &task(),
        &NoSupport,
    )
    .expect("the vessel's cascade is designed");
    let (inner, outer) = (design.inner, design.outer);
    for gains in [inner, outer] {
        assert!(
            gains.proportional_gain.is_finite() && gains.proportional_gain > 0.0,
            "{gains:?}"
        );
        assert!(
            gains.integral_gain.is_finite() && gains.integral_gain > 0.0,
            "{gains:?}"
        );
        assert_eq!(gains.derivative_gain, 0.0);
    }

    // Both PIs on the model, the outer output setting the jacket setpoint, as the TypeScript test simulates them: the vessel
    // must settle by 60 s within 0.5 °C of its 2 °C change.
    let (step, target) = (0.01, 2.0);
    let (mut jacket, mut vessel, mut inner_integral, mut outer_integral) = (0.0, 0.0, 0.0, 0.0);
    let mut error = 0.0_f64;
    for index in 0..12_000_u32 {
        let time = f64::from(index) * step;
        let outer_error = target - vessel;
        let jacket_setpoint = outer.proportional_gain * outer_error + outer_integral;
        let inner_error = jacket_setpoint - jacket;
        let heater = inner.proportional_gain * inner_error + inner_integral;
        outer_integral += outer.integral_gain * outer_error * step;
        inner_integral += inner.integral_gain * inner_error * step;
        jacket += step * (-0.5 * jacket + 0.1 * heater);
        vessel += step * (0.1 * jacket - 0.1 * vessel);
        if time >= 60.0 {
            error = error.max((vessel - target).abs());
        }
    }
    assert!(
        error <= 0.5,
        "the cascade is {error} °C from the target after 60 s; 0.5 °C is allowed"
    );
    assert!(
        inner.integral_gain / inner.proportional_gain
            > outer.integral_gain / outer.proportional_gain,
        "the inner loop integrates faster than the outer one"
    );
}

#[test]
fn without_inner_requirements_the_inner_loop_settles_in_a_fifth_of_the_outer_time_within_two_fifths_of_it(
) {
    let outer = requirements(2.0, 0.5, 0.2, 60.0, 120.0);
    let scaled = scaled_inner(&outer, 0.5);
    assert!((scaled.reference_change - 1.0).abs() < 1e-12);
    assert!((scaled.maximum_error - 0.25).abs() < 1e-12);
    assert!((scaled.settling_band - 0.1).abs() < 1e-12);
    assert!((scaled.settling_time_seconds - 12.0).abs() < 1e-12);
    assert!((scaled.duration_seconds - 24.0).abs() < 1e-12);
    assert_eq!(
        scaled.tuning_objective, None,
        "the rest of the loop's requirements stay"
    );
}

#[test]
fn a_transfer_function_is_not_a_cascade_model() {
    let refusal = design_cascade(
        &smith_heater_model(),
        &cascade_loop(),
        &measurement(0),
        &measurement(0),
        &task(),
        &NoSupport,
    )
    .expect_err("a cascade is designed on a state-space model");
    assert_eq!(refusal.0, texts::CASCADE_MODEL);
}

#[test]
fn a_measurement_outside_the_model_is_a_channel_mismatch() {
    let refusal = design_cascade(
        &vessel_model(),
        &cascade_loop(),
        &measurement(0),
        &measurement(2),
        &task(),
        &NoSupport,
    )
    .expect_err("the vessel has two outputs");
    assert_eq!(refusal.0, texts::CASCADE_CHANNELS);
}
