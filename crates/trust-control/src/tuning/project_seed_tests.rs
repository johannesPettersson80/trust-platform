#![allow(clippy::float_cmp)] // The feedforward gains are exact quotients of exact inputs.

use serde_json::{json, Value};

use super::*;
use crate::model::decode_general_model;

fn seed() -> ParallelPidSeed {
    ParallelPidSeed {
        family: Structure::PI,
        proportional_symbol: "Main.Kp".to_string(),
        integral_symbol: Some("Main.Ki".to_string()),
        derivative_symbol: None,
    }
}

/// The trajectory seed of a PID law: its five variables, all distinct.
fn trajectory_seed() -> TrajectoryFeedforwardSeed {
    TrajectoryFeedforwardSeed {
        pid: ParallelPidSeed {
            family: Structure::PID,
            proportional_symbol: "Main.Kp".to_string(),
            integral_symbol: Some("Main.Ki".to_string()),
            derivative_symbol: Some("Main.Kd".to_string()),
        },
        rate_symbol: "Main.Kv".to_string(),
        acceleration_symbol: "Main.Ka".to_string(),
    }
}

/// The rotary axis of lesson 90 as a continuous state-space model: `J·s² + B·s` with `J = B = 1`, so `1/(s² + s)`.
fn rotary_axis_model() -> Value {
    json!({
        "kind": "continuous-state-space", "revision": "rotary-axis-j1-b1", "source": "provided",
        "validation": "validated", "evidence": ["sha256:provided"], "delaySeconds": 0,
        "inputs": [{ "id": "P1.TorqueNm", "unit": "N·m", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }],
        "outputs": [{ "id": "P1.AngleRad", "unit": "rad", "operatingRange": { "minimum": -2.0, "maximum": 2.0 } }],
        "states": [
            { "id": "angle", "unit": "rad", "operatingRange": { "minimum": -2.0, "maximum": 2.0 } },
            { "id": "velocity", "unit": "rad/s", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } }
        ],
        "a": [[0.0, 1.0], [0.0, -1.0]], "b": [[0.0], [1.0]], "c": [[1.0, 0.0]], "d": [[0.0]],
        "operatingPoint": { "inputs": [0.0], "outputs": [0.0] }
    })
}

#[test]
fn the_gains_are_read_from_the_named_variables() {
    let parameters = BTreeMap::from([("Main.Kp".to_string(), 2.0), ("Main.Ki".to_string(), 0.5)]);
    let gains = parallel_seed_gains(&seed(), &parameters).expect("bound");
    assert_eq!(gains.proportional_gain, 2.0);
    assert_eq!(gains.integral_gain, 0.5);
    assert_eq!(gains.derivative_gain, 0.0);
}

#[test]
fn a_pi_law_without_an_integral_variable_is_refused() {
    let mut without = seed();
    without.integral_symbol = None;
    assert_eq!(
        check_family_terms(&without, Structure::PI)
            .expect_err("no integral")
            .0,
        "A PI design needs an integral gain; the program's law has no integral gain."
    );
}

#[test]
fn the_designed_gains_are_written_to_the_named_variables_only() {
    let gains = ParallelGains {
        proportional_gain: 1.5,
        integral_gain: 0.3,
        derivative_gain: 0.0,
    };
    let values = seed_values(&seed(), gains);
    assert_eq!(values.len(), 2);
    assert_eq!(values["Main.Ki"], 0.3);
}

#[test]
fn the_trajectory_feedforward_is_b_over_n_and_a_over_n() {
    // n/(a s² + b s) with n = 2, a = 0.5, b = 4: K_v = 4/2 = 2, K_a = 0.5/2 = 0.25.
    assert_eq!(
        trajectory_feedforward(2.0, 0.5, 4.0).expect("integrator"),
        (2.0, 0.25)
    );
    assert!(trajectory_feedforward(0.0, 0.5, 4.0).is_err());
    assert!(trajectory_feedforward(2.0, 0.5, 0.0).is_err());
}

#[test]
fn a_trajectory_seed_names_its_five_variables_in_order() {
    assert_eq!(
        trajectory_seed().variables(),
        vec!["Main.Kp", "Main.Ki", "Main.Kd", "Main.Kv", "Main.Ka"]
    );
}

#[test]
fn the_coefficients_of_the_lesson_90_transfer_give_unit_feedforward_gains() {
    // The state-space conversion of lesson 90 gives 1/(s² + s): the numerator is the constant 1, the denominator s² + s.
    assert_eq!(
        trajectory_feedforward_of(&[0.0, 0.0, 1.0], &[1.0, 1.0, 0.0]).expect("n/(a s² + b s)"),
        (1.0, 1.0)
    );
}

#[test]
fn a_scaled_integrator_with_a_lag_gives_the_same_gains_as_its_monic_form() {
    // 2/(0.5 s² + 4 s): K_v = 4/2 = 2 and K_a = 0.5/2 = 0.25.
    assert_eq!(
        trajectory_feedforward_of(&[2.0], &[0.5, 4.0, 0.0]).expect("integrator"),
        (2.0, 0.25)
    );
}

#[test]
fn an_integrator_without_a_lag_has_no_acceleration_gain() {
    // 1/(2 s): K_v = 2 and K_a = 0.
    assert_eq!(
        trajectory_feedforward_of(&[1.0], &[2.0, 0.0]).expect("integrator"),
        (2.0, 0.0)
    );
}

#[test]
fn other_transfer_functions_are_refused_with_the_trajectory_sentence() {
    let sentence = texts::TRAJECTORY_REFUSED;
    let refused = [
        (&[1.0][..], &[1.0, 1.0][..]),
        (&[1.0, 1.0][..], &[1.0, 1.0, 0.0][..]),
        (&[1.0][..], &[1.0, 1.0, 1.0][..]),
        (&[0.0][..], &[1.0, 1.0, 0.0][..]),
        (&[1.0][..], &[1.0, 0.0, 0.0][..]),
    ];
    for (numerator, denominator) in refused {
        let refusal = trajectory_feedforward_of(numerator, denominator).expect_err("another form");
        assert_eq!(refusal.0, sentence);
    }
}

#[test]
fn a_continuous_state_space_model_gives_the_feedforward_of_its_transfer() {
    let model = decode_general_model(&rotary_axis_model()).expect("the lesson 90 axis");
    assert_eq!(
        trajectory_feedforward_of_model(&model).expect("1/(s² + s)"),
        (1.0, 1.0)
    );
}

#[test]
fn a_transfer_function_model_gives_the_feedforward_of_its_coefficients() {
    let model = decode_general_model(&json!({
        "kind": "transfer-function", "revision": "integrator", "source": "provided",
        "validation": "validated", "evidence": ["sha256:provided"], "delaySeconds": 0,
        "inputs": [{ "id": "P1.TorqueNm", "unit": "N·m", "operatingRange": { "minimum": -20.0, "maximum": 20.0 } }],
        "outputs": [{ "id": "P1.AngleRad", "unit": "rad", "operatingRange": { "minimum": -2.0, "maximum": 2.0 } }],
        "numerator": [1.0], "denominator": [2.0, 0.0]
    }))
    .expect("an integrating transfer function");
    assert_eq!(
        trajectory_feedforward_of_model(&model).expect("1/(2 s)"),
        (2.0, 0.0)
    );
}
