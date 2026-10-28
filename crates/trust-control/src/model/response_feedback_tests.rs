//! The stepped response of a plant through a controller (`generalModelTime.ts`): the feedback connection of `initialState` (Q2).
//! Ported assertions name the TypeScript test they come from.

use std::cell::Cell;

use serde_json::{json, Value};
use trust_control_math::model::{transfer_function_state_space, Timing};
use trust_control_math::simulate::{DelayedSystem, TimeSettings};

use super::{stepped_response, SCALAR_FEEDBACK, SINGULAR_FEEDTHROUGH};
use crate::failure::{Cause, Failure, NextAction};
use crate::model::prediction::delayed_system_of;
use crate::model::{decode_general_model, GeneralModel};

fn channel(id: &str, unit: &str) -> Value {
    json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -10.0, "maximum": 10.0 } })
}

/// The plant of `general-model-session.test.ts`'s `separates initial-state and delayed zero-state responses`: `ẏ = −y + u`, input delay
/// 0.5 s, output `y`.
fn delayed_plant() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "plant", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.5,
        "inputs": [channel("command", "rpm")], "outputs": [channel("speed", "rpm")],
        "states": [channel("speed-state", "rpm")],
        "a": [[-1.0]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]],
    }))
    .expect("the plant is valid")
}

/// A static transfer function `k` as a delayed system (no state, no delay).
fn static_gain(k: f64) -> DelayedSystem {
    DelayedSystem {
        system: transfer_function_state_space(&[k], &[1.0])
            .expect("a static gain")
            .system,
        timing: Timing::Continuous,
        delay_seconds: 0.0,
    }
}

fn settings(initial_state: Vec<f64>, input: Vec<f64>) -> TimeSettings {
    TimeSettings {
        step_seconds: 0.25,
        duration_seconds: 1.0,
        initial_state,
        input,
    }
}

#[test]
fn a_unit_feedback_keeps_the_initial_response_until_the_delay_and_differs_from_the_open_loop_after_it(
) {
    // Ported from `separates initial-state and delayed zero-state responses through the selected-model session`
    // (control-studio-general-model-session.test.ts): the feedback initial response is still 2·e^(−t) at 0.25 s, before the delayed input can
    // reach the plant; at 1 s the feedback total differs from the open loop total by more than 0.01.
    let plant = delayed_system_of(&delayed_plant()).expect("the plant realizes");
    let controller = static_gain(1.0);
    let open = stepped_response(&plant, None, &settings(vec![2.0], vec![1.0]), 512, &|| {
        Ok(())
    })
    .expect("the open loop responds");
    let closed = stepped_response(
        &plant,
        Some(&controller),
        &settings(vec![2.0], vec![1.0]),
        512,
        &|| Ok(()),
    )
    .expect("the feedback responds");
    assert_eq!(closed.points.len(), 5);
    let at_quarter = &closed.points[1];
    assert!(
        (at_quarter.initial_condition[0] - 2.0 * (-0.25_f64).exp()).abs() < 1e-10,
        "{:?}",
        at_quarter.initial_condition
    );
    // The open loop of the same plant: zero input history, so the zero-state response is zero until the delay.
    assert!(open.points[1].zero_state[0].abs() < 1e-12);
    let difference = (closed.points[4].total[0] - open.points[4].total[0]).abs();
    assert!(
        difference > 0.01,
        "the feedback changes the delayed response: {difference}"
    );
    for point in &closed.points {
        assert!(
            (point.total[0] - point.zero_state[0] - point.initial_condition[0]).abs() < 1e-10,
            "the total is the sum of its parts at {}",
            point.time_seconds
        );
    }
}

#[test]
fn a_static_plant_responds_to_its_held_input_without_states() {
    // A static gain of 2 with the held input 1: the zero-state response is 2 at every sample, and the response to the (empty) initial state is zero.
    let plant = static_gain(2.0);
    let response = stepped_response(&plant, None, &settings(Vec::new(), vec![1.0]), 512, &|| {
        Ok(())
    })
    .expect("a static plant responds");
    assert_eq!(response.points.len(), 5);
    for point in &response.points {
        assert!((point.zero_state[0] - 2.0).abs() < 1e-12);
        assert!(point.initial_condition[0].abs() < 1e-12);
        assert!((point.total[0] - 2.0).abs() < 1e-12);
    }
}

#[test]
fn a_singular_direct_feedthrough_is_refused_and_a_multi_channel_plant_is_not_a_scalar_loop() {
    // 1 + Dc·Dp = 1 + (−1)(1) = 0: the connection has no solution.
    let plant = static_gain(1.0);
    let singular = stepped_response(
        &plant,
        Some(&static_gain(-1.0)),
        &settings(Vec::new(), vec![1.0]),
        512,
        &|| Ok(()),
    )
    .expect_err("a singular feedthrough is refused");
    assert_eq!(singular.message, SINGULAR_FEEDTHROUGH);
    assert_eq!(singular.cause, Cause::ModelOrData);
    assert_eq!(singular.next_action, NextAction::None);
    // A plant with two inputs is not the scalar loop of the feedback connection.
    let two_inputs = decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "two", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("u1", "V"), channel("u2", "V")], "outputs": [channel("y", "m")],
        "states": [channel("x", "m")], "a": [[-1.0]], "b": [[1.0, 0.0]], "c": [[1.0]], "d": [[0.0, 0.0]],
    }))
    .expect("the two-input model is valid");
    let scalar = delayed_system_of(&two_inputs).expect("the plant realizes");
    let refused = stepped_response(
        &scalar,
        Some(&static_gain(1.0)),
        &settings(vec![0.0], vec![1.0, 0.0]),
        512,
        &|| Ok(()),
    )
    .expect_err("a two-input plant is refused");
    assert_eq!(refused.message, SCALAR_FEEDBACK);
}

#[test]
fn a_check_before_a_sample_that_fails_ends_the_response_with_its_failure() {
    // The budget (A7) is checked before each sample: the first sample is answered, and the failure of the second ends the response.
    let plant = delayed_system_of(&delayed_plant()).expect("the plant realizes");
    let samples = Cell::new(0_u32);
    let check = || {
        samples.set(samples.get() + 1);
        if samples.get() > 1 {
            Err(Failure::new(
                "Analysis was cancelled.",
                Cause::Unclassified,
                NextAction::None,
            ))
        } else {
            Ok(())
        }
    };
    let stopped = stepped_response(&plant, None, &settings(vec![2.0], vec![1.0]), 512, &check)
        .expect_err("the second sample is refused");
    assert_eq!(stopped.message, "Analysis was cancelled.");
    assert_eq!(samples.get(), 2);
}
