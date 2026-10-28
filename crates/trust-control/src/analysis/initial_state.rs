//! The initial-state response (`initialState`, B9.9; N4.0; draft §2.3): the output of a model over a duration, from its initial state
//! under a held input, open loop, or through the feedback with a controller (N4.4). The settings are checked first, each rule naming its
//! input (A7); the response is the product's (`model::response`).

use trust_control_math::model::{transfer_function_state_space, Timing};
use trust_control_math::simulate::{open_loop_response, DelayedSystem, TimeSettings};

use crate::analysis::budget::Budget;
use crate::analysis::general_feedback::FeedbackDefinition;
use crate::analysis::tool::{input_failure, SAMPLE_TIME_TOLERANCE, WHOLE_STEP_TOLERANCE};
use crate::failure::{Cause, Failure, NextAction};
use crate::messages::{AnalysisInputs, ChannelRef, InitialStateAnswer};
use crate::model::limits::{MAXIMUM_DELAY_STATES, MAXIMUM_TIME_RESPONSE_VALUES};
use crate::model::prediction::delayed_system_of;
use crate::model::refusal::simulation_refusal;
use crate::model::response::{
    stepped_response, ModelTimeSettings, ResponseConnection, CONTINUOUS_TRANSFORM,
    DISCRETE_TRANSFORM, FEEDBACK_SCOPE, FEEDBACK_TRANSFORM, OPEN_LOOP_SCOPE,
};
use crate::model::GeneralModel;

/// The output step when the request gives none, in seconds (B9.9).
pub const DEFAULT_STEP_SECONDS: f64 = 0.1;
/// The duration when the request gives none, in seconds (B9.9).
pub const DEFAULT_DURATION_SECONDS: f64 = 10.0;

/// A time setting of the response, the output step or the duration, must be a finite time above 0 s (A7). A text that no duration reads
/// arrives as `NaN` (`messages/analyze.rs`): it is refused with the form a time takes, so the engineer sees what to type. Each refusal
/// names the page's field, so the page can mark it.
fn positive_time(value: f64, name: &str, field: &str) -> Result<(), Failure> {
    if value.is_nan() {
        let text = format!("The {name} must be a time such as 90 s, 15 min or 1 h 30 min.");
        return Err(input_failure(&text).with_field(field));
    }
    if !(value.is_finite() && value > 0.0) {
        let text = format!("The {name} must be a finite time above 0 s.");
        return Err(input_failure(&text).with_field(field));
    }
    Ok(())
}

/// The settings of an initial-state response, checked against the model (N4.0, A7). The step and the duration are positive, the duration
/// is a whole number of steps and at least one, the response has at most 4,096 output values, a discrete model's step is its sample time,
/// the initial state and the held input have one finite value per state and per input, and a feedback connection has a controller.
///
/// # Errors
/// Returns the input failure that names the first input the rules refuse.
pub fn checked_settings(
    model: &GeneralModel,
    inputs: &AnalysisInputs,
    controller_given: bool,
) -> Result<ModelTimeSettings, Failure> {
    let plant = plant_of(model)?;
    let step = inputs.step_seconds.unwrap_or(DEFAULT_STEP_SECONDS);
    let duration = inputs.duration_seconds.unwrap_or(DEFAULT_DURATION_SECONDS);
    let initial_state = inputs
        .initial_state
        .clone()
        .unwrap_or_else(|| vec![0.0; plant.system.states()]);
    let input = inputs
        .input
        .clone()
        .unwrap_or_else(|| vec![0.0; plant.system.inputs()]);
    let connection = inputs.connection.unwrap_or(ResponseConnection::OpenLoop);
    positive_time(step, "step", "response-step")?;
    positive_time(duration, "duration", "response-duration")?;
    let count = duration / step;
    let steps = count.round();
    if !count.is_finite() || steps < 1.0 || (count - steps).abs() > WHOLE_STEP_TOLERANCE {
        return Err(input_failure(
            "The duration must be a whole number of steps, at least one step.",
        )
        .with_field("response-duration"));
    }
    let outputs = f64::from(u32::try_from(plant.system.outputs()).unwrap_or(u32::MAX));
    let maximum = f64::from(u32::try_from(MAXIMUM_TIME_RESPONSE_VALUES).unwrap_or(u32::MAX));
    if (steps + 1.0) * outputs > maximum {
        return Err(input_failure(
            "The response would have more than 4,096 output values: lengthen the step or shorten the duration.",
        ));
    }
    if let Timing::Discrete {
        sample_time_seconds,
    } = plant.timing
        && (step - sample_time_seconds).abs() > SAMPLE_TIME_TOLERANCE
    {
        return Err(
            input_failure("The step must equal the sample time of this discrete model.")
                .with_field("response-step"),
        );
    }
    if initial_state.len() != plant.system.states()
        || initial_state.iter().any(|value| !value.is_finite())
    {
        return Err(input_failure(
            "The initial state needs one finite value for each state.",
        ));
    }
    if input.len() != plant.system.inputs() || input.iter().any(|value| !value.is_finite()) {
        return Err(input_failure(
            "The held input needs one finite value for each input.",
        ));
    }
    if connection == ResponseConnection::Feedback && !controller_given {
        return Err(input_failure(
            "The feedback connection needs a controller: give one, or declare the loop's law.",
        ));
    }
    Ok(ModelTimeSettings {
        time: TimeSettings {
            step_seconds: step,
            duration_seconds: duration,
            initial_state,
            input,
        },
        connection,
    })
}

/// The answer of `initialState` (B9.9; N4.0; `generalModelTime.ts`): the response of the model from its initial state under the held input,
/// open loop or through the feedback with `controller` (N4.4). The budget is checked before the response and, for a feedback
/// response, before each sample (A7).
///
/// # Errors
/// Returns the input failure of [`checked_settings`], the failure of a model or controller that the response cannot realize, or the
/// failure of the budget: the A7 sentence, or the cancellation.
pub fn initial_state_answer(
    model: &GeneralModel,
    controller: Option<&FeedbackDefinition>,
    inputs: &AnalysisInputs,
    budget: &Budget<'_>,
) -> Result<InitialStateAnswer, Failure> {
    let settings = checked_settings(model, inputs, controller.is_some())?;
    budget.step()?;
    let plant = plant_of(model)?;
    let check = || budget.step();
    let response = match settings.connection {
        ResponseConnection::OpenLoop => {
            open_loop_response(&plant, &settings.time, MAXIMUM_DELAY_STATES)
                .map_err(|error| failure(&simulation_refusal(error).to_string()))?
        }
        ResponseConnection::Feedback => {
            let definition = controller.ok_or_else(|| {
                input_failure("The feedback connection needs a controller: give one, or declare the loop's law.")
            })?;
            let law = controller_system(definition, settings.time.step_seconds)?;
            stepped_response(
                &plant,
                Some(&law),
                &settings.time,
                MAXIMUM_DELAY_STATES,
                &check,
            )?
        }
    };
    let feedback = settings.connection == ResponseConnection::Feedback;
    let discrete = matches!(model, GeneralModel::DiscreteStateSpace { .. });
    let (transform, scope) = if feedback {
        (FEEDBACK_TRANSFORM, FEEDBACK_SCOPE)
    } else if discrete {
        (DISCRETE_TRANSFORM, OPEN_LOOP_SCOPE)
    } else {
        (CONTINUOUS_TRANSFORM, OPEN_LOOP_SCOPE)
    };
    let state_coordinates: Vec<String> = match model {
        GeneralModel::TransferFunction { .. } => (0..settings.time.initial_state.len())
            .map(|index| format!("Companion state x{}", index + 1))
            .collect(),
        GeneralModel::ContinuousStateSpace { states, .. }
        | GeneralModel::DiscreteStateSpace { states, .. } => states
            .iter()
            .map(|state| format!("{} ({})", state.id, state.unit))
            .collect(),
    };
    let summary = model
        .identity()
        .outputs
        .iter()
        .map(|output| format!("{} initial-state response.", output.id))
        .collect();
    Ok(InitialStateAnswer {
        settings,
        state_coordinates,
        transform: transform.to_owned(),
        scope: scope.to_owned(),
        points: response.points,
        outputs: model
            .identity()
            .outputs
            .iter()
            .map(|output| ChannelRef {
                id: output.id.clone(),
                unit: output.unit.clone(),
            })
            .collect(),
        summary,
        details: vec![format!("{scope} Transform: {transform}")],
    })
}

/// The controller of a feedback response as a delayed system. A declared law is a continuous transfer function without delay (as
/// `generalModelTime.ts` takes it); the controller model is used as it is, and a sampled controller must have the step as its sample time.
fn controller_system(definition: &FeedbackDefinition, step: f64) -> Result<DelayedSystem, Failure> {
    if let Some(declared) = &definition.declared_controller {
        if declared.numerator.len() > declared.denominator.len() {
            return Err(failure(
                "Time response requires the controller's proper derivative filter model.",
            ));
        }
        let companion = transfer_function_state_space(&declared.numerator, &declared.denominator)
            .map_err(|_| {
            failure("The declared controller's transfer function cannot be realized.")
        })?;
        return Ok(DelayedSystem {
            system: companion.system,
            timing: Timing::Continuous,
            delay_seconds: 0.0,
        });
    }
    let system = plant_of(&definition.controller)?;
    if let Timing::Discrete {
        sample_time_seconds,
    } = system.timing
        && (sample_time_seconds - step).abs() > SAMPLE_TIME_TOLERANCE
    {
        return Err(failure("Controller and plant sample periods must match."));
    }
    Ok(system)
}

/// The plant of a model, realized as the response uses it (`delayedModelRealization`).
fn plant_of(model: &GeneralModel) -> Result<DelayedSystem, Failure> {
    delayed_system_of(model).map_err(|refusal| failure(&refusal.to_string()))
}

/// A failure of the response with its sentence; the model and its settings are the engineer's to correct, so no next action is offered.
fn failure(text: &str) -> Failure {
    Failure::new(text, Cause::ModelOrData, NextAction::None)
}

#[cfg(test)]
#[path = "initial_state_tests.rs"]
mod tests;
