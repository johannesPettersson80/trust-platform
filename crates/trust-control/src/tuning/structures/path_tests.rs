//! Tests of the path reductions (specification 35, C6 to C9; numerical annex N4.7 and N5.15). Each expected value is a closed form, or
//! a tolerance that the reduction's own accuracy on the heater's response allows.

#![allow(clippy::float_cmp)] // The reduction's exact outputs are compared on purpose.

use trust_control_math::model::{StateSpace, Timing};

use super::*;
use crate::tuning::structures::fixtures;

/// A first-order plant with the steady gain `gain`, the pole `a` and the delay `delay`, in the given timing.
fn lag(a: f64, gain: f64, delay_seconds: f64, timing: Timing) -> DelayedSystem {
    DelayedSystem {
        system: StateSpace {
            a: vec![vec![a]],
            b: vec![vec![-a * gain]],
            c: vec![vec![1.0]],
            d: vec![vec![0.0]],
        },
        timing,
        delay_seconds,
    }
}

#[test]
fn the_settling_horizon_of_a_continuous_lag_is_ten_time_constants_plus_the_delay() {
    // The pole −0.5 decays at 0.5 per second: ten time constants are 20 s, and the delay adds 1 s.
    let horizon =
        settling_horizon(&lag(-0.5, 1.0, 1.0, Timing::Continuous), "plant").expect("settles");
    assert!((horizon - 21.0).abs() < 1e-9, "horizon {horizon}");
}

#[test]
fn the_settling_horizon_of_a_discrete_lag_uses_the_log_of_its_pole() {
    // The pole 0.5 at a sample period of 0.1 s decays at |ln 0.5|/0.1 per second, so the horizon is 10·0.1/|ln 0.5|.
    let plant = lag(
        0.5,
        1.0,
        0.0,
        Timing::Discrete {
            sample_time_seconds: 0.1,
        },
    );
    let horizon = settling_horizon(&plant, "plant").expect("settles");
    assert!(
        (horizon - 1.0 / 0.5_f64.ln().abs() * 0.1 * 10.0).abs() < 1e-9,
        "horizon {horizon}"
    );
}

#[test]
fn a_plant_that_does_not_decay_has_no_settling_horizon() {
    let refusal = settling_horizon(&lag(0.0, 1.0, 0.0, Timing::Continuous), "plant")
        .expect_err("a pole at zero does not settle");
    assert_eq!(
        refusal.0,
        "A plant path does not settle: the design needs a stable model."
    );
}

#[test]
fn the_heater_step_response_reaches_its_steady_state_on_the_grid_of_the_annex() {
    // Its slowest pole is −1/8, so the horizon is 80 s. A continuous plant with one output takes 4095 steps: 4096 samples.
    let plant = delayed_plant(&fixtures::heater_model()).expect("the heater is realized");
    let response =
        open_loop_step_response(&plant, 0, "controller output").expect("the step settles");
    assert_eq!(response.times.len(), 4096);
    assert!(
        (response.finals[0] - 0.2).abs() < 1e-3,
        "final value {}",
        response.finals[0]
    );
}

#[test]
fn the_two_point_reduction_of_the_heater_path_is_its_gain_time_constant_and_no_delay() {
    // The path is 0.2/(8s + 1): the two-point method reads τ = 8 s and θ = 0 from the grid.
    let plant = delayed_plant(&fixtures::heater_model()).expect("the heater is realized");
    let response =
        open_loop_step_response(&plant, 0, "controller output").expect("the step settles");
    let path = two_point_path(&response.times, &response.outputs[0], "controller output")
        .expect("reduces");
    assert!((path.gain - 0.2).abs() < 0.002, "gain {}", path.gain);
    assert!(
        (path.time_constant_seconds - 8.0).abs() < 0.05,
        "time constant {}",
        path.time_constant_seconds
    );
    assert!(
        path.delay_seconds.abs() < 0.05,
        "delay {}",
        path.delay_seconds
    );
}

#[test]
fn a_flat_response_has_no_steady_state_gain_to_reduce() {
    let refusal = two_point_path(&[0.0, 1.0, 2.0], &[0.0, 0.0, 0.0], "x").expect_err("no change");
    assert_eq!(refusal.0, "The x path has no steady-state gain.");
}

#[test]
fn a_continuous_model_is_sampled_at_the_task_period_and_a_transfer_function_is_refused() {
    let sampled =
        sampled_model(&fixtures::heater_model(), 0.1, "cascade").expect("the heater samples");
    assert_eq!(sampled.sample_time_seconds, 0.1);
    assert_eq!(sampled.system.states(), 2);
    let refusal = sampled_model(&fixtures::smith_heater_model(), 0.1, "cascade")
        .expect_err("a transfer function is not designed on");
    assert_eq!(refusal.0, "The cascade is designed on a state-space model.");
}

#[test]
fn the_reduced_plant_is_the_first_order_plant_with_the_given_delay() {
    let path = TwoPointReduction {
        gain: 0.2,
        time_constant_seconds: 8.0,
        delay_seconds: 1.5,
    };
    match reduced_plant(&path, 0.0) {
        TuningPlant::FirstOrder(model) => {
            assert_eq!(model.gain, 0.2);
            assert_eq!(model.time_constant_seconds, 8.0);
            assert_eq!(model.delay_seconds, 0.0);
        }
        other => panic!("expected a first-order plant, got {other:?}"),
    }
}

#[test]
fn an_operating_point_is_read_by_index_and_is_zero_where_it_has_no_value() {
    let (inputs, outputs) = operating_point(&fixtures::heater_model());
    assert_eq!(inputs, vec![80.0, 10.0]);
    assert_eq!(outputs, vec![35.0]);
    assert_eq!(operating_value(&inputs, 1), 10.0);
    assert_eq!(operating_value(&outputs, 3), 0.0);
}
