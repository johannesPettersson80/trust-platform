//! The traces of an identification experiment (`identificationExperiment.ts`): the recorded values of the plant inputs and
//! of the measured outputs at each sample, with the unit of each signal. A stored model comparison keeps them, so their
//! construction and their decoding are rules here; the page only draws them (specification 35, A10 and B3).
//!
//! The traces follow the recorded coordinates. A value that is not good or not finite is missing (`null`). A recording
//! is never resampled or resimulated for display.
//!
//! The loop's own channel list (`loop.channels`, V7) is not read: the traces use the loop's manipulated variable and its
//! measured output, or the signals the caller selects.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use trust_control_math::identify::SampleQuality;

use crate::identification::connected_record::ValidationRun;
use crate::identification::data::IdentificationSample;
use crate::identification::refusal::Refusal;
use crate::loop_definition::LoopDefinition;
use crate::recording::channels::ChannelQuality;

/// The most traces one list of inputs or outputs may hold (`traces.length > 32`).
const MAXIMUM_TRACES: usize = 32;

/// The most points one trace may hold (`trace.points.length > 20000`).
const MAXIMUM_POINTS: usize = 20_000;

/// One recorded value of a trace: its time, and the value when it is good and finite.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracePoint {
    /// The sample's time in seconds.
    pub time_seconds: f64,
    /// The value, or `None` when the sample has no good value for the signal.
    pub value: Option<f64>,
}

/// One signal of the experiment over time (`ExperimentTrace`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentTrace {
    /// The signal's identifier: its symbol in the recording.
    pub id: String,
    /// The signal's unit.
    pub unit: String,
    /// The values at each sample, in time order.
    pub points: Vec<TracePoint>,
}

/// The traces of an identification experiment (`IdentificationExperiment`): the plant inputs and the measured outputs,
/// and the recording they come from when there is one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentificationExperiment {
    /// The identifier of the recording the traces were read from, when they were read from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_id: Option<String>,
    /// The plant inputs.
    pub inputs: Vec<ExperimentTrace>,
    /// The measured outputs.
    pub outputs: Vec<ExperimentTrace>,
}

/// A signal to trace: its identifier and its unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentSignal {
    /// The signal's identifier: its symbol in the recording.
    pub id: String,
    /// The signal's unit.
    pub unit: String,
}

/// The signals a set of traces shows: the plant inputs and the measured outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentSignals {
    /// The plant inputs.
    pub inputs: Vec<ExperimentSignal>,
    /// The measured outputs.
    pub outputs: Vec<ExperimentSignal>,
}

/// The traces of a recording (`experimentFromRun`). Without `selected`, the traces are the loop's manipulated variable and
/// its measured output.
///
/// # Errors
/// A traced signal is not in the recording with the same unit.
pub fn experiment_from_run(
    loop_definition: &LoopDefinition,
    run: &ValidationRun,
    selected: Option<&ExperimentSignals>,
) -> Result<IdentificationExperiment, Refusal> {
    let loop_signals = ExperimentSignals {
        inputs: vec![signal_of(
            &loop_definition.signals.manipulated_variable.symbol,
            &loop_definition.signals.manipulated_variable.unit,
        )],
        outputs: vec![signal_of(
            &loop_definition.signals.process_variable.symbol,
            &loop_definition.signals.process_variable.unit,
        )],
    };
    let signals = selected.unwrap_or(&loop_signals);
    Ok(IdentificationExperiment {
        recording_id: Some(run.id.clone()),
        inputs: traces_of_run(run, &signals.inputs)?,
        outputs: traces_of_run(run, &signals.outputs)?,
    })
}

/// The traces of a list of identification samples (`experimentFromSamples`): the plant input and the measured output of
/// the loop, from samples that are good. A sample that is not good or not finite is missing.
#[must_use]
pub fn experiment_from_samples(
    loop_definition: &LoopDefinition,
    samples: &[IdentificationSample],
) -> IdentificationExperiment {
    let trace =
        |id: &str, unit: &str, value_of: fn(&IdentificationSample) -> f64| ExperimentTrace {
            id: id.to_owned(),
            unit: unit.to_owned(),
            points: samples
                .iter()
                .map(|sample| {
                    let value = value_of(sample);
                    TracePoint {
                        time_seconds: sample.time_seconds,
                        value: (sample.quality == SampleQuality::Good && value.is_finite())
                            .then_some(value),
                    }
                })
                .collect(),
        };
    let input = &loop_definition.signals.manipulated_variable;
    let output = &loop_definition.signals.process_variable;
    IdentificationExperiment {
        recording_id: None,
        inputs: vec![trace(&input.symbol, &input.unit, |sample| sample.input)],
        outputs: vec![trace(&output.symbol, &output.unit, |sample| sample.output)],
    }
}

/// Decodes stored traces and returns them only when they are well formed (`decodeIdentificationExperiment`): an identifier
/// and a unit on every trace, one to 32 traces per list with distinct identifiers, at most 20,000 points, and times that
/// are finite, not negative and strictly increasing.
#[must_use]
pub fn decode_identification_experiment(value: &Value) -> Option<IdentificationExperiment> {
    let experiment: IdentificationExperiment = crate::json::decode_value(value.clone()).ok()?;
    if experiment.recording_id.as_deref() == Some("") {
        return None;
    }
    (traces_valid(&experiment.inputs) && traces_valid(&experiment.outputs)).then_some(experiment)
}

fn signal_of(symbol: &str, unit: &str) -> ExperimentSignal {
    ExperimentSignal {
        id: symbol.to_owned(),
        unit: unit.to_owned(),
    }
}

/// The traces of the signals in one run. A signal the run does not hold with the same unit is refused.
fn traces_of_run(
    run: &ValidationRun,
    signals: &[ExperimentSignal],
) -> Result<Vec<ExperimentTrace>, Refusal> {
    signals
        .iter()
        .map(|signal| {
            let held = run
                .channels
                .iter()
                .any(|channel| channel.id == signal.id && channel.unit == signal.unit);
            if !held {
                return Err(Refusal::new(
                    "The experiment recording does not contain its selected signal and unit.",
                ));
            }
            Ok(ExperimentTrace {
                id: signal.id.clone(),
                unit: signal.unit.clone(),
                points: run
                    .samples
                    .iter()
                    .map(|sample| TracePoint {
                        time_seconds: sample.time_seconds,
                        value: sample
                            .values
                            .get(&signal.id)
                            .filter(|reading| {
                                matches!(reading.quality, ChannelQuality::Good)
                                    && reading.value.is_some_and(f64::is_finite)
                            })
                            .and_then(|reading| reading.value),
                    })
                    .collect(),
            })
        })
        .collect()
}

/// Whether one list of traces is well formed: one to 32 traces with distinct identifiers, each with an identifier, a unit
/// and well-formed points.
fn traces_valid(traces: &[ExperimentTrace]) -> bool {
    if traces.is_empty() || traces.len() > MAXIMUM_TRACES {
        return false;
    }
    let identifiers: HashSet<&str> = traces.iter().map(|trace| trace.id.as_str()).collect();
    identifiers.len() == traces.len()
        && traces.iter().all(|trace| {
            !trace.id.is_empty()
                && !trace.unit.is_empty()
                && trace.points.len() <= MAXIMUM_POINTS
                && points_valid(&trace.points)
        })
}

/// Whether the points of a trace have finite, non-negative and strictly increasing times and finite or missing values.
fn points_valid(points: &[TracePoint]) -> bool {
    let mut previous = f64::NEG_INFINITY;
    points.iter().all(|point| {
        let valid = point.time_seconds.is_finite()
            && point.time_seconds >= 0.0
            && point.time_seconds > previous
            && point.value.is_none_or(f64::is_finite);
        previous = point.time_seconds;
        valid
    })
}

#[cfg(test)]
#[path = "experiment_trace_tests.rs"]
mod tests;
