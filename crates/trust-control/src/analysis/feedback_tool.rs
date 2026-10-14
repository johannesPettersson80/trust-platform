//! The general feedback tool of `controlStudio/analyze` (`feedback`, B9.9, N4.3, N4.4): the loop that a request closes, with
//! the controller it carries or the loop's declared law, its channels and its grid, and the analysis of that loop on the grid.
//! The analysis itself is in `general_feedback`.

use trust_control_math::frequency::logarithmic_grid;

use crate::analysis::budget::Budget;
use crate::analysis::declared::DeclaredControlAnalysis;
use crate::analysis::feedback_declared::declared_feedback_definition;
use crate::analysis::frequency::checked_grid;
use crate::analysis::general_feedback::{
    analyze_general_feedback, ChannelPair, FeedbackDefinition, FeedbackResult,
};
use crate::analysis::tool::{
    channel_count, channel_point_budget, default_grid, first_pair, input_failure,
    MAXIMUM_CHANNEL_FREQUENCY_POINTS,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::messages::{AnalysisInputs, FrequencyGrid};
use crate::model::{GeneralModel, ModelChannel};

const NO_CONTROLLER: &str =
    "This analysis needs a controller: send one with the request, or declare the loop's control law.";
const LAW_DOES_NOT_FIT: &str = "The loop's declared control law does not close a loop with this plant: the plant must be a single-input, single-output model in the law's units (N7.6).";
const DECLARED_CHANNEL: &str =
    "plantChannel must be the only input and output of the plant for a declared control law (N7.6).";
const PLANT_CHANNEL: &str = "plantChannel must name an input and an output of the plant (N4.4).";
const CONTROLLER_CHANNEL: &str =
    "controllerChannel must name an input and an output of the controller (N4.4).";
const CONTROLLER_OUTPUT_UNIT: &str =
    "The controller's output unit must be the plant's input unit (N4.4).";
const CONTROLLER_INPUT_UNIT: &str =
    "The controller's input unit must be the plant's output unit (N4.4).";
const CHANNEL_LIMIT: &str = "The grid's points times the loop's channels must be at most 16,384, the plant's channels counted twice (N4.3).";
const GRID_RANGE: &str =
    "The grid's frequencies must be finite and positive, with the maximum above the minimum.";

/// The feedback definition of the loop that `feedback` analyses (N4.4): the plant and its controller, on the channels the
/// request names (`plantChannel`, `controllerChannel`; the first input and output of each model when it names none). Without a
/// controller model, the loop closes with the loop's declared law, when the law fits the plant (N7.6).
///
/// # Errors
/// Returns the input failure that names the channel or the unit concerned, or, with no controller at all, the failure whose
/// next action is the loop setup.
pub fn feedback_definition(
    model: &GeneralModel,
    controller: Option<&GeneralModel>,
    declared: Option<&DeclaredControlAnalysis>,
    inputs: &AnalysisInputs,
) -> Result<FeedbackDefinition, Failure> {
    if let Some(controller) = controller {
        let plant_channel = inputs
            .plant_channel
            .clone()
            .unwrap_or_else(|| first_pair(model));
        let controller_channel = inputs
            .controller_channel
            .clone()
            .unwrap_or_else(|| first_pair(controller));
        check_channels(model, controller, &plant_channel, &controller_channel)?;
        return Ok(FeedbackDefinition {
            plant: model.clone(),
            controller: controller.clone(),
            plant_channel,
            controller_channel,
            declared_controller: None,
        });
    }
    let declared = declared
        .ok_or_else(|| Failure::new(NO_CONTROLLER, Cause::Settings, NextAction::OpenLoopSetup))?;
    let definition = declared_feedback_definition(declared, model).ok_or_else(|| {
        Failure::new(
            LAW_DOES_NOT_FIT,
            Cause::ModelOrData,
            NextAction::OpenLoopSetup,
        )
    })?;
    if inputs
        .plant_channel
        .as_ref()
        .is_some_and(|channel| *channel != definition.plant_channel)
    {
        return Err(input_failure(DECLARED_CHANNEL));
    }
    Ok(definition)
}

/// The channels of the loop exist in their models, and the units agree at its two junctions (N4.4). Each failure names
/// the channel or the unit concerned.
fn check_channels(
    model: &GeneralModel,
    controller: &GeneralModel,
    plant: &ChannelPair,
    controller_channel: &ChannelPair,
) -> Result<(), Failure> {
    let plant_identity = model.identity();
    let controller_identity = controller.identity();
    let plant_input = channel_named(&plant_identity.inputs, &plant.input_id)
        .ok_or_else(|| input_failure(PLANT_CHANNEL))?;
    let plant_output = channel_named(&plant_identity.outputs, &plant.output_id)
        .ok_or_else(|| input_failure(PLANT_CHANNEL))?;
    let controller_input = channel_named(&controller_identity.inputs, &controller_channel.input_id)
        .ok_or_else(|| input_failure(CONTROLLER_CHANNEL))?;
    let controller_output =
        channel_named(&controller_identity.outputs, &controller_channel.output_id)
            .ok_or_else(|| input_failure(CONTROLLER_CHANNEL))?;
    if controller_output.unit != plant_input.unit {
        return Err(input_failure(CONTROLLER_OUTPUT_UNIT));
    }
    if controller_input.unit != plant_output.unit {
        return Err(input_failure(CONTROLLER_INPUT_UNIT));
    }
    Ok(())
}

fn channel_named<'a>(channels: &'a [ModelChannel], id: &str) -> Option<&'a ModelChannel> {
    channels.iter().find(|channel| channel.id == id)
}

/// The grid of `feedback` (N4.3): the request's grid, or the default of the plant lowered to the channel limit of the loop.
/// The channel-frequency points count the plant's channels twice and the controller's once.
///
/// # Errors
/// Returns the input failure of the first rule of N4.3 that the grid breaks.
pub fn feedback_grid(
    definition: &FeedbackDefinition,
    given: Option<FrequencyGrid>,
) -> Result<FrequencyGrid, Failure> {
    let weight = 2 * channel_count(&definition.plant) + channel_count(&definition.controller);
    let defaults = default_grid(&definition.plant);
    let defaults = FrequencyGrid {
        points: defaults.points.min(channel_point_budget(weight)),
        ..defaults
    };
    let grid = checked_grid(&definition.plant, Some(given.unwrap_or(defaults)))?;
    let weight = u64::try_from(weight).unwrap_or(u64::MAX);
    if u64::from(grid.points).saturating_mul(weight) > MAXIMUM_CHANNEL_FREQUENCY_POINTS {
        return Err(input_failure(CHANNEL_LIMIT));
    }
    Ok(grid)
}

fn frequencies_of(grid: FrequencyGrid) -> Result<Vec<f64>, Failure> {
    let points = usize::try_from(grid.points).unwrap_or(0);
    logarithmic_grid(grid.minimum_frequency, grid.maximum_frequency, points)
        .map_err(|_| input_failure(GRID_RANGE))
}

/// The answer of `feedback` (B9.9; N4.4, N7.7 to N7.11): the open loop, the sensitivities, the Nyquist count, the closed-loop
/// poles and the margins of the loop on its grid, with the sentences of the result. `budget` is checked before the grid and
/// before the analysis (A7).
///
/// # Errors
/// Returns the input failure of the grid, the budget's failure, or the failure of the refusal of the loop (A11.1).
pub fn feedback_answer(
    definition: &FeedbackDefinition,
    inputs: &AnalysisInputs,
    budget: &Budget<'_>,
) -> Result<FeedbackResult, Failure> {
    budget.step()?;
    let grid = feedback_grid(definition, inputs.grid)?;
    let frequencies = frequencies_of(grid)?;
    budget.step()?;
    analyze_general_feedback(definition, &frequencies).map_err(Failure::from)
}

#[cfg(test)]
#[path = "feedback_tool_tests.rs"]
mod tests;
