//! The product's open-loop model time response (generalModelTime.ts): the settings with their
//! connection, the response with its state coordinates and its transform and scope sentences, and the
//! guard that checks a stored response. The library computes the response. Closed-loop feedback needs
//! a controller, which belongs to a later stage, so a feedback connection is refused after the plant
//! has been checked, as the `TypeScript` does.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_control_math::simulate::{
    is_time_response, open_loop_response, time_settings, TimeResponse, TimeResponsePoint,
    TimeSettings,
};

use super::general::{decode_general_model, numbers, GeneralModel};
use super::limits::{MAXIMUM_DELAY_STATES, MAXIMUM_TIME_RESPONSE_VALUES};
use super::prediction::{delayed_system_of, plant_shape};
use super::refusal::{simulation_refusal, ModelRefusal};

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
        "Y(z) = [C(zI−A)⁻¹B + D] z⁻ᴸ U(z) + C(zI−A)⁻¹ z x(0)"
    } else {
        "Y(s) = [C(sI−A)⁻¹B + D] e⁻ˢᴸ U(s) + C(sI−A)⁻¹ x(0)"
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

/// The scope sentence of an open-loop response.
const OPEN_LOOP_SCOPE: &str = "Linear-model output deviations. The zero-state input response includes the declared delay; the separate initial-state response retains its original time origin. Prior input history is zero. Runtime limits and nonlinear validity remain separate.";

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
