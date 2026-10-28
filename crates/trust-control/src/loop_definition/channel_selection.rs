//! The channel selection of a multichannel loop (specification 35, A8.1, B10.1 and the Tune page's **Channels…**): the plant signal that
//! each model channel is bound to, chosen among the signals the loop declares for its side. The loop file declares the bindings; the
//! study keeps the engineer's selection, and every stage reads the loop with the selection applied ([`effective_loop`]), so tuning,
//! validation and Apply use the same mapping. The rules are the TypeScript's (`controllerChannelPreferences.ts`, `loopChannels.ts`
//! and `pidControllerGroup.ts`), now the engine's; `LoopChannels::check_bindings` holds the binding rules themselves.

#![allow(clippy::module_name_repetitions)] // The type names its concept: the channel selection.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{channel_signal, LoopDefinition};
use crate::failure::{Cause, Failure, NextAction};
use crate::study::StudySection;

/// The most channels a side may hold (`LoopChannels::check`).
const MAXIMUM_CHANNELS: usize = 16;

/// The refusal of a selection for a loop that declares no channels.
const NO_CHANNELS: &str = "This loop declares no multichannel binding to map.";

/// The refusal of a selection that does not name every declared channel once.
const MAP_EVERY: &str = "Map every selected plant channel.";

/// The refusal of a selection that gives two channels one signal.
const REPEATED: &str =
    "Choose a different signal; this signal is already assigned to another channel.";

/// The refusal of a signal that is not declared for its side, or whose unit differs from the declared one.
const REVIEW: &str =
    "Review the plant signal for {channel}; keep its declared limits and channel order.";

/// The refusal of a selection that changes the signal of a PID group's channel.
const GROUP_WIRING: &str =
    "Keep the PID group's declared signal {signal}; select a compatible controller group for different wiring.";

/// The refusal of a stored selection that breaks one of its own rules.
const INVALID_SECTION: &str = "The stored channel selection is not valid.";

/// The plant signal one model channel is bound to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChannelPick {
    /// The model channel's identifier.
    pub model_channel_id: String,
    /// The identifier of the loop signal the channel is bound to.
    pub signal_id: String,
}

/// The engineer's selection of plant signals for the model channels, by side (A8.1). Each side names every declared channel once.
/// The engine keeps it as the study's channel mapping, and `controlStudio/inputs` records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChannelSelection {
    /// The model inputs: one pick for each declared input.
    pub inputs: Vec<ChannelPick>,
    /// The model outputs: one pick for each declared output.
    pub outputs: Vec<ChannelPick>,
}

/// One signal a model channel can be bound to, as the Channels editor names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SignalChoice {
    /// The signal's identifier in the loop.
    pub signal_id: String,
    /// The variable's path.
    pub symbol: String,
    /// The signal's unit; empty means dimensionless.
    pub unit: String,
}

/// One model channel of the Channels editor: the plant signal it has now, the signals of its side it may take, and the PID group
/// member that owns it, when the loop is a group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChannelRow {
    /// The model channel's identifier.
    pub model_channel_id: String,
    /// The PID group member's instance that owns the channel, for a group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The plant signal the channel is bound to now.
    pub signal_id: String,
    /// The signals of the channel's side, in channel order.
    pub choices: Vec<SignalChoice>,
}

/// The rows of the Channels editor (the Tune page's **Channels…**) for a multichannel loop: its inputs and outputs in channel order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChannelChoices {
    /// The model inputs, in channel order.
    pub inputs: Vec<ChannelRow>,
    /// The model outputs, in channel order.
    pub outputs: Vec<ChannelRow>,
}

impl StudySection for ChannelSelection {
    const KEY: &'static str = "channelMapping";

    /// Each side names at most sixteen channels, each once, with text identifiers; a stored selection that breaks this is never read.
    fn check(&self) -> Result<(), Failure> {
        for side in [&self.inputs, &self.outputs] {
            let bounded = (1..=MAXIMUM_CHANNELS).contains(&side.len());
            let text = side.iter().all(|pick| {
                !pick.model_channel_id.trim().is_empty() && !pick.signal_id.trim().is_empty()
            });
            let distinct = |value: fn(&ChannelPick) -> &str| {
                let mut seen: Vec<&str> = side.iter().map(value).collect();
                seen.sort_unstable();
                seen.windows(2).all(|pair| pair[0] != pair[1])
            };
            if !bounded
                || !text
                || !distinct(|pick| pick.model_channel_id.as_str())
                || !distinct(|pick| pick.signal_id.as_str())
            {
                return Err(Failure::new(
                    INVALID_SECTION,
                    Cause::Settings,
                    NextAction::OpenChannels,
                ));
            }
        }
        Ok(())
    }

    fn recording_ids(&self) -> Vec<&str> {
        Vec::new()
    }
}

/// The loop the stages read: the declared loop with the study's selection applied to its channels (A8.1). Without a selection the
/// declared loop is returned unchanged. A selection that does not fit the declared channels, or that leaves a binding rule broken,
/// is refused with the engine's sentence.
///
/// # Errors
/// The selection breaks a rule of `check_selection`, or the bindings it leaves break a rule of `LoopChannels::check_bindings`.
pub fn effective_loop(
    declared: &LoopDefinition,
    selection: Option<&ChannelSelection>,
) -> Result<LoopDefinition, Failure> {
    let Some(selection) = selection else {
        return Ok(declared.clone());
    };
    check_selection(declared, selection)?;
    let effective = applied(declared, selection);
    if let Some(channels) = &effective.channels {
        channels
            .check_bindings(&effective)
            .map_err(|sentence| refuse(&sentence))?;
    }
    Ok(effective)
}

/// Checks a selection against the declared channels of the loop: each side names every declared channel once, no two channels share a
/// signal, each signal is one the loop declares for that side with the channel's unit, and a PID group keeps its declared wiring.
///
/// # Errors
/// The loop declares no channels, or the selection breaks one of these rules; the sentence names the rule and the row.
pub fn check_selection(
    declared: &LoopDefinition,
    selection: &ChannelSelection,
) -> Result<(), Failure> {
    let channels = declared
        .channels
        .as_ref()
        .ok_or_else(|| Failure::new(NO_CHANNELS, Cause::Settings, NextAction::None))?;
    let inputs: Vec<(&str, &str)> = channels
        .inputs
        .iter()
        .map(|input| (input.model_channel_id.as_str(), input.signal_id.as_str()))
        .collect();
    let outputs: Vec<(&str, &str)> = channels
        .outputs
        .iter()
        .map(|output| (output.model_channel_id.as_str(), output.signal_id.as_str()))
        .collect();
    check_side(declared, &inputs, &selection.inputs)?;
    check_side(declared, &outputs, &selection.outputs)
}

/// The rows of the Channels editor for a multichannel loop, as the loop's channels now bind them (the loop the stages read, so a
/// selection shows as chosen). Absent for a loop with one input and one output, which has no mapping to choose.
#[must_use]
pub fn channel_choices(loop_definition: &LoopDefinition) -> Option<ChannelChoices> {
    let channels = loop_definition.channels.as_ref()?;
    if channels.inputs.len() < 2 && channels.outputs.len() < 2 {
        return None;
    }
    let members = loop_definition
        .controller
        .group
        .as_ref()
        .map_or(&[][..], |group| group.members.as_slice());
    let input_signals: Vec<&str> = channels
        .inputs
        .iter()
        .map(|input| input.signal_id.as_str())
        .collect();
    let output_signals: Vec<&str> = channels
        .outputs
        .iter()
        .map(|output| output.signal_id.as_str())
        .collect();
    let inputs = channels
        .inputs
        .iter()
        .map(|input| ChannelRow {
            model_channel_id: input.model_channel_id.clone(),
            owner: members
                .iter()
                .find(|member| member.input_channel_id == input.model_channel_id)
                .map(|member| member.instance.clone()),
            signal_id: input.signal_id.clone(),
            choices: side_choices(loop_definition, &input_signals),
        })
        .collect();
    let outputs = channels
        .outputs
        .iter()
        .map(|output| ChannelRow {
            model_channel_id: output.model_channel_id.clone(),
            owner: members
                .iter()
                .find(|member| member.output_channel_id == output.model_channel_id)
                .map(|member| member.instance.clone()),
            signal_id: output.signal_id.clone(),
            choices: side_choices(loop_definition, &output_signals),
        })
        .collect();
    Some(ChannelChoices { inputs, outputs })
}

/// Checks one side of a selection against its declared pairs (model channel, signal), in channel order.
fn check_side(
    declared: &LoopDefinition,
    side: &[(&str, &str)],
    picks: &[ChannelPick],
) -> Result<(), Failure> {
    if picks.len() != side.len() {
        return Err(refuse(MAP_EVERY));
    }
    for (model, _) in side {
        if picks
            .iter()
            .filter(|pick| pick.model_channel_id == *model)
            .count()
            != 1
        {
            return Err(refuse(MAP_EVERY));
        }
    }
    for pick in picks {
        let Some((_, original)) = side
            .iter()
            .find(|(model, _)| *model == pick.model_channel_id)
        else {
            return Err(refuse(MAP_EVERY));
        };
        if picks
            .iter()
            .filter(|other| other.signal_id == pick.signal_id)
            .count()
            > 1
        {
            return Err(refuse(REPEATED));
        }
        if pick.signal_id != *original && declared.controller.group.is_some() {
            let symbol =
                channel_signal(declared, original).map_or(*original, |signal| signal.symbol);
            return Err(refuse(&GROUP_WIRING.replace("{signal}", symbol)));
        }
        let declared_here = side.iter().any(|(_, signal)| *signal == pick.signal_id);
        let same_unit = match (
            channel_signal(declared, &pick.signal_id),
            channel_signal(declared, original),
        ) {
            (Some(chosen), Some(before)) => chosen.unit == before.unit,
            _ => false,
        };
        if !declared_here || !same_unit {
            return Err(refuse(&REVIEW.replace("{channel}", &pick.model_channel_id)));
        }
    }
    Ok(())
}

/// The loop with the selection's signals in its channels, matched by model channel identifier.
fn applied(declared: &LoopDefinition, selection: &ChannelSelection) -> LoopDefinition {
    let mut effective = declared.clone();
    if let Some(channels) = effective.channels.as_mut() {
        for pick in &selection.inputs {
            if let Some(input) = channels
                .inputs
                .iter_mut()
                .find(|input| input.model_channel_id == pick.model_channel_id)
            {
                input.signal_id.clone_from(&pick.signal_id);
            }
        }
        for pick in &selection.outputs {
            if let Some(output) = channels
                .outputs
                .iter_mut()
                .find(|output| output.model_channel_id == pick.model_channel_id)
            {
                output.signal_id.clone_from(&pick.signal_id);
            }
        }
    }
    effective
}

/// The signals of one side, each with its symbol and unit, in the order the side lists them.
fn side_choices(loop_definition: &LoopDefinition, signal_ids: &[&str]) -> Vec<SignalChoice> {
    signal_ids
        .iter()
        .filter_map(|id| {
            channel_signal(loop_definition, id).map(|signal| SignalChoice {
                signal_id: (*id).to_owned(),
                symbol: signal.symbol.to_owned(),
                unit: signal.unit.to_owned(),
            })
        })
        .collect()
}

/// The refusal of a selection, with the next action that opens its editor (A11.2).
fn refuse(sentence: &str) -> Failure {
    Failure::new(sentence, Cause::Settings, NextAction::OpenChannels)
}

#[cfg(test)]
#[path = "channel_selection_tests.rs"]
mod tests;
