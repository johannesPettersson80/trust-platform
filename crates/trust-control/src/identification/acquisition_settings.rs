//! The acquisition settings of an identification (specification 35, A7 and B3.3): the method, duration, amplitude and
//! ranges the engineer asks for, their checks, and the loop they configure for the experiment (`acquisitionSettings.ts`).
//! A range may narrow a signal's declared range, never widen it. The ranges are kept by signal, one entry for each plant input and
//! measured output (`input_ranges`, `output_ranges`, B3.2); a record of the earlier form holds the range of the manipulated variable
//! and of the measured output as `input_range` and `output_range`, which still read. The multichannel forms (channel bindings) are
//! parked with stage V7 and are not decoded here.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::method_applicability::MethodRefusal;
use crate::identification::texts::{acquisition_text, method_refusal_text};
use crate::json::number_or_text;
use crate::limits::{MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS};
use crate::loop_definition::{
    AuxiliaryRole, AuxiliarySignal, Execution, Experiment, ExperimentKind, LoopDefinition,
    LoopSignal, Range, RelayDirection,
};
use crate::setup::loop_bindings::duration_seconds_of_text;

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

/// A range the settings give: the lowest and the highest value the experiment may use. The page sends each bound as the number or the
/// text the engineer typed (`"20"`), which `number_of_bound` reads. A bound that no finite number reads, blank text included, is
/// NaN, which no JSON number is, so the checks refuse the range under its field (B3.2, `check_settings`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RangeSetting {
    /// The lowest value.
    #[serde(deserialize_with = "number_of_bound")]
    #[schemars(schema_with = "crate::json::number_or_text_schema")]
    pub minimum: f64,
    /// The highest value.
    #[serde(deserialize_with = "number_of_bound")]
    #[schemars(schema_with = "crate::json::number_or_text_schema")]
    pub maximum: f64,
}

impl RangeSetting {
    /// The range as the loop's signals hold it.
    #[must_use]
    pub fn range(self) -> Range {
        Range {
            minimum: self.minimum,
            maximum: self.maximum,
        }
    }
}

/// A bound of a range as the settings give it: a number, or the text the page typed. Anything no finite number reads is NaN.
fn number_of_bound<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(number_or_text(&Value::deserialize(deserializer)?))
}

/// The request for an identification experiment: what it excites, for how long and how far, and the ranges it may use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionSettings {
    /// The method of the experiment.
    pub method: AcquisitionMethod,
    /// The duration of the experiment in seconds, from one second to 24 hours. The page sends the time it typed (`15 min`), which
    /// `duration_of_setting` reads as seconds; a time that no duration reads is refused under the duration's field.
    #[serde(deserialize_with = "duration_of_setting")]
    #[schemars(schema_with = "crate::json::number_or_text_schema")]
    pub duration_seconds: f64,
    /// The amplitude of the experiment; zero asks for the declared one.
    pub amplitude: f64,
    /// The declared signal a setpoint step moves, when the loop has no setpoint of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setpoint_signal_id: Option<String>,
    /// The first direction of a relay experiment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_direction: Option<RelayDirection>,
    /// Whether the experiment may reuse a recording of the same loop. Absent means it may not, which is what every request asks.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reuse_recordings: bool,
    /// The range of the manipulated variable the experiment may use, in the form of a record written before ranges were kept by
    /// signal. It reads as the range `input_ranges` names for the manipulated variable, which takes precedence. The page sends
    /// `input_ranges` alone (B3.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_range: Option<RangeSetting>,
    /// The range of the measured output the experiment may use, in the earlier form of `input_range`: it reads as the range
    /// `output_ranges` names for the process variable, which takes precedence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_range: Option<RangeSetting>,
    /// The range of each plant input the experiment may use, by symbol: at most sixteen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_ranges: Option<BTreeMap<String, RangeSetting>>,
    /// The range of each measured output the experiment may use, by symbol: at most sixteen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_ranges: Option<BTreeMap<String, RangeSetting>>,
}

/// The channel direction of a range: a manipulated variable and its companions, or a measured output and its companions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelDirection {
    /// An input of the plant.
    Input,
    /// A measured output of the plant.
    Output,
}

impl ChannelDirection {
    /// The word of the direction in a sentence and in a field's key: `input` or `output`.
    fn word(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
        }
    }
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
    /// A range leaves the signal's declared limits.
    RangeOutsideLimits {
        /// The signal's symbol.
        signal: String,
        /// The smallest value the signal may be given.
        minimum: f64,
        /// The largest value the signal may be given.
        maximum: f64,
        /// The page's field of the bound the range breaks (`threePageLimits.ts`, see `limit_field`).
        field: String,
    },
    /// A range is not finite, or its minimum is not below its maximum; the page marks such a range at its maximum.
    RangeNotValid {
        /// The signal's symbol.
        signal: String,
        /// The page's field of the maximum (`threePageLimits.ts`, see `limit_field`).
        field: String,
    },
}

impl AcquisitionError {
    /// The failure the engineer sees: its sentence, its cause, the one next action it offers, and for a range the field it breaks.
    #[must_use]
    pub fn failure(&self) -> Failure {
        let next_action = match self {
            Self::LimitsRequired
            | Self::UnknownChannelRange(_)
            | Self::RangeOutsideLimits { .. }
            | Self::RangeNotValid { .. } => NextAction::OpenLimits,
            Self::SetpointReference => NextAction::None,
        };
        let failure = Failure::new(&acquisition_text(self), Cause::Settings, next_action);
        match self {
            Self::RangeOutsideLimits { field, .. } | Self::RangeNotValid { field, .. } => {
                failure.with_field(field)
            }
            // The missing limit is the maximum duration: the Limits editor shows the sentence under that field (B3.2, B8).
            Self::LimitsRequired => failure.with_field(DURATION_FIELD),
            Self::SetpointReference | Self::UnknownChannelRange(_) => failure,
        }
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

/// The sentence of acquisition settings that do not decode as Identify's settings (B3.2).
const NOT_IDENTIFY_SETTINGS: &str = "The acquisition settings are not Identify's settings.";
/// The sentence of a duration outside one second to 24 hours (B3.2, A7).
const DURATION_RULE: &str = "The experiment duration must be between 1 s and 24 h.";
/// The sentence of a duration that no time reads (B3.2); its refusal names the field `DURATION_FIELD`.
const DURATION_TEXT: &str =
    "The experiment duration must be a time such as 90 s, 15 min or 1 h 30 min.";
/// The page's field of the experiment duration, which a refusal names (`data-studio-limit`).
pub(crate) const DURATION_FIELD: &str = "durationSeconds";
/// The sentence of an amplitude that is not finite (B3.2).
const AMPLITUDE_RULE: &str = "The experiment amplitude must be a finite number.";
/// The sentence of a setpoint signal whose identifier is empty or longer than 256 characters (B3.2).
const SETPOINT_RULE: &str = "The setpoint signal needs an identifier of at most 256 characters.";
/// The sentence of a range map that holds more than sixteen ranges (B3.2).
const RANGE_MAP_RULE: &str = "Each map of ranges holds at most sixteen signals.";

/// A duration the settings give: a number of seconds, or the time the page typed (`15 min`, `1 h 30 min`), which
/// `duration_seconds_of_text` reads as the identify form does. Any other value, and a time that no duration reads, is NaN, which no
/// JSON number is: `check_settings` refuses it under the duration's field (`unreadable_duration_failure`).
pub(crate) fn duration_of_setting<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<f64, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(number) => number.as_f64().unwrap_or(f64::NAN),
        Value::String(text) => duration_seconds_of_text(&text).unwrap_or(f64::NAN),
        _ => f64::NAN,
    })
}

/// The failure of an experiment duration that no time reads: its sentence, the settings' cause, the limits to open, and the field it
/// names, so that the page shows the sentence under the duration.
#[must_use]
pub fn unreadable_duration_failure() -> Failure {
    Failure::new(DURATION_TEXT, Cause::Settings, NextAction::OpenLimits).with_field(DURATION_FIELD)
}

/// Decodes acquisition settings from their JSON and returns them only when every check holds (see
/// [`check_acquisition_settings`]).
#[must_use]
pub fn decode_acquisition_settings(value: &Value) -> Option<AcquisitionSettings> {
    check_acquisition_settings(value).ok()
}

/// Decodes acquisition settings from their JSON and checks them, naming the first rule they break. The rules, in order: the duration
/// reads as a time and is between one second and 24 hours; the amplitude is finite; a setpoint signal names a non-empty identifier of at most 256
/// characters; a relay names its direction; each range is finite with a minimum below its maximum; and no range map holds more than
/// sixteen entries. A refused duration or range names its field, and Limits is its next action.
///
/// # Errors
/// The JSON does not decode as acquisition settings, or breaks a rule: the failure's sentence says which (B3.2, `inputs`).
pub fn check_acquisition_settings(value: &Value) -> Result<AcquisitionSettings, Failure> {
    let settings: AcquisitionSettings =
        crate::json::decode_value(value.clone()).map_err(|_| refused(NOT_IDENTIFY_SETTINGS))?;
    check_settings(&settings)?;
    Ok(settings)
}

/// The first rule of [`check_acquisition_settings`] that the settings break, as the failure that names it.
///
/// A range's field is the single-channel form (`inputMaximum`), because this check has no loop. [`range_refusal`] runs first on every
/// path that a page sends settings to, and it names a loop with several channels by its signal (`inputMaximum:P1.MV`). A map of ranges
/// is sent only by a loop with several channels, so its field always names the signal.
///
/// # Errors
/// The failure of the first broken rule.
fn check_settings(settings: &AcquisitionSettings) -> Result<(), Failure> {
    if settings.duration_seconds.is_nan() {
        return Err(unreadable_duration_failure());
    }
    let duration_ok = settings.duration_seconds.is_finite()
        && (milliseconds_as_seconds(MINIMUM_EXPERIMENT_DURATION_MS)
            ..=milliseconds_as_seconds(MAXIMUM_EXPERIMENT_DURATION_MS))
            .contains(&settings.duration_seconds);
    if !duration_ok {
        return Err(range_failure(DURATION_RULE, DURATION_FIELD));
    }
    if !settings.amplitude.is_finite() {
        return Err(refused(AMPLITUDE_RULE));
    }
    if settings
        .setpoint_signal_id
        .as_ref()
        .is_some_and(|id| id.is_empty() || id.encode_utf16().count() > 256)
    {
        return Err(refused(SETPOINT_RULE));
    }
    if settings.method == AcquisitionMethod::Relay && settings.relay_direction.is_none() {
        return Err(refused(method_refusal_text(MethodRefusal::RelayDirection)));
    }
    for (direction, range) in [
        (
            ChannelDirection::Input,
            settings.input_range.map(RangeSetting::range),
        ),
        (
            ChannelDirection::Output,
            settings.output_range.map(RangeSetting::range),
        ),
    ] {
        if range.is_some_and(|range| !valid_range(range)) {
            let sentence = format!(
                "The {} range must be finite, with its minimum below its maximum.",
                direction.word()
            );
            let field = limit_field(direction, Bound::Maximum, None);
            return Err(range_failure(&sentence, &field));
        }
    }
    let maps = [
        (ChannelDirection::Input, settings.input_ranges.as_ref()),
        (ChannelDirection::Output, settings.output_ranges.as_ref()),
    ];
    for (direction, map) in maps {
        let Some(map) = map else { continue };
        if let Some((symbol, _)) = map.iter().find(|(_, range)| !valid_range(range.range())) {
            let sentence = format!(
                "The range of {symbol} must be finite, with its minimum below its maximum."
            );
            let field = limit_field(direction, Bound::Maximum, Some(symbol.as_str()));
            return Err(range_failure(&sentence, &field));
        }
    }
    if maps
        .iter()
        .filter_map(|(_, map)| *map)
        .any(|map| map.len() > 16)
    {
        return Err(refused(RANGE_MAP_RULE));
    }
    Ok(())
}

/// The failure of a settings rule that no field of the page's Limits holds (Identify's settings, B3.2): its sentence, the settings
/// cause, and no next action.
fn refused(sentence: &str) -> Failure {
    Failure::new(sentence, Cause::Settings, NextAction::None)
}

/// The failure of a duration or range that the page's Limits holds (B3.2): its sentence, the settings cause, Limits as the next
/// action, and the field the refusal names.
fn range_failure(sentence: &str, field: &str) -> Failure {
    Failure::new(sentence, Cause::Settings, NextAction::OpenLimits).with_field(field)
}

/// The bound of a range that a field of the page's Limits holds (`threePageLimits.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bound {
    /// The field `Minimum`.
    Minimum,
    /// The field `Maximum`.
    Maximum,
}

/// The page's key of one range field (`threePageLimits.ts`, `fieldKey`): `inputMinimum`, `outputMaximum` and so on. The field of a
/// signal carries the signal's symbol after the bound (`inputMinimum:P1.MV`) when the loop has several channels, so `signal` is given
/// then and `None` otherwise.
fn limit_field(direction: ChannelDirection, bound: Bound, signal: Option<&str>) -> String {
    let name = match bound {
        Bound::Minimum => "Minimum",
        Bound::Maximum => "Maximum",
    };
    match signal {
        Some(symbol) => format!("{}{name}:{symbol}", direction.word()),
        None => format!("{}{name}", direction.word()),
    }
}

/// Whether the loop has several input or several output channels. The page then names each range field after its signal, as the
/// Limits editor does (`threePageLimits.ts`, `multichannel`).
fn several_channels(loop_definition: &LoopDefinition) -> bool {
    loop_definition
        .channels
        .as_ref()
        .is_some_and(|channels| channels.inputs.len() > 1 || channels.outputs.len() > 1)
}

/// The refusal of a range that the settings give beyond the loop's declared limits (B3.2, B3.4 step 1), as the failure that names the
/// field of Limits it breaks, with the signal named as the loop's channels call for. The checks of the settings that need the loop
/// run it first, so that the page shows the refusal under that field.
#[must_use]
pub fn range_refusal(
    loop_definition: &LoopDefinition,
    settings: &AcquisitionSettings,
) -> Option<Failure> {
    configured_acquisition(loop_definition, Some(settings))
        .err()
        .filter(|error| {
            matches!(
                error,
                AcquisitionError::RangeOutsideLimits { .. }
                    | AcquisitionError::RangeNotValid { .. }
            )
        })
        .map(|error| error.failure())
}

/// A limit in milliseconds as seconds. The limits are far below the range where a float loses integer precision.
#[allow(clippy::cast_precision_loss)]
fn milliseconds_as_seconds(milliseconds: u64) -> f64 {
    milliseconds as f64 / 1_000.0
}

fn valid_range(range: Range) -> bool {
    range.minimum.is_finite() && range.maximum.is_finite() && range.minimum < range.maximum
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
    let requested_inputs = settings.input_ranges.as_ref().map(plain_ranges);
    let requested_outputs = settings.output_ranges.as_ref().map(plain_ranges);
    let input_ranges = channel_ranges(
        loop_definition,
        requested_inputs.as_ref(),
        ChannelDirection::Input,
    )?;
    let output_ranges = channel_ranges(
        loop_definition,
        requested_outputs.as_ref(),
        ChannelDirection::Output,
    )?;
    let several = several_channels(loop_definition);
    let input_range = bounded_range(
        requested(
            input_ranges,
            &input.symbol,
            settings.input_range.map(RangeSetting::range),
        ),
        input_bounds,
        &input.symbol,
        ChannelDirection::Input,
        several,
    )?;
    let output_range = bounded_range(
        requested(
            output_ranges,
            &output.symbol,
            settings.output_range.map(RangeSetting::range),
        ),
        output.range,
        &output.symbol,
        ChannelDirection::Output,
        several,
    )?;
    let additional = match &loop_definition.signals.additional {
        Some(signals) => Some(narrowed_additional(
            signals,
            input_ranges,
            output_ranges,
            several,
        )?),
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

/// The declared further signal the settings name as the setpoint, when they name one for a method that moves a setpoint. Only the
/// setpoint step uses the signal (B3.3, B3.6): the page sends the one the engineer chose whatever the method, and another method
/// runs as if none were named, so that a choice made for a setpoint step never changes what Automatic or a relay does. It must be a
/// setpoint of the loop's further signals, in the measured output's unit, and a setpoint of the loop itself must have the same symbol.
fn reference_signal(
    loop_definition: &LoopDefinition,
    settings: &AcquisitionSettings,
) -> Result<Option<LoopSignal>, AcquisitionError> {
    let Some(id) = settings
        .setpoint_signal_id
        .as_deref()
        .filter(|id| !id.is_empty() && settings.method == AcquisitionMethod::SetpointStep)
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

/// The ranges of a map of settings as the loop's signals hold them.
fn plain_ranges(ranges: &BTreeMap<String, RangeSetting>) -> BTreeMap<String, Range> {
    ranges
        .iter()
        .map(|(symbol, range)| (symbol.clone(), range.range()))
        .collect()
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
/// lie within the declared range. The refusal names the bound it breaks: a range that is not valid, or whose maximum is beyond the
/// declared maximum, names the maximum; one whose minimum is below the declared minimum names the minimum.
fn bounded_range(
    requested: Option<Range>,
    declared: Range,
    signal: &str,
    direction: ChannelDirection,
    several: bool,
) -> Result<Range, AcquisitionError> {
    let range = requested.unwrap_or(declared);
    let field = |bound| limit_field(direction, bound, several.then_some(signal));
    if !valid_range(range) {
        return Err(AcquisitionError::RangeNotValid {
            signal: signal.to_string(),
            field: field(Bound::Maximum),
        });
    }
    if range.minimum >= declared.minimum && range.maximum <= declared.maximum {
        return Ok(range);
    }
    let bound = if range.minimum < declared.minimum {
        Bound::Minimum
    } else {
        Bound::Maximum
    };
    Err(AcquisitionError::RangeOutsideLimits {
        signal: signal.to_string(),
        minimum: declared.minimum,
        maximum: declared.maximum,
        field: field(bound),
    })
}

/// The further signals with their ranges narrowed by the settings where the settings name them.
fn narrowed_additional(
    signals: &[AuxiliarySignal],
    input_ranges: Option<&BTreeMap<String, Range>>,
    output_ranges: Option<&BTreeMap<String, Range>>,
    several: bool,
) -> Result<Vec<AuxiliarySignal>, AcquisitionError> {
    let mut narrowed = Vec::with_capacity(signals.len());
    for signal in signals {
        let request = match signal.role {
            AuxiliaryRole::ManipulatedVariable => input_ranges
                .and_then(|ranges| ranges.get(&signal.symbol))
                .map(|range| (ChannelDirection::Input, *range)),
            AuxiliaryRole::ProcessVariable | AuxiliaryRole::State => output_ranges
                .and_then(|ranges| ranges.get(&signal.symbol))
                .map(|range| (ChannelDirection::Output, *range)),
            _ => None,
        };
        let mut signal = signal.clone();
        if let Some((direction, requested)) = request {
            signal.range = bounded_range(
                Some(requested),
                signal.range,
                &signal.symbol,
                direction,
                several,
            )?;
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
