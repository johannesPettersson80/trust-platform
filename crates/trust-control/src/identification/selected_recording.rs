//! The model fitted from two recordings the engineer selected in an archive (`selectedRecordingIdentification.ts`;
//! specification 35, A10 and B5.2). The two recordings must be independent and come from the same project revision. Their
//! signals are the engineer's selection, with the operating range each signal took in the recordings (an observed range
//! describes the data only and never grants a live excitation limit). A recording is read as a [`SelectedRun`]: a connected
//! record's run and an isolated simulation's run alike, as `selectedRecordingIdentification.ts` takes both as runs.
//!
//! Only the single-input, single-output fit is ported. The general state fit (`generalIdentificationWorker.ts`, V7) is
//! not, so where the `TypeScript` would fall back to it this returns [`RecordedFitError::StateModelRequired`] or
//! [`RecordedFitError::Parked`].

use crate::identification::automatic::good_value;
use crate::identification::connected_record::{RuntimeVerificationResult, ValidationRun};
use crate::identification::data::IdentificationSample;
use crate::identification::experiment_trace::{
    experiment_from_run, ExperimentSignal, ExperimentSignals,
};
use crate::identification::model_comparison::{comparison_presentation, ComparisonPresentation};
use crate::identification::recorded_model::maximum_normalized_error;
use crate::identification::recorded_model::{accept_independent_samples, RecordedFitError};
use crate::identification::refusal::Refusal;
use crate::identification::selected_model_analysis::{accepted_identification_model, ChannelSet};
use crate::loop_definition::LoopDefinition;
use crate::model::{GeneralModel, ModelChannel, OperatingRange};
use crate::recording::opened_channels::RecordingSignalSelection;
use serde_json::Value;

/// The model of a selected pair of recordings, with the presentation that shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectedRecordingFit {
    /// The validated model.
    pub model: GeneralModel,
    /// What the page shows of the model, with the traces of the validation recording.
    pub presentation: ComparisonPresentation,
}

/// One recording a selected fit reads: the program revision it was taken on and its run. A connected record and an isolated
/// simulation's run are both read this way (`selectedRecordingIdentification.ts`, B5.2).
#[derive(Debug, Clone, Copy)]
pub struct SelectedRun<'a> {
    /// The program revision the run was taken on.
    pub project_revision: &'a str,
    /// The run: its identifier, channels, conditions and samples.
    pub run: &'a ValidationRun,
}

impl<'a, E> From<&'a RuntimeVerificationResult<E>> for SelectedRun<'a> {
    fn from(result: &'a RuntimeVerificationResult<E>) -> Self {
        Self {
            project_revision: result.project_revision.as_str(),
            run: &result.run,
        }
    }
}

/// Fits the model of two independent recordings with the engineer's selected signals (`fitSelectedRecordingModel`).
///
/// # Errors
/// The recordings are not two independent ones of one project revision; a selected signal is missing from a recording;
/// or the single-input, single-output fit gave no model (the state fit, V7, would be tried).
pub fn fit_selected_recording_model(
    loop_definition: &LoopDefinition,
    runs: &[SelectedRun<'_>],
    selection: &RecordingSignalSelection,
) -> Result<SelectedRecordingFit, RecordedFitError> {
    let [estimation, validation] = runs else {
        return Err(refused(
            "Two independent recordings from the same project revision are required.",
        ));
    };
    if estimation.run.id == validation.run.id
        || estimation.project_revision != validation.project_revision
    {
        return Err(refused(
            "Two independent recordings from the same project revision are required.",
        ));
    }
    let (inputs, outputs) =
        selected_channels(runs, selection).map_err(RecordedFitError::Refused)?;
    let model_channels = ChannelSet {
        inputs: inputs
            .iter()
            .map(|channel| model_channel_with_id(loop_definition, "inputs", channel))
            .collect(),
        outputs: outputs
            .iter()
            .map(|channel| model_channel_with_id(loop_definition, "outputs", channel))
            .collect(),
    };
    if let ([input], [output]) = (inputs.as_slice(), outputs.as_slice()) {
        // The attempt that runs first; a failure of it falls through to the state fit, as the TypeScript does.
        if let Ok(fit) = single_channel_fit(
            loop_definition,
            (estimation, validation),
            (input, output),
            &model_channels,
            &ExperimentSignals {
                inputs: signals_of(&inputs),
                outputs: signals_of(&outputs),
            },
        ) {
            return Ok(fit);
        }
        return Err(RecordedFitError::StateModelRequired {
            attempts: Vec::new(),
        });
    }
    Err(RecordedFitError::Parked {
        path: "the state model fit of several channels (generalIdentificationWorker.ts, V7)",
    })
}

/// The single-input, single-output fit of two selected recordings: the independent acceptance, the accepted plant model
/// and the presentation with the traces of the validation recording.
fn single_channel_fit(
    loop_definition: &LoopDefinition,
    (estimation, validation): (&SelectedRun<'_>, &SelectedRun<'_>),
    (input, output): (&ModelChannel, &ModelChannel),
    model_channels: &ChannelSet,
    signals: &ExperimentSignals,
) -> Result<SelectedRecordingFit, Refusal> {
    let estimation_samples = selected_samples(estimation, input, output);
    let validation_samples = selected_samples(validation, input, output);
    let accepted = accept_independent_samples(
        estimation_samples,
        validation_samples,
        &estimation.run.id,
        &validation.run.id,
        maximum_normalized_error(loop_definition),
    )
    .map_err(Refusal::new)?;
    let model = accepted_identification_model(&accepted, model_channels)?;
    let experiment = experiment_from_run(loop_definition, validation.run, Some(signals))?;
    let presentation =
        comparison_presentation(experiment, input, output, &accepted.model, &accepted.fit);
    Ok(SelectedRecordingFit {
        model,
        presentation,
    })
}

/// The samples of one selected recording (`samples` in `fitSelectedRecordingModel`): every sample, marked good, with the
/// selected input and output. A missing value is `NaN`; the `TypeScript` passes it as `null`.
fn selected_samples(
    result: &SelectedRun<'_>,
    input: &ModelChannel,
    output: &ModelChannel,
) -> Vec<IdentificationSample> {
    result
        .run
        .samples
        .iter()
        .map(|sample| IdentificationSample {
            time_seconds: sample.time_seconds,
            input: sample
                .values
                .get(&input.id)
                .and_then(|value| value.value)
                .unwrap_or(f64::NAN),
            output: sample
                .values
                .get(&output.id)
                .and_then(|value| value.value)
                .unwrap_or(f64::NAN),
            quality: trust_control_math::identify::SampleQuality::Good,
            input_unit: Some(input.unit.clone()),
            output_unit: Some(output.unit.clone()),
            clipped: None,
            saturated: None,
        })
        .collect()
}

/// The selected channels of two or more recordings, with the range each took (`selectedRecordingChannels`): the plant inputs
/// and the measured outputs. Each signal must be in every recording with one unit, and every sample of it must be good.
///
/// # Errors
/// A signal is missing or has inconsistent units, has no samples, or has a value that is missing or not good.
fn selected_channels(
    runs: &[SelectedRun<'_>],
    selection: &RecordingSignalSelection,
) -> Result<(Vec<ModelChannel>, Vec<ModelChannel>), Refusal> {
    let inputs = selection
        .inputs
        .iter()
        .map(|id| selected_channel(runs, id))
        .collect::<Result<Vec<_>, _>>()?;
    let outputs = selection
        .outputs
        .iter()
        .map(|id| selected_channel(runs, id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((inputs, outputs))
}

/// One selected signal: its unit from the first recording, and the smallest and largest value it took in all of them.
fn selected_channel(runs: &[SelectedRun<'_>], id: &str) -> Result<ModelChannel, Refusal> {
    let missing = || {
        Refusal::new(format!(
            "Recording signal {id} is missing or has inconsistent units."
        ))
    };
    let declared = runs
        .first()
        .and_then(|run| run.run.channels.iter().find(|channel| channel.id == id))
        .ok_or_else(missing)?;
    let unit = declared.unit.clone();
    if runs.iter().any(|run| {
        !run.run
            .channels
            .iter()
            .any(|channel| channel.id == id && channel.unit == unit)
    }) {
        return Err(missing());
    }
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for run in runs {
        for sample in &run.run.samples {
            let Some(value) = sample.values.get(id).and_then(good_value) else {
                return Err(Refusal::new(format!(
                    "Recording signal {id} has incomplete or invalid samples."
                )));
            };
            minimum = minimum.min(value);
            maximum = maximum.max(value);
        }
    }
    if !minimum.is_finite() {
        return Err(Refusal::new(format!(
            "Recording signal {id} has no samples."
        )));
    }
    // A single value is widened by a tiny padding, so it is still a range.
    let padding = if maximum > minimum {
        0.0
    } else {
        (minimum.abs() * 1e-9).max(1e-9)
    };
    Ok(ModelChannel {
        id: id.to_owned(),
        unit,
        operating_range: OperatingRange {
            minimum: minimum - padding,
            maximum: maximum + padding,
        },
    })
}

/// A selected channel with its model identifier: the loop's `modelChannelId` for the signal when the loop gives one, or the
/// signal's symbol.
fn model_channel_with_id(
    loop_definition: &LoopDefinition,
    kind: &str,
    channel: &ModelChannel,
) -> ModelChannel {
    ModelChannel {
        id: model_id_of(loop_definition, kind, &channel.id),
        unit: channel.unit.clone(),
        operating_range: channel.operating_range,
    }
}

/// The model identifier of the loop's channel of one kind that names the signal `symbol` (`inputId` and `outputId` in
/// `fitSelectedRecordingModel`), or the symbol itself.
fn model_id_of(loop_definition: &LoopDefinition, kind: &str, symbol: &str) -> String {
    loop_definition
        .channels
        .as_ref()
        .and_then(|channels: &Value| channels.get(kind))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|entry| {
            entry
                .get("signalId")
                .and_then(Value::as_str)
                .and_then(|signal_id| signal_symbol(loop_definition, signal_id))
                .as_deref()
                == Some(symbol)
        })
        .and_then(|entry| entry.get("modelChannelId")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| symbol.to_owned())
}

/// The symbol of a loop signal by its identifier (`loopSignal`): the primary signals by their role names, and the
/// auxiliary signals by their identifiers.
fn signal_symbol(loop_definition: &LoopDefinition, id: &str) -> Option<String> {
    let signals = &loop_definition.signals;
    match id {
        "setpoint" => signals
            .setpoint
            .as_ref()
            .map(|signal| signal.symbol.clone()),
        "processVariable" => Some(signals.process_variable.symbol.clone()),
        "manipulatedVariable" => Some(signals.manipulated_variable.symbol.clone()),
        "actuator" => signals
            .actuator
            .as_ref()
            .map(|signal| signal.symbol.clone()),
        _ => signals
            .additional
            .as_ref()?
            .iter()
            .find(|signal| signal.id == id)
            .map(|signal| signal.symbol.clone()),
    }
}

fn signals_of(channels: &[ModelChannel]) -> Vec<ExperimentSignal> {
    channels
        .iter()
        .map(|channel| ExperimentSignal {
            id: channel.id.clone(),
            unit: channel.unit.clone(),
        })
        .collect()
}

fn refused(sentence: &str) -> RecordedFitError {
    RecordedFitError::Refused(Refusal::new(sentence))
}

#[cfg(test)]
#[path = "selected_recording_tests.rs"]
mod tests;
