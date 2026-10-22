//! The controlled outputs of the Requirements editor (specification 35, B4.3 and C7): the outputs of a loop with several that the
//! editor lists, each with the requirements its rows start from. The open answer carries them, and the page lists the outputs from that
//! answer. It decides neither which outputs have requirements nor what a row starts from.

use schemars::JsonSchema;
use serde::Serialize;

use crate::identification::connected_run::loop_signal_of;
use crate::loop_definition::{ChannelOutput, LoopDefinition};
use crate::tuning::output_requirements::OutputRequirements;
use crate::tuning::requirements::TuningRequirements;

/// One controlled output of the Requirements editor (B4.3, C7).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RequirementOutput {
    /// The output's identifier in the model: the channel its record is kept under in the requirements' `channelRequirements`, and the
    /// channel the editor names its rows with.
    pub channel_id: String,
    /// The identifier of the loop signal the output measures. `liveSides` names that signal's variable and unit.
    pub signal_id: String,
    /// The requirements the output is held to when the loop opens, which its rows start from: its own record in the requirements the
    /// study holds, else the loop's declared requirements for the channel, else, for the loop's process variable, the record's own
    /// values (the order Validate reads them). Absent when the output is held to none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirements: Option<OutputRequirements>,
}

/// The controlled outputs of the Requirements editor (B4.3, C7), in channel order: the outputs that declare a setpoint, or every output
/// when none does. An output whose signal the loop does not declare is left out, as `live_sides` leaves it out. A loop without channels
/// lists none: its requirements are the record's own. `held` is the requirements the study holds (the inputs' record, else the one Tune
/// designed to).
#[must_use]
pub fn requirement_outputs(
    loop_definition: &LoopDefinition,
    held: Option<&TuningRequirements>,
) -> Vec<RequirementOutput> {
    let Some(channels) = &loop_definition.channels else {
        return Vec::new();
    };
    let declared: Vec<&ChannelOutput> = channels
        .outputs
        .iter()
        .filter(|output| loop_signal_of(loop_definition, &output.signal_id).is_some())
        .collect();
    let controlled: Vec<&ChannelOutput> = declared
        .iter()
        .copied()
        .filter(|output| output.setpoint_signal_id.is_some())
        .collect();
    let listed = if controlled.is_empty() {
        declared
    } else {
        controlled
    };
    listed
        .into_iter()
        .map(|output| RequirementOutput {
            channel_id: output.model_channel_id.clone(),
            signal_id: output.signal_id.clone(),
            requirements: held_by(loop_definition, held, output),
        })
        .collect()
}

/// The requirements an output is held to (B4.3, C7), in the order Validate reads them (`output_task_of`): its own record in the record's
/// `channelRequirements`, else the channel's declared requirements, else, for the loop's process variable, the record's own values.
fn held_by(
    loop_definition: &LoopDefinition,
    held: Option<&TuningRequirements>,
    output: &ChannelOutput,
) -> Option<OutputRequirements> {
    let own = held
        .and_then(|record| record.channel_requirements.as_ref())
        .and_then(|records| records.get(&output.model_channel_id));
    if let Some(own) = own {
        return Some(*own);
    }
    if let Some(declared) = output.requirements {
        return Some(OutputRequirements::of_channel(declared));
    }
    let process_variable = loop_signal_of(loop_definition, &output.signal_id)
        .is_some_and(|signal| signal.symbol == loop_definition.signals.process_variable.symbol);
    held.filter(|_| process_variable)
        .map(OutputRequirements::of_record)
}

#[cfg(test)]
#[path = "requirement_outputs_tests.rs"]
mod tests;
