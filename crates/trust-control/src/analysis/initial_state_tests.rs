//! The initial-state response (`initialState`, B9.9; N4.0): its settings and its response, open loop and through the feedback. Ported
//! assertions name the TypeScript test they come from.

use std::time::Duration;

use serde_json::{json, Value};

use super::{checked_settings, initial_state_answer};
use crate::analysis::budget::{Budget, ANALYSIS_BUDGET};
use crate::analysis::general_feedback::{ChannelPair, FeedbackDefinition};
use crate::json::decode_value;
use crate::messages::AnalysisInputs;
use crate::model::response::{
    ResponseConnection, FEEDBACK_SCOPE, FEEDBACK_TRANSFORM, OPEN_LOOP_SCOPE,
};
use crate::model::{decode_general_model, GeneralModel};

fn channel(id: &str, unit: &str) -> Value {
    json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -10.0, "maximum": 10.0 } })
}

/// The plant of `separates initial-state and delayed zero-state responses`: `ẏ = −y + u` with an input delay of 0.5 s.
fn delayed_plant() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "plant", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.5,
        "inputs": [channel("command", "V")], "outputs": [channel("speed", "rpm")],
        "states": [channel("speed-state", "rpm")],
        "a": [[-1.0]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]],
    }))
    .expect("the plant is valid")
}

/// The unit controller of the same test: a static transfer function `1`, reading the speed and commanding the command.
fn unit_controller() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "unit-feedback", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("speed", "rpm")], "outputs": [channel("command", "V")],
        "numerator": [1.0], "denominator": [1.0],
    }))
    .expect("the controller is valid")
}

fn settings(connection: Option<ResponseConnection>) -> AnalysisInputs {
    AnalysisInputs {
        step_seconds: Some(0.25),
        duration_seconds: Some(1.0),
        initial_state: Some(vec![2.0]),
        input: Some(vec![1.0]),
        connection,
        ..AnalysisInputs::default()
    }
}

/// Runs `run` with a budget that has spent no time and was not cancelled.
fn calm<T>(run: impl FnOnce(&Budget<'_>) -> T) -> T {
    let elapsed = || Duration::ZERO;
    let stop = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &stop);
    run(&budget)
}

fn feedback_definition() -> FeedbackDefinition {
    FeedbackDefinition {
        plant: delayed_plant(),
        controller: unit_controller(),
        plant_channel: ChannelPair {
            input_id: "command".to_owned(),
            output_id: "speed".to_owned(),
        },
        controller_channel: ChannelPair {
            input_id: "speed".to_owned(),
            output_id: "command".to_owned(),
        },
        declared_controller: None,
    }
}

#[test]
fn the_zero_state_response_waits_for_the_delay_and_the_initial_response_does_not_shift() {
    // Ported from `separates initial-state and delayed zero-state responses through the selected-model session` (control-studio-general-model-session.test.ts):
    // the zero-state response is 1 − e^(−(t − 0.5)) after the delay and zero before it; the response to the initial state is 2·e^(−t), and the
    // total is their sum, at every sample of 0.25 s to 1 s.
    let model = delayed_plant();
    let answer = calm(|budget| {
        initial_state_answer(
            &model,
            None,
            &settings(Some(ResponseConnection::OpenLoop)),
            budget,
        )
    })
    .expect("the open-loop response qualifies");
    assert_eq!(answer.points.len(), 5);
    for point in &answer.points {
        let t = point.time_seconds;
        let forced = if t <= 0.5 {
            0.0
        } else {
            1.0 - (-(t - 0.5)).exp()
        };
        assert!(
            (point.zero_state[0] - forced).abs() < 1e-10,
            "zero state at {t}"
        );
        assert!(
            (point.initial_condition[0] - 2.0 * (-t).exp()).abs() < 1e-10,
            "initial condition at {t}"
        );
        assert!((point.total[0] - point.zero_state[0] - point.initial_condition[0]).abs() < 1e-10);
    }
    assert_eq!(
        answer.summary,
        vec!["speed initial-state response.".to_owned()]
    );
    assert!(
        answer.details[0].starts_with(OPEN_LOOP_SCOPE),
        "{:?}",
        answer.details
    );
    assert!(answer.details[0].contains("Transform: Y(s) = "));
    assert_eq!(
        answer.state_coordinates,
        vec!["speed-state (rpm)".to_owned()]
    );
}

#[test]
fn a_delayed_feedback_loop_retains_its_unshifted_initial_response_and_recomputes_the_delayed_plant()
{
    // Ported from `separates initial-state and delayed zero-state responses through the selected-model session` (control-studio-general-model-session.test.ts),
    // its feedback part: the feedback initial response is still 2·e^(−0.25) at 0.25 s, and it differs from the open loop at 1 s by more than 0.01.
    let model = delayed_plant();
    let definition = feedback_definition();
    let closed = calm(|budget| {
        initial_state_answer(
            &model,
            Some(&definition),
            &settings(Some(ResponseConnection::Feedback)),
            budget,
        )
    })
    .expect("the feedback response qualifies");
    let open = calm(|budget| {
        initial_state_answer(
            &model,
            None,
            &settings(Some(ResponseConnection::OpenLoop)),
            budget,
        )
    })
    .expect("the open-loop response qualifies");
    assert!(
        (closed.points[1].initial_condition[0] - 2.0 * (-0.25_f64).exp()).abs() < 1e-10,
        "{:?}",
        closed.points[1].initial_condition
    );
    let difference = (closed.points[4].total[0] - open.points[4].total[0]).abs();
    assert!(
        difference > 0.01,
        "the delayed feedback changes the total: {difference}"
    );
    assert_eq!(closed.transform, FEEDBACK_TRANSFORM);
    assert_eq!(closed.scope, FEEDBACK_SCOPE);
}

#[test]
fn a_transfer_function_names_its_companion_states_and_keeps_the_initial_state_on_its_output() {
    // A transfer function `1 / (s + 1)` has one companion state, which is its output: the initial state 1 is the output at 0 s.
    let model = decode_general_model(&json!({
        "kind": "transfer-function", "revision": "first-order", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("u", "V")], "outputs": [channel("y", "m")],
        "numerator": [1.0], "denominator": [1.0, 1.0],
    }))
    .expect("the transfer function is valid");
    let inputs = AnalysisInputs {
        step_seconds: Some(0.25),
        duration_seconds: Some(1.0),
        initial_state: Some(vec![1.0]),
        input: Some(vec![0.0]),
        ..AnalysisInputs::default()
    };
    let answer = calm(|budget| initial_state_answer(&model, None, &inputs, budget))
        .expect("the response qualifies");
    assert_eq!(
        answer.state_coordinates,
        vec!["Companion state x1".to_owned()]
    );
    assert!((answer.points[0].initial_condition[0] - 1.0).abs() < 1e-12);
    assert!((answer.points[1].initial_condition[0] - (-0.25_f64).exp()).abs() < 1e-10);
}

#[test]
fn each_setting_is_checked_and_its_refusal_names_the_input() {
    let model = delayed_plant();
    let refused = |inputs: &AnalysisInputs| {
        checked_settings(&model, inputs, false).expect_err("a setting outside its rule")
    };
    let mut bad = settings(None);
    bad.step_seconds = Some(0.3);
    assert!(
        refused(&bad).message.contains("whole number"),
        "a duration that is not whole steps"
    );
    let mut short = settings(None);
    short.duration_seconds = Some(0.1);
    assert!(
        refused(&short).message.contains("whole number"),
        "a duration below one step"
    );
    let mut states = settings(None);
    states.initial_state = Some(vec![1.0, 2.0]);
    assert!(refused(&states).message.contains("initial state"));
    let mut inputs = settings(None);
    inputs.input = Some(Vec::new());
    assert!(refused(&inputs).message.contains("held input"));
    let mut long = settings(None);
    long.step_seconds = Some(0.001);
    long.duration_seconds = Some(10.0);
    assert!(
        refused(&long).message.contains("4,096"),
        "more than 4,096 output values"
    );
    let feedback = settings(Some(ResponseConnection::Feedback));
    assert!(checked_settings(&model, &feedback, false)
        .expect_err("a feedback connection needs a controller")
        .message
        .contains("controller"));
    assert!(checked_settings(&model, &feedback, true).is_ok());
}

#[test]
fn a_discrete_model_takes_its_sample_time_as_the_step() {
    let discrete = decode_general_model(&json!({
        "kind": "discrete-state-space", "revision": "discrete", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("u", "V")], "outputs": [channel("y", "m")],
        "states": [channel("x", "m")], "a": [[0.5]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]],
        "sampleTimeSeconds": 0.1,
    }))
    .expect("the discrete model is valid");
    let mut inputs = settings(None);
    assert!(checked_settings(&discrete, &inputs, false)
        .expect_err("0.25 s is not 0.1 s")
        .message
        .contains("sample time"));
    inputs.step_seconds = Some(0.1);
    inputs.duration_seconds = Some(0.5);
    assert!(checked_settings(&discrete, &inputs, false).is_ok());
}

#[test]
fn a_cancelled_response_stops_before_it_is_computed() {
    let model = delayed_plant();
    let elapsed = || Duration::ZERO;
    let stop = || true;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &stop);
    let stopped = initial_state_answer(&model, None, &settings(None), &budget)
        .expect_err("a cancelled response is not answered");
    assert_eq!(stopped.message, "Analysis was cancelled.");
}

#[test]
fn a_typed_time_is_checked_as_its_number_of_seconds() {
    let model = delayed_plant();
    let typed: AnalysisInputs = decode_value(json!({
        "stepSeconds": "0.25 s", "durationSeconds": "1 s", "initialState": [2.0], "input": [1.0],
    }))
    .expect("the typed inputs decode");
    let number: AnalysisInputs = decode_value(json!({
        "stepSeconds": 0.25, "durationSeconds": 1.0, "initialState": [2.0], "input": [1.0],
    }))
    .expect("the numeric inputs decode");
    let typed_time = checked_settings(&model, &typed, false).expect("the typed time qualifies");
    let numeric_time = checked_settings(&model, &number, false).expect("the number qualifies");
    assert_eq!(typed_time.time.step_seconds, numeric_time.time.step_seconds);
    assert_eq!(
        typed_time.time.duration_seconds,
        numeric_time.time.duration_seconds
    );
}

#[test]
fn a_refused_time_names_the_page_field_it_came_from() {
    let model = delayed_plant();
    let refused = |entries: Value| {
        let inputs: AnalysisInputs = decode_value(entries).expect("the inputs decode");
        checked_settings(&model, &inputs, false).expect_err("a time outside its rule")
    };
    let unreadable = refused(json!({
        "stepSeconds": "soon", "durationSeconds": 1.0, "initialState": [2.0], "input": [1.0],
    }));
    assert_eq!(unreadable.field.as_deref(), Some("response-step"));
    assert!(
        unreadable
            .message
            .contains("must be a time such as 90 s, 15 min or 1 h 30 min"),
        "{}",
        unreadable.message
    );
    let zero = refused(json!({
        "stepSeconds": 0, "durationSeconds": "1 s", "initialState": [2.0], "input": [1.0],
    }));
    assert_eq!(zero.field.as_deref(), Some("response-step"));
    let uneven = refused(json!({
        "stepSeconds": "0.3", "durationSeconds": "1 s", "initialState": [2.0], "input": [1.0],
    }));
    assert_eq!(uneven.field.as_deref(), Some("response-duration"));
    let unread_duration = refused(json!({
        "stepSeconds": "0.25 s", "durationSeconds": "12 m", "initialState": [2.0], "input": [1.0],
    }));
    assert_eq!(unread_duration.field.as_deref(), Some("response-duration"));
    assert!(
        unread_duration.message.contains("must be a time"),
        "{}",
        unread_duration.message
    );
}
