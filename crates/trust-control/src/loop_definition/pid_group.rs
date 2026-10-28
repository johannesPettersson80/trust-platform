//! The PID group of a loop (specification 35, C7; `pidControllerGroup.ts`, `validPidControllerGroup`): two to sixteen `Control.PID`
//! members, each on one input channel and one output channel of the loop, with the group's shared Auto/Manual signal and the
//! member's gain variables. The group is declared, never inferred from the selected signals. This module holds the declaration's
//! shape and its match with the loop's channels; the coherent transaction that writes the members is checked by the adapter
//! (`commissioning::group_adapter`).

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{LoopChannels, LoopDefinition, ProjectSeedBlock};

/// The fewest members a PID group declares (`validPidControllerGroup`).
pub const MINIMUM_MEMBERS: usize = 2;
/// The most members a PID group declares (`validPidControllerGroup`; numerics annex N10).
pub const MAXIMUM_MEMBERS: usize = 16;
/// The most symbols a member's list, or the group's, may hold (`symbols` in `pidControllerGroup.ts`).
const MAXIMUM_SYMBOLS: usize = 128;
/// The only type a group member may have (`pidControllerGroup.ts`).
pub const MEMBER_TYPE: &str = "Control.PID";

/// The refusal of a group whose shape or channels do not match (`validPidControllerGroup`).
const GROUP_SHAPE: &str = "controller.group must declare two to sixteen Control.PID members, each with one input channel and one output channel of the loop, each channel used once.";
/// The refusal of a group seed that names the group's members, when the loop declares no group.
const SEED_NEEDS_GROUP: &str =
    "A decentralized or decoupled seed needs the loop's PID group members (controller.group).";
/// The refusal of a member that repeats another member's instance.
const GROUP_INSTANCE: &str = "controller.group declares one member instance twice.";

/// A PID group (C7): its members, each a `Control.PID` on one input and one output channel of the loop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PidControllerGroup {
    /// The members, in the order that the loop's channels follow.
    pub members: Vec<PidGroupMember>,
}

/// One member of a PID group (C7): its instance, its input and output channel, the group's shared mode and its gain variables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PidGroupMember {
    /// The member's instance path, for example `P1.Zone1`.
    pub instance: String,
    /// Always `Control.PID`.
    #[serde(rename = "type")]
    pub type_name: String,
    /// The identifier of the model input that the member drives.
    pub input_channel_id: String,
    /// The identifier of the model output that the member measures.
    pub output_channel_id: String,
    /// The group's shared Auto/Manual signal, which every member names (C7 Setup).
    pub mode_symbol: String,
    /// The member's gain variables: the proportional, integral and derivative gains of its parameter record, by their paths.
    pub parameter_symbols: Vec<String>,
    /// The member's retained states that its program owns, when the declaration names them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_state_symbols: Option<Vec<String>>,
}

impl PidControllerGroup {
    /// Checks the declaration against the loop's channels (`validPidControllerGroup`): two to sixteen members, each a `Control.PID` with
    /// non-empty identifiers and distinct symbols, unique instances (compared in upper case), and one member for each input and each
    /// output channel of the loop, each channel used once.
    ///
    /// # Errors
    /// The declaration breaks one of these rules; the text names the rule.
    pub fn check(&self, channels: Option<&LoopChannels>) -> Result<(), String> {
        let Some(channels) = channels else {
            return Err(GROUP_SHAPE.to_owned());
        };
        if !(MINIMUM_MEMBERS..=MAXIMUM_MEMBERS).contains(&self.members.len()) {
            return Err(GROUP_SHAPE.to_owned());
        }
        for member in &self.members {
            member.check()?;
        }
        let instances: BTreeSet<String> = self
            .members
            .iter()
            .map(|member| member.instance.to_uppercase())
            .collect();
        if instances.len() != self.members.len() {
            return Err(GROUP_INSTANCE.to_owned());
        }
        let count = self.members.len();
        if channels.inputs.len() != count || channels.outputs.len() != count {
            return Err(GROUP_SHAPE.to_owned());
        }
        let inputs_covered = channels.inputs.iter().all(|channel| {
            self.members
                .iter()
                .filter(|member| member.input_channel_id == channel.model_channel_id)
                .count()
                == 1
        });
        let outputs_covered = channels.outputs.iter().all(|channel| {
            self.members
                .iter()
                .filter(|member| member.output_channel_id == channel.model_channel_id)
                .count()
                == 1
        });
        if inputs_covered && outputs_covered {
            Ok(())
        } else {
            Err(GROUP_SHAPE.to_owned())
        }
    }
}

impl PidGroupMember {
    /// The member's own rules (`validPidControllerGroup`): the instance and both channel identifiers are text, the type is
    /// `Control.PID`, the mode is text, the gain variables are a non-empty list of distinct symbols, and the application states, when
    /// they are declared, are distinct symbols too.
    fn check(&self) -> Result<(), String> {
        let member_shape = !self.instance.trim().is_empty()
            && self.type_name == MEMBER_TYPE
            && !self.input_channel_id.trim().is_empty()
            && !self.output_channel_id.trim().is_empty()
            && !self.mode_symbol.trim().is_empty()
            && !self.parameter_symbols.is_empty()
            && distinct_symbols(&self.parameter_symbols)
            && self
                .application_state_symbols
                .as_ref()
                .is_none_or(|symbols| distinct_symbols(symbols));
        if member_shape {
            Ok(())
        } else {
            Err(GROUP_SHAPE.to_owned())
        }
    }
}

/// Whether a list of symbols is short enough, holds only text, and repeats no symbol (compared in upper case).
fn distinct_symbols(symbols: &[String]) -> bool {
    let upper: BTreeSet<String> = symbols.iter().map(|symbol| symbol.to_uppercase()).collect();
    symbols.len() <= MAXIMUM_SYMBOLS
        && symbols.iter().all(|symbol| !symbol.trim().is_empty())
        && upper.len() == symbols.len()
}

impl LoopDefinition {
    /// Checks the loop's PID group, when it declares one, against the loop's channels (C7). Called with the other rules of the
    /// controller.
    ///
    /// # Errors
    /// The group breaks one of the rules of [`PidControllerGroup::check`].
    pub(super) fn check_pid_group(&self) -> Result<(), String> {
        let seed_needs_group = matches!(
            self.controller
                .project
                .as_ref()
                .and_then(|project| project.seed.as_ref()),
            Some(ProjectSeedBlock::Decentralized | ProjectSeedBlock::Decoupled(_))
        );
        match &self.controller.group {
            Some(group) => group.check(self.channels.as_ref()),
            None if seed_needs_group => Err(SEED_NEEDS_GROUP.to_owned()),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "pid_group_tests.rs"]
mod tests;
