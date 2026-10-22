//! The signals of an opened recording (`openedRecordingChannels.ts`): which recorded channels exist, which the
//! engineer selected as plant inputs and measured outputs, and the operating range each selected channel covers.
//!
//! A channel is named by its loop signal's symbol, never by a runtime address, and its unit comes from the loop
//! definition: a recorded run carries the binding's identifier, not its unit (annex E4.7). Old recordings without
//! runs are `legacy`: their channels are `appliedMv` and `pv`, with the units of the file.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_run::loop_signal_of;
use crate::identification::selected_recording::selected_model_id;
use crate::loop_definition::{
    ChannelInput, ChannelOutput, Direction, LoopChannels, LoopDefinition, LoopSignal, Range,
};
use crate::model::simulation::{ActuatorCapabilities, ActuatorKind};
use crate::recording::channels::{
    ACTUATOR_BINDING, MANIPULATED_VARIABLE_BINDING, PROCESS_VARIABLE_BINDING, SETPOINT_BINDING,
};
use crate::recording::RecordedRun;
use crate::signals::{Quality, SignalBinding};

/// The selected plant inputs and measured outputs of a recording, by channel name (`RecordingSignalSelection`). The study keeps
/// the engineer's selection in its inputs, so it is read back the same way (B3.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
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

/// The words a legacy file's applied input is shown with (B3.6).
const LEGACY_INPUT_LABEL: &str = "Applied input";
/// The words a legacy file's measured output is shown with (B3.6).
const LEGACY_OUTPUT_LABEL: &str = "Measured output";

/// One channel a recording offers: its name, its unit and, for a legacy file, the words it is shown with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogChannel {
    /// The channel's name, the symbol of its loop signal in an archive.
    pub id: String,
    /// The channel's unit.
    pub unit: String,
    /// The words the selector shows for the channel in place of its name: a legacy file's `appliedMv` and `pv` carry them (B3.6), and
    /// an archive's channels, which are named by their symbols, carry none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// The words a legacy file's channel `id` is shown with, or `None` for any other channel.
fn legacy_label(id: &str) -> Option<String> {
    match id {
        "appliedMv" => Some(LEGACY_INPUT_LABEL.to_string()),
        "pv" => Some(LEGACY_OUTPUT_LABEL.to_string()),
        _ => None,
    }
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
    /// The channels the inputs selector offers: a signal the loop declares as a measured output is not offered (`side_channels`).
    pub inputs: Vec<CatalogChannel>,
    /// The channels the outputs selector offers: a signal the loop declares as a plant input is not offered, so an input is never a
    /// measured output; an extra output that the loop does not bind stays offered (B3.6).
    pub outputs: Vec<CatalogChannel>,
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

/// The channel a binding of a recorded run stands for, named by its symbol with its unit (`binding_channel`). A binding that keeps
/// its unit is an extra channel of an archived run (`archived_recording`), named by its symbol. Any other binding stands for the
/// loop signal its identifier names, and a mode binding stands for none.
#[must_use]
pub(crate) fn binding_channel(
    loop_definition: &LoopDefinition,
    binding: &SignalBinding,
) -> Option<(String, String)> {
    match &binding.unit {
        Some(unit) => Some((binding.symbol.clone()?, unit.clone())),
        None => coordinate(loop_definition, &binding.id)
            .map(|found| (found.symbol.to_string(), found.unit.to_string())),
    }
}

/// The channels a run offers: each binding that stands for a channel, named by its symbol with its unit (`binding_channel`).
/// Mode bindings are not channels.
fn run_channels(loop_definition: &LoopDefinition, run: &RecordedRun) -> Vec<(String, String)> {
    run.bindings
        .iter()
        .filter_map(|binding| binding_channel(loop_definition, binding))
        .collect()
}

/// The index of the binding that carries the channel `channel` in `run`.
fn channel_index(
    loop_definition: &LoopDefinition,
    run: &RecordedRun,
    channel: &str,
) -> Option<usize> {
    run.bindings.iter().position(|binding| {
        binding_channel(loop_definition, binding)
            .is_some_and(|(symbol, _)| symbol.as_str() == channel)
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
            .map(|(id, unit)| CatalogChannel {
                id,
                unit,
                label: None,
            })
            .collect(),
        None => vec![
            CatalogChannel {
                id: "appliedMv".to_string(),
                unit: units.mv.clone(),
                label: legacy_label("appliedMv"),
            },
            CatalogChannel {
                id: "pv".to_string(),
                unit: units.pv.clone(),
                label: legacy_label("pv"),
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
    let kind = if runs.is_empty() {
        CatalogKind::Legacy
    } else {
        CatalogKind::Archive
    };
    select_catalog(loop_definition, channels, defaults, kind, requested)
}

/// The catalogue of an archive whose runs hold `channels` (`openedRecordingCatalog` with runs): the loop's plant input and
/// measured output are the defaults, and `requested` is the engineer's selection when it names one.
///
/// # Errors
/// The requested selection is malformed, or it names a channel the archive does not have.
pub fn archive_catalog(
    channels: Vec<CatalogChannel>,
    loop_definition: &LoopDefinition,
    requested: Option<&Value>,
) -> Result<RecordingSignalCatalog, Failure> {
    let configured = configured_recording_selection(loop_definition);
    let defaults = RecordingSignalSelection {
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
    };
    select_catalog(
        loop_definition,
        channels,
        defaults,
        CatalogKind::Archive,
        requested,
    )
}

/// The catalogue with `channels`, and the selection `requested` or else `defaults`. A selection that names a channel the
/// recording does not have is refused. Each selector's channels come from the loop (`side_channels`).
fn select_catalog(
    loop_definition: &LoopDefinition,
    channels: Vec<CatalogChannel>,
    defaults: RecordingSignalSelection,
    kind: CatalogKind,
    requested: Option<&Value>,
) -> Result<RecordingSignalCatalog, Failure> {
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
        return Err(not_present());
    }
    let (inputs, outputs) = side_channels(loop_definition, &channels, kind);
    Ok(RecordingSignalCatalog {
        channels,
        selection,
        kind,
        inputs,
        outputs,
    })
}

/// The channels each selector offers (`fillRecording`, B3.6): a plant input on the inputs selector and a measured output on the outputs
/// selector, so that a signal the loop declares on the other side is offered on neither. An archive's channels are filtered by the loop's
/// declared sides; a legacy file offers its `appliedMv` as the input and its `pv` as the output.
fn side_channels(
    loop_definition: &LoopDefinition,
    channels: &[CatalogChannel],
    kind: CatalogKind,
) -> (Vec<CatalogChannel>, Vec<CatalogChannel>) {
    match kind {
        CatalogKind::Legacy => (
            channels_named(channels, "appliedMv"),
            channels_named(channels, "pv"),
        ),
        CatalogKind::Archive => {
            let loop_inputs = declared_symbols(loop_definition, Side::Inputs);
            let loop_outputs = declared_symbols(loop_definition, Side::Outputs);
            (
                channels
                    .iter()
                    .filter(|channel| !loop_outputs.contains(&channel.id))
                    .cloned()
                    .collect(),
                channels
                    .iter()
                    .filter(|channel| !loop_inputs.contains(&channel.id))
                    .cloned()
                    .collect(),
            )
        }
    }
}

/// The side of a loop whose declared signals `declared_symbols` names.
#[derive(Clone, Copy)]
enum Side {
    Inputs,
    Outputs,
}

/// The symbols of the loop's declared plant inputs or measured outputs, as the loop declares them: the signals its channels bind, or
/// for a loop with no channels its manipulated variable or its process variable (`loopSideSymbols`).
fn declared_symbols(loop_definition: &LoopDefinition, side: Side) -> Vec<String> {
    let Some(channels) = &loop_definition.channels else {
        let signals = &loop_definition.signals;
        return vec![match side {
            Side::Inputs => signals.manipulated_variable.symbol.clone(),
            Side::Outputs => signals.process_variable.symbol.clone(),
        }];
    };
    let signal_ids: Vec<&str> = match side {
        Side::Inputs => channels
            .inputs
            .iter()
            .map(|input| input.signal_id.as_str())
            .collect(),
        Side::Outputs => channels
            .outputs
            .iter()
            .map(|output| output.signal_id.as_str())
            .collect(),
    };
    signal_ids
        .into_iter()
        .filter_map(|id| loop_signal_of(loop_definition, id))
        .map(|signal| signal.symbol)
        .collect()
}

/// The channels of `channels` that carry the identifier `id`: at most one, as a legacy file names its two channels.
fn channels_named(channels: &[CatalogChannel], id: &str) -> Vec<CatalogChannel> {
    channels
        .iter()
        .filter(|channel| channel.id == id)
        .cloned()
        .collect()
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
            label: if kind == CatalogKind::Legacy {
                legacy_label(id)
            } else {
                None
            },
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
    // A stored catalog holds no loop, so its selectors offer every channel: the loop's declared sides are applied where a catalog is built.
    Some(RecordingSignalCatalog {
        inputs: channels.clone(),
        outputs: channels.clone(),
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

/// The units a legacy catalogue names for the loop's manipulated variable and measurement (`WorkflowUnits`).
#[must_use]
pub fn workflow_units(loop_definition: &LoopDefinition) -> WorkflowUnits {
    WorkflowUnits {
        mv: loop_definition.signals.manipulated_variable.unit.clone(),
        pv: loop_definition.signals.process_variable.unit.clone(),
    }
}

/// Checks a selection the study keeps (`decodeSelection`): distinct names of 1 to 1,024 UTF-16 units, at most 16 of each, and
/// no name both an input and an output.
///
/// # Errors
/// The selection breaks one of those rules.
pub fn check_selection(selection: &RecordingSignalSelection) -> Result<(), Failure> {
    let value = serde_json::to_value(selection).map_err(|_| selection_refused())?;
    decode_selection(&value).map(|_| ())
}

/// The loop as the fit of one recording sees it when the engineer selected `input` as the plant input and `output` as the
/// measured output (B3.6): the loop with those two signals in place of its own, so the fit reads them from the recording and
/// names them in the model. A selected name that the loop does not have is a channel the run holds as an extra output, and the view takes
/// it as that signal (`extra_signal`). A selected name that is not the loop's own signal of its role is named in the view with the model
/// identifier the loop gives the selected signal (`name_selected_role`), so the model names the loop's channel while the samples keep
/// the recorded name.
///
/// # Errors
/// A selected name is neither a signal of the loop nor a channel of the run, or an extra channel's samples are missing or not good.
pub fn selection_loop(
    loop_definition: &LoopDefinition,
    run: &RecordedRun,
    input: &str,
    output: &str,
) -> Result<LoopDefinition, Failure> {
    let manipulated = selected_signal(loop_definition, run, input)?;
    let process = selected_signal(loop_definition, run, output)?;
    let mut view = loop_definition.clone();
    if input != loop_definition.signals.manipulated_variable.symbol.as_str() {
        name_selected_role(
            &mut view,
            loop_definition,
            "inputs",
            "manipulatedVariable",
            input,
        );
    }
    if output != loop_definition.signals.process_variable.symbol.as_str() {
        name_selected_role(
            &mut view,
            loop_definition,
            "outputs",
            "processVariable",
            output,
        );
    }
    view.signals.manipulated_variable = manipulated;
    view.signals.process_variable = process;
    Ok(view)
}

/// The loop signal a selected name stands for in the view: the loop's own signal, or else an extra output of the run.
fn selected_signal(
    loop_definition: &LoopDefinition,
    run: &RecordedRun,
    symbol: &str,
) -> Result<LoopSignal, Failure> {
    if let Some(signal) = loop_signal_named(loop_definition, symbol) {
        return Ok(signal);
    }
    extra_signal(run, symbol)?.ok_or_else(not_present)
}

/// The signal of an extra output of the run named `symbol` (B3.6): a channel the loop does not bind, which the run holds with its unit.
/// The fit only reads it, so it is read only, as a REAL signal without an address. Its range is the one its samples took, which
/// describes the recorded data and grants no excitation limit (`observed_range`). `None` when the run holds no such channel.
fn extra_signal(run: &RecordedRun, symbol: &str) -> Result<Option<LoopSignal>, Failure> {
    let extra = run
        .bindings
        .iter()
        .enumerate()
        .find_map(|(index, binding)| {
            let unit = binding.unit.as_ref()?;
            (binding.symbol.as_deref() == Some(symbol)).then(|| (index, unit.clone()))
        });
    let Some((index, unit)) = extra else {
        return Ok(None);
    };
    Ok(Some(LoopSignal {
        symbol: symbol.to_string(),
        address: None,
        iec_type: "REAL".to_string(),
        direction: Direction::Read,
        unit,
        range: observed_range(symbol, channel_readings(run, index))?,
    }))
}

/// Names the role of the view that takes the selected signal `selected`, which is not the loop's own signal of that role, with the
/// model identifier the loop gives the selected signal (`selected_model_id`). The role's channel is the one channel of its kind on
/// the role's signal `signal_id` in the view, so the model names the loop's channel while the fit reads the recorded one (B3.6). A kind
/// with several channels keeps them: the fit refuses several channels (V7).
fn name_selected_role(
    view: &mut LoopDefinition,
    loop_definition: &LoopDefinition,
    kind: &str,
    signal_id: &str,
    selected: &str,
) {
    let model_id = selected_model_id(loop_definition, kind, selected);
    let channels = view.channels.get_or_insert_with(|| LoopChannels {
        inputs: Vec::new(),
        outputs: Vec::new(),
        initial_state: None,
    });
    if kind == "inputs" {
        if channels.inputs.len() <= 1 {
            // The view's input keeps the loop's actuator when the loop declares one, else a continuous one over the plant input's range.
            let actuator = channels.inputs.first().map_or_else(
                || continuous_actuator(loop_definition.signals.manipulated_variable.range),
                |entry| entry.actuator,
            );
            channels.inputs = vec![ChannelInput {
                model_channel_id: model_id,
                signal_id: signal_id.to_owned(),
                manual_output_signal: None,
                manual_output_type: None,
                applied_signal_id: None,
                actuator,
                experiment_step: None,
            }];
        }
    } else if channels.outputs.len() <= 1 {
        channels.outputs = vec![ChannelOutput {
            model_channel_id: model_id,
            signal_id: signal_id.to_owned(),
            setpoint_signal_id: None,
            requirements: None,
        }];
    }
}

/// A continuous actuator over `range`, with no rate or dwell limits: the view's plant input when the loop declares no channel for it.
fn continuous_actuator(range: Range) -> ActuatorCapabilities {
    ActuatorCapabilities {
        kind: ActuatorKind::Continuous,
        minimum: range.minimum,
        maximum: range.maximum,
        rate_up_per_s: None,
        rate_down_per_s: None,
        minimum_on_seconds: None,
        minimum_off_seconds: None,
    }
}

/// The refusal of a selected name that the recording does not hold.
fn not_present() -> Failure {
    Failure::new(
        "Choose input and output signals present in this recording.",
        Cause::Settings,
        NextAction::None,
    )
}

/// The loop signal named `symbol`: a primary signal, or a further signal of the loop as a [`LoopSignal`].
fn loop_signal_named(loop_definition: &LoopDefinition, symbol: &str) -> Option<LoopSignal> {
    let signals = &loop_definition.signals;
    let primary = signals
        .setpoint
        .iter()
        .chain([&signals.process_variable, &signals.manipulated_variable])
        .chain(signals.actuator.iter())
        .find(|signal| signal.symbol == symbol);
    if let Some(signal) = primary {
        return Some(signal.clone());
    }
    signals
        .additional
        .iter()
        .flatten()
        .find(|signal| signal.symbol == symbol)
        .map(|signal| LoopSignal {
            symbol: signal.symbol.clone(),
            address: signal.address.clone(),
            iec_type: signal.iec_type.clone(),
            direction: signal.direction,
            unit: signal.unit.clone(),
            range: signal.range,
        })
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
    let mut readings = Vec::new();
    for run in runs {
        let index = channel_index(loop_definition, run, id).ok_or_else(missing)?;
        readings.extend(channel_readings(run, index));
    }
    Ok(ModelChannel {
        id: id.to_string(),
        unit: unit.to_string(),
        operating_range: observed_range(id, readings)?,
    })
}

/// The value and quality each sample of `run` holds for the binding at `index`, in the order of the samples.
fn channel_readings(run: &RecordedRun, index: usize) -> Vec<(Option<f64>, Quality)> {
    run.samples
        .iter()
        .map(|sample| {
            (
                sample.values.get(index).copied().flatten(),
                sample.quality.get(index).copied().unwrap_or(Quality::Bad),
            )
        })
        .collect()
}

/// The operating range of a recorded channel as its samples took it (`selectedRecordingChannels`): the smallest and the largest good
/// value, widened by `padding_of` when the samples took one value only. Observed ranges describe the recorded data and grant no
/// excitation limit (B3.6).
///
/// # Errors
/// A sample of the channel has no value or is not good, or the channel has no sample.
fn observed_range(
    id: &str,
    readings: impl IntoIterator<Item = (Option<f64>, Quality)>,
) -> Result<Range, Failure> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for (value, quality) in readings {
        let Some(value) = value.filter(|value| quality == Quality::Good && value.is_finite())
        else {
            return Err(Failure::new(
                &format!("Recording signal {id} has incomplete or invalid samples."),
                Cause::Settings,
                NextAction::None,
            ));
        };
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    }
    if !minimum.is_finite() {
        return Err(Failure::new(
            &format!("Recording signal {id} has no samples."),
            Cause::Settings,
            NextAction::None,
        ));
    }
    let padding = padding_of(minimum, maximum);
    Ok(Range {
        minimum: minimum - padding,
        maximum: maximum + padding,
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
