//! The mapping of the plant's signals to a loop, answered by the engine (specification 35, B1.2 and B3.6): which loop the selected
//! inputs and measured outputs name, the controller choices that mapping offers, the sentence of an ambiguous or unmatched mapping,
//! whether that mapping blocks the work, and whether a recording's selection has changed. The page shows the answer and sends the
//! loop choice it names; it keeps no matching rule of its own.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::discovery::{
    BindingSelection, BindingTarget, ControllerCompatibility, Discovery, SelectableSignal,
};
use crate::recording::opened_channels::RecordingSignalSelection;

/// The sentence of a mapping that more than one loop has, when the loop the panel shows is not one of them (B3.6).
const AMBIGUOUS: &str = "Select a controller for this plant mapping on Tune.";
/// The sentence of a mapping that no loop of the project has (B3.6).
const UNMATCHED: &str = "Choose the complete input and output mapping of a configured loop.";

/// The mapping the page sends with a binding request (B1.2, B3.6): the loop the panel shows, the plant inputs and the measured outputs
/// that the selectors name by symbol, and, while a recording is shown, the selection that the recording holds.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MappingRequest {
    /// The loop the panel shows.
    pub loop_id: String,
    /// The plant inputs that the inputs selector names, by symbol.
    pub inputs: Vec<String>,
    /// The measured outputs that the outputs selector names, by symbol.
    pub outputs: Vec<String>,
    /// The selection of the opened recording, while a recording is shown (B3.6); absent for a live mapping.
    #[serde(default)]
    pub recording_selection: Option<RecordingSignalSelection>,
}

/// The outcome of a mapping (B3.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MappingOutcome {
    /// The loop the panel shows has exactly the selected signals.
    Current,
    /// One other loop has exactly the selected signals, and the page chooses it.
    Other,
    /// More than one loop has them, and the loop the panel shows is not one of them: the engineer chooses the controller on Tune.
    Ambiguous,
    /// No loop has exactly the selected signals.
    Unmatched,
    /// The selectors name the channels of an opened recording, which no loop is matched by; `changed` says whether they differ from
    /// the recording's selection.
    Recorded,
}

/// One controller choice of a mapping (B3.6): a family that a loop's adapter realizes, with the selection that opens the loop.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MappingOption {
    /// The loop's id.
    pub loop_id: String,
    /// The family that the loop's adapter realizes.
    pub family: String,
    /// The label the selector shows: the controller's instance and the family.
    pub label: String,
    /// The selection that names the loop, its controller and its channels (B1.3).
    pub selection: BindingSelection,
}

/// The engine's answer to a mapping request (B1.2, B3.6).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MappingAnswer {
    /// What the mapping is.
    pub outcome: MappingOutcome,
    /// Whether the recording's selection differs from the selected one; false outside a recording.
    pub changed: bool,
    /// Whether the mapping stops the work until the engineer chooses a controller (B3.6): the ambiguous and the unmatched outcomes.
    /// The page gates its actions on this and keeps no rule of its own.
    pub blocks_run: bool,
    /// The sentence the page shows for an ambiguous or unmatched mapping; empty otherwise.
    pub sentence: String,
    /// The loop that the page chooses for the `other` outcome; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<BindingSelection>,
    /// The controller choices of the mapping, other than the loop the panel shows (B3.6).
    pub options: Vec<MappingOption>,
}

/// Answers a mapping request from the discovery (B1.2, B3.6). A loop has the selected signals when its input symbols and its output
/// symbols are the selected ones, each side in any order. The loop the panel shows wins; one other loop is chosen; more than one
/// without the current loop is ambiguous; none is unmatched. A selection that a recording holds is `recorded`, and says whether it changed.
#[must_use]
pub fn match_mapping(discovery: &Discovery, request: &MappingRequest) -> MappingAnswer {
    let matches: Vec<&BindingTarget> = discovery
        .targets
        .iter()
        .filter(|target| has_signals(target, request))
        .collect();
    let (outcome, sentence, selection) = match &request.recording_selection {
        Some(_) => (MappingOutcome::Recorded, String::new(), None),
        None => outcome_of(&matches, &request.loop_id),
    };
    let changed = request
        .recording_selection
        .as_ref()
        .is_some_and(|recorded| {
            recorded.inputs != request.inputs || recorded.outputs != request.outputs
        });
    let options = matches
        .iter()
        .copied()
        .filter(|target| target.loop_definition.id != request.loop_id)
        .flat_map(options_of)
        .collect();
    MappingAnswer {
        outcome,
        changed,
        blocks_run: matches!(
            outcome,
            MappingOutcome::Ambiguous | MappingOutcome::Unmatched
        ),
        sentence,
        selection,
        options,
    }
}

/// The choices of the loop the panel shows, answered from the loop's own signals when the panel names no mapping (B1.2, B3.6): the
/// loop has exactly its own signals, so the outcome is `current` and nothing blocks, and the choices are the other loops that bind the
/// same plant signals. `None` when the discovery does not list the loop.
#[must_use]
pub fn own_mapping(discovery: &Discovery, loop_id: &str) -> Option<MappingAnswer> {
    let shown = discovery
        .targets
        .iter()
        .find(|target| target.loop_definition.id == loop_id)?;
    Some(match_mapping(
        discovery,
        &MappingRequest {
            loop_id: loop_id.to_owned(),
            inputs: symbols_of(&shown.inputs),
            outputs: symbols_of(&shown.outputs),
            recording_selection: None,
        },
    ))
}

/// The symbols of a side's signals, in the order the loop offers them.
fn symbols_of(signals: &[SelectableSignal]) -> Vec<String> {
    signals.iter().map(|signal| signal.symbol.clone()).collect()
}

/// The outcome of a live mapping, its sentence and the selection of the one loop that is chosen.
fn outcome_of(
    matches: &[&BindingTarget],
    loop_id: &str,
) -> (MappingOutcome, String, Option<BindingSelection>) {
    if matches
        .iter()
        .any(|target| target.loop_definition.id == loop_id)
    {
        return (MappingOutcome::Current, String::new(), None);
    }
    match matches {
        [only] => (
            MappingOutcome::Other,
            String::new(),
            Some(selection_of(only)),
        ),
        [] => (MappingOutcome::Unmatched, UNMATCHED.to_owned(), None),
        _ => (MappingOutcome::Ambiguous, AMBIGUOUS.to_owned(), None),
    }
}

/// Whether a loop's input symbols and output symbols are the selected ones (`mappingMatches` in `threePageBindings.ts`).
fn has_signals(target: &BindingTarget, request: &MappingRequest) -> bool {
    same_symbols(&request.inputs, &target.inputs) && same_symbols(&request.outputs, &target.outputs)
}

/// Whether the selected symbols and a side's signals name the same symbols, each once, in any order.
fn same_symbols(selected: &[String], signals: &[SelectableSignal]) -> bool {
    selected.len() == signals.len()
        && selected
            .iter()
            .all(|symbol| signals.iter().any(|signal| signal.symbol == *symbol))
}

/// The selection that names a loop, its controller and every declared input and output (B1.3): the choice that opens the loop.
fn selection_of(target: &BindingTarget) -> BindingSelection {
    BindingSelection {
        loop_id: target.loop_definition.id.clone(),
        controller_instance: target.controller.instance.clone(),
        input_ids: target
            .inputs
            .iter()
            .map(|signal| signal.id.clone())
            .collect(),
        output_ids: target
            .outputs
            .iter()
            .map(|signal| signal.id.clone())
            .collect(),
    }
}

/// The controller choices of one loop: one per family that its adapter realizes. A loop whose adapter realizes none offers none.
fn options_of(target: &BindingTarget) -> Vec<MappingOption> {
    let ControllerCompatibility::Accepted { families, .. } = &target.controller_compatibility
    else {
        return Vec::new();
    };
    families
        .iter()
        .map(|family| MappingOption {
            loop_id: target.loop_definition.id.clone(),
            family: family.clone(),
            label: format!("{} · {family}", target.controller.instance),
            selection: selection_of(target),
        })
        .collect()
}

#[cfg(test)]
#[path = "mapping_match_tests.rs"]
mod tests;
