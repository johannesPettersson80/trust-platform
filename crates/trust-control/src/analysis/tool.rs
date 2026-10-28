//! The catalogue of Control Studio's analysis tools (specification 35, B9.9 and A7; draft §2.2): which tools apply to a model and a
//! loop, the words of their labels and buttons, the inputs each takes by default, and the helpers that the tools' calculations share.
//! Every rule and every word the page shows is here, so the page renders the engine's answer and checks nothing itself.

// `ToolOffer` and the tool catalogue repeat the module's name, which is the name the wire types carry.
#![allow(clippy::module_name_repetitions)]

use std::f64::consts::PI;

use trust_control_math::model::Timing;

use crate::analysis::declared::DeclaredControlAnalysis;
use crate::analysis::general_feedback::ChannelPair;
use crate::analysis::lead_lag::LeadLagMode;
use crate::failure::{Cause, Failure, NextAction};
use crate::messages::{
    AnalysisInputs, AnalysisTool, ChannelRef, FrequencyGrid, PoleZeroView, ToolOffer,
};
use crate::model::{linear_transfer_model, GeneralModel, ResponseConnection};

/// The tools in the order the page lists them (draft §2.2).
pub const ALL_TOOLS: [AnalysisTool; 11] = [
    AnalysisTool::RootLocus,
    AnalysisTool::Routh,
    AnalysisTool::PoleZero,
    AnalysisTool::Frequency,
    AnalysisTool::MeasuredFrequency,
    AnalysisTool::MeasuredPaths,
    AnalysisTool::Feedback,
    AnalysisTool::LeadLag,
    AnalysisTool::InitialState,
    AnalysisTool::StateStructure,
    AnalysisTool::RelativeGain,
];

/// The highest frequency of a default grid in rad/s (C23).
const DEFAULT_MAXIMUM_FREQUENCY: f64 = 100.0;
/// The lowest frequency of a default grid in rad/s (C23).
const DEFAULT_MINIMUM_FREQUENCY: f64 = 0.1;
/// The points of a default grid (C23): 128, lowered when the channel limit needs it.
const DEFAULT_POINTS: u32 = 128;
/// The most channel-frequency points of one analysis: channels times grid points (N4.3, A7).
pub const MAXIMUM_CHANNEL_FREQUENCY_POINTS: u64 = 16_384;
/// The fewest and the most points of a frequency grid (N4.3, A7).
pub const GRID_POINTS: std::ops::RangeInclusive<u32> = 2..=512;
/// The smallest tolerance of a sample time agreement, as the time-response settings check it (A7).
pub const SAMPLE_TIME_TOLERANCE: f64 = 1e-10;
/// The tolerance of a whole number of steps in a duration (A7).
pub const WHOLE_STEP_TOLERANCE: f64 = 1e-8;

/// The words of one tool in the page: its label, its button's text, the text of its abort button when it is an operation, and
/// whether it is an operation (B9.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Presentation {
    /// The name of the tool in the page.
    pub label: &'static str,
    /// The text of the button that calculates it.
    pub action: &'static str,
    /// The text of the button that aborts it, for an operation.
    pub abort_action: Option<&'static str>,
    /// Whether the tool is an operation.
    pub operation: bool,
}

/// The words of `tool` (B9.9: the page's labels, action texts and abort texts, which move here).
#[must_use]
pub fn presentation(tool: AnalysisTool) -> Presentation {
    let (label, action, operation) = match tool {
        AnalysisTool::RootLocus => ("Root locus", "Calculate root locus", true),
        AnalysisTool::Routh => ("Routh–Hurwitz stability", "Calculate Routh–Hurwitz", true),
        AnalysisTool::PoleZero => ("Pole-zero map", "Calculate poles and zeros", false),
        AnalysisTool::Frequency => (
            "Bode (frequency response)",
            "Calculate frequency response",
            true,
        ),
        AnalysisTool::MeasuredFrequency => (
            "Measured frequency response",
            "Calculate measured frequency response",
            true,
        ),
        AnalysisTool::MeasuredPaths => ("Measured MIMO paths", "Calculate measured paths", true),
        AnalysisTool::Feedback => (
            "Nyquist and loop response",
            "Calculate feedback analysis",
            true,
        ),
        AnalysisTool::LeadLag => ("Lead/lag design", "Calculate lead/lag design", false),
        AnalysisTool::InitialState => (
            "Initial-state response",
            "Calculate initial-state response",
            true,
        ),
        AnalysisTool::StateStructure => (
            "Controllability and observability",
            "Calculate state structure",
            false,
        ),
        AnalysisTool::RelativeGain => (
            "Relative gain array",
            "Calculate relative gain array",
            false,
        ),
    };
    Presentation {
        label,
        action,
        abort_action: operation.then_some("Abort analysis"),
        operation,
    }
}

/// What the engine knows of the loop and its study when it answers the tools: the model, the controller that the request carries,
/// the declared analysis of the loop file, and the recordings the study lists (empty for a loop the engine does not serve).
#[derive(Debug, Clone, Copy)]
pub struct Facts<'a> {
    /// The model to analyse, when there is one.
    pub model: Option<&'a GeneralModel>,
    /// The controller the request carries, for `feedback`.
    pub controller: Option<&'a GeneralModel>,
    /// The declared analysis of the loop file, when the loop declares one.
    pub declared: Option<&'a DeclaredControlAnalysis>,
    /// The recordings the loop's study lists, by identifier.
    pub recordings: &'a [String],
    /// Whether the loop declares as many bound inputs as outputs, two or more (C7): what `measuredPaths` needs.
    pub multichannel: bool,
}

/// Whether `tool` applies to the facts (B9.9, draft §2.2).
#[must_use]
pub fn applies(tool: AnalysisTool, facts: &Facts<'_>) -> bool {
    match tool {
        AnalysisTool::RootLocus | AnalysisTool::Routh => facts.declared.is_some(),
        AnalysisTool::PoleZero | AnalysisTool::Frequency | AnalysisTool::InitialState => {
            facts.model.is_some()
        }
        AnalysisTool::MeasuredFrequency => !facts.recordings.is_empty(),
        AnalysisTool::MeasuredPaths => facts.multichannel && !facts.recordings.is_empty(),
        AnalysisTool::Feedback => {
            facts.model.is_some() && (facts.controller.is_some() || facts.declared.is_some())
        }
        AnalysisTool::LeadLag => facts.model.is_some_and(|model| {
            model.identity().inputs.len() == 1
                && model.identity().outputs.len() == 1
                && converts_to_transfer(model)
        }),
        AnalysisTool::StateStructure => facts
            .model
            .is_some_and(|model| !matches!(model, GeneralModel::TransferFunction { .. })),
        AnalysisTool::RelativeGain => facts.model.is_some_and(|model| {
            let channels = model.identity().inputs.len();
            !matches!(model, GeneralModel::TransferFunction { .. })
                && channels > 1
                && channels == model.identity().outputs.len()
        }),
    }
}

/// The offers of the tools that apply, in the order of [`ALL_TOOLS`], each with its words and its default inputs (B9.9).
#[must_use]
pub fn offers(facts: &Facts<'_>) -> Vec<ToolOffer> {
    ALL_TOOLS
        .into_iter()
        .filter(|tool| applies(*tool, facts))
        .map(|tool| {
            let words = presentation(tool);
            ToolOffer {
                tool,
                label: words.label.to_owned(),
                action: words.action.to_owned(),
                abort_action: words.abort_action.map(str::to_owned),
                operation: words.operation,
                defaults: defaults_for(tool, facts),
            }
        })
        .collect()
}

/// The inputs `tool` takes when the request gives none, for the facts (draft §2.2). Fields that need a model are absent without one.
#[must_use]
pub fn defaults_for(tool: AnalysisTool, facts: &Facts<'_>) -> AnalysisInputs {
    let mut defaults = AnalysisInputs::default();
    match tool {
        AnalysisTool::RootLocus | AnalysisTool::Routh => {
            defaults.selected_value = facts
                .declared
                .map(|declared| declared.root_locus.selected_value);
        }
        AnalysisTool::PoleZero => defaults.view = Some(PoleZeroView::Unreduced),
        AnalysisTool::Frequency => defaults.grid = facts.model.map(default_grid),
        AnalysisTool::MeasuredFrequency => {
            defaults.frequencies = Some(vec![1.0, 2.0, 4.0]);
            defaults.segment_count = Some(4);
            defaults.minimum_coherence = Some(0.8);
            defaults.startup_samples = Some(16);
        }
        AnalysisTool::Feedback => {
            defaults.grid = facts.model.map(default_grid);
            defaults.plant_channel = facts.model.map(first_pair);
            defaults.controller_channel = facts.controller.map(first_pair);
        }
        AnalysisTool::LeadLag => {
            defaults.mode = Some(LeadLagMode::Lead);
            defaults.crossover_rad_per_second = Some(1.0);
            defaults.value = Some(default_lead_lag_value(LeadLagMode::Lead));
        }
        AnalysisTool::InitialState => {
            defaults.step_seconds = Some(0.1);
            defaults.duration_seconds = Some(10.0);
            defaults.initial_state = facts.model.map(|model| vec![0.0; state_count(model)]);
            defaults.input = facts
                .model
                .map(|model| vec![0.0; model.identity().inputs.len()]);
            defaults.connection = Some(ResponseConnection::OpenLoop);
        }
        AnalysisTool::StateStructure | AnalysisTool::RelativeGain | AnalysisTool::MeasuredPaths => {
        }
    }
    defaults
}

/// The inputs `tool` reads: the request's own fields, with the defaults filled in, and no field that another tool reads. The
/// envelope lists exactly these (draft §2.1).
#[must_use]
pub fn read_inputs(
    tool: AnalysisTool,
    given: &AnalysisInputs,
    facts: &Facts<'_>,
) -> AnalysisInputs {
    let defaults = defaults_for(tool, facts);
    let mut inputs = AnalysisInputs::default();
    match tool {
        AnalysisTool::RootLocus | AnalysisTool::Routh => {
            inputs.selected_value = given.selected_value.or(defaults.selected_value);
        }
        AnalysisTool::PoleZero => inputs.view = given.view.or(defaults.view),
        AnalysisTool::Frequency => inputs.grid = given.grid.or(defaults.grid),
        AnalysisTool::MeasuredFrequency => {
            inputs.recording_id.clone_from(&given.recording_id);
            inputs
                .recording_revision
                .clone_from(&given.recording_revision);
            inputs.frequencies = given.frequencies.clone().or(defaults.frequencies);
            inputs.segment_count = given.segment_count.or(defaults.segment_count);
            inputs.minimum_coherence = given.minimum_coherence.or(defaults.minimum_coherence);
            inputs.startup_samples = given.startup_samples.or(defaults.startup_samples);
        }
        AnalysisTool::Feedback => {
            inputs.grid = given.grid.or(defaults.grid);
            inputs.plant_channel = given.plant_channel.clone().or(defaults.plant_channel);
            inputs.controller_channel = given
                .controller_channel
                .clone()
                .or(defaults.controller_channel);
        }
        AnalysisTool::LeadLag => {
            let mode = given.mode.or(defaults.mode);
            inputs.mode = mode;
            inputs.crossover_rad_per_second = given
                .crossover_rad_per_second
                .or(defaults.crossover_rad_per_second);
            // The default value depends on the mode: 30° for a lead, a ratio of 5 for a lag.
            inputs.value = given
                .value
                .or_else(|| mode.map(default_lead_lag_value))
                .or(defaults.value);
        }
        AnalysisTool::InitialState => {
            inputs.step_seconds = given.step_seconds.or(defaults.step_seconds);
            inputs.duration_seconds = given.duration_seconds.or(defaults.duration_seconds);
            inputs.initial_state = given.initial_state.clone().or(defaults.initial_state);
            inputs.input = given.input.clone().or(defaults.input);
            inputs.connection = given.connection.or(defaults.connection);
        }
        AnalysisTool::MeasuredPaths => {
            inputs.recording_id.clone_from(&given.recording_id);
            inputs
                .recording_revision
                .clone_from(&given.recording_revision);
            inputs.settled_trials.clone_from(&given.settled_trials);
            inputs.maximum_unexcited_input_drift = given.maximum_unexcited_input_drift;
        }
        AnalysisTool::StateStructure | AnalysisTool::RelativeGain => {}
    }
    inputs
}

/// The default value of a lead/lag design (B9.9): 30 degrees for a lead, the low-frequency gain ratio 5 for a lag.
#[must_use]
pub fn default_lead_lag_value(mode: LeadLagMode) -> f64 {
    match mode {
        LeadLagMode::Lead => 30.0,
        LeadLagMode::Lag => 5.0,
    }
}

/// The default frequency grid of a model (C23): 0.1 to 100 rad/s with 128 points, the maximum at most `π / (2·Ts)` for a discrete
/// model, and the points lowered so that the channel-frequency points stay within the limit (N4.3).
#[must_use]
pub fn default_grid(model: &GeneralModel) -> FrequencyGrid {
    let maximum = match model.timing() {
        Timing::Continuous => DEFAULT_MAXIMUM_FREQUENCY,
        Timing::Discrete {
            sample_time_seconds,
        } => DEFAULT_MAXIMUM_FREQUENCY.min(0.5 * PI / sample_time_seconds),
    };
    FrequencyGrid {
        minimum_frequency: DEFAULT_MINIMUM_FREQUENCY,
        maximum_frequency: maximum,
        points: DEFAULT_POINTS.min(channel_point_budget(channel_count(model))),
    }
}

/// The points one channel may have at most, so that the channel-frequency points stay within the limit (A7).
#[must_use]
pub fn channel_point_budget(channels: usize) -> u32 {
    let channels = u64::try_from(channels.max(1)).unwrap_or(u64::MAX);
    u32::try_from(MAXIMUM_CHANNEL_FREQUENCY_POINTS / channels).unwrap_or(u32::MAX)
}

/// The number of input-output pairs of a model (its inputs times its outputs).
#[must_use]
pub fn channel_count(model: &GeneralModel) -> usize {
    model.identity().inputs.len() * model.identity().outputs.len()
}

/// The number of states of a model's realization: a transfer function's denominator order, or the states of a state-space model.
/// This is the length of an initial state (the page's rule, `generalModelTime.ts`).
#[must_use]
pub fn state_count(model: &GeneralModel) -> usize {
    match model {
        GeneralModel::TransferFunction { transfer, .. } => {
            transfer.denominator.len().saturating_sub(1)
        }
        GeneralModel::ContinuousStateSpace { states, .. }
        | GeneralModel::DiscreteStateSpace { states, .. } => states.len(),
    }
}

/// The first input and output of a model, as a channel pair.
#[must_use]
pub fn first_pair(model: &GeneralModel) -> ChannelPair {
    let identity = model.identity();
    ChannelPair {
        input_id: identity
            .inputs
            .first()
            .map(|channel| channel.id.clone())
            .unwrap_or_default(),
        output_id: identity
            .outputs
            .first()
            .map(|channel| channel.id.clone())
            .unwrap_or_default(),
    }
}

/// The channel of a model's input or output, by its identifier and unit.
#[must_use]
pub fn channel_ref(id: &str, unit: &str) -> ChannelRef {
    ChannelRef {
        id: id.to_owned(),
        unit: unit.to_owned(),
    }
}

/// Whether the model is a single-input, single-output model that the linear-model conversion turns into a transfer function.
#[must_use]
pub fn converts_to_transfer(model: &GeneralModel) -> bool {
    serde_json::to_value(model).is_ok_and(|value| linear_transfer_model(&value).is_ok())
}

/// The failure of a tool that does not apply to the model or the loop (A11): the next action is to choose another method.
#[must_use]
pub fn not_applicable(tool: AnalysisTool) -> Failure {
    Failure::new(
        &format!(
            "{} does not apply to this model or loop. Choose another analysis.",
            presentation(tool).label
        ),
        Cause::ModelOrData,
        NextAction::ChooseMethod,
    )
}

/// The failure of a calculation that needs a model, when none is at hand (A11): the next action is to identify the plant.
#[must_use]
pub fn no_model() -> Failure {
    Failure::new(
        "There is no model to analyse. Identify the plant first, or supply a model.",
        Cause::ModelOrData,
        NextAction::Identify,
    )
}

/// The failure of an input outside its rule (A7): it names the input, and the next action is none, because the engineer corrects
/// the value in the page.
#[must_use]
pub fn input_failure(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::None)
}

/// A number as the sentences of an analysis write it: four decimals, trailing zeros removed, or a power of ten for very small and
/// very large values.
#[must_use]
pub fn number(value: f64) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let magnitude = value.abs();
    if magnitude > 0.0 && !(1e-3..1e6).contains(&magnitude) {
        return format!("{value:.3e}");
    }
    let fixed = format!("{value:.4}");
    let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
    if trimmed == "-0" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
