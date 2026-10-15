//! Which identification method a run uses, the size and direction of its excitation, and whether a plant input is an ON/OFF
//! actuator (`identificationPolicy.ts`, specification 35, B3.3 and B5.2). The live sample is the runtime's scan of the
//! loop (`ExperimentSample`), so the rules run in Rust with the runtime's values; the panel's monitor is presentation.
#![allow(clippy::module_name_repetitions)] // The policy names carry the module's subject.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::experiment::plan::{ControlExperimentKind, ControllerMode, ExperimentSample};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::campaign::plant_input_range;
use crate::identification::texts::policy_error_text;
use crate::loop_definition::{Execution, Experiment, ExperimentKind, LoopDefinition, Topology};

/// What a plant reveals in an identification: a basic response, its frequency content, or several modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PlantInformation {
    /// The step response: enough for a first-order or a second-order model.
    Basic,
    /// The frequency response: a binary or multisine excitation is wanted.
    Frequency,
    /// Several modes: the identification fits more than one dynamic.
    MultipleModes,
}

/// A process the policy declares, so an identification may skip the integrating check (B1.4, N2.3a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PolicyProcess {
    /// The process integrates its input.
    Integrating,
}

/// How Identify may treat a plant (`IdentificationPolicy`): the error limit, the highest model order, whether open-loop
/// and sustained tests are permitted, and what the plant reveals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentificationPolicy {
    /// The largest normalised error a model may have (N3.2), above zero and below one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_normalized_error: Option<f64>,
    /// The highest order of a state model, from one to eight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_order: Option<u8>,
    /// Whether a test in Manual, with the loop open, is permitted.
    pub open_loop_permitted: bool,
    /// Whether a plant input may be held as a step; when not, a pulse is used.
    pub sustained_hold_permitted: bool,
    /// What the plant reveals.
    pub information: PlantInformation,
    /// The process the policy declares, when it integrates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<PolicyProcess>,
}

/// Decodes a stored identification policy and returns it only when its limits hold: a maximum normalised error above
/// zero and below one, and a maximum order from one to eight.
#[must_use]
pub fn decode_identification_policy(value: &Value) -> Option<IdentificationPolicy> {
    let policy: IdentificationPolicy = crate::json::decode_value(value.clone()).ok()?;
    let error_ok = policy
        .maximum_normalized_error
        .is_none_or(|limit| limit.is_finite() && limit > 0.0 && limit < 1.0);
    let order_ok = policy
        .maximum_order
        .is_none_or(|order| (1..=8).contains(&order));
    (error_ok && order_ok).then_some(policy)
}

/// The policy a loop declares. A stored policy that does not decode is treated as absent.
fn policy_of(loop_definition: &LoopDefinition) -> Option<IdentificationPolicy> {
    loop_definition
        .identification_policy
        .as_ref()
        .and_then(decode_identification_policy)
}

/// The experiment a project scripts (`execution: project-scripted`, C12), or `None` for one Control Studio runs. A
/// project-scripted loop needs no mode change and no mode for its method (`needs_mode_change`).
#[must_use]
pub fn project_scripted(loop_definition: &LoopDefinition) -> Option<&Experiment> {
    loop_definition
        .experiment
        .as_ref()
        .filter(|experiment| experiment.execution == Execution::ProjectScripted)
}

/// The method one identification runs (`IdentificationMethod`): its kind, whether it writes the plant input in Manual
/// (A6.2), and whether Identify chose it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentificationMethod {
    /// The experiment kind the runtime runs.
    pub kind: ControlExperimentKind,
    /// The experiment writes the plant input in Manual, so the loop changes to Manual and back (A6.2).
    pub manual: bool,
    /// Identify chose the method from the loop's mode and policy, not the engineer.
    pub automatic: bool,
}

/// Why an identification cannot choose or size its method (`identificationPolicy.ts` and `identificationAmplitude`).
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyError {
    /// No fresh synchronized sample of the setpoint, measured output and manipulated variable is available.
    SampleNotAvailable,
    /// Automatic cannot choose a method while the loop's Auto or Manual mode is unavailable.
    ModeUnavailable,
    /// A loop in Manual has no safe automatic method: open-loop tests are not permitted or have no Manual command.
    NoSafeMethodInManual,
    /// A loop without a setpoint has no safe automatic method: open-loop tests are not permitted or have no Manual command.
    NoSafeMethodWithoutSetpoint,
    /// The engineer pinned a load step, which needs a declared disturbance channel.
    PinnedLoadStep,
    /// The engineer pinned an open-loop method, which the policy does not permit.
    PinnedClosedLoopOnly,
    /// The engineer pinned a sustained step, which the policy does not permit.
    PinnedHoldRestriction,
    /// The engineer asked for an excitation of a project-scripted loop, whose project owns the excitation: Control Studio
    /// would write a duplicate one (C12).
    DuplicateExcitation,
    /// The plant input's limits, which an automatic plant-input step needs, are missing or not ordered. The failure is the
    /// campaign's own, which names the missing field and opens Limits there (B3.4 step 1, B3.9).
    PlantInputLimits(Failure),
    /// An automatic step has no room to move the signal from its current value.
    NoHeadroom {
        /// The symbol of the signal the step would move.
        symbol: String,
        /// The current value of the signal.
        value: f64,
        /// The unit of the current value.
        unit: String,
    },
}

impl PolicyError {
    /// The failure the engineer sees: its sentence, its cause, and the one next action it offers.
    #[must_use]
    pub fn failure(&self) -> Failure {
        let (cause, next_action) = match self {
            Self::PlantInputLimits(failure) => return failure.clone(),
            Self::SampleNotAvailable => (Cause::Runtime, NextAction::Wait),
            Self::ModeUnavailable => (Cause::Runtime, NextAction::ConnectRuntime),
            Self::NoSafeMethodInManual
            | Self::PinnedLoadStep
            | Self::PinnedClosedLoopOnly
            | Self::PinnedHoldRestriction
            | Self::DuplicateExcitation => (Cause::Settings, NextAction::ChooseMethod),
            Self::NoSafeMethodWithoutSetpoint => (Cause::Implementation, NextAction::OpenLoopSetup),
            Self::NoHeadroom { .. } => (Cause::Settings, NextAction::OpenLimits),
        };
        Failure::new(&policy_error_text(self), cause, next_action)
    }
}

/// The experiment kind the runtime runs for a loop's declared kind.
fn control_kind(kind: ExperimentKind) -> ControlExperimentKind {
    match kind {
        ExperimentKind::Observation => ControlExperimentKind::Observation,
        ExperimentKind::SetpointStep => ControlExperimentKind::SetpointStep,
        ExperimentKind::LoadStep => ControlExperimentKind::LoadStep,
        ExperimentKind::MvStep => ControlExperimentKind::MvStep,
        ExperimentKind::MvPulse => ControlExperimentKind::MvPulse,
        ExperimentKind::Relay => ControlExperimentKind::Relay,
        ExperimentKind::Prbs => ControlExperimentKind::Prbs,
        ExperimentKind::Multisine => ControlExperimentKind::Multisine,
    }
}

/// The method one identification runs (`identificationMethod`). An explicit method the loop declares is kept as declared,
/// after its checks against the policy. Otherwise Automatic decides from the controller's mode in the live sample, or
/// from the loop being open.
///
/// # Errors
/// Returns the [`PolicyError`] of the explicit method's checks, or of the automatic choice.
pub fn identification_method(
    loop_definition: &LoopDefinition,
    sample: Option<&ExperimentSample>,
) -> Result<IdentificationMethod, PolicyError> {
    let requested = loop_definition
        .experiment
        .as_ref()
        .map(|experiment| experiment.kind)
        .filter(|kind| *kind != ExperimentKind::Observation);
    if let Some(requested) = requested {
        validate_explicit_method(loop_definition, requested)?;
        let kind = control_kind(requested);
        return Ok(IdentificationMethod {
            kind,
            manual: kind != ControlExperimentKind::SetpointStep,
            automatic: false,
        });
    }
    let mode = sample.map_or(ControllerMode::Unavailable, |sample| sample.controller_mode);
    automatic_method(loop_definition, mode)
}

/// The method Automatic chooses for a mode (B3.3): a setpoint step in Auto for a loop with a setpoint; otherwise a
/// plant-input step in Manual, a pulse when sustained holds are not permitted, and PRBS when the policy asks for
/// frequency information. A project-scripted loop takes the project's scenario whatever the mode (C12).
///
/// # Errors
/// Returns [`PolicyError::ModeUnavailable`] for an unknown mode, and the no-safe-method errors when open-loop tests are
/// not possible. A project-scripted loop never fails here.
pub fn automatic_method(
    loop_definition: &LoopDefinition,
    mode: ControllerMode,
) -> Result<IdentificationMethod, PolicyError> {
    if let Some(experiment) = project_scripted(loop_definition) {
        return Ok(project_scenario_method(experiment));
    }
    let policy = policy_of(loop_definition);
    if loop_definition.topology == Some(Topology::OpenLoop) {
        return Ok(plant_input_method(policy.as_ref()));
    }
    if mode == ControllerMode::Unavailable {
        return Err(PolicyError::ModeUnavailable);
    }
    let open_loop_permitted = policy
        .as_ref()
        .is_none_or(|policy| policy.open_loop_permitted);
    let has_manual_command = loop_definition
        .controller
        .manual_output_signal
        .as_deref()
        .is_some_and(|signal| !signal.is_empty());
    let open_loop_ok = open_loop_permitted && has_manual_command;
    let has_setpoint = loop_definition.signals.setpoint.is_some();
    if has_setpoint && mode == ControllerMode::Auto {
        return Ok(IdentificationMethod {
            kind: ControlExperimentKind::SetpointStep,
            manual: false,
            automatic: true,
        });
    }
    if !open_loop_ok {
        // Automatic never puts a loop in Manual into Auto: the controller may not hold the loop yet.
        return Err(if has_setpoint {
            PolicyError::NoSafeMethodInManual
        } else {
            PolicyError::NoSafeMethodWithoutSetpoint
        });
    }
    Ok(plant_input_method(policy.as_ref()))
}

/// The method of a project-scripted loop (C12, B3.3): the project's scenario, whatever the mode, with no Manual output and
/// no mode change. Control Studio writes only the activation, as a `project_sequence`; without an activation binding it
/// observes the declared duration.
fn project_scenario_method(experiment: &Experiment) -> IdentificationMethod {
    let kind = if experiment.activation.is_some() {
        ControlExperimentKind::ProjectSequence
    } else {
        ControlExperimentKind::Observation
    };
    IdentificationMethod {
        kind,
        manual: false,
        automatic: true,
    }
}

/// A plant-input step in Manual: a pulse when sustained holds are not permitted, PRBS when the policy asks for frequency
/// content, and a step otherwise.
fn plant_input_method(policy: Option<&IdentificationPolicy>) -> IdentificationMethod {
    let kind = match policy {
        Some(policy) if policy.information == PlantInformation::Frequency => {
            ControlExperimentKind::Prbs
        }
        Some(policy) if !policy.sustained_hold_permitted => ControlExperimentKind::MvPulse,
        _ => ControlExperimentKind::MvStep,
    };
    IdentificationMethod {
        kind,
        manual: true,
        automatic: true,
    }
}

/// The checks of an explicit method against the loop's policy: a load step needs a declared disturbance channel, and
/// the policy may forbid open-loop methods or sustained steps. A project-scripted loop takes no explicit excitation (C12).
/// An input's own identification step is checked the same way (`channel_steps`, C7 and C8).
pub(crate) fn validate_explicit_method(
    loop_definition: &LoopDefinition,
    requested: ExperimentKind,
) -> Result<(), PolicyError> {
    if project_scripted(loop_definition).is_some() {
        return Err(PolicyError::DuplicateExcitation);
    }
    if requested == ExperimentKind::LoadStep {
        return Err(PolicyError::PinnedLoadStep);
    }
    let policy = policy_of(loop_definition);
    let open_loop = matches!(
        requested,
        ExperimentKind::MvStep
            | ExperimentKind::MvPulse
            | ExperimentKind::Relay
            | ExperimentKind::Prbs
            | ExperimentKind::Multisine
    );
    if open_loop
        && policy
            .as_ref()
            .is_some_and(|policy| !policy.open_loop_permitted)
    {
        return Err(PolicyError::PinnedClosedLoopOnly);
    }
    if requested == ExperimentKind::MvStep
        && policy
            .as_ref()
            .is_some_and(|policy| !policy.sustained_hold_permitted)
    {
        return Err(PolicyError::PinnedHoldRestriction);
    }
    Ok(())
}

/// Whether the plant input is an ON/OFF actuator, which steps between its two levels (B3.3).
#[must_use]
pub fn on_off_actuator(loop_definition: &LoopDefinition) -> bool {
    loop_definition.controller.on_off.is_some()
}

/// The inputs of one excitation's size and direction (`IdentificationAmplitudeInput`).
#[derive(Debug, Clone, Copy)]
pub struct AmplitudeInput<'a> {
    /// The loop of the input being stepped: its limits, signals and declared amplitude.
    pub loop_definition: &'a LoopDefinition,
    /// The fresh sample at the step, or `None` when no synchronized sample is available.
    pub sample: Option<&'a ExperimentSample>,
    /// The experiment kind of the excitation.
    pub kind: ControlExperimentKind,
    /// Whether Automatic chose the size and the direction.
    pub automatic: bool,
    /// The noise σ measured on the steady baseline.
    pub noise: f64,
    /// Whether the excitation is the independent model check, which uses the opposite excitation (B5.2).
    pub validation: bool,
    /// The amplitude an explicit method uses when the loop declares none.
    pub fallback_amplitude: f64,
    /// Whether the input is an ON/OFF actuator.
    pub on_off: bool,
}

/// The signed excitation of one experiment (`identificationAmplitude`): the declared amplitude with its sign, or
/// Automatic's size and direction (B3.3, B5.2). The model check uses 0.75 of an explicit or plant-input amplitude, and
/// the opposite direction when there is room for it.
///
/// # Errors
/// Returns [`PolicyError::SampleNotAvailable`] without a sample, [`PolicyError::PlantInputLimits`] when an automatic
/// plant-input step has no limits to size it, and [`PolicyError::NoHeadroom`] when Automatic cannot step the signal from
/// its current value.
pub fn identification_amplitude(input: &AmplitudeInput<'_>) -> Result<f64, PolicyError> {
    let sample = input.sample.ok_or(PolicyError::SampleNotAvailable)?;
    // A project's scenario and an observation have no amplitude of their own: Control Studio writes none (C12, B3.3).
    if matches!(
        input.kind,
        ControlExperimentKind::Observation | ControlExperimentKind::ProjectSequence
    ) {
        return Ok(0.0);
    }
    let reduced = if input.validation && input.kind != ControlExperimentKind::SetpointStep {
        0.75
    } else {
        1.0
    };
    if !input.automatic {
        let configured = input
            .loop_definition
            .experiment
            .as_ref()
            .map(|experiment| experiment.amplitude)
            .filter(|amplitude| *amplitude != 0.0 && !amplitude.is_nan())
            .unwrap_or(input.fallback_amplitude);
        let direction = if input.validation { -1.0 } else { 1.0 };
        let sign = if configured < 0.0 { -1.0 } else { 1.0 };
        return Ok(direction * sign * configured.abs() * reduced);
    }
    let direction = step_direction(input.loop_definition, sample, input.kind, input.on_off)?;
    // An on/off actuator has two levels: its check steps to the other one, as the identification did.
    let opposite = if input.validation && !input.on_off {
        automatic_amplitude(input, sample, -direction, reduced).ok()
    } else {
        None
    };
    match opposite {
        Some(amplitude) => Ok(amplitude),
        None => automatic_amplitude(input, sample, direction, reduced),
    }
}

/// The direction Automatic steps in: upward for a PRBS; toward the larger headroom for a setpoint step and a plant-input
/// step; for an ON/OFF actuator, toward its other level.
fn step_direction(
    loop_definition: &LoopDefinition,
    sample: &ExperimentSample,
    kind: ControlExperimentKind,
    on_off: bool,
) -> Result<f64, PolicyError> {
    if kind == ControlExperimentKind::Prbs {
        return Ok(1.0);
    }
    if kind == ControlExperimentKind::SetpointStep {
        let range = loop_definition.signals.process_variable.range;
        let value = sample.process_variable.value;
        let upward = range.maximum - value >= value - range.minimum;
        return Ok(if upward { 1.0 } else { -1.0 });
    }
    let (minimum, maximum) = plant_input_limits(loop_definition)?;
    let value = sample.manipulated_variable.value;
    if on_off {
        let upward = value <= f64::midpoint(minimum, maximum);
        return Ok(if upward { 1.0 } else { -1.0 });
    }
    let upward = maximum - value >= value - minimum;
    Ok(if upward { 1.0 } else { -1.0 })
}

/// The plant input's limits an automatic plant-input step needs (B3.4 step 1): both are set and ordered. A refusal is the
/// campaign's own failure, which names the missing field and opens Limits (`plant_input_range`).
fn plant_input_limits(loop_definition: &LoopDefinition) -> Result<(f64, f64), PolicyError> {
    plant_input_range(loop_definition)
        .map(|range| (range.minimum, range.maximum))
        .map_err(PolicyError::PlantInputLimits)
}

/// The size of an automatic step in one direction (`automaticAmplitude`), or the refusal when it has no room.
fn automatic_amplitude(
    input: &AmplitudeInput<'_>,
    sample: &ExperimentSample,
    direction: f64,
    reduced: f64,
) -> Result<f64, PolicyError> {
    let loop_definition = input.loop_definition;
    let setpoint_step = input.kind == ControlExperimentKind::SetpointStep;
    let amplitude = if setpoint_step {
        setpoint_amplitude(loop_definition, sample, input.noise, direction)
    } else {
        output_amplitude(loop_definition, sample, input, direction, reduced)?
    };
    if amplitude.is_finite() && amplitude != 0.0 {
        return Ok(amplitude);
    }
    let (signal, current) = if setpoint_step {
        (
            &loop_definition.signals.process_variable,
            &sample.process_variable,
        )
    } else {
        (
            &loop_definition.signals.manipulated_variable,
            &sample.manipulated_variable,
        )
    };
    Err(PolicyError::NoHeadroom {
        symbol: signal.symbol.clone(),
        value: current.value,
        unit: current.unit.clone(),
    })
}

/// A setpoint step (B3.3): the larger of 20 times the noise and 2 % of the measured output's range, capped at 10 % of
/// the range and by the headroom to the permitted measured-output range, with the chosen direction.
fn setpoint_amplitude(
    loop_definition: &LoopDefinition,
    sample: &ExperimentSample,
    noise: f64,
    direction: f64,
) -> f64 {
    let range = loop_definition.signals.process_variable.range;
    let span = range.maximum - range.minimum;
    let value = sample.process_variable.value;
    let headroom = if direction > 0.0 {
        range.maximum - value
    } else {
        value - range.minimum
    };
    let limited = (20.0 * noise)
        .max(0.02 * span)
        .min(0.1 * span)
        .min(headroom);
    direction * limited.max(0.0)
}

/// A plant-input step (B3.3): 10 % of the input range, capped by the headroom to the input limits. A PRBS takes the same
/// size with no direction; an ON/OFF actuator steps to its other level.
fn output_amplitude(
    loop_definition: &LoopDefinition,
    sample: &ExperimentSample,
    input: &AmplitudeInput<'_>,
    direction: f64,
    reduced: f64,
) -> Result<f64, PolicyError> {
    let (minimum, maximum) = plant_input_limits(loop_definition)?;
    let value = sample.manipulated_variable.value;
    let amplitude = if input.kind == ControlExperimentKind::Prbs {
        (0.1 * (maximum - minimum))
            .min(value - minimum)
            .min(maximum - value)
            .max(0.0)
    } else if input.on_off {
        if direction > 0.0 {
            maximum - value
        } else {
            minimum - value
        }
    } else {
        let headroom = if direction > 0.0 {
            maximum - value
        } else {
            value - minimum
        };
        direction * (0.1 * (maximum - minimum) * reduced).min(headroom).max(0.0)
    };
    Ok(amplitude)
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
