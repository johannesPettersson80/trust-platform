//! The product's model time response (`generalModelTime.ts`): the settings with their connection, the response with its state
//! coordinates and its transform and scope sentences, and the guard that checks a stored response. The library computes the
//! open-loop response. A feedback response needs a controller: [`stepped_response`] steps the plant and the controller one sample at a
//! time, as `generalModelTime.ts` does, with the plant's delay and the controller's own states. `analyze_general_model_time` still
//! refuses a feedback connection that carries no controller.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_control_math::linalg::DenseMatrix;
use trust_control_math::model::StateSpace;
use trust_control_math::simulate::{
    delayed_realization, is_time_response, open_loop_response, time_settings, DelayedSystem,
    TimeResponse, TimeResponsePoint, TimeSettings,
};

use super::general::{decode_general_model, numbers, GeneralModel};
use super::limits::{MAXIMUM_DELAY_STATES, MAXIMUM_TIME_RESPONSE_VALUES};
use super::prediction::{delayed_system_of, plant_shape};
use super::refusal::{simulation_refusal, ModelRefusal};
use crate::failure::{Cause, Failure, NextAction};

/// The transform equation of a discrete open-loop response.
pub const DISCRETE_TRANSFORM: &str = "Y(z) = [C(zI−A)⁻¹B + D] z⁻ᴸ U(z) + C(zI−A)⁻¹ z x(0)";
/// The transform equation of a continuous open-loop response.
pub const CONTINUOUS_TRANSFORM: &str = "Y(s) = [C(sI−A)⁻¹B + D] e⁻ˢᴸ U(s) + C(sI−A)⁻¹ x(0)";
/// The transform equation of a sampled feedback response.
pub const FEEDBACK_TRANSFORM: &str = "zX(z) − z x(0) = A_cl X(z) + B_cl R(z)";
/// The scope sentence of an open-loop response.
pub const OPEN_LOOP_SCOPE: &str = "Linear-model output deviations. The zero-state input response includes the declared delay; the separate initial-state response retains its original time origin. Prior input history is zero. Runtime limits and nonlinear validity remain separate.";
/// The scope sentence of a sampled feedback response (`generalModelTime.ts`).
pub const FEEDBACK_SCOPE: &str = "Sampled negative feedback with held inputs and the declared delay. Controller states and prior input history start at zero. Curves are deviations; actuator limits and nonlinear effects are checked by actual-ST validation.";
/// The sentence of a feedback response whose plant or controller is not single-input and single-output.
pub const SCALAR_FEEDBACK: &str =
    "This feedback response needs a single-input, single-output plant and controller.";
/// The sentence of a feedback response whose direct feedthrough is singular (`generalModelTime.ts`).
pub const SINGULAR_FEEDTHROUGH: &str =
    "The selected feedback has a singular direct-feedthrough connection.";
/// The denominator below which the direct feedthrough is singular (`generalModelTime.ts`).
const SINGULAR_DENOMINATOR: f64 = 1e-12;

/// How the response's model is connected: open loop (the held input drives the plant directly) or
/// feedback (through a controller).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ResponseConnection {
    /// The held input drives the plant directly.
    OpenLoop,
    /// The plant is connected through a controller.
    Feedback,
}

/// The settings of a response with their connection, as the `TypeScript` stores them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelTimeSettings {
    /// The step, duration, initial state and input.
    #[serde(flatten)]
    pub time: TimeSettings,
    /// How the model is connected.
    pub connection: ResponseConnection,
}

/// An open-loop response of a model, with the sentences that describe it. Its JSON keys are the
/// `TypeScript`'s, so a stored response reads the same.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelTimeResponse {
    /// The model the response belongs to.
    pub model: GeneralModel,
    /// The settings the response was computed with.
    pub settings: ModelTimeSettings,
    /// The name of each state of the response, in order.
    pub state_coordinates: Vec<String>,
    /// The transform equation of the response.
    pub transform: String,
    /// The scope sentence of the response.
    pub scope: String,
    /// The samples at `0, T, …, duration`.
    pub points: Vec<TimeResponsePoint>,
}

/// The settings of a response for a model, from their raw JSON. The connection must be exactly
/// `open-loop` or `feedback`, and the numbers must satisfy the library's settings rules for the
/// model's plant.
#[must_use]
pub fn model_time_settings(raw: &Value, model: &GeneralModel) -> Option<ModelTimeSettings> {
    let object = raw.as_object()?;
    let connection = match object.get("connection").and_then(Value::as_str)? {
        "open-loop" => ResponseConnection::OpenLoop,
        "feedback" => ResponseConnection::Feedback,
        _ => return None,
    };
    let step = object.get("stepSeconds").and_then(Value::as_f64)?;
    let duration = object.get("durationSeconds").and_then(Value::as_f64)?;
    let input = numbers(object.get("input")?)?;
    let initial_state = numbers(object.get("initialState")?)?;
    let (system, timing) = plant_shape(model)?;
    let time = time_settings(
        &system,
        timing,
        step,
        duration,
        &initial_state,
        &input,
        MAXIMUM_TIME_RESPONSE_VALUES,
    )
    .ok()?;
    Some(ModelTimeSettings { time, connection })
}

/// The open-loop response of a model to its held input, from the initial state in the settings.
///
/// # Errors
/// Returns the sentence of the first failed rule: the model, the settings, the plant's sample period
/// and realization, the response's finiteness, and, for a feedback connection, the missing controller.
pub fn analyze_general_model_time(
    value: &Value,
    raw_settings: &Value,
) -> Result<ModelTimeResponse, ModelRefusal> {
    let model = decode_general_model(value).ok_or(ModelRefusal::SelectValidModel)?;
    let settings =
        model_time_settings(raw_settings, &model).ok_or(ModelRefusal::ChooseResponseSettings)?;
    let plant = delayed_system_of(&model)?;
    let response = open_loop_response(&plant, &settings.time, MAXIMUM_DELAY_STATES)
        .map_err(simulation_refusal)?;
    if settings.connection == ResponseConnection::Feedback {
        return Err(ModelRefusal::FeedbackControllerMissing);
    }
    let state_coordinates: Vec<String> = match &model {
        GeneralModel::TransferFunction { .. } => (0..settings.time.initial_state.len())
            .map(|index| format!("Companion state x{}", index + 1))
            .collect(),
        GeneralModel::ContinuousStateSpace { states, .. }
        | GeneralModel::DiscreteStateSpace { states, .. } => states
            .iter()
            .map(|state| format!("{} ({})", state.id, state.unit))
            .collect(),
    };
    let transform = if matches!(model, GeneralModel::DiscreteStateSpace { .. }) {
        DISCRETE_TRANSFORM
    } else {
        CONTINUOUS_TRANSFORM
    };
    Ok(ModelTimeResponse {
        model,
        settings,
        state_coordinates,
        transform: transform.to_owned(),
        scope: OPEN_LOOP_SCOPE.to_owned(),
        points: response.points,
    })
}

/// Whether a stored response was computed for the selected model: its model is the selected model,
/// its settings are valid for it, its names and sentences are text, and its samples are the library's
/// response shape (one value per output, sample times at index times step).
#[must_use]
pub fn is_model_time_response(value: &Value, selected: &GeneralModel) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let model_matches = object
        .get("model")
        .and_then(|stored| crate::json::decode_value::<GeneralModel>(stored.clone()).ok())
        .is_some_and(|stored| stored == *selected);
    if !model_matches {
        return false;
    }
    let Some(settings) = object
        .get("settings")
        .and_then(|raw| model_time_settings(raw, selected))
    else {
        return false;
    };
    let names_are_text = object
        .get("stateCoordinates")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().all(Value::is_string));
    let sentences_are_text = object.get("transform").is_some_and(Value::is_string)
        && object.get("scope").is_some_and(Value::is_string);
    if !names_are_text || !sentences_are_text {
        return false;
    }
    let Some(points) = object.get("points").and_then(Value::as_array) else {
        return false;
    };
    let Ok(points) = points
        .iter()
        .map(|point| crate::json::decode_value::<TimeResponsePoint>(point.clone()))
        .collect::<Result<Vec<_>, _>>()
    else {
        return false;
    };
    let Some((system, timing)) = plant_shape(selected) else {
        return false;
    };
    is_time_response(
        &TimeResponse {
            settings: settings.time,
            points,
        },
        &system,
        timing,
        MAXIMUM_TIME_RESPONSE_VALUES,
    )
}

/// The response of a plant, or of the plant through negative feedback with a controller, one sample at a time (`generalModelTime.ts`).
/// With no controller the held input drives the plant directly. With one, the controller takes the reference (the held input) and the
/// plant's measured output, and its command is the plant's input, which the plant's delay holds back. The zero-state response and the
/// response to the initial state are computed separately, as the open loop computes them. `between_samples` is checked before each
/// sample (A7); a failure it returns ends the response.
///
/// # Errors
/// Returns the failure of a plant or controller that is not single-input and single-output, of a realization that the state budget or
/// the exact conversion refuses, of a singular direct feedthrough, of a response that is not finite, or of the check before a sample.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // The count is a whole number of steps, at most 4,096 samples.
pub fn stepped_response(
    plant: &DelayedSystem,
    controller: Option<&DelayedSystem>,
    settings: &TimeSettings,
    maximum_states: usize,
    between_samples: &dyn Fn() -> Result<(), Failure>,
) -> Result<TimeResponse, Failure> {
    let scalar =
        |system: &DelayedSystem| system.system.inputs() == 1 && system.system.outputs() == 1;
    if !scalar(plant) || controller.is_some_and(|law| !scalar(law)) {
        return Err(failure(SCALAR_FEEDBACK));
    }
    let step = settings.step_seconds;
    let (plant_matrices, plant_initial) =
        realize(plant, step, &settings.initial_state, maximum_states)?;
    let controller_realization = match controller {
        Some(law) => Some(realize(
            law,
            step,
            &vec![0.0; law.system.states()],
            maximum_states,
        )?),
        None => None,
    };
    let controller_zero = controller_realization
        .as_ref()
        .map_or_else(Vec::new, |(_, initial)| vec![0.0; initial.len()]);
    let law = controller_realization
        .as_ref()
        .map(|(matrices, _)| matrices);
    let steps = (settings.duration_seconds / step).round() as usize;
    let no_input = vec![0.0; settings.input.len()];
    let mut driven_state = vec![0.0; plant_initial.len()];
    let mut released_state = plant_initial;
    let mut driven_controller = controller_zero.clone();
    let mut released_controller = controller_zero;
    let mut points = Vec::with_capacity(steps + 1);
    for index in 0..=steps {
        between_samples()?;
        let driven = advance(
            &plant_matrices,
            &driven_state,
            &settings.input,
            law,
            &driven_controller,
        )?;
        let released = advance(
            &plant_matrices,
            &released_state,
            &no_input,
            law,
            &released_controller,
        )?;
        let total: Vec<f64> = driven
            .output
            .iter()
            .zip(&released.output)
            .map(|(forced, initial)| forced + initial)
            .collect();
        let finite = [
            &driven.output,
            &released.output,
            &total,
            &driven.state,
            &released.state,
        ]
        .iter()
        .all(|values| values.iter().all(|value| value.is_finite()));
        if !finite {
            return Err(failure(NONFINITE_RESPONSE));
        }
        points.push(TimeResponsePoint {
            time_seconds: f64::from(u32::try_from(index).unwrap_or(u32::MAX)) * step,
            zero_state: driven.output,
            initial_condition: released.output,
            total,
        });
        driven_state = driven.state;
        released_state = released.state;
        driven_controller = driven.controller_state;
        released_controller = released.controller_state;
    }
    Ok(TimeResponse {
        settings: settings.clone(),
        points,
    })
}

/// The sentence of a response that is not finite.
const NONFINITE_RESPONSE: &str = "The response exceeded finite model coordinates.";

/// The matrices of a realization as the sample loop uses them. For a static gain with no delay `a` and `b` have no rows, and `c` has one
/// empty row per output.
struct Matrices {
    a: DenseMatrix,
    b: DenseMatrix,
    c: DenseMatrix,
    d: DenseMatrix,
}

/// One sample of the loop: the output, the next state of the plant, and the next state of the controller.
struct Advanced {
    output: Vec<f64>,
    state: Vec<f64>,
    controller_state: Vec<f64>,
}

/// The matrices of a plant or a controller and its initial coordinates, the plant's delayed input included, all zero. A static gain with
/// no delay has no state, so its matrices are used as they are (the exact realization needs at least one state).
fn realize(
    system: &DelayedSystem,
    step: f64,
    initial_state: &[f64],
    maximum_states: usize,
) -> Result<(Matrices, Vec<f64>), Failure> {
    if system.system.states() == 0 && system.delay_seconds <= 0.0 {
        let matrices = Matrices {
            a: Vec::new(),
            b: Vec::new(),
            c: vec![Vec::new(); system.system.outputs()],
            d: system.system.d.clone(),
        };
        return Ok((matrices, Vec::new()));
    }
    let zero_input = vec![0.0; system.system.inputs()];
    let realization = delayed_realization(system, step, initial_state, &zero_input, maximum_states)
        .map_err(|error| refusal_failure(&simulation_refusal(error)))?;
    let StateSpace { a, b, c, d } = realization.system;
    Ok((Matrices { a, b, c, d }, realization.initial_state))
}

/// One sample: the output with the held reference, or, with a controller, the command that the feedback gives, and the next states.
fn advance(
    plant: &Matrices,
    state: &[f64],
    reference: &[f64],
    controller: Option<&Matrices>,
    controller_state: &[f64],
) -> Result<Advanced, Failure> {
    let (input, controller_next) = match controller {
        None => (reference.to_vec(), Vec::new()),
        Some(law) => {
            let feedthrough = plant.d[0][0];
            let denominator = 1.0 + law.d[0][0] * feedthrough;
            if denominator.abs() < SINGULAR_DENOMINATOR {
                return Err(failure(SINGULAR_FEEDTHROUGH));
            }
            let measured = dot(&plant.c[0], state);
            let command = (dot(&law.c[0], controller_state)
                + law.d[0][0] * (reference[0] - measured))
                / denominator;
            let error = reference[0] - measured - feedthrough * command;
            let next: Vec<f64> = law
                .a
                .iter()
                .enumerate()
                .map(|(row, values)| dot(values, controller_state) + law.b[row][0] * error)
                .collect();
            (vec![command], next)
        }
    };
    let output = plant
        .c
        .iter()
        .enumerate()
        .map(|(row, values)| dot(values, state) + dot(&plant.d[row], &input))
        .collect();
    let next_state = plant
        .a
        .iter()
        .enumerate()
        .map(|(row, values)| dot(values, state) + dot(&plant.b[row], &input))
        .collect();
    Ok(Advanced {
        output,
        state: next_state,
        controller_state: controller_next,
    })
}

/// The dot product of two vectors of the same length.
fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

/// A failure of the response with its sentence, and no next action the engineer can take from the step.
fn failure(text: &str) -> Failure {
    Failure::new(text, Cause::ModelOrData, NextAction::None)
}

fn refusal_failure(refusal: &ModelRefusal) -> Failure {
    failure(&refusal.to_string())
}

#[cfg(test)]
#[path = "response_feedback_tests.rs"]
mod feedback_tests;
