//! The loop's channels (`loopChannels.ts`; specification 35, C7 and C8 and §17): the model's inputs and outputs, each bound to a
//! loop signal, with an input's actuator and Manual command and an output's setpoint and requirements. [`LoopChannels::check`]
//! holds the shape rules of `validLoopChannels`, and [`LoopChannels::check_bindings`] the binding rules of
//! `validateLoopChannelBindings`. The gate still parks a loop with channels until the multichannel shapes are served (checklist
//! V7); identification and its tests reach these rules now.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AuxiliaryRole, AuxiliarySignal, LoopDefinition, LoopSignal, ManualOutputType, Range};
use crate::model::simulation::{valid_actuator, ActuatorCapabilities, ActuatorKind};

/// The most inputs, and the most outputs, a loop's channels may hold.
const MAXIMUM_CHANNELS: usize = 16;

/// The most state deviations a loop's channels may declare.
const MAXIMUM_INITIAL_STATES: usize = 32;

/// The refusal of channels whose shape breaks a rule.
const INVALID_CHANNELS: &str = "Invalid multichannel binding, actuator or requirement.";

/// The refusal of a channel whose signal role does not match its binding.
const ROLE_MISMATCH: &str =
    "Model input, output and setpoint bindings must match their declared signal roles.";

/// The refusal of a Boolean input that is not an ON/OFF actuator on the levels 0 and 1 in dimensionless coordinates.
const BOOLEAN_LEVELS: &str =
    "Boolean actuation requires ON/OFF levels 0/1 in dimensionless coordinates.";

/// The refusal of applied feedback that is not a separate input in command coordinates.
const APPLIED_FEEDBACK: &str =
    "Applied actuator feedback requires a separate input binding in command coordinates.";

/// The loop's channels: the model's inputs and outputs, and the state deviations of the selected model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoopChannels {
    /// The model's inputs, in order: one to sixteen.
    pub inputs: Vec<ChannelInput>,
    /// The model's outputs, in order: one to sixteen.
    pub outputs: Vec<ChannelOutput>,
    /// The state deviations in the selected model's declared coordinates: at most 32 and finite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_state: Option<Vec<f64>>,
}

/// One model input: the loop signal it drives or reads, its Manual command and its actuator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInput {
    /// The input's identifier in the model.
    pub model_channel_id: String,
    /// The identifier of the loop signal the input is bound to.
    pub signal_id: String,
    /// The Manual command the input is stepped through, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_output_signal: Option<String>,
    /// The type of the Manual command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_output_type: Option<ManualOutputType>,
    /// The identifier of the loop signal that reports the actuator's applied value, when it is not the default one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_signal_id: Option<String>,
    /// The input's actuator: its kind, its range and its rate or minimum-time limits.
    pub actuator: ActuatorCapabilities,
    /// The input's own identification step, in its unit: positive and within the actuator's span (specification 35, §17.9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment_step: Option<f64>,
}

/// One model output: the loop signal it measures, its setpoint and its own requirements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChannelOutput {
    /// The output's identifier in the model.
    pub model_channel_id: String,
    /// The identifier of the loop signal the output is bound to.
    pub signal_id: String,
    /// The identifier of the loop signal that holds the output's setpoint, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setpoint_signal_id: Option<String>,
    /// The output's own requirements, in place of the loop's, when it has them: they need a setpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirements: Option<ChannelRequirements>,
}

/// A measurement's own requirements: the setpoint change of its own test, the largest error allowed at its settling time, the
/// band around its final value, and the time by which it settles.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRequirements {
    /// The setpoint change of the measurement's own test.
    pub reference_change: f64,
    /// The largest error allowed at the settling time; positive.
    pub maximum_error: f64,
    /// The band around the final value; positive and below the largest error.
    pub settling_band: f64,
    /// The time by which the response settles, in seconds; positive.
    pub settling_time_seconds: f64,
}

/// A loop signal as a channel reads it: its symbol, unit, IEC type, address and range (`loopSignal`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelSignal<'a> {
    /// The variable's path.
    pub symbol: &'a str,
    /// The unit; `"1"` is dimensionless.
    pub unit: &'a str,
    /// The variable's IEC type.
    pub iec_type: &'a str,
    /// The variable's address, when it has one.
    pub address: Option<&'a str>,
    /// The signal's range.
    pub range: Range,
}

impl<'a> ChannelSignal<'a> {
    fn of_primary(signal: &'a LoopSignal) -> Self {
        Self {
            symbol: &signal.symbol,
            unit: &signal.unit,
            iec_type: &signal.iec_type,
            address: signal.address.as_deref(),
            range: signal.range,
        }
    }

    fn of_auxiliary(signal: &'a AuxiliarySignal) -> Self {
        Self {
            symbol: &signal.symbol,
            unit: &signal.unit,
            iec_type: &signal.iec_type,
            address: signal.address.as_deref(),
            range: signal.range,
        }
    }
}

impl LoopChannels {
    /// Checks the shape of the channels (`validLoopChannels`): one to sixteen inputs and outputs, text and unique identifiers on
    /// each side, valid actuators, a positive experiment step within the actuator's span, valid requirements, and at most 32
    /// finite state deviations.
    ///
    /// # Errors
    /// The channels break one of these rules; the sentence names the group of rules.
    pub fn check(&self) -> Result<(), String> {
        let inputs = self
            .inputs
            .iter()
            .map(|input| (input.model_channel_id.as_str(), input.signal_id.as_str()));
        let outputs = self
            .outputs
            .iter()
            .map(|output| (output.model_channel_id.as_str(), output.signal_id.as_str()));
        let shaped = counted(self.inputs.len())
            && counted(self.outputs.len())
            && identifiers_valid(inputs)
            && identifiers_valid(outputs)
            && self.inputs.iter().all(ChannelInput::is_valid)
            && self.outputs.iter().all(ChannelOutput::is_valid)
            && self.initial_state.as_ref().is_none_or(|state| {
                state.len() <= MAXIMUM_INITIAL_STATES && state.iter().all(|value| value.is_finite())
            });
        if shaped {
            Ok(())
        } else {
            Err(INVALID_CHANNELS.to_owned())
        }
    }

    /// Checks the channels against the loop's signals (`validateLoopChannelBindings`): the shape, the roles of the bound signals,
    /// that no signal is claimed twice, each input's Boolean, applied-feedback and range rules, and each output's setpoint.
    ///
    /// # Errors
    /// The channels break a rule of their shape or of their binding; the sentence names the rule.
    pub fn check_bindings(&self, loop_definition: &LoopDefinition) -> Result<(), String> {
        self.check()?;
        check_roles(self, loop_definition)?;
        check_claims(self, loop_definition)?;
        for input in &self.inputs {
            check_input(input, loop_definition)?;
        }
        for output in &self.outputs {
            check_output(output, loop_definition)?;
        }
        Ok(())
    }
}

impl ChannelInput {
    /// The actuator is valid, the optional texts are text, and an experiment step is positive and within the actuator's span.
    fn is_valid(&self) -> bool {
        valid_actuator(&self.actuator)
            && self.manual_output_signal.as_deref().is_none_or(is_text)
            && self.applied_signal_id.as_deref().is_none_or(is_text)
            && self.experiment_step.is_none_or(|step| {
                step.is_finite()
                    && step > 0.0
                    && step <= self.actuator.maximum - self.actuator.minimum
            })
    }
}

impl ChannelOutput {
    /// The setpoint identifier is text when given, and the requirements are valid when given.
    fn is_valid(&self) -> bool {
        self.setpoint_signal_id.as_deref().is_none_or(is_text)
            && self
                .requirements
                .as_ref()
                .is_none_or(ChannelRequirements::is_valid)
    }
}

impl ChannelRequirements {
    /// The error and the time are positive, and the settling band is positive and below the largest error.
    fn is_valid(&self) -> bool {
        self.reference_change.is_finite()
            && self.maximum_error.is_finite()
            && self.maximum_error > 0.0
            && self.settling_band.is_finite()
            && self.settling_band > 0.0
            && self.settling_band < self.maximum_error
            && self.settling_time_seconds.is_finite()
            && self.settling_time_seconds > 0.0
    }
}

/// Whether a model input is a measured disturbance: the loop declares its signal with the disturbance role, so the controller
/// reads it and never writes it (specification 35, §17.12).
#[must_use]
pub fn measured_disturbance(loop_definition: &LoopDefinition, input: &ChannelInput) -> bool {
    auxiliary(loop_definition, &input.signal_id)
        .is_some_and(|signal| signal.role == AuxiliaryRole::Disturbance)
}

/// The signal a channel identifier names: a fixed primary signal, or an additional signal of the loop by its identifier. `None`
/// when the loop declares no such signal (`loopSignal`).
#[must_use]
pub fn channel_signal<'a>(
    loop_definition: &'a LoopDefinition,
    id: &str,
) -> Option<ChannelSignal<'a>> {
    let primary = match id {
        "setpoint" => loop_definition.signals.setpoint.as_ref(),
        "processVariable" => Some(&loop_definition.signals.process_variable),
        "manipulatedVariable" => Some(&loop_definition.signals.manipulated_variable),
        "actuator" => loop_definition.signals.actuator.as_ref(),
        _ => None,
    };
    primary
        .map(ChannelSignal::of_primary)
        .or_else(|| auxiliary(loop_definition, id).map(ChannelSignal::of_auxiliary))
}

/// The additional signal of the loop with this identifier.
fn auxiliary<'a>(loop_definition: &'a LoopDefinition, id: &str) -> Option<&'a AuxiliarySignal> {
    loop_definition
        .signals
        .additional
        .as_ref()?
        .iter()
        .find(|signal| signal.id == id)
}

/// The role a channel identifier has: the fixed roles of the primary signals, or the additional signal's own role.
fn role_of(loop_definition: &LoopDefinition, id: &str) -> Option<AuxiliaryRole> {
    match id {
        "manipulatedVariable" => Some(AuxiliaryRole::ManipulatedVariable),
        "processVariable" => Some(AuxiliaryRole::ProcessVariable),
        "setpoint" => Some(AuxiliaryRole::Setpoint),
        _ => auxiliary(loop_definition, id).map(|signal| signal.role),
    }
}

/// The signal a channel identifier names, or the refusal that it names none.
fn declared<'a>(
    loop_definition: &'a LoopDefinition,
    id: &str,
) -> Result<ChannelSignal<'a>, String> {
    channel_signal(loop_definition, id)
        .ok_or_else(|| format!("Channel binding {id} does not identify a declared signal."))
}

/// Each input is the manipulated variable, or after the first input a measured disturbance, and each output is a process variable
/// or a state, with a setpoint that has the setpoint role when it has one.
fn check_roles(channels: &LoopChannels, loop_definition: &LoopDefinition) -> Result<(), String> {
    let inputs_bound = channels.inputs.iter().enumerate().all(|(index, input)| {
        let role = role_of(loop_definition, &input.signal_id);
        role == Some(AuxiliaryRole::ManipulatedVariable)
            || (index > 0 && role == Some(AuxiliaryRole::Disturbance))
    });
    let outputs_bound = channels.outputs.iter().all(|output| {
        matches!(
            role_of(loop_definition, &output.signal_id),
            Some(AuxiliaryRole::ProcessVariable | AuxiliaryRole::State)
        ) && output
            .setpoint_signal_id
            .as_deref()
            .is_none_or(|id| role_of(loop_definition, id) == Some(AuxiliaryRole::Setpoint))
    });
    if inputs_bound && outputs_bound {
        Ok(())
    } else {
        Err(ROLE_MISMATCH.to_owned())
    }
}

/// No two channels claim the same loop signal, and every bound signal is declared.
fn check_claims(channels: &LoopChannels, loop_definition: &LoopDefinition) -> Result<(), String> {
    let mut claimed = BTreeSet::new();
    let bound = channels
        .inputs
        .iter()
        .map(|input| input.signal_id.as_str())
        .chain(
            channels
                .outputs
                .iter()
                .map(|output| output.signal_id.as_str()),
        );
    for id in bound {
        let signal = declared(loop_definition, id)?;
        if !claimed.insert(signal.symbol) {
            return Err(format!(
                "Signal {} is assigned to multiple model channels.",
                signal.symbol
            ));
        }
    }
    Ok(())
}

/// An input's signal is declared; a Boolean signal is an ON/OFF actuator on the levels 0 and 1 in dimensionless coordinates;
/// applied feedback is a separate input in command coordinates; and the actuator's range lies within the signal's range.
fn check_input(input: &ChannelInput, loop_definition: &LoopDefinition) -> Result<(), String> {
    let signal = declared(loop_definition, &input.signal_id)?;
    if signal.iec_type.eq_ignore_ascii_case("BOOL")
        && (signal.unit != "1" || !is_binary_on_off(&input.actuator))
    {
        return Err(BOOLEAN_LEVELS.to_owned());
    }
    if let Some(applied) = applied_feedback(input, &signal, loop_definition)?
        && (applied.unit != signal.unit
            || applied.symbol == signal.symbol
            || !applied
                .address
                .is_some_and(|address| address.starts_with("%I")))
    {
        return Err(APPLIED_FEEDBACK.to_owned());
    }
    if input.actuator.minimum < signal.range.minimum
        || input.actuator.maximum > signal.range.maximum
    {
        return Err(format!(
            "Actuator limits exceed {}'s declared range.",
            signal.symbol
        ));
    }
    Ok(())
}

/// The signal an input's applied feedback reads: the one it names, or for the manipulated variable the loop's actuator position.
fn applied_feedback<'a>(
    input: &ChannelInput,
    signal: &ChannelSignal<'_>,
    loop_definition: &'a LoopDefinition,
) -> Result<Option<ChannelSignal<'a>>, String> {
    match input.applied_signal_id.as_deref() {
        Some(id) => declared(loop_definition, id).map(Some),
        None if signal.symbol == loop_definition.signals.manipulated_variable.symbol.as_str() => {
            Ok(loop_definition
                .signals
                .actuator
                .as_ref()
                .map(ChannelSignal::of_primary))
        }
        None => Ok(None),
    }
}

/// An output's signal is declared; its requirements need a setpoint; and a setpoint has the output's unit.
fn check_output(output: &ChannelOutput, loop_definition: &LoopDefinition) -> Result<(), String> {
    let signal = declared(loop_definition, &output.signal_id)?;
    if output.requirements.is_some() && output.setpoint_signal_id.is_none() {
        return Err(format!(
            "Controlled output {} needs a setpoint.",
            output.model_channel_id
        ));
    }
    if let Some(id) = &output.setpoint_signal_id
        && declared(loop_definition, id)?.unit != signal.unit
    {
        return Err(format!(
            "Setpoint units disagree for {}.",
            output.model_channel_id
        ));
    }
    Ok(())
}

/// Whether an actuator is ON/OFF on exactly the levels 0 and 1.
fn is_binary_on_off(actuator: &ActuatorCapabilities) -> bool {
    actuator.kind == ActuatorKind::OnOff
        && equal(actuator.minimum, 0.0)
        && equal(actuator.maximum, 1.0)
}

/// Whether two numbers are equal, compared exactly.
fn equal(value: f64, target: f64) -> bool {
    value.partial_cmp(&target) == Some(Ordering::Equal)
}

/// Whether a count of channels is one to the maximum.
fn counted(count: usize) -> bool {
    (1..=MAXIMUM_CHANNELS).contains(&count)
}

/// Whether a text is not blank.
fn is_text(value: &str) -> bool {
    !value.trim().is_empty()
}

/// Whether every item has a text model identifier and a text signal identifier, and no two items share either.
fn identifiers_valid<'a>(mut items: impl Iterator<Item = (&'a str, &'a str)>) -> bool {
    let mut models = BTreeSet::new();
    let mut signals = BTreeSet::new();
    items.all(|(model, signal)| {
        is_text(model) && is_text(signal) && models.insert(model) && signals.insert(signal)
    })
}

#[cfg(test)]
#[path = "channels_tests.rs"]
mod tests;
