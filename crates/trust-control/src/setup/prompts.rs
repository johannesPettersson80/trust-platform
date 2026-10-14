//! The step-by-step loop setup (specification 35, B1.3, C12 and C13): the questions that `configureLoop.ts` asks for a
//! program the form does not recognise, the choices each one offers, the rule that accepts each answer, and the typed
//! answers the loop is built from. The `QuickPick`, the input box and the modal confirmation stay in TypeScript
//! (`nativeSetupPrompts.ts`); this module decides what is asked next and what is accepted. `setupPromptTypes.ts` supplies
//! the phases.

use crate::setup::catalog::{
    CatalogAnswer, CompiledController, CompiledSignal, CompiledSymbol, CompiledTask,
};
use crate::setup::texts::prompts as text;

/// The four phases of the step-by-step setup (`SETUP_PHASE_NAMES`, `setupPromptTypes.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupPhase {
    /// Phase 1: the loop, its controller and its signals.
    LoopAndSignals,
    /// Phase 2: the units and bounds of the process and the output.
    UnitsAndBounds,
    /// Phase 3: the controller's law and the experiment.
    ControllerAndExperiment,
    /// Phase 4: the channels and the actuator.
    ChannelsAndActuators,
}

impl SetupPhase {
    /// The phase's number, 1 to 4, as the presenter shows it.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::LoopAndSignals => 1,
            Self::UnitsAndBounds => 2,
            Self::ControllerAndExperiment => 3,
            Self::ChannelsAndActuators => 4,
        }
    }

    /// The phase's name (`SETUP_PHASE_NAMES`).
    #[must_use]
    pub fn name(self) -> &'static str {
        text::phase_name(self.number())
    }
}

/// The control workflow the engineer picks (`selectWorkflow`, `configureLoop.ts`; B1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupWorkflow {
    /// Record the existing program without declaring its control law (C12, project-scripted).
    Observation,
    /// Import a typed controller adapter mapping (C0, C1, C2).
    Adapter,
    /// Tune an existing ON/OFF controller (C3). Parked for checklist V7.
    OnOff,
    /// Tune an existing proportional controller (C13).
    Proportional,
}

/// The workflows offered: observation always, and the other three only when the loop has a setpoint (`selectWorkflow`).
#[must_use]
pub fn workflows_offered(has_setpoint: bool) -> Vec<SetupWorkflow> {
    let mut offered = vec![SetupWorkflow::Observation];
    if has_setpoint {
        offered.extend([
            SetupWorkflow::Adapter,
            SetupWorkflow::OnOff,
            SetupWorkflow::Proportional,
        ]);
    }
    offered
}

/// The rule a number answer must pass (`number`, `configureLoop.ts`; `configureLoopChannels.ts`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NumberRule {
    /// Any finite number.
    Finite,
    /// Above zero (the task period).
    Positive,
    /// Above a bound, strictly (the process and output maximums are above their minimums).
    AboveBound(f64),
    /// At or above a bound (the gain maximum is at or above the gain minimum).
    AtLeast(f64),
    /// Within both bounds, inclusive.
    Within {
        /// The smallest value accepted.
        minimum: f64,
        /// The largest value accepted.
        maximum: f64,
    },
}

/// The observation duration of C12, in seconds: 1 s to 24 h (A7, `limits.rs`).
pub const DURATION_RULE: NumberRule = NumberRule::Within {
    minimum: 1.0,
    maximum: 86_400.0,
};

impl NumberRule {
    /// Whether `value` passes the rule. A value that is not finite never does.
    #[must_use]
    pub fn accepts(self, value: f64) -> bool {
        value.is_finite()
            && match self {
                Self::Finite => true,
                Self::Positive => value > 0.0,
                Self::AboveBound(bound) => value > bound,
                Self::AtLeast(bound) => value >= bound,
                Self::Within { minimum, maximum } => value >= minimum && value <= maximum,
            }
    }
}

/// Whether a character is whitespace to JavaScript (`String.prototype.trim` and `Number()`): the white space and the
/// line terminators of ECMAScript, which differ from Rust's `char::is_whitespace`.
const fn is_js_space(character: char) -> bool {
    matches!(
        character,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// The text without its leading and trailing JavaScript whitespace (`String.prototype.trim`).
#[must_use]
pub fn js_trim(text: &str) -> &str {
    text.trim_matches(is_js_space)
}

/// The value of `Number(text)` in JavaScript when it is finite, and `None` when it is not a number or is infinite. Decimal
/// text parses as Rust parses it; `0x`, `0o` and `0b` literals are read as JavaScript reads them, up to 128 bits.
#[must_use]
pub fn js_number(text: &str) -> Option<f64> {
    let trimmed = js_trim(text);
    let value = match trimmed.get(..2) {
        Some("0x" | "0X") => radix_value(&trimmed[2..], 16),
        Some("0o" | "0O") => radix_value(&trimmed[2..], 8),
        Some("0b" | "0B") => radix_value(&trimmed[2..], 2),
        _ => trimmed.parse::<f64>().ok(),
    };
    value.filter(|value| value.is_finite())
}

/// A non-decimal literal's digits as a number; `None` when a digit is not in the radix, or the value needs more than
/// 128 bits.
#[allow(clippy::cast_precision_loss)] // A u128 rounds to the nearest f64, as JavaScript's conversion of an integer does.
fn radix_value(digits: &str, radix: u32) -> Option<f64> {
    if digits.is_empty() || !digits.chars().all(|digit| digit.is_digit(radix)) {
        return None;
    }
    u128::from_str_radix(digits, radix)
        .ok()
        .map(|value| value as f64)
}

/// A text answer, trimmed; a blank answer is refused (`input`, `configureLoop.ts`).
///
/// # Errors
/// The answer is blank once trimmed.
pub fn accept_text(answer: &str) -> Result<String, String> {
    let value = js_trim(answer);
    if value.is_empty() {
        return Err(text::enter_value().to_owned());
    }
    Ok(value.to_owned())
}

/// A number answer that passes its rule (`number`, `configureLoop.ts`).
///
/// # Errors
/// The answer is not a finite number, or does not pass the rule.
pub fn accept_number(answer: &str, rule: NumberRule) -> Result<f64, String> {
    js_number(answer)
        .filter(|value| rule.accepts(*value))
        .ok_or_else(|| text::invalid_number().to_owned())
}

/// One choice a question offers. `index` is its position in the list that [`next_step`] returns, which is the answer the
/// presenter sends back; `description` is the second line of the item when it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceItem {
    /// The item's position in the offered list.
    pub index: usize,
    /// The label the engineer reads.
    pub label: String,
    /// The second line, when the item has one.
    pub description: Option<String>,
}

/// The question a step asks.
#[derive(Debug, Clone, PartialEq)]
pub enum StepKind {
    /// A text answer that is not blank once trimmed.
    Text,
    /// A number answer that passes the rule.
    Number(NumberRule),
    /// A rate that may be left blank (no limit) and is otherwise a nonnegative finite number.
    OptionalRate,
    /// One of the items offered.
    Choice(Vec<ChoiceItem>),
    /// A controller adapter mapping file, read by the engine and checked by [`crate::setup::configure_adapter`].
    File,
}

/// Which question a step is. The order of the variants is not the order of the questions; [`next_step`] decides that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepId {
    /// The loop's name.
    LoopName,
    /// The compiled controller, or a custom one.
    Controller,
    /// A custom controller's instance.
    ControllerInstance,
    /// A custom controller's type.
    ControllerType,
    /// The task period, when the controller has no single periodic task.
    TaskPeriod,
    /// The process variable.
    ProcessVariable,
    /// The controller output.
    ControllerOutput,
    /// The setpoint, or an open loop.
    Setpoint,
    /// The control workflow.
    Workflow,
    /// The process unit.
    ProcessUnit,
    /// The process minimum.
    ProcessMinimum,
    /// The process maximum.
    ProcessMaximum,
    /// The output unit.
    OutputUnit,
    /// The output minimum.
    OutputMinimum,
    /// The output maximum.
    OutputMaximum,
    /// The proportional gain's symbol.
    ProportionalGain,
    /// The gain's minimum.
    GainMinimum,
    /// The gain's maximum.
    GainMaximum,
    /// The manual command's symbol.
    ManualCommand,
    /// The manual mode's symbol.
    ManualMode,
    /// The TRUE value of the manual mode.
    ManualModePolarity,
    /// The controller task of a proportional setup, when the controller has no single task.
    ControllerTask,
    /// The observation duration (C12).
    ObservationDuration,
    /// The controller adapter mapping file.
    AdapterFile,
    /// Single or multiple channels.
    ChannelMode,
    /// The actuator's kind.
    ActuatorCapability,
    /// The rising rate limit.
    RisingRate,
    /// The falling rate limit.
    FallingRate,
    /// The applied-actuator feedback.
    AppliedFeedback,
}

/// One question of the step-by-step setup, as the presenter shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct SetupStep {
    /// Which question this is.
    pub step: StepId,
    /// The phase the question belongs to.
    pub phase: SetupPhase,
    /// The question's title.
    pub title: String,
    /// What the question accepts.
    pub kind: StepKind,
}

/// The controller the engineer picked: a controller the catalog compiled, or a custom identity typed in.
#[derive(Debug, Clone, PartialEq)]
pub enum ControllerChoice {
    /// A controller of the compiled catalog, with its tasks.
    Compiled(CompiledController),
    /// A custom controller; its instance and type are typed in.
    Custom,
}

/// The setpoint the engineer picked (`configureLoop.ts`, the setpoint pick).
#[derive(Debug, Clone, PartialEq)]
pub enum SetpointChoice {
    /// The loop has no reference: an open loop (C12).
    OpenLoop,
    /// The setpoint is this signal.
    Signal(CompiledSignal),
}

/// The applied-actuator feedback the engineer picked (`configureLoopChannels.ts`).
#[derive(Debug, Clone, PartialEq)]
pub enum FeedbackChoice {
    /// No separate feedback: the controller output is the applied action.
    UseOutput,
    /// This signal reports the applied action.
    Signal(CompiledSignal),
}

/// Single or multiple channels (`configureLoopChannels.ts`). Multiple channels are parked for checklist V7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    /// The selected PV, MV and setpoint.
    Single,
    /// Several channels: refused until the channel model is ported.
    Multiple,
}

/// The kind of actuator (`actuatorSetup`). The ON/OFF kind is parked for checklist V7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActuatorChoice {
    /// A continuous output, with optional rate limits.
    Continuous,
    /// An ON/OFF output, with dwell times. Parked for checklist V7.
    OnOff,
}

/// One rate limit answer: blank declares no limit, otherwise the rate per second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RateAnswer {
    /// No limit (the answer was blank).
    Unlimited,
    /// The largest change per second, in output units.
    PerSecond(f64),
}

/// The answers of the step-by-step setup, in the order the questions are asked. A field is `None` until its question is
/// answered; [`next_step`] returns the first question whose answer is missing, so a field is only read when the questions
/// before it are answered.
///
/// `custom_instance` and `custom_type` are asked only when the controller is custom or the catalog has no compiled
/// controller. The task period is asked only when the controller has no single periodic task. The proportional fields are
/// asked only for [`SetupWorkflow::Proportional`]; the observation duration only for [`SetupWorkflow::Observation`]; the
/// mapping file only for [`SetupWorkflow::Adapter`]. The channel fields are asked in phase 4 for every workflow that is
/// not parked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SetupStepAnswers {
    /// The loop's name, as typed (trimmed by [`accept_text`]).
    pub name: Option<String>,
    /// The compiled controller picked, or [`ControllerChoice::Custom`].
    pub controller: Option<ControllerChoice>,
    /// A custom controller's instance.
    pub custom_instance: Option<String>,
    /// A custom controller's type.
    pub custom_type: Option<String>,
    /// The task period in ms, when the controller has no single periodic task.
    pub task_period_ms: Option<f64>,
    /// The process variable's signal.
    pub process_variable: Option<CompiledSignal>,
    /// The controller output's signal.
    pub controller_output: Option<CompiledSignal>,
    /// The setpoint, or an open loop.
    pub setpoint: Option<SetpointChoice>,
    /// The workflow picked.
    pub workflow: Option<SetupWorkflow>,
    /// The process unit.
    pub process_unit: Option<String>,
    /// The process minimum.
    pub process_minimum: Option<f64>,
    /// The process maximum, above the minimum.
    pub process_maximum: Option<f64>,
    /// The output unit.
    pub output_unit: Option<String>,
    /// The output minimum.
    pub output_minimum: Option<f64>,
    /// The output maximum, above the minimum.
    pub output_maximum: Option<f64>,
    /// The proportional gain's REAL or LREAL symbol.
    pub gain: Option<CompiledSymbol>,
    /// The gain's minimum.
    pub gain_minimum: Option<f64>,
    /// The gain's maximum, at or above the minimum.
    pub gain_maximum: Option<f64>,
    /// The manual command's REAL or LREAL symbol.
    pub manual_command: Option<CompiledSymbol>,
    /// The manual mode's BOOL symbol.
    pub manual_mode: Option<CompiledSymbol>,
    /// Whether the mode is Manual when TRUE.
    pub manual_when_true: Option<bool>,
    /// The controller task of a proportional setup, when the controller has no single task.
    pub controller_task: Option<String>,
    /// The observation duration in seconds (C12).
    pub observation_duration_seconds: Option<f64>,
    /// The bytes of the adapter mapping file, as the engine read them (at most 256 001).
    pub adapter_mapping: Option<Vec<u8>>,
    /// Single or multiple channels.
    pub channel_mode: Option<ChannelMode>,
    /// The actuator's kind.
    pub actuator: Option<ActuatorChoice>,
    /// The rising rate limit.
    pub rising_rate: Option<RateAnswer>,
    /// The falling rate limit.
    pub falling_rate: Option<RateAnswer>,
    /// The applied-actuator feedback.
    pub feedback: Option<FeedbackChoice>,
}

/// The controller's only periodic task, when it has exactly one with a positive interval (`configureLoop.ts`, `task`).
#[must_use]
pub fn sole_task(controller: Option<&ControllerChoice>) -> Option<&CompiledTask> {
    match controller? {
        ControllerChoice::Compiled(compiled) => match compiled.tasks.as_slice() {
            [task] if task.interval_ms > 0.0 => Some(task),
            _ => None,
        },
        ControllerChoice::Custom => None,
    }
}

/// The picker's description of a compiled controller: its type, unless the type is the instance's own name
/// (`configureLoop.ts`).
#[must_use]
pub fn controller_item_description(instance: &str, type_name: &str) -> Option<String> {
    (instance != type_name).then(|| type_name.to_owned())
}

/// The first question that is still unanswered, or `None` when the setup has every answer it needs. A catalog without a
/// compiled process-image signal asks nothing: the build refuses it (`configureLoop.ts`).
#[must_use]
pub fn next_step(catalog: &CatalogAnswer, answers: &SetupStepAnswers) -> Option<SetupStep> {
    if catalog.signals.is_empty() {
        return None;
    }
    loop_step(catalog, answers)
        .or_else(|| signal_step(catalog, answers))
        .or_else(|| bounds_step(answers))
        .or_else(|| experiment_step(catalog, answers))
        .or_else(|| channel_step(catalog, answers))
}

#[allow(clippy::unnecessary_wraps)] // Each caller answers an optional next step; this keeps every call site one expression.
fn step(id: StepId, phase: SetupPhase, title: &str, kind: StepKind) -> Option<SetupStep> {
    Some(SetupStep {
        step: id,
        phase,
        title: title.to_owned(),
        kind,
    })
}

fn choice(id: StepId, phase: SetupPhase, title: &str, items: Vec<ChoiceItem>) -> Option<SetupStep> {
    step(id, phase, title, StepKind::Choice(items))
}

fn is_compiled(answers: &SetupStepAnswers) -> bool {
    matches!(answers.controller, Some(ControllerChoice::Compiled(_)))
}

fn loop_step(catalog: &CatalogAnswer, answers: &SetupStepAnswers) -> Option<SetupStep> {
    let phase = SetupPhase::LoopAndSignals;
    if answers.name.is_none() {
        return step(StepId::LoopName, phase, text::loop_name(), StepKind::Text);
    }
    let controllers = &catalog.control.controllers;
    if answers.controller.is_none() && !controllers.is_empty() {
        let mut items: Vec<ChoiceItem> = controllers
            .iter()
            .enumerate()
            .map(|(index, controller)| ChoiceItem {
                index,
                label: controller.instance.clone(),
                description: controller_item_description(
                    &controller.instance,
                    &controller.type_name,
                ),
            })
            .collect();
        items.push(ChoiceItem {
            index: controllers.len(),
            label: text::custom_controller().to_owned(),
            description: Some(text::custom_controller_description().to_owned()),
        });
        return choice(StepId::Controller, phase, text::controller(), items);
    }
    if !is_compiled(answers) {
        if answers.custom_instance.is_none() {
            return step(
                StepId::ControllerInstance,
                phase,
                text::controller_instance(),
                StepKind::Text,
            );
        }
        if answers.custom_type.is_none() {
            return step(
                StepId::ControllerType,
                phase,
                text::controller_type(),
                StepKind::Text,
            );
        }
    }
    if sole_task(answers.controller.as_ref()).is_none() && answers.task_period_ms.is_none() {
        return step(
            StepId::TaskPeriod,
            phase,
            text::task_period(),
            StepKind::Number(NumberRule::Positive),
        );
    }
    None
}

/// The list of signals as items, numbered from `first`.
fn signal_items<'a>(
    signals: impl Iterator<Item = &'a CompiledSignal>,
    first: usize,
) -> Vec<ChoiceItem> {
    signals
        .enumerate()
        .map(|(offset, signal)| ChoiceItem {
            index: first + offset,
            label: signal.symbol.clone(),
            description: Some(format!("{} · {}", signal.address, signal.iec_type)),
        })
        .collect()
}

/// The list of symbols as items, numbered from `first`, each with its type.
fn symbol_items<'a>(
    symbols: impl Iterator<Item = &'a CompiledSymbol>,
    first: usize,
) -> Vec<ChoiceItem> {
    symbols
        .enumerate()
        .map(|(offset, symbol)| ChoiceItem {
            index: first + offset,
            label: symbol.symbol.clone(),
            description: Some(symbol.iec_type.clone()),
        })
        .collect()
}

fn signal_step(catalog: &CatalogAnswer, answers: &SetupStepAnswers) -> Option<SetupStep> {
    let phase = SetupPhase::LoopAndSignals;
    let signals = &catalog.signals;
    let Some(process) = answers.process_variable.as_ref() else {
        return choice(
            StepId::ProcessVariable,
            phase,
            text::process_variable(),
            signal_items(signals.iter(), 0),
        );
    };
    let Some(output) = answers.controller_output.as_ref() else {
        let candidates = signals
            .iter()
            .filter(|signal| signal.address != process.address);
        return choice(
            StepId::ControllerOutput,
            phase,
            text::controller_output(),
            signal_items(candidates, 0),
        );
    };
    let Some(setpoint) = answers.setpoint.as_ref() else {
        let mut items = vec![ChoiceItem {
            index: 0,
            label: text::no_setpoint().to_owned(),
            description: None,
        }];
        let candidates = signals
            .iter()
            .filter(|signal| signal.address != process.address && signal.address != output.address);
        items.extend(signal_items(candidates, 1));
        return choice(StepId::Setpoint, phase, text::setpoint(), items);
    };
    if answers.workflow.is_none() {
        let has_setpoint = matches!(setpoint, SetpointChoice::Signal(_));
        let items = workflows_offered(has_setpoint)
            .into_iter()
            .enumerate()
            .map(|(index, workflow)| workflow_item(index, workflow))
            .collect();
        return choice(StepId::Workflow, phase, text::control_workflow(), items);
    }
    None
}

fn workflow_item(index: usize, workflow: SetupWorkflow) -> ChoiceItem {
    let (label, description) = match workflow {
        SetupWorkflow::Observation => (text::observation(), text::observation_description()),
        SetupWorkflow::Adapter => (
            text::adapter_workflow(),
            text::adapter_workflow_description(),
        ),
        SetupWorkflow::OnOff => (text::on_off_workflow(), text::on_off_workflow_description()),
        SetupWorkflow::Proportional => (
            text::proportional_workflow(),
            text::proportional_workflow_description(),
        ),
    };
    ChoiceItem {
        index,
        label: label.to_owned(),
        description: Some(description.to_owned()),
    }
}

fn bounds_step(answers: &SetupStepAnswers) -> Option<SetupStep> {
    let phase = SetupPhase::UnitsAndBounds;
    if answers.process_unit.is_none() {
        return step(
            StepId::ProcessUnit,
            phase,
            text::process_unit(),
            StepKind::Text,
        );
    }
    let Some(process_minimum) = answers.process_minimum else {
        return step(
            StepId::ProcessMinimum,
            phase,
            text::process_minimum(),
            StepKind::Number(NumberRule::Finite),
        );
    };
    if answers.process_maximum.is_none() {
        return step(
            StepId::ProcessMaximum,
            phase,
            text::process_maximum(),
            StepKind::Number(NumberRule::AboveBound(process_minimum)),
        );
    }
    if answers.output_unit.is_none() {
        return step(
            StepId::OutputUnit,
            phase,
            text::output_unit(),
            StepKind::Text,
        );
    }
    let Some(output_minimum) = answers.output_minimum else {
        return step(
            StepId::OutputMinimum,
            phase,
            text::output_minimum(),
            StepKind::Number(NumberRule::Finite),
        );
    };
    if answers.output_maximum.is_none() {
        return step(
            StepId::OutputMaximum,
            phase,
            text::output_maximum(),
            StepKind::Number(NumberRule::AboveBound(output_minimum)),
        );
    }
    None
}

fn experiment_step(catalog: &CatalogAnswer, answers: &SetupStepAnswers) -> Option<SetupStep> {
    let phase = SetupPhase::ControllerAndExperiment;
    match answers.workflow? {
        SetupWorkflow::Proportional => proportional_step(catalog, answers),
        SetupWorkflow::Observation if answers.observation_duration_seconds.is_none() => step(
            StepId::ObservationDuration,
            phase,
            text::observation_duration(),
            StepKind::Number(DURATION_RULE),
        ),
        SetupWorkflow::Adapter if answers.adapter_mapping.is_none() => step(
            StepId::AdapterFile,
            phase,
            text::adapter_mapping(),
            StepKind::File,
        ),
        _ => None,
    }
}

fn proportional_step(catalog: &CatalogAnswer, answers: &SetupStepAnswers) -> Option<SetupStep> {
    let phase = SetupPhase::ControllerAndExperiment;
    let symbols = &catalog.control.symbols;
    let numeric = || {
        symbols
            .iter()
            .filter(|symbol| matches!(symbol.iec_type.as_str(), "REAL" | "LREAL"))
    };
    if answers.gain.is_none() {
        return choice(
            StepId::ProportionalGain,
            phase,
            text::proportional_gain(),
            symbol_items(numeric(), 0),
        );
    }
    let Some(gain_minimum) = answers.gain_minimum else {
        return step(
            StepId::GainMinimum,
            phase,
            text::gain_minimum(),
            StepKind::Number(NumberRule::Finite),
        );
    };
    if answers.gain_maximum.is_none() {
        return step(
            StepId::GainMaximum,
            phase,
            text::gain_maximum(),
            StepKind::Number(NumberRule::AtLeast(gain_minimum)),
        );
    }
    if answers.manual_command.is_none() {
        return choice(
            StepId::ManualCommand,
            phase,
            text::manual_command(),
            symbol_items(numeric(), 0),
        );
    }
    if answers.manual_mode.is_none() {
        let booleans = symbols.iter().filter(|symbol| symbol.iec_type == "BOOL");
        return choice(
            StepId::ManualMode,
            phase,
            text::manual_mode(),
            symbol_items(booleans, 0),
        );
    }
    if answers.manual_when_true.is_none() {
        let items = vec![
            ChoiceItem {
                index: 0,
                label: text::true_means_manual().to_owned(),
                description: None,
            },
            ChoiceItem {
                index: 1,
                label: text::true_means_auto().to_owned(),
                description: None,
            },
        ];
        return choice(
            StepId::ManualModePolarity,
            phase,
            text::manual_mode_polarity(),
            items,
        );
    }
    if sole_task(answers.controller.as_ref()).is_none() && answers.controller_task.is_none() {
        return step(
            StepId::ControllerTask,
            phase,
            text::controller_task(),
            StepKind::Text,
        );
    }
    None
}

fn channel_step(catalog: &CatalogAnswer, answers: &SetupStepAnswers) -> Option<SetupStep> {
    let phase = SetupPhase::ChannelsAndActuators;
    if answers.workflow? == SetupWorkflow::OnOff {
        return None;
    }
    let Some(mode) = answers.channel_mode else {
        let items = vec![
            ChoiceItem {
                index: 0,
                label: text::single_channel().to_owned(),
                description: Some(text::single_channel_description().to_owned()),
            },
            ChoiceItem {
                index: 1,
                label: text::multiple_channels().to_owned(),
                description: Some(text::multiple_channels_description().to_owned()),
            },
        ];
        return choice(StepId::ChannelMode, phase, text::control_channels(), items);
    };
    if mode == ChannelMode::Multiple {
        return None;
    }
    // The output of a BOOL is an ON/OFF actuator, which the build refuses (parked for V7).
    if answers.controller_output.as_ref()?.iec_type == "BOOL" {
        return None;
    }
    let Some(actuator) = answers.actuator else {
        let items = vec![
            ChoiceItem {
                index: 0,
                label: text::continuous_output().to_owned(),
                description: None,
            },
            ChoiceItem {
                index: 1,
                label: text::on_off_output().to_owned(),
                description: None,
            },
        ];
        return choice(
            StepId::ActuatorCapability,
            phase,
            text::actuator_capability(),
            items,
        );
    };
    if actuator == ActuatorChoice::OnOff {
        return None;
    }
    if answers.rising_rate.is_none() {
        return step(
            StepId::RisingRate,
            phase,
            text::rising_rate(),
            StepKind::OptionalRate,
        );
    }
    if answers.falling_rate.is_none() {
        return step(
            StepId::FallingRate,
            phase,
            text::falling_rate(),
            StepKind::OptionalRate,
        );
    }
    if answers.feedback.is_none() {
        return feedback_step(catalog, answers);
    }
    None
}

/// The symbols the loop already uses for its process variable, its output and its setpoint: an applied-action feedback
/// cannot be one of them.
#[must_use]
pub fn claimed_symbols(answers: &SetupStepAnswers) -> Vec<&str> {
    let mut claimed = Vec::new();
    if let Some(process) = &answers.process_variable {
        claimed.push(process.symbol.as_str());
    }
    if let Some(output) = &answers.controller_output {
        claimed.push(output.symbol.as_str());
    }
    if let Some(SetpointChoice::Signal(setpoint)) = &answers.setpoint {
        claimed.push(setpoint.symbol.as_str());
    }
    claimed
}

/// Whether a signal may report the applied action: an input (`%I`) of type REAL, LREAL or BOOL that the loop does not
/// already use (`configureLoopChannels.ts`, the applied-actuator feedback list).
#[must_use]
pub fn is_applied_feedback_candidate(signal: &CompiledSignal, claimed: &[&str]) -> bool {
    signal.address.starts_with("%I")
        && !claimed.contains(&signal.symbol.as_str())
        && matches!(signal.iec_type.as_str(), "REAL" | "LREAL" | "BOOL")
}

fn feedback_step(catalog: &CatalogAnswer, answers: &SetupStepAnswers) -> Option<SetupStep> {
    let phase = SetupPhase::ChannelsAndActuators;
    let output = answers.controller_output.as_ref()?;
    let claimed = claimed_symbols(answers);
    let unit = answers.output_unit.as_deref().unwrap_or_default();
    let mut items = vec![ChoiceItem {
        index: 0,
        label: text::use_controller_output().to_owned(),
        description: Some(text::use_controller_output_description().to_owned()),
    }];
    let candidates = catalog
        .signals
        .iter()
        .filter(|signal| is_applied_feedback_candidate(signal, &claimed));
    for (offset, signal) in candidates.enumerate() {
        items.push(ChoiceItem {
            index: 1 + offset,
            label: signal.symbol.clone(),
            description: Some(format!("{} · applied action in {unit}", signal.iec_type)),
        });
    }
    choice(
        StepId::AppliedFeedback,
        phase,
        &text::applied_feedback(&output.symbol),
        items,
    )
}

/// What the engineer did at a question (`SetupPromptResult`, `setupPromptTypes.ts`): an accepted answer, Back to the question
/// before, or Cancel of the whole setup, which writes nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum StepResult<T> {
    /// The answer, accepted.
    Accepted(T),
    /// Back: the question before, whose answer may change.
    Back,
    /// Cancel: the setup stops and writes nothing.
    Cancelled,
}

/// The index of the question Back returns to: the nearest earlier answer that the engineer may still change
/// (`setupNavigation.ts`, `canGoBack`). `None` when there is none, so Back is not offered at the first question.
#[must_use]
pub fn back_target(editable: &[bool], index: usize) -> Option<usize> {
    editable[..index.min(editable.len())]
        .iter()
        .rposition(|editable| *editable)
}

/// Whether an accepted answer that differs from the previous one discards the answers after it, which may depend on it
/// (`setupNavigation.ts`). An unchanged answer, or the first answer, keeps them.
#[must_use]
pub fn changed_answer_discards_later<T: PartialEq>(previous: Option<&T>, accepted: &T) -> bool {
    previous.is_some_and(|previous| previous != accepted)
}

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;
