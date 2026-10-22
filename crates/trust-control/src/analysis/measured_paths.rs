//! The measured MIMO paths of a retained recording (`measuredPaths`; specification 35, C7 "Measured paths"; numerical annex N4.5).
//!
//! The request gives one settled trial for each input: the input's baseline window, before its step, and its settled window, after
//! it. The means of every input and every output over each window give the trial's changes. The least-squares fit of those changes is
//! the steady-state gain `K`, and `K` gives the relative gain array `Λ = K ∘ (K⁻¹)ᵀ`, the static decoupler `D = K⁻¹·diag(K)` and the
//! pairing that `Λ` names. The matrix rules are `trust_control_math::multichannel`'s; the window rules and their sentences are here.
//! The rules are the TypeScript analysis's (`threePageMimoAnalysis.ts`, `mimoPairing.ts`, `numerics/mimoRobustAdaptive.ts`) and its
//! page checks (`webview/threePageMimoAnalysis.ts`), except where the answer names a difference: a pairing is named only among those
//! whose diagonal relative gains are all positive (N4.5), and the answer names no pairing that the TypeScript would have refused.

#![allow(clippy::module_name_repetitions)] // The result keeps the name of its tool.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use trust_control_math::linalg::{DenseMatrix, NumericalTolerance};
use trust_control_math::multichannel;

use crate::analysis::budget::Budget;
use crate::analysis::tool::input_failure;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::messages::AnalysisInputs;
use crate::recording::channels::{
    applied_mv, binding_value, process_value, sample_quality, ChannelQuality, ACTUATOR_BINDING,
    MANIPULATED_VARIABLE_BINDING, PROCESS_VARIABLE_BINDING, SETPOINT_BINDING,
};
use crate::recording::samples::seconds_between;
use crate::recording::{RecordedRun, RecordedSample};
use crate::signals::Quality;

/// The most input/output pairs one analysis compares: the pairing is found by enumeration, so the size is bounded (N4.5).
const MAXIMUM_PAIRS: usize = 8;

/// The share of an input step that a settled window may hold. A window that spans more is across the step (C7).
const STRADDLE_FRACTION: f64 = 0.01;

/// The tolerance of the gain fit and its matrices: the tolerance the TypeScript analysis used (`threePageMimoAnalysis.ts:16`).
const TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-12,
    relative: 1e-10,
    condition_limit: 1e12,
};

const SQUARE: &str = "Declare as many bound inputs as outputs.";
const PAIR_LIMIT: &str = "At most eight input/output pairs can be compared.";
const ONE_TRIAL: &str = "Enter one baseline and one settled window for each input.";
const NO_SAMPLES: &str = "The selected recording has no samples.";
const NO_RECORDING_KEPT: &str = "The study keeps no recording to measure.";
const QUALITY: &str = "A window lies outside the recording or has no good sample of every channel.";
const DRIFT: &str = "An input that was not stepped moved during another input's trial.";
const STRADDLE: &str = "A window contains the input step. Place the baseline before the step and the settled window after it.";
const POOR_EXCITATION: &str =
    "The input steps do not excite the inputs independently, so the static gain cannot be fitted.";
const SINGULAR: &str =
    "The gain matrix is singular or ill-conditioned, so its relative gain array and decoupler are unavailable.";
const NOT_PAIRED: &str =
    "No pairing has a positive relative gain on every diagonal element, so no pairing is named.";
const UNKNOWN_SIGNAL: &str = "A channel names a signal that the loop does not declare.";
const WINDOWS: &str =
    "Enter finite, ordered baseline and settled windows for every declared input.";
const DRIFT_TOLERANCE: &str = "Enter a finite nonnegative drift tolerance or leave it blank.";
const NO_RECORDING: &str = "recordingId must name the recording of the study.";
const NO_REVISION: &str = "recordingRevision must be the revision of that recording.";

/// One channel of the analysis: its identifier in the model, and the symbol and unit of the loop signal it reads. Every matrix keeps
/// the names and the units of its channels (N4.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PathChannel {
    /// The channel's identifier in the model.
    pub id: String,
    /// The symbol of the loop signal the channel reads.
    pub symbol: String,
    /// The unit of that signal.
    pub unit: String,
}

/// A time window in seconds from the first sample of the recording. Both ends belong to the window. An end that the request leaves out
/// is `None`: the request checks refuse the trial that holds it, and name the missing window (C7).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimeWindow {
    /// The start of the window, in seconds; absent when the request leaves it out. The page sends the text the engineer typed, which
    /// `number_of_entry` reads; text that is no number is refused under the window's field.
    #[serde(
        default,
        deserialize_with = "number_of_entry",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(schema_with = "crate::json::optional_number_or_text_schema")]
    pub start_seconds: Option<f64>,
    /// The end of the window, in seconds; absent when the request leaves it out. It is read as `start_seconds` is.
    #[serde(
        default,
        deserialize_with = "number_of_entry",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(schema_with = "crate::json::optional_number_or_text_schema")]
    pub end_seconds: Option<f64>,
}

/// A window end or a drift tolerance as the request sends it: a number, or the text the engineer typed. A blank entry is absent (`None`).
/// An entry that is no finite number is kept as `NaN`, which no JSON number is, so the checks refuse it under its field (C7), as a
/// duration that no time reads is refused (`duration_of_setting`).
pub(crate) fn number_of_entry<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f64>, D::Error> {
    Ok(match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => None,
        Some(Value::Number(number)) => Some(
            number
                .as_f64()
                .filter(|value| value.is_finite())
                .unwrap_or(f64::NAN),
        ),
        Some(Value::String(text)) if text.trim().is_empty() => None,
        Some(Value::String(text)) => Some(
            text.trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .unwrap_or(f64::NAN),
        ),
        Some(_) => Some(f64::NAN),
    })
}

/// One settled trial (C7): the input stepped, its baseline window (before the step) and its settled window (after the output has
/// settled).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettledTrial {
    /// The identifier of the stepped input in the model.
    pub input_id: String,
    /// The window before the step.
    pub baseline: TimeWindow,
    /// The window after the step.
    pub settled: TimeWindow,
}

/// One pair of a pairing: an output, the input it is paired with, and the relative gain of the pair.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PathPair {
    /// The output of the pair.
    pub output_id: String,
    /// The input of the pair.
    pub input_id: String,
    /// The pair's diagonal element of the relative gain array.
    pub relative_gain: f64,
}

/// Whether a pairing is named (N4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PairingStatus {
    /// The pairing whose diagonal relative gains are all positive and closest to one.
    Named,
    /// No pairing has all its diagonal relative gains positive, so no pairing is recommended.
    Refused,
}

/// The pairing that the relative gain array names (N4.5): its pairs in output order when it is named, and the sentence of its refusal
/// when it is not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PathPairing {
    /// Whether the pairing is named.
    pub status: PairingStatus,
    /// The named pairs, one for each output; empty when the pairing is refused.
    pub pairs: Vec<PathPair>,
    /// The sentence of a refused pairing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The answer of `measuredPaths` (C7). `gainMatrix` and `relativeGainArray` have one row per output and one column per input.
/// `decoupler` (`D = K⁻¹·diag(K)`) has one row per input and one column per output. The trials are in input order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeasuredPathsResult {
    /// The recording's identifier in the study.
    pub recording_id: String,
    /// The revision of the recording.
    pub recording_revision: String,
    /// The inputs, in the model's order.
    pub inputs: Vec<PathChannel>,
    /// The outputs, in the model's order.
    pub outputs: Vec<PathChannel>,
    /// The trials that the fit used, one for each input.
    pub trials: Vec<SettledTrial>,
    /// The samples between each trial's baseline start and its settled end, each counted once.
    pub sample_count: usize,
    /// The steady-state gain `K`: one row per output, one column per input.
    pub gain_matrix: DenseMatrix,
    /// The relative gain array `Λ = K ∘ (K⁻¹)ᵀ`.
    pub relative_gain_array: DenseMatrix,
    /// The static decoupler `D = K⁻¹·diag(K)`: one row per input, one column per output.
    pub decoupler: DenseMatrix,
    /// The condition number `σ_max / σ_min` of `K`.
    pub gain_condition_number: f64,
    /// The numerical rank of the input steps.
    pub input_rank: usize,
    /// The condition number of the input steps.
    pub input_condition_number: f64,
    /// The norm of the fit's residuals over every output.
    pub residual_norm: f64,
    /// The pairing of the relative gain array.
    pub pairing: PathPairing,
    /// The sentences of the result area.
    pub summary: Vec<String>,
    /// The sentences of the details disclosure.
    pub details: Vec<String>,
}

/// The checks of the request that need no loop: each trial gives all four window ends, each window is finite and ordered, and the drift
/// tolerance, when given, is finite and nonnegative (the page's checks, `webview/threePageMimoAnalysis.ts:64,68`, which the engine now
/// makes).
///
/// # Errors
/// Returns the input failure of the first trial that leaves a window end out, then of the first window or tolerance that breaks its rule.
pub fn checked_request(inputs: &AnalysisInputs) -> Result<(), Failure> {
    if let Some(failure) = inputs
        .settled_trials
        .iter()
        .flatten()
        .find_map(incomplete_trial)
    {
        return Err(failure);
    }
    if let Some(failure) = inputs
        .settled_trials
        .iter()
        .flatten()
        .find_map(unreadable_trial)
    {
        return Err(failure);
    }
    let windows_ordered = inputs.settled_trials.iter().flatten().all(|trial| {
        ordered_ends(trial.baseline).is_some() && ordered_ends(trial.settled).is_some()
    });
    if !windows_ordered {
        return Err(input_failure(WINDOWS));
    }
    match inputs.maximum_unexcited_input_drift {
        Some(limit) if !(limit.is_finite() && limit >= 0.0) => Err(input_failure(DRIFT_TOLERANCE)),
        _ => Ok(()),
    }
}

/// The refusal of a trial that leaves a window end out (C7). Its sentence names the input and the missing ends, in the page's order, and
/// its field is the page's control of the first missing end: `<inputId>:<window key>`, such as `heater-1:baseline-end`.
fn incomplete_trial(trial: &SettledTrial) -> Option<Failure> {
    let ends = [
        (
            "baseline-start",
            "baseline start",
            trial.baseline.start_seconds,
        ),
        ("baseline-end", "baseline end", trial.baseline.end_seconds),
        (
            "settled-start",
            "settled start",
            trial.settled.start_seconds,
        ),
        ("settled-end", "settled end", trial.settled.end_seconds),
    ];
    let missing: Vec<(&str, &str)> = ends
        .iter()
        .filter(|(_, _, value)| value.is_none())
        .map(|(key, name, _)| (*key, *name))
        .collect();
    let (first_key, _) = *missing.first()?;
    let names: Vec<&str> = missing.iter().map(|(_, name)| *name).collect();
    let text = format!(
        "Enter the {} of input {}.",
        join_words(&names),
        trial.input_id
    );
    Some(input_failure(&text).with_field(&format!("{}:{first_key}", trial.input_id)))
}

/// The refusal of the first window end that the request sent as text no number reads (C7). Its sentence names the input and the window end,
/// and its field is the window's control, `<inputId>:<window key>`, as the field of a missing end is.
fn unreadable_trial(trial: &SettledTrial) -> Option<Failure> {
    let ends = [
        (
            "baseline-start",
            "baseline start",
            trial.baseline.start_seconds,
        ),
        ("baseline-end", "baseline end", trial.baseline.end_seconds),
        (
            "settled-start",
            "settled start",
            trial.settled.start_seconds,
        ),
        ("settled-end", "settled end", trial.settled.end_seconds),
    ];
    let (key, name, _) = ends
        .into_iter()
        .find(|(_, _, value)| matches!(value, Some(number) if !number.is_finite()))?;
    let text = format!(
        "The {name} of input {} is not a number of seconds.",
        trial.input_id
    );
    Some(input_failure(&text).with_field(&format!("{}:{key}", trial.input_id)))
}

/// The words as a list: `a`, `a and b`, or `a, b and c`.
fn join_words(words: &[&str]) -> String {
    match words {
        [] => String::new(),
        [only] => (*only).to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The recording that the study keeps last, when a request names none: the answer names it (§0.1a, C7).
///
/// # Errors
/// The study keeps no recording, or the last one cannot be read.
pub fn last_recording(
    recordings: &[String],
    read: impl FnOnce(&str) -> Result<RecordedRun, Failure>,
) -> Result<(String, RecordedRun), Failure> {
    let id = recordings
        .last()
        .ok_or_else(|| refusal(NO_RECORDING_KEPT, NextAction::Identify))?;
    let run = read(id)?;
    Ok((id.clone(), run))
}

/// The measured MIMO paths of a retained recording (`measuredPaths`, C7; N4.5).
///
/// `recording` is the run the study keeps under `recording_id`, and `loop_definition` gives its channels and signals. The request's
/// settled trials give one window pair for each input. `budget` is checked before the windows are read and before the fit (A7).
///
/// # Errors
/// The request names another recording or revision; the loop's channels are not square with two or more inputs, or are more than
/// eight; the trials do not give one pair per input; the recording has no sample; a window is outside the recording, has no good sample
/// of every channel, holds an input step, or moves an input that was not stepped past the drift tolerance; the input steps do not excite
/// the inputs independently; or the gain matrix is singular or ill-conditioned. Each refusal is a failure with its sentence.
pub fn measured_paths_answer(
    recording_id: &str,
    recording: &RecordedRun,
    loop_definition: &LoopDefinition,
    inputs: &AnalysisInputs,
    budget: &Budget<'_>,
) -> Result<MeasuredPathsResult, Failure> {
    budget.step()?;
    check_recording(recording_id, recording, inputs)?;
    let channels = loop_definition
        .channels
        .as_ref()
        .filter(|channels| {
            channels.inputs.len() >= 2 && channels.inputs.len() == channels.outputs.len()
        })
        .ok_or_else(|| refusal(SQUARE, NextAction::OpenChannels))?;
    if channels.inputs.len() > MAXIMUM_PAIRS {
        return Err(refusal(PAIR_LIMIT, NextAction::None));
    }
    let input_ids: Vec<&str> = channels
        .inputs
        .iter()
        .map(|channel| channel.model_channel_id.as_str())
        .collect();
    let output_ids: Vec<&str> = channels
        .outputs
        .iter()
        .map(|channel| channel.model_channel_id.as_str())
        .collect();
    let trials =
        trials_in_input_order(inputs.settled_trials.as_deref().unwrap_or(&[]), &input_ids)?;
    let input_signals: Vec<&str> = channels
        .inputs
        .iter()
        .map(|channel| {
            channel
                .applied_signal_id
                .as_deref()
                .unwrap_or(channel.signal_id.as_str())
        })
        .collect();
    let output_signals: Vec<&str> = channels
        .outputs
        .iter()
        .map(|channel| channel.signal_id.as_str())
        .collect();
    let input_channels = path_channels(loop_definition, &input_ids, &input_signals)?;
    let output_channels = path_channels(loop_definition, &output_ids, &output_signals)?;
    let timeline = Timeline::new(recording, loop_definition);
    if timeline.samples.is_empty() {
        return Err(refusal(NO_SAMPLES, NextAction::Identify));
    }
    let (input_changes, output_changes) = trial_changes(
        &timeline,
        &trials,
        (&input_signals, &output_signals),
        inputs.maximum_unexcited_input_drift,
        budget,
    )?;
    budget.step()?;
    gain_answer(
        GainInputs {
            recording_id,
            recording_revision: &recording.loop_revision,
            input_channels,
            output_channels,
            trials: &trials,
            sample_count: timeline.retained(&trials),
        },
        &input_changes,
        &output_changes,
    )
}

/// What the gain answer names beside the matrices: the recording, the channels, the trials and the retained samples.
struct GainInputs<'a> {
    recording_id: &'a str,
    recording_revision: &'a str,
    input_channels: Vec<PathChannel>,
    output_channels: Vec<PathChannel>,
    trials: &'a [&'a SettledTrial],
    sample_count: usize,
}

/// The gain, the relative gain array, the decoupler and the pairing of the changes (C7, N4.5), with their sentences.
fn gain_answer(
    named: GainInputs<'_>,
    input_changes: &DenseMatrix,
    output_changes: &DenseMatrix,
) -> Result<MeasuredPathsResult, Failure> {
    let fit = multichannel::identify_static_gain(input_changes, output_changes, &TOLERANCE)
        .map_err(|_| refusal(POOR_EXCITATION, NextAction::None))?;
    let relative_gain_array = multichannel::relative_gain_array(&fit.gain, &TOLERANCE)
        .map_err(|_| refusal(SINGULAR, NextAction::None))?;
    let decoupler = multichannel::static_decoupler(&fit.gain, &TOLERANCE)
        .map_err(|_| refusal(SINGULAR, NextAction::None))?;
    let gain_condition_number = multichannel::condition_number(&fit.gain)
        .ok_or_else(|| refusal(SINGULAR, NextAction::None))?;
    let pairing = pairing_of(
        &relative_gain_array,
        &named.input_channels,
        &named.output_channels,
    );
    let summary = vec![format!(
        "{} samples from {} independent input steps.",
        named.sample_count,
        named.trials.len()
    )];
    let details = vec![
        "The gain matrix has one row per output and one column per input; the decoupler has one row per input and one column per output."
            .to_string(),
        format!(
            "Input rank {}; input condition number {:.3e}; gain condition number {:.3e}; residual norm {:.3e}.",
            fit.input_rank, fit.input_condition_number, gain_condition_number, fit.residual_norm
        ),
    ];
    Ok(MeasuredPathsResult {
        recording_id: named.recording_id.to_string(),
        recording_revision: named.recording_revision.to_string(),
        inputs: named.input_channels,
        outputs: named.output_channels,
        trials: named.trials.iter().copied().cloned().collect(),
        sample_count: named.sample_count,
        gain_matrix: fit.gain,
        relative_gain_array,
        decoupler,
        gain_condition_number,
        input_rank: fit.input_rank,
        input_condition_number: fit.input_condition_number,
        residual_norm: fit.residual_norm,
        pairing,
        summary,
        details,
    })
}

/// The pairing that `relative` names (N4.5): the pairs in output order, or the refusal when no pairing has every diagonal element
/// positive.
fn pairing_of(
    relative: &DenseMatrix,
    inputs: &[PathChannel],
    outputs: &[PathChannel],
) -> PathPairing {
    match multichannel::named_pairing(relative) {
        Some(columns) => PathPairing {
            status: PairingStatus::Named,
            pairs: outputs
                .iter()
                .enumerate()
                .map(|(row, output)| PathPair {
                    output_id: output.id.clone(),
                    input_id: inputs[columns[row]].id.clone(),
                    relative_gain: relative[row][columns[row]],
                })
                .collect(),
            reason: None,
        },
        None => PathPairing {
            status: PairingStatus::Refused,
            pairs: Vec::new(),
            reason: Some(NOT_PAIRED.to_string()),
        },
    }
}

/// The changes of every input and output in each trial (C7): the settled mean minus the baseline mean. Each window must pass the
/// window checks, and the excited input must hold within the share of its step that a window may span.
fn trial_changes(
    timeline: &Timeline<'_>,
    trials: &[&SettledTrial],
    signals: (&[&str], &[&str]),
    drift: Option<f64>,
    budget: &Budget<'_>,
) -> Result<(DenseMatrix, DenseMatrix), Failure> {
    let (input_signals, output_signals) = signals;
    let mut input_changes = Vec::with_capacity(trials.len());
    let mut output_changes = Vec::with_capacity(trials.len());
    for (excited, trial) in trials.iter().enumerate() {
        budget.step()?;
        let (
            Some(baseline_inputs),
            Some(settled_inputs),
            Some(baseline_outputs),
            Some(settled_outputs),
        ) = (
            timeline.mean(trial.baseline, input_signals),
            timeline.mean(trial.settled, input_signals),
            timeline.mean(trial.baseline, output_signals),
            timeline.mean(trial.settled, output_signals),
        )
        else {
            return Err(refusal(QUALITY, NextAction::None));
        };
        let input_change = difference(&settled_inputs, &baseline_inputs);
        let step = input_change[excited].abs();
        let excited_signal = input_signals[excited];
        let straddles = [trial.baseline, trial.settled]
            .iter()
            .any(|window| timeline.span(*window, excited_signal) > STRADDLE_FRACTION * step);
        if straddles {
            return Err(refusal(STRADDLE, NextAction::None));
        }
        let drifted = drift.is_some_and(|limit| {
            input_change
                .iter()
                .enumerate()
                .any(|(index, change)| index != excited && change.abs() > limit)
        });
        if drifted {
            return Err(refusal(DRIFT, NextAction::None));
        }
        input_changes.push(input_change);
        output_changes.push(difference(&settled_outputs, &baseline_outputs));
    }
    Ok((input_changes, output_changes))
}

/// The trials in input order, when there is exactly one trial for each input and no other.
fn trials_in_input_order<'a>(
    trials: &'a [SettledTrial],
    input_ids: &[&str],
) -> Result<Vec<&'a SettledTrial>, Failure> {
    let one_each = trials.len() == input_ids.len()
        && input_ids
            .iter()
            .all(|id| trials.iter().filter(|trial| trial.input_id == *id).count() == 1);
    if !one_each {
        return Err(refusal(ONE_TRIAL, NextAction::None));
    }
    Ok(input_ids
        .iter()
        .filter_map(|id| trials.iter().find(|trial| trial.input_id == *id))
        .collect())
}

/// The channels with the symbol and unit of the loop signal each one reads.
fn path_channels(
    loop_definition: &LoopDefinition,
    ids: &[&str],
    signals: &[&str],
) -> Result<Vec<PathChannel>, Failure> {
    ids.iter()
        .zip(signals)
        .map(|(id, signal)| {
            let (symbol, unit) = channel_signal(loop_definition, signal)
                .ok_or_else(|| refusal(UNKNOWN_SIGNAL, NextAction::OpenChannels))?;
            Ok(PathChannel {
                id: (*id).to_string(),
                symbol,
                unit,
            })
        })
        .collect()
}

/// The symbol and the unit of the loop signal that `id` names: the setpoint, the process variable, the manipulated variable, the
/// actuator or an additional signal of the loop.
fn channel_signal(loop_definition: &LoopDefinition, id: &str) -> Option<(String, String)> {
    let signals = &loop_definition.signals;
    let signal = match id {
        PROCESS_VARIABLE_BINDING => Some((
            &signals.process_variable.symbol,
            &signals.process_variable.unit,
        )),
        MANIPULATED_VARIABLE_BINDING => Some((
            &signals.manipulated_variable.symbol,
            &signals.manipulated_variable.unit,
        )),
        SETPOINT_BINDING => signals
            .setpoint
            .as_ref()
            .map(|signal| (&signal.symbol, &signal.unit)),
        ACTUATOR_BINDING => signals
            .actuator
            .as_ref()
            .map(|signal| (&signal.symbol, &signal.unit)),
        other => signals
            .additional
            .iter()
            .flatten()
            .find(|signal| signal.id == other)
            .map(|signal| (&signal.symbol, &signal.unit)),
    };
    signal.map(|(symbol, unit)| (symbol.clone(), unit.clone()))
}

/// The recording's samples placed on its time axis: each sample with a runtime time, at the seconds since the first sample of the run
/// (A3.2). A sample without a runtime time is not placed, so no window reads it.
struct Timeline<'a> {
    recording: &'a RecordedRun,
    loop_definition: &'a LoopDefinition,
    samples: Vec<(f64, &'a RecordedSample)>,
}

impl<'a> Timeline<'a> {
    fn new(recording: &'a RecordedRun, loop_definition: &'a LoopDefinition) -> Self {
        let timed: Vec<(i64, &'a RecordedSample)> = recording
            .samples
            .iter()
            .filter_map(|sample| sample.runtime_time_ns.map(|time| (time, sample)))
            .collect();
        let origin = timed.first().map_or(0, |(time, _)| *time);
        let samples: Vec<(f64, &'a RecordedSample)> = timed
            .into_iter()
            .map(|(time, sample)| (seconds_between(origin, time), sample))
            .collect();
        Self {
            recording,
            loop_definition,
            samples,
        }
    }

    /// The first and the last time of the placed samples.
    fn bounds(&self) -> (f64, f64) {
        self.samples.iter().fold(
            (f64::INFINITY, f64::NEG_INFINITY),
            |(low, high), (time, _)| (low.min(*time), high.max(*time)),
        )
    }

    /// The samples whose time lies in the window, both ends included; a window without both ends holds none.
    fn within(&self, window: TimeWindow) -> Vec<(f64, &'a RecordedSample)> {
        let (Some(start), Some(end)) = (window.start_seconds, window.end_seconds) else {
            return Vec::new();
        };
        self.samples
            .iter()
            .copied()
            .filter(|(time, _)| (start..=end).contains(time))
            .collect()
    }

    /// The value of the loop signal `signal` in `sample`, when it is finite and good (A4.2). The primary signals are good when the whole
    /// sample is; an additional signal is good when its own value is not forced and of good quality.
    fn value(&self, sample: &RecordedSample, signal: &str) -> Option<f64> {
        let value = match signal {
            PROCESS_VARIABLE_BINDING => process_value(self.recording, sample),
            MANIPULATED_VARIABLE_BINDING => {
                applied_mv(self.recording, self.loop_definition, sample)
            }
            other => binding_value(self.recording, sample, other),
        }?;
        let good = match signal {
            PROCESS_VARIABLE_BINDING | MANIPULATED_VARIABLE_BINDING | SETPOINT_BINDING => {
                sample_quality(self.recording, self.loop_definition, sample) == ChannelQuality::Good
            }
            other => binding_is_good(self.recording, sample, other),
        };
        (good && value.is_finite()).then_some(value)
    }

    /// The mean of each signal over the window (TypeScript `meanWindow`). It is refused when the window is not ordered or lacks an end,
    /// lies outside the placed samples, holds no sample, or holds a sample without a good value of some signal.
    fn mean(&self, window: TimeWindow, signals: &[&str]) -> Option<Vec<f64>> {
        let (first, last) = self.bounds();
        let (start, end) = ordered_ends(window)?;
        if start < first || end > last {
            return None;
        }
        let selected = self.within(window);
        if selected.is_empty() {
            return None;
        }
        signals
            .iter()
            .map(|signal| {
                let values = selected
                    .iter()
                    .map(|(_, sample)| self.value(sample, signal))
                    .collect::<Option<Vec<f64>>>()?;
                Some(mean_of(&values))
            })
            .collect()
    }

    /// The span of the signal over the window's samples, a missing value left out (TypeScript `spanWindow`); zero when none is good.
    fn span(&self, window: TimeWindow, signal: &str) -> f64 {
        let values: Vec<f64> = self
            .within(window)
            .into_iter()
            .filter_map(|(_, sample)| self.value(sample, signal))
            .collect();
        if values.is_empty() {
            return 0.0;
        }
        let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let low = values.iter().copied().fold(f64::INFINITY, f64::min);
        high - low
    }

    /// The samples between each trial's baseline start and its settled end, each counted once (C7).
    fn retained(&self, trials: &[&SettledTrial]) -> usize {
        self.samples
            .iter()
            .filter(|(time, _)| {
                trials.iter().any(|trial| {
                    trial
                        .baseline
                        .start_seconds
                        .is_some_and(|start| *time >= start)
                        && trial.settled.end_seconds.is_some_and(|end| *time <= end)
                })
            })
            .count()
    }
}

/// Whether a binding holds a good value in `sample`: it is not forced and its quality is good (A4.2).
fn binding_is_good(recording: &RecordedRun, sample: &RecordedSample, id: &str) -> bool {
    recording
        .bindings
        .iter()
        .position(|binding| binding.id == id)
        .is_some_and(|index| {
            sample.quality.get(index) == Some(&Quality::Good)
                && sample.forced.get(index) == Some(&false)
        })
}

/// The start and the end of a window that has both ends, when both are finite and the window runs forward; `None` for any other window.
fn ordered_ends(window: TimeWindow) -> Option<(f64, f64)> {
    match (window.start_seconds, window.end_seconds) {
        (Some(start), Some(end)) if start.is_finite() && end.is_finite() && start < end => {
            Some((start, end))
        }
        _ => None,
    }
}

/// The arithmetic mean of a non-empty series.
fn mean_of(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / f64::from(u32::try_from(values.len()).unwrap_or(u32::MAX))
}

/// The element-wise difference `after − before`.
fn difference(after: &[f64], before: &[f64]) -> Vec<f64> {
    after
        .iter()
        .zip(before)
        .map(|(after, before)| after - before)
        .collect()
}

/// Checks that the request names this recording and, when it names one, its revision.
fn check_recording(
    recording_id: &str,
    recording: &RecordedRun,
    inputs: &AnalysisInputs,
) -> Result<(), Failure> {
    if inputs
        .recording_id
        .as_deref()
        .is_some_and(|id| id != recording_id)
    {
        return Err(input_failure(NO_RECORDING));
    }
    if inputs
        .recording_revision
        .as_deref()
        .is_some_and(|revision| revision != recording.loop_revision)
    {
        return Err(input_failure(NO_REVISION));
    }
    Ok(())
}

/// A refusal of the calculation: its sentence, with the cause that a measured recording gives.
fn refusal(text: &str, next_action: NextAction) -> Failure {
    Failure::new(text, Cause::ModelOrData, next_action)
}

#[cfg(test)]
#[path = "measured_paths_tests.rs"]
mod tests;
