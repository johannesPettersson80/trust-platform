//! The limits of an identification experiment (specification 35, B3.2): the range each plant input and measured output may use and
//! the maximum duration. The loop declares the defaults (`experiment_limits`); the Limits editor starts from the limits the open
//! answer gives, which are those defaults with the range the study holds (`controlStudio/inputs`) over them
//! (`held_experiment_limits`).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Serialize;

use crate::identification::acquisition_settings::{AcquisitionSettings, RangeSetting};
use crate::loop_definition::{channel_signal, ChannelSignal, LoopDefinition, RelayDirection};
use crate::setup::duration_words::duration_words;

/// The range a signal may use in an identification experiment: its symbol, which names its field of a multichannel loop, its unit, and
/// its smallest and largest value.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentRange {
    /// The signal's symbol.
    pub symbol: String,
    /// The signal's unit.
    pub unit: String,
    /// The smallest value the experiment may use.
    pub minimum: f64,
    /// The largest value the experiment may use.
    pub maximum: f64,
}

/// The limits of the loop's identification experiment (B3.2): every plant input and measured output with its range, and the maximum
/// duration, as the Limits editor shows them. The experiment's own defaults that a project test's review and the acquisition settings
/// start from (B3.3, C12) are the loop's declared ones.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentLimits {
    /// The maximum duration the loop declares, in seconds; absent when the loop declares none, and the Limits editor then asks for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    /// The maximum duration in the engine's words (`duration_words`, B9.9), such as `90 s` or `1 h 30 min`: the Limits editor shows the
    /// text as it is and sends back what the engineer typed. The words read back as exactly `duration_seconds`, so a duration that the
    /// engineer leaves alone is never changed by the way it is shown. Present exactly when `duration_seconds` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_words: Option<String>,
    /// The range of each plant input, in the loop's channel order; the manipulated variable alone for a loop without channels.
    pub inputs: Vec<ExperimentRange>,
    /// The range of each measured output, in the loop's channel order; the process variable alone for a loop without channels.
    pub outputs: Vec<ExperimentRange>,
    /// The amplitude the loop declares for the experiment; zero asks for the declared one (B3.2).
    pub amplitude: f64,
    /// The first direction of a relay the loop declares (B3.3); absent when it declares none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_direction: Option<RelayDirection>,
    /// The duration a project test's review asks for, in seconds: the loop's declared duration, or zero when it declares none, which the
    /// engine refuses under Limits (C12).
    pub review_duration_seconds: f64,
}

/// The default limits of the loop's identification experiment (B3.2). A plant input starts from its signal's declared range, narrowed by
/// its channel's actuator range and, for the manipulated variable, by the output limits the loop declares. A measured output keeps its
/// declared range. A limit the loop does not declare narrows nothing.
#[must_use]
pub fn experiment_limits(loop_definition: &LoopDefinition) -> ExperimentLimits {
    let (inputs, outputs) = match &loop_definition.channels {
        Some(channels) => (
            channels
                .inputs
                .iter()
                .filter_map(|input| channel_signal(loop_definition, &input.signal_id))
                .collect::<Vec<_>>(),
            channels
                .outputs
                .iter()
                .filter_map(|output| channel_signal(loop_definition, &output.signal_id))
                .collect::<Vec<_>>(),
        ),
        None => (
            channel_signal(loop_definition, "manipulatedVariable")
                .into_iter()
                .collect(),
            channel_signal(loop_definition, "processVariable")
                .into_iter()
                .collect(),
        ),
    };
    let experiment = loop_definition.experiment.as_ref();
    let duration_seconds = experiment.and_then(|experiment| experiment.duration_seconds);
    ExperimentLimits {
        duration_seconds,
        duration_words: duration_seconds.map(duration_words),
        amplitude: experiment.map_or(0.0, |experiment| experiment.amplitude),
        relay_direction: experiment.and_then(|experiment| experiment.relay_direction),
        review_duration_seconds: experiment
            .and_then(|experiment| experiment.duration_seconds)
            .unwrap_or(0.0),
        inputs: inputs
            .iter()
            .map(|signal| input_range(loop_definition, signal))
            .collect(),
        outputs: outputs.iter().map(output_range).collect(),
    }
}

/// The range of one plant input: its declared range, narrowed by its channel's actuator range and, when it is the manipulated variable,
/// by the loop's output limits.
fn input_range(loop_definition: &LoopDefinition, signal: &ChannelSignal<'_>) -> ExperimentRange {
    let actuator = loop_definition
        .channels
        .as_ref()
        .and_then(|channels| {
            channels.inputs.iter().find(|input| {
                channel_signal(loop_definition, &input.signal_id)
                    .is_some_and(|candidate| candidate.symbol == signal.symbol)
            })
        })
        .map(|input| &input.actuator);
    let (output_minimum, output_maximum) =
        if signal.symbol == loop_definition.signals.manipulated_variable.symbol {
            (
                loop_definition.limits.output_minimum,
                loop_definition.limits.output_maximum,
            )
        } else {
            (None, None)
        };
    let minimum = actuator.map_or(signal.range.minimum, |actuator| {
        signal.range.minimum.max(actuator.minimum)
    });
    let minimum = output_minimum.map_or(minimum, |limit| minimum.max(limit));
    let maximum = actuator.map_or(signal.range.maximum, |actuator| {
        signal.range.maximum.min(actuator.maximum)
    });
    let maximum = output_maximum.map_or(maximum, |limit| maximum.min(limit));
    ExperimentRange {
        symbol: signal.symbol.to_owned(),
        unit: signal.unit.to_owned(),
        minimum,
        maximum,
    }
}

/// The range of one measured output: its declared range.
fn output_range(signal: &ChannelSignal<'_>) -> ExperimentRange {
    ExperimentRange {
        symbol: signal.symbol.to_owned(),
        unit: signal.unit.to_owned(),
        minimum: signal.range.minimum,
        maximum: signal.range.maximum,
    }
}

/// The limits the Limits editor shows (B3.2, A8.6): the loop's declared defaults ([`experiment_limits`]) with the study's own record
/// over them, so that the page merges nothing. Every plant input and measured output is listed, in channel order. The record's
/// duration replaces the declared one, and each range the record names replaces the range of the signal it names. A range is named by
/// its signal's symbol (`input_ranges`, `output_ranges`); a record of the earlier form holds the range of the manipulated variable
/// and of the measured output as `input_range` and `output_range`, which still read, a range named by symbol taking precedence. The
/// amplitude, the relay direction and the review's duration stay the loop's declared ones: the page carries them and does not edit
/// them.
#[must_use]
pub fn held_experiment_limits(
    loop_definition: &LoopDefinition,
    record: Option<&AcquisitionSettings>,
) -> ExperimentLimits {
    let mut limits = experiment_limits(loop_definition);
    let Some(record) = record else {
        return limits;
    };
    if record.duration_seconds.is_finite() {
        limits.duration_seconds = Some(record.duration_seconds);
        limits.duration_words = Some(duration_words(record.duration_seconds));
    }
    held_ranges(
        &mut limits.inputs,
        record.input_ranges.as_ref(),
        record.input_range,
        &loop_definition.signals.manipulated_variable.symbol,
    );
    held_ranges(
        &mut limits.outputs,
        record.output_ranges.as_ref(),
        record.output_range,
        &loop_definition.signals.process_variable.symbol,
    );
    limits
}

/// Gives each listed signal the range the record names for it: the entry of `by_symbol`, else, for the primary signal, the range of
/// the earlier form. A bound that is not a finite number leaves the declared range as it is.
fn held_ranges(
    listed: &mut [ExperimentRange],
    by_symbol: Option<&BTreeMap<String, RangeSetting>>,
    earlier_form: Option<RangeSetting>,
    primary_symbol: &str,
) {
    for range in listed {
        let named = by_symbol
            .and_then(|ranges| ranges.get(&range.symbol))
            .copied()
            .or(if range.symbol == primary_symbol {
                earlier_form
            } else {
                None
            });
        if let Some(named) =
            named.filter(|named| named.minimum.is_finite() && named.maximum.is_finite())
        {
            range.minimum = named.minimum;
            range.maximum = named.maximum;
        }
    }
}

#[cfg(test)]
#[path = "experiment_limits_tests.rs"]
mod tests;
