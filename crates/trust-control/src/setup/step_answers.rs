//! The answers of the step-by-step setup as the page sends them (specification 35, B1.3, C12 and C13; `configure.rs`). The draft is one
//! JSON object whose keys are the setup's questions, with the `structure` key `step-by-step`. Each answer names its identity: a
//! controller by its instance, a signal by its address, a symbol by its name. The engine finds each one in the compiled catalog and
//! refuses an identity the catalog does not hold, under its question, so the page keeps no copy of the catalog.

use serde_json::Value;

use super::catalog::{CatalogAnswer, CompiledSignal, CompiledSymbol};
use super::configure::configure_loop;
use super::configure_channels::accept_rate;
use super::prompts::{
    ActuatorChoice, ChannelMode, ControllerChoice, FeedbackChoice, RateAnswer, SetpointChoice,
    SetupStepAnswers, SetupWorkflow,
};
use super::save::{plan_loop, PlannedLoop};
use super::texts::answers as texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopScope;

/// The `structure` a step-by-step draft names.
pub const STRUCTURE: &str = "step-by-step";

/// Whether a draft is a step-by-step draft.
#[must_use]
pub fn is_step_by_step(draft: &Value) -> bool {
    draft.get("structure").and_then(Value::as_str) == Some(STRUCTURE)
}

/// Decodes the answers of a step-by-step draft against the compiled catalog. The `structure` key is not a question.
///
/// # Errors
/// The draft is not an object; a key is not a question of the setup; an answer is not of its question's kind; or an identity is not
/// in the catalog.
pub fn decode_answers(draft: &Value, catalog: &CatalogAnswer) -> Result<SetupStepAnswers, Failure> {
    let Value::Object(fields) = draft else {
        return Err(refusal(texts::not_object()));
    };
    let mut answers = SetupStepAnswers::default();
    for (key, value) in fields {
        if key != "structure" {
            set_answer(&mut answers, catalog, key, value)?;
        }
    }
    Ok(answers)
}

/// The file of the loop a step-by-step draft builds: the answers are decoded, the loop is built and checked (`configure_loop`), and its
/// file is planned. Nothing is written.
///
/// # Errors
/// The answers do not decode, the loop does not build, or it does not satisfy the loop contract.
pub fn plan(
    draft: &Value,
    catalog: &CatalogAnswer,
    scope: &LoopScope,
) -> Result<PlannedLoop, Failure> {
    let answers = decode_answers(draft, catalog)?;
    plan_loop(configure_loop(catalog, scope, &answers)?, scope)
}

fn set_answer(
    answers: &mut SetupStepAnswers,
    catalog: &CatalogAnswer,
    key: &str,
    value: &Value,
) -> Result<(), Failure> {
    match key {
        "name" => answers.name = text(key, value)?,
        "controllerInstance" => answers.controller = controller(catalog, key, value)?,
        "customInstance" => answers.custom_instance = text(key, value)?,
        "customType" => answers.custom_type = text(key, value)?,
        "taskPeriodMs" => answers.task_period_ms = number(key, value)?,
        "processVariable" => answers.process_variable = signal(catalog, key, value)?,
        "controllerOutput" => answers.controller_output = signal(catalog, key, value)?,
        "setpoint" => answers.setpoint = setpoint(catalog, key, value)?,
        "workflow" => answers.workflow = workflow(key, value)?,
        "processUnit" => answers.process_unit = text(key, value)?,
        "processMinimum" => answers.process_minimum = number(key, value)?,
        "processMaximum" => answers.process_maximum = number(key, value)?,
        "outputUnit" => answers.output_unit = text(key, value)?,
        "outputMinimum" => answers.output_minimum = number(key, value)?,
        "outputMaximum" => answers.output_maximum = number(key, value)?,
        "gain" => answers.gain = symbol(catalog, key, value)?,
        "gainMinimum" => answers.gain_minimum = number(key, value)?,
        "gainMaximum" => answers.gain_maximum = number(key, value)?,
        "manualCommand" => answers.manual_command = symbol(catalog, key, value)?,
        "manualMode" => answers.manual_mode = symbol(catalog, key, value)?,
        "manualWhenTrue" => answers.manual_when_true = boolean(key, value)?,
        "controllerTask" => answers.controller_task = text(key, value)?,
        "observationDurationSeconds" => {
            answers.observation_duration_seconds = number(key, value)?;
        }
        "adapterMapping" => answers.adapter_mapping = text(key, value)?.map(String::into_bytes),
        "channelMode" => answers.channel_mode = channel_mode(key, value)?,
        "actuator" => answers.actuator = actuator(key, value)?,
        "risingRate" => answers.rising_rate = rate(key, value)?,
        "fallingRate" => answers.falling_rate = rate(key, value)?,
        "feedback" => answers.feedback = feedback(catalog, key, value)?,
        other => return Err(refusal_at(other, &texts::unknown_question(other))),
    }
    Ok(())
}

fn refusal(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenLoopSetup)
}

fn refusal_at(key: &str, text: &str) -> Failure {
    refusal(text).with_field(key)
}

/// A text answer; null is no answer.
fn text(key: &str, value: &Value) -> Result<Option<String>, Failure> {
    match value {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text.clone())),
        _ => Err(refusal_at(key, &texts::not_text(key))),
    }
}

/// A number answer; null is no answer.
fn number(key: &str, value: &Value) -> Result<Option<f64>, Failure> {
    match value {
        Value::Null => Ok(None),
        Value::Number(number) => number
            .as_f64()
            .map(Some)
            .ok_or_else(|| refusal_at(key, &texts::not_number(key))),
        _ => Err(refusal_at(key, &texts::not_number(key))),
    }
}

/// A true or false answer; null is no answer.
fn boolean(key: &str, value: &Value) -> Result<Option<bool>, Failure> {
    match value {
        Value::Null => Ok(None),
        Value::Bool(flag) => Ok(Some(*flag)),
        _ => Err(refusal_at(key, &texts::not_boolean(key))),
    }
}

/// The controller: null is a custom controller, typed by `customInstance` and `customType`; text is a compiled instance.
fn controller(
    catalog: &CatalogAnswer,
    key: &str,
    value: &Value,
) -> Result<Option<ControllerChoice>, Failure> {
    let Some(instance) = text(key, value)? else {
        return Ok(Some(ControllerChoice::Custom));
    };
    catalog
        .control
        .controllers
        .iter()
        .find(|controller| controller.instance == instance)
        .cloned()
        .map(|controller| Some(ControllerChoice::Compiled(controller)))
        .ok_or_else(|| refusal_at(key, &texts::unknown_controller(&instance)))
}

fn signal(
    catalog: &CatalogAnswer,
    key: &str,
    value: &Value,
) -> Result<Option<CompiledSignal>, Failure> {
    let Some(address) = text(key, value)? else {
        return Ok(None);
    };
    catalog
        .signals
        .iter()
        .find(|signal| signal.address == address)
        .cloned()
        .map(Some)
        .ok_or_else(|| refusal_at(key, &texts::unknown_signal(&address)))
}

/// The setpoint: null is an open loop, text is the setpoint's signal address.
fn setpoint(
    catalog: &CatalogAnswer,
    key: &str,
    value: &Value,
) -> Result<Option<SetpointChoice>, Failure> {
    if value.is_null() {
        return Ok(Some(SetpointChoice::OpenLoop));
    }
    Ok(signal(catalog, key, value)?.map(SetpointChoice::Signal))
}

fn symbol(
    catalog: &CatalogAnswer,
    key: &str,
    value: &Value,
) -> Result<Option<CompiledSymbol>, Failure> {
    let Some(name) = text(key, value)? else {
        return Ok(None);
    };
    catalog
        .control
        .symbols
        .iter()
        .find(|symbol| symbol.symbol == name)
        .cloned()
        .map(Some)
        .ok_or_else(|| refusal_at(key, &texts::unknown_symbol(&name)))
}

fn workflow(key: &str, value: &Value) -> Result<Option<SetupWorkflow>, Failure> {
    let Some(name) = text(key, value)? else {
        return Ok(None);
    };
    match name.as_str() {
        "observation" => Ok(Some(SetupWorkflow::Observation)),
        "adapter" => Ok(Some(SetupWorkflow::Adapter)),
        "on-off" => Ok(Some(SetupWorkflow::OnOff)),
        "proportional" => Ok(Some(SetupWorkflow::Proportional)),
        _ => Err(refusal_at(key, &texts::not_offered(key, &name))),
    }
}

fn channel_mode(key: &str, value: &Value) -> Result<Option<ChannelMode>, Failure> {
    let Some(name) = text(key, value)? else {
        return Ok(None);
    };
    match name.as_str() {
        "single" => Ok(Some(ChannelMode::Single)),
        "multiple" => Ok(Some(ChannelMode::Multiple)),
        _ => Err(refusal_at(key, &texts::not_offered(key, &name))),
    }
}

fn actuator(key: &str, value: &Value) -> Result<Option<ActuatorChoice>, Failure> {
    let Some(name) = text(key, value)? else {
        return Ok(None);
    };
    match name.as_str() {
        "continuous" => Ok(Some(ActuatorChoice::Continuous)),
        "on-off" => Ok(Some(ActuatorChoice::OnOff)),
        _ => Err(refusal_at(key, &texts::not_offered(key, &name))),
    }
}

/// A rate limit: the text as typed, blank for no limit (`accept_rate`).
fn rate(key: &str, value: &Value) -> Result<Option<RateAnswer>, Failure> {
    let Some(answer) = text(key, value)? else {
        return Ok(None);
    };
    accept_rate(&answer)
        .map(Some)
        .map_err(|reason| refusal_at(key, &reason))
}

/// The applied-action feedback: `output` is the controller output itself, text is the feedback's signal address.
fn feedback(
    catalog: &CatalogAnswer,
    key: &str,
    value: &Value,
) -> Result<Option<FeedbackChoice>, Failure> {
    if value.as_str() == Some("output") {
        return Ok(Some(FeedbackChoice::UseOutput));
    }
    Ok(signal(catalog, key, value)?.map(FeedbackChoice::Signal))
}

#[cfg(test)]
#[path = "step_answers_tests.rs"]
mod tests;
