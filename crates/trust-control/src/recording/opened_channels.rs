//! The signals of an opened recording (`openedRecordingChannels.ts`): which recorded channels exist, which the
//! engineer selected as plant inputs and measured outputs, and the operating range each selected channel covers.
//!
//! A channel is named by its loop signal's symbol, never by a runtime address, and its unit comes from the loop
//! definition: a recorded run carries the binding's identifier, not its unit (annex E4.7). Old recordings without
//! runs are `legacy`: their channels are `appliedMv` and `pv`, with the units of the file.

use serde::Serialize;
use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, Range};
use crate::recording::channels::{
    ACTUATOR_BINDING, MANIPULATED_VARIABLE_BINDING, PROCESS_VARIABLE_BINDING, SETPOINT_BINDING,
};
use crate::recording::RecordedRun;
use crate::signals::Quality;

/// The selected plant inputs and measured outputs of a recording, by channel name (`RecordingSignalSelection`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordingSignalSelection {
    /// The plant inputs, in order.
    pub inputs: Vec<String>,
    /// The measured outputs, in order.
    pub outputs: Vec<String>,
}

/// Whether a recording comes from runs of the loop (`archive`) or from an older file (`legacy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CatalogKind {
    /// Recorded runs of the loop.
    Archive,
    /// An older file whose channels are `appliedMv` and `pv`.
    Legacy,
}

/// One channel a recording offers: its name and its unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogChannel {
    /// The channel's name, the symbol of its loop signal in an archive.
    pub id: String,
    /// The channel's unit.
    pub unit: String,
}

/// What an opened recording offers and what is selected (`RecordingSignalCatalog`). It is stored with the study, so
/// it is read only through [`decode_recording_catalog`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecordingSignalCatalog {
    /// The channels the recording offers.
    pub channels: Vec<CatalogChannel>,
    /// The selected plant inputs and measured outputs.
    pub selection: RecordingSignalSelection,
    /// Whether the channels come from runs or from a legacy file.
    pub kind: CatalogKind,
}

/// The units of a recording file that a legacy recording reads (`ControlStudioWorkflowResult.units`, the fields of
/// `openedRecordingChannels.ts`). From workflowResult.ts; moves with the recording import (V1.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowUnits {
    /// The manipulated variable's unit.
    pub mv: String,
    /// The process variable's unit.
    pub pv: String,
}

/// The model channels of a selected recording: a channel with its unit and the range its values cover. From
/// generalModel.ts (`ModelChannel`); moves with the model port (V1.1).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelChannel {
    /// The channel's name.
    pub id: String,
    /// The channel's unit.
    pub unit: String,
    /// The range the recorded values cover.
    pub operating_range: Range,
}

/// The inputs and outputs of a selected recording (`IdentificationChannels`). From identifiedStateModel.ts; the
/// lists are kept for the fit, and the multichannel form moves with V7.9.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IdentificationChannels {
    /// The plant inputs.
    pub inputs: Vec<ModelChannel>,
    /// The measured outputs.
    pub outputs: Vec<ModelChannel>,
}

/// The loop signal a binding of a recorded run stands for: its symbol, which names the channel, and its unit.
struct Coordinate<'a> {
    symbol: &'a str,
    unit: &'a str,
}

/// The loop signal a binding identifier stands for (`signals::loop_inspection_bindings`), if the loop has it.
fn coordinate<'a>(loop_definition: &'a LoopDefinition, binding_id: &str) -> Option<Coordinate<'a>> {
    let signals = &loop_definition.signals;
    let (symbol, unit) = match binding_id {
        SETPOINT_BINDING => {
            let signal = signals.setpoint.as_ref()?;
            (signal.symbol.as_str(), signal.unit.as_str())
        }
        PROCESS_VARIABLE_BINDING => (
            signals.process_variable.symbol.as_str(),
            signals.process_variable.unit.as_str(),
        ),
        MANIPULATED_VARIABLE_BINDING => (
            signals.manipulated_variable.symbol.as_str(),
            signals.manipulated_variable.unit.as_str(),
        ),
        ACTUATOR_BINDING => {
            let signal = signals.actuator.as_ref()?;
            (signal.symbol.as_str(), signal.unit.as_str())
        }
        id => {
            let signal = signals
                .additional
                .iter()
                .flatten()
                .find(|signal| signal.id == id)?;
            (signal.symbol.as_str(), signal.unit.as_str())
        }
    };
    Some(Coordinate { symbol, unit })
}

/// The unit of the loop signal named `symbol`.
fn signal_unit<'a>(loop_definition: &'a LoopDefinition, symbol: &str) -> Option<&'a str> {
    let signals = &loop_definition.signals;
    let mut signals_by_symbol = signals
        .setpoint
        .iter()
        .chain(std::iter::once(&signals.process_variable))
        .chain(std::iter::once(&signals.manipulated_variable))
        .chain(signals.actuator.iter())
        .map(|signal| (signal.symbol.as_str(), signal.unit.as_str()))
        .chain(
            signals
                .additional
                .iter()
                .flatten()
                .map(|signal| (signal.symbol.as_str(), signal.unit.as_str())),
        );
    signals_by_symbol
        .find(|(name, _)| *name == symbol)
        .map(|(_, unit)| unit)
}

/// The channels a run offers: each binding that stands for a loop signal, named by its symbol with its unit.
/// Mode bindings are not channels.
fn run_channels(loop_definition: &LoopDefinition, run: &RecordedRun) -> Vec<(String, String)> {
    run.bindings
        .iter()
        .filter_map(|binding| coordinate(loop_definition, &binding.id))
        .map(|found| (found.symbol.to_string(), found.unit.to_string()))
        .collect()
}

/// The index of the binding that carries the channel `channel` in `run`.
fn channel_index(
    loop_definition: &LoopDefinition,
    run: &RecordedRun,
    channel: &str,
) -> Option<usize> {
    run.bindings.iter().position(|binding| {
        coordinate(loop_definition, &binding.id).is_some_and(|found| found.symbol == channel)
    })
}

/// The catalog of a recording opened from `runs` (archive) or from a legacy file (`units`), with the selection
/// given by `requested` or else the default: the loop's manipulated variable as the input and its measurement as
/// the output (`openedRecordingCatalog`). Identities are data coordinates, never inferred runtime addresses.
///
/// # Errors
/// The requested selection is malformed, or it names a channel the recording does not have.
pub fn opened_recording_catalog(
    units: &WorkflowUnits,
    loop_definition: &LoopDefinition,
    runs: &[RecordedRun],
    requested: Option<&Value>,
) -> Result<RecordingSignalCatalog, Failure> {
    let channels = match runs.split_first() {
        Some((first, rest)) => run_channels(loop_definition, first)
            .into_iter()
            .filter(|(id, unit)| {
                rest.iter().all(|run| {
                    run_channels(loop_definition, run)
                        .iter()
                        .any(|(other, other_unit)| other == id && other_unit == unit)
                })
            })
            .map(|(id, unit)| CatalogChannel { id, unit })
            .collect(),
        None => vec![
            CatalogChannel {
                id: "appliedMv".to_string(),
                unit: units.mv.clone(),
            },
            CatalogChannel {
                id: "pv".to_string(),
                unit: units.pv.clone(),
            },
        ],
    };
    let defaults = if runs.is_empty() {
        RecordingSignalSelection {
            inputs: vec!["appliedMv".to_string()],
            outputs: vec!["pv".to_string()],
        }
    } else {
        let configured = configured_recording_selection(loop_definition);
        RecordingSignalSelection {
            inputs: configured
                .inputs
                .into_iter()
                .filter(|id| has_channel(&channels, id))
                .collect(),
            outputs: configured
                .outputs
                .into_iter()
                .filter(|id| has_channel(&channels, id))
                .collect(),
        }
    };
    let selection = match requested {
        None => defaults,
        Some(value) => decode_selection(value)?,
    };
    if selection
        .inputs
        .iter()
        .chain(&selection.outputs)
        .any(|id| !has_channel(&channels, id))
    {
        return Err(Failure::new(
            "Choose input and output signals present in this recording.",
            Cause::Settings,
            NextAction::None,
        ));
    }
    let kind = if runs.is_empty() {
        CatalogKind::Legacy
    } else {
        CatalogKind::Archive
    };
    Ok(RecordingSignalCatalog {
        channels,
        selection,
        kind,
    })
}

fn has_channel(channels: &[CatalogChannel], id: &str) -> bool {
    channels.iter().any(|channel| channel.id == id)
}

/// Reads a stored catalog, or refuses it (`decodeRecordingCatalog`): only the keys `kind`, `channels` and
/// `selection`; at most 256 channels with unique identifiers of at most 1,024 UTF-16 units and units of at most 64;
/// a valid selection whose channels are all present.
#[must_use]
pub fn decode_recording_catalog(value: &Value) -> Option<RecordingSignalCatalog> {
    let object = value.as_object()?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "kind" | "channels" | "selection"))
    {
        return None;
    }
    let kind = match object.get("kind")?.as_str()? {
        "archive" => CatalogKind::Archive,
        "legacy" => CatalogKind::Legacy,
        _ => return None,
    };
    let items = object.get("channels")?.as_array()?;
    if items.len() > 256 {
        return None;
    }
    let mut channels: Vec<CatalogChannel> = Vec::with_capacity(items.len());
    for item in items {
        let channel = item.as_object()?;
        let id = channel.get("id")?.as_str()?;
        let unit = channel.get("unit")?.as_str()?;
        if id.is_empty()
            || utf16_len(id) > 1024
            || utf16_len(unit) > 64
            || channels.iter().any(|seen| seen.id == id)
        {
            return None;
        }
        channels.push(CatalogChannel {
            id: id.to_string(),
            unit: unit.to_string(),
        });
    }
    let selection = decode_selection(object.get("selection")?).ok()?;
    if selection
        .inputs
        .iter()
        .chain(&selection.outputs)
        .any(|id| !has_channel(&channels, id))
    {
        return None;
    }
    Some(RecordingSignalCatalog {
        channels,
        selection,
        kind,
    })
}

/// The default selection of a loop (`configuredRecordingSelection`): its manipulated variable as the input and its
/// measurement as the output. The multichannel selection of a loop's channels moves with V7.9.
#[must_use]
pub fn configured_recording_selection(
    loop_definition: &LoopDefinition,
) -> RecordingSignalSelection {
    RecordingSignalSelection {
        inputs: vec![loop_definition.signals.manipulated_variable.symbol.clone()],
        outputs: vec![loop_definition.signals.process_variable.symbol.clone()],
    }
}

/// The selected channels with their operating ranges (`selectedRecordingChannels`). The range is the smallest and
/// largest value each channel took in the runs; a single value is widened by a tiny padding, so it stays a range.
/// Observed ranges describe the recorded data only, and never grant live excitation limits.
///
/// # Errors
/// A selected channel is missing from a run, has no samples, or has a value that is missing or not good.
pub fn selected_recording_channels(
    runs: &[RecordedRun],
    loop_definition: &LoopDefinition,
    selection: &RecordingSignalSelection,
) -> Result<IdentificationChannels, Failure> {
    let inputs = selection
        .inputs
        .iter()
        .map(|id| selected_channel(runs, loop_definition, id))
        .collect::<Result<Vec<ModelChannel>, Failure>>()?;
    let outputs = selection
        .outputs
        .iter()
        .map(|id| selected_channel(runs, loop_definition, id))
        .collect::<Result<Vec<ModelChannel>, Failure>>()?;
    Ok(IdentificationChannels { inputs, outputs })
}

fn selected_channel(
    runs: &[RecordedRun],
    loop_definition: &LoopDefinition,
    id: &str,
) -> Result<ModelChannel, Failure> {
    let missing = || {
        Failure::new(
            &format!("Recording signal {id} is missing or has inconsistent units."),
            Cause::Settings,
            NextAction::None,
        )
    };
    let unit = signal_unit(loop_definition, id).ok_or_else(missing)?;
    if runs.is_empty()
        || runs
            .iter()
            .any(|run| channel_index(loop_definition, run, id).is_none())
    {
        return Err(missing());
    }
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for run in runs {
        let index = channel_index(loop_definition, run, id).ok_or_else(missing)?;
        for sample in &run.samples {
            let value = sample.values.get(index).copied().flatten();
            let good = sample.quality.get(index) == Some(&Quality::Good);
            let Some(value) = value.filter(|value| good && value.is_finite()) else {
                return Err(Failure::new(
                    &format!("Recording signal {id} has incomplete or invalid samples."),
                    Cause::Settings,
                    NextAction::None,
                ));
            };
            minimum = minimum.min(value);
            maximum = maximum.max(value);
        }
    }
    if !minimum.is_finite() {
        return Err(Failure::new(
            &format!("Recording signal {id} has no samples."),
            Cause::Settings,
            NextAction::None,
        ));
    }
    let padding = padding_of(minimum, maximum);
    Ok(ModelChannel {
        id: id.to_string(),
        unit: unit.to_string(),
        operating_range: Range {
            minimum: minimum - padding,
            maximum: maximum + padding,
        },
    })
}

/// The padding of an operating range: a single value gets `max(1e-9, |value| · 1e-9)` on each side, so the range
/// has a width; a range with two different ends gets none.
#[allow(clippy::float_cmp)] // A range is a single value exactly when its two ends are equal.
fn padding_of(minimum: f64, maximum: f64) -> f64 {
    if minimum == maximum {
        1e-9_f64.max(minimum.abs() * 1e-9)
    } else {
        0.0
    }
}

/// The selection a request or a stored catalog names (`decodeSelection`): only the keys `inputs` and `outputs`,
/// each a list of at most 16 distinct names of 1 to 1,024 UTF-16 units, and no name both an input and an output.
fn decode_selection(value: &Value) -> Result<RecordingSignalSelection, Failure> {
    let object = value.as_object().ok_or_else(selection_refused)?;
    if object.keys().any(|key| key != "inputs" && key != "outputs") {
        return Err(selection_refused());
    }
    let inputs = signal_ids(object.get("inputs")).ok_or_else(selection_refused)?;
    let outputs = signal_ids(object.get("outputs")).ok_or_else(selection_refused)?;
    if inputs.iter().any(|id| outputs.contains(id)) {
        return Err(selection_refused());
    }
    Ok(RecordingSignalSelection { inputs, outputs })
}

fn signal_ids(value: Option<&Value>) -> Option<Vec<String>> {
    let items = value?.as_array()?;
    if items.len() > 16 {
        return None;
    }
    let mut ids: Vec<String> = Vec::with_capacity(items.len());
    for item in items {
        let id = item.as_str()?;
        if id.is_empty() || utf16_len(id) > 1024 || ids.iter().any(|seen| seen == id) {
            return None;
        }
        ids.push(id.to_string());
    }
    Some(ids)
}

fn selection_refused() -> Failure {
    Failure::new(
        "Choose distinct plant input and measured output signals from the recording.",
        Cause::Settings,
        NextAction::None,
    )
}

/// The length of a text in UTF-16 code units, as JavaScript counts a string's length.
fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

#[cfg(test)]
#[path = "opened_channels_tests.rs"]
mod tests;
