//! The acquisition settings of an identification (specification 35, A7 and B3.3): the method, duration, amplitude and
//! ranges the engineer asks for, their checks, and the loop they configure for the experiment (`acquisitionSettings.ts`).
//! A range may narrow a signal's declared range, never widen it. The multichannel forms (channel bindings) are parked
//! with stage V7 and are not decoded here.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::texts::acquisition_text;
use crate::limits::{MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS};
use crate::loop_definition::{
    AuxiliaryRole, AuxiliarySignal, Execution, Experiment, ExperimentKind, LoopDefinition,
    LoopSignal, Range, RelayDirection,
};

/// The identification method the settings ask for. `Configured` keeps the method the loop declares; `Automatic` lets
/// Identify choose from the loop's mode and policy (B3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionMethod {
    /// The method the loop declares.
    Configured,
    /// Identify chooses the method.
    Automatic,
    /// A step of the manual output.
    MvStep,
    /// A pulse of the manual output.
    MvPulse,
    /// A step of the setpoint, in Auto.
    SetpointStep,
    /// A relay on the measured process value.
    Relay,
    /// A pseudo-random binary sequence.
    Prbs,
    /// A multisine.
    Multisine,
}

/// The request for an identification experiment: what it excites, for how long and how far, and the ranges it may use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionSettings {
    /// The method of the experiment.
    pub method: AcquisitionMethod,
    /// The duration of the experiment in seconds, from one second to 24 hours.
    pub duration_seconds: f64,
    /// The amplitude of the experiment; zero asks for the declared one.
    pub amplitude: f64,
    /// The declared signal a setpoint step moves, when the loop has no setpoint of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setpoint_signal_id: Option<String>,
    /// The first direction of a relay experiment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_direction: Option<RelayDirection>,
    /// Whether the experiment may reuse a recording of the same loop.
    pub reuse_recordings: bool,
    /// The range of the manipulated variable the experiment may use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_range: Option<Range>,
    /// The range of the measured output the experiment may use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_range: Option<Range>,
    /// The ranges of further input signals, by symbol: at most sixteen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_ranges: Option<BTreeMap<String, Range>>,
    /// The ranges of further output signals, by symbol: at most sixteen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_ranges: Option<BTreeMap<String, Range>>,
}

/// The channel direction of a range: a manipulated variable and its companions, or a measured output and its companions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelDirection {
    /// An input of the plant.
    Input,
    /// A measured output of the plant.
    Output,
}

/// Why acquisition settings cannot configure the loop (`configuredAcquisition`).
#[derive(Debug, Clone, PartialEq)]
pub enum AcquisitionError {
    /// No settings were given and the loop declares no positive experiment duration.
    LimitsRequired,
    /// The setpoint reference is not a declared further signal of the measured output's unit.
    SetpointReference,
    /// A range names a signal that the loop does not declare for that direction.
    UnknownChannelRange(ChannelDirection),
    /// A range is not valid, or it leaves the signal's declared limits.
    RangeOutsideLimits {
        /// The signal's symbol.
        signal: String,
        /// The smallest value the signal may be given.
        minimum: f64,
        /// The largest value the signal may be given.
        maximum: f64,
    },
}

impl AcquisitionError {
    /// The failure the engineer sees: its sentence, its cause, and the one next action it offers.
    #[must_use]
    pub fn failure(&self) -> Failure {
        let next_action = match self {
            Self::LimitsRequired
            | Self::UnknownChannelRange(_)
            | Self::RangeOutsideLimits { .. } => NextAction::OpenLimits,
            Self::SetpointReference => NextAction::None,
        };
        Failure::new(&acquisition_text(self), Cause::Settings, next_action)
    }
}

/// The settings an identification starts from when the engineer edits none, as on the command line (specification 35, §0.1b
/// and B3.2; `acquisitionPreferences` with the declared limits): the method given, and the loop's declared experiment for the
/// rest, its maximum duration, its amplitude (zero when it declares none, which sizes the step by the method) and its relay
/// direction; no setpoint reference, no reuse of recordings, and no narrowed range, so the declared ranges apply. A loop that
/// declares no experiment, or no duration, gets a duration of zero, which the campaign refuses with Limits as the next action.
#[must_use]
pub fn declared_settings(
    loop_definition: &LoopDefinition,
    method: AcquisitionMethod,
) -> AcquisitionSettings {
    let declared = loop_definition.experiment.as_ref();
    AcquisitionSettings {
        method,
        duration_seconds: declared
            .and_then(|experiment| experiment.duration_seconds)
            .unwrap_or(0.0),
        amplitude: declared.map_or(0.0, |experiment| experiment.amplitude),
        setpoint_signal_id: None,
        relay_direction: declared.and_then(|experiment| experiment.relay_direction),
        reuse_recordings: false,
        input_range: None,
        output_range: None,
        input_ranges: None,
        output_ranges: None,
    }
}

/// Decodes acquisition settings from their JSON and returns them only when every check holds: the duration is between
/// one second and 24 hours, the amplitude is finite, a setpoint signal names a non-empty identifier of at most 256
/// characters, a relay names its direction, each range is finite with a minimum below its maximum, and no range map
/// holds more than sixteen entries.
#[must_use]
pub fn decode_acquisition_settings(value: &Value) -> Option<AcquisitionSettings> {
    let settings: AcquisitionSettings = crate::json::decode_value(value.clone()).ok()?;
    settings_are_valid(&settings).then_some(settings)
}

fn settings_are_valid(settings: &AcquisitionSettings) -> bool {
    let duration_ok = settings.duration_seconds.is_finite()
        && (milliseconds_as_seconds(MINIMUM_EXPERIMENT_DURATION_MS)
            ..=milliseconds_as_seconds(MAXIMUM_EXPERIMENT_DURATION_MS))
            .contains(&settings.duration_seconds);
    let setpoint_ok = settings
        .setpoint_signal_id
        .as_ref()
        .is_none_or(|id| !id.is_empty() && id.encode_utf16().count() <= 256);
    let relay_ok =
        settings.method != AcquisitionMethod::Relay || settings.relay_direction.is_some();
    duration_ok
        && settings.amplitude.is_finite()
        && setpoint_ok
        && relay_ok
        && settings.input_range.is_none_or(valid_range)
        && settings.output_range.is_none_or(valid_range)
        && valid_range_map(settings.input_ranges.as_ref())
        && valid_range_map(settings.output_ranges.as_ref())
}

/// A limit in milliseconds as seconds. The limits are far below the range where a float loses integer precision.
#[allow(clippy::cast_precision_loss)]
fn milliseconds_as_seconds(milliseconds: u64) -> f64 {
    milliseconds as f64 / 1_000.0
}

fn valid_range(range: Range) -> bool {
    range.minimum.is_finite() && range.maximum.is_finite() && range.minimum < range.maximum
}

fn valid_range_map(ranges: Option<&BTreeMap<String, Range>>) -> bool {
    ranges.is_none_or(|map| map.len() <= 16 && map.values().all(|range| valid_range(*range)))
}

/// The loop the experiment runs against, with the settings applied (`configuredAcquisition`): the experiment's method,
/// duration and amplitude; the setpoint the settings name, when they name one; the manipulated variable's and the
/// measured output's ranges, narrowed within their declared limits; the output limits the input range sets, where the loop
/// sets them (a limit the loop does not set stays unset, so Identify names it, B3.4 step 1); and the ranges of further
/// signals. Without settings a declared duration must be positive, and the loop is returned unchanged; a missing duration is
/// left to the campaign's precondition, which names it. The loop passed in is never changed.
///
/// # Errors
/// Returns the [`AcquisitionError`] that names the setting that cannot be applied, in the order the checks run.
pub fn configured_acquisition(
    loop_definition: &LoopDefinition,
    settings: Option<&AcquisitionSettings>,
) -> Result<LoopDefinition, AcquisitionError> {
    let Some(settings) = settings else {
        let declared = loop_definition
            .experiment
            .as_ref()
            .and_then(|experiment| experiment.duration_seconds);
        return match declared {
            Some(seconds) if !(seconds.is_finite() && seconds > 0.0) => {
                Err(AcquisitionError::LimitsRequired)
            }
            _ => Ok(loop_definition.clone()),
        };
    };
    let reference = reference_signal(loop_definition, settings)?;
    let input = loop_definition.signals.manipulated_variable.clone();
    let output = loop_definition.signals.process_variable.clone();
    let limits = &loop_definition.limits;
    let input_bounds = Range {
        minimum: limits
            .output_minimum
            .map_or(input.range.minimum, |minimum| {
                input.range.minimum.max(minimum)
            }),
        maximum: limits
            .output_maximum
            .map_or(input.range.maximum, |maximum| {
                input.range.maximum.min(maximum)
            }),
    };
    let input_ranges = channel_ranges(
        loop_definition,
        settings.input_ranges.as_ref(),
        ChannelDirection::Input,
    )?;
    let output_ranges = channel_ranges(
        loop_definition,
        settings.output_ranges.as_ref(),
        ChannelDirection::Output,
    )?;
    let input_range = bounded_range(
        requested(input_ranges, &input.symbol, settings.input_range),
        input_bounds,
        &input.symbol,
    )?;
    let output_range = bounded_range(
        requested(output_ranges, &output.symbol, settings.output_range),
        output.range,
        &output.symbol,
    )?;
    let additional = match &loop_definition.signals.additional {
        Some(signals) => Some(narrowed_additional(signals, input_ranges, output_ranges)?),
        None => None,
    };

    let mut configured = loop_definition.clone();
    if let Some(reference) = reference {
        configured.signals.setpoint = Some(reference);
    }
    configured.signals.manipulated_variable = LoopSignal {
        range: input_range,
        ..input
    };
    configured.signals.process_variable = LoopSignal {
        range: output_range,
        ..output
    };
    if let Some(additional) = additional {
        configured.signals.additional = Some(additional);
    }
    configured.limits.output_minimum = limits.output_minimum.map(|_| input_range.minimum);
    configured.limits.output_maximum = limits.output_maximum.map(|_| input_range.maximum);
    configured.experiment = Some(experiment_for(
        settings,
        loop_definition.experiment.as_ref(),
    ));
    Ok(configured)
}

/// The declared further signal the settings name as the setpoint, when they name one. It must be a setpoint of the
/// loop's further signals, in the measured output's unit, and a setpoint of the loop itself must have the same symbol.
fn reference_signal(
    loop_definition: &LoopDefinition,
    settings: &AcquisitionSettings,
) -> Result<Option<LoopSignal>, AcquisitionError> {
    let Some(id) = settings
        .setpoint_signal_id
        .as_deref()
        .filter(|id| !id.is_empty())
    else {
        return Ok(None);
    };
    let reference = additional_signals(loop_definition)
        .iter()
        .find(|signal| signal.id == id && signal.role == AuxiliaryRole::Setpoint);
    let mismatched = match reference {
        None => true,
        Some(reference) => {
            reference.unit != loop_definition.signals.process_variable.unit
                || loop_definition
                    .signals
                    .setpoint
                    .as_ref()
                    .is_some_and(|setpoint| reference.symbol != setpoint.symbol)
        }
    };
    if mismatched {
        return Err(AcquisitionError::SetpointReference);
    }
    Ok(reference.map(loop_signal_of))
}

/// The further signals of the loop, empty when it declares none.
fn additional_signals(loop_definition: &LoopDefinition) -> &[AuxiliarySignal] {
    loop_definition.signals.additional.as_deref().unwrap_or(&[])
}

/// A further signal as a loop signal: the same variable, without its identifier, channel and role.
fn loop_signal_of(signal: &AuxiliarySignal) -> LoopSignal {
    LoopSignal {
        symbol: signal.symbol.clone(),
        address: signal.address.clone(),
        iec_type: signal.iec_type.clone(),
        direction: signal.direction,
        unit: signal.unit.clone(),
        range: signal.range,
    }
}

/// The ranges of one direction, when the settings give any. Each named symbol must be a signal the loop declares for
/// that direction: the manipulated variable and its manipulated companions for an input, the measured output and its
/// measured or state companions for an output.
fn channel_ranges<'a>(
    loop_definition: &LoopDefinition,
    ranges: Option<&'a BTreeMap<String, Range>>,
    direction: ChannelDirection,
) -> Result<Option<&'a BTreeMap<String, Range>>, AcquisitionError> {
    let Some(ranges) = ranges else {
        return Ok(None);
    };
    let declared = declared_symbols(loop_definition, direction);
    if ranges
        .keys()
        .any(|symbol| !declared.contains(&symbol.as_str()))
    {
        return Err(AcquisitionError::UnknownChannelRange(direction));
    }
    Ok(Some(ranges))
}

fn declared_symbols(loop_definition: &LoopDefinition, direction: ChannelDirection) -> Vec<&str> {
    let mut symbols = Vec::new();
    match direction {
        ChannelDirection::Input => {
            symbols.push(loop_definition.signals.manipulated_variable.symbol.as_str());
            symbols.extend(
                additional_signals(loop_definition)
                    .iter()
                    .filter(|signal| signal.role == AuxiliaryRole::ManipulatedVariable)
                    .map(|signal| signal.symbol.as_str()),
            );
        }
        ChannelDirection::Output => {
            symbols.push(loop_definition.signals.process_variable.symbol.as_str());
            symbols.extend(
                additional_signals(loop_definition)
                    .iter()
                    .filter(|signal| {
                        matches!(
                            signal.role,
                            AuxiliaryRole::ProcessVariable | AuxiliaryRole::State
                        )
                    })
                    .map(|signal| signal.symbol.as_str()),
            );
        }
    }
    symbols
}

/// The range a signal asks for: the range the map names for it, or the settings' own range for the primary signal.
fn requested(
    ranges: Option<&BTreeMap<String, Range>>,
    symbol: &str,
    fallback: Option<Range>,
) -> Option<Range> {
    ranges
        .and_then(|ranges| ranges.get(symbol).copied())
        .or(fallback)
}

/// The range a signal is given: the requested range, or the declared one when none is requested. It must be valid and
/// lie within the declared range.
fn bounded_range(
    requested: Option<Range>,
    declared: Range,
    signal: &str,
) -> Result<Range, AcquisitionError> {
    let range = requested.unwrap_or(declared);
    if !valid_range(range) || range.minimum < declared.minimum || range.maximum > declared.maximum {
        return Err(AcquisitionError::RangeOutsideLimits {
            signal: signal.to_string(),
            minimum: declared.minimum,
            maximum: declared.maximum,
        });
    }
    Ok(range)
}

/// The further signals with their ranges narrowed by the settings where the settings name them.
fn narrowed_additional(
    signals: &[AuxiliarySignal],
    input_ranges: Option<&BTreeMap<String, Range>>,
    output_ranges: Option<&BTreeMap<String, Range>>,
) -> Result<Vec<AuxiliarySignal>, AcquisitionError> {
    let mut narrowed = Vec::with_capacity(signals.len());
    for signal in signals {
        let range = match signal.role {
            AuxiliaryRole::ManipulatedVariable => input_ranges
                .and_then(|ranges| ranges.get(&signal.symbol))
                .copied(),
            AuxiliaryRole::ProcessVariable | AuxiliaryRole::State => output_ranges
                .and_then(|ranges| ranges.get(&signal.symbol))
                .copied(),
            _ => None,
        };
        let mut signal = signal.clone();
        if let Some(requested) = range {
            signal.range = bounded_range(Some(requested), signal.range, &signal.symbol)?;
        }
        narrowed.push(signal);
    }
    Ok(narrowed)
}

/// The experiment the settings give the loop. A configured method keeps the loop's experiment kind, and the loop's own
/// execution, seed, baseline, hold parameter and activation variables are kept; the duration, amplitude and direction
/// come from the settings.
fn experiment_for(settings: &AcquisitionSettings, previous: Option<&Experiment>) -> Experiment {
    Experiment {
        execution: previous.map_or(Execution::ControlStudio, |experiment| experiment.execution),
        kind: experiment_kind(settings.method, previous),
        amplitude: settings.amplitude,
        relay_direction: settings
            .relay_direction
            .or_else(|| previous.and_then(|experiment| experiment.relay_direction)),
        duration_seconds: Some(settings.duration_seconds),
        baseline_seconds: previous.and_then(|experiment| experiment.baseline_seconds),
        hold_parameter: previous.and_then(|experiment| experiment.hold_parameter.clone()),
        seed: previous.map_or(1, |experiment| experiment.seed),
        activation: previous.and_then(|experiment| experiment.activation.clone()),
        abort: previous.and_then(|experiment| experiment.abort.clone()),
    }
}

fn experiment_kind(method: AcquisitionMethod, previous: Option<&Experiment>) -> ExperimentKind {
    match method {
        AcquisitionMethod::Configured => {
            previous.map_or(ExperimentKind::Observation, |experiment| experiment.kind)
        }
        AcquisitionMethod::Automatic => ExperimentKind::Observation,
        AcquisitionMethod::MvStep => ExperimentKind::MvStep,
        AcquisitionMethod::MvPulse => ExperimentKind::MvPulse,
        AcquisitionMethod::SetpointStep => ExperimentKind::SetpointStep,
        AcquisitionMethod::Relay => ExperimentKind::Relay,
        AcquisitionMethod::Prbs => ExperimentKind::Prbs,
        AcquisitionMethod::Multisine => ExperimentKind::Multisine,
    }
}

#[cfg(test)]
#[path = "acquisition_settings_tests.rs"]
mod tests;
