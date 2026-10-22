//! The candidate of a PID group (specification 35, C7 and B4.1): each member's SIMC PI on its own path, the static decoupler `D` when
//! the group has one, and the value of each variable that Apply writes. It is unapplied, as every candidate is (N8.9). The designs are
//! [`crate::tuning::structures::pid_group`]'s; this module maps them to the group's own variables, by the member's declared symbols.
//!
//! A group keeps its values in one map, because a [`ProjectCandidate`](crate::tuning::project_candidate::ProjectCandidate) holds one
//! set of gains and a group has one set per member.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_run::loop_signal_of;
use crate::loop_definition::{LoopDefinition, PidControllerGroup, PidGroupMember};
use crate::tuning::candidate::CandidateSource;
use crate::tuning::structure::ParallelGains;
use crate::tuning::structures::pid_group::{DecentralizedMember, StaticDecoupler2x2Design};

/// A PID group's candidate (C7), unapplied: the members' designs, the decoupler when the group has one, and the value of each variable
/// that Apply writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupCandidate {
    /// Always false: a candidate is never applied by Tune (N8.9).
    pub applied: bool,
    /// Where the members' gains came from: the SIMC formula for every member (N5.2, N5.3).
    pub source: CandidateSource,
    /// Each member's design, in the group's member order.
    pub members: Vec<GroupMemberDesign>,
    /// The static decoupler `D = K⁻¹·diag(K)` and its channels, when the group's program applies one (C7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decoupler: Option<GroupDecoupler>,
    /// The value of each variable that Apply writes, by symbol: every member's proportional, integral and derivative gain, and each
    /// entry of the decoupler when the program has one (C7). Apply writes all of them in one transaction.
    pub values: BTreeMap<String, f64>,
}

/// One member's design (C7): its own path, its relative gain, the gain it sees with the other loops closed, and its PI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupMemberDesign {
    /// The member's instance path.
    pub instance: String,
    /// The identifier of the model input the member drives.
    pub input_channel_id: String,
    /// The identifier of the model output the member measures.
    pub output_channel_id: String,
    /// The name of the plant input the member drives: the symbol of the signal the loop binds to its input channel, else the channel's
    /// identifier (L19). Tune names it, so that the page resolves no signal; a candidate kept before holds none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_name: Option<String>,
    /// The name of the measured output the member measures, as `input_name` names the input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_name: Option<String>,
    /// The symbol of the member's proportional gain in its parameter record; the page shows it with the record's unit.
    pub proportional_symbol: String,
    /// The symbol of the member's integral gain in its parameter record; the page shows it with the record's unit.
    pub integral_symbol: String,
    /// The steady-state gain of the member's own path, `Kᵢᵢ` (N4.5).
    pub path_gain: f64,
    /// The relative gain of the pairing, `λᵢᵢ`; one through a decoupler (C7).
    pub relative_gain: f64,
    /// `Kᵢᵢ/λᵢᵢ`: the gain the member sees when the other loops are closed.
    pub effective_gain: f64,
    /// The time constant of the member's own path, in seconds (N5.15).
    pub time_constant_seconds: f64,
    /// The delay of the member's own path, in seconds (N5.15).
    pub delay_seconds: f64,
    /// The member's PI: its proportional and integral gains, and a derivative gain of zero.
    pub gains: ParallelGains,
}

/// The static decoupler of a group (C7): `D` by plant input (row) and member (column), the channels its rows and columns name, and
/// the two-member block's configuration when the group has two members.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupDecoupler {
    /// `D = K⁻¹·diag(K)`, by plant input (row) and member (column).
    pub matrix: Vec<Vec<f64>>,
    /// The members' input channel identifiers, in member order: the rows of `D`.
    pub input_channel_ids: Vec<String>,
    /// The names of the rows of `D`, in member order, as `input_name` of a member gives them. Tune names them; a candidate kept before
    /// holds none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_names: Option<Vec<String>>,
    /// The members' instance paths, in member order: the columns of `D`.
    pub member_instances: Vec<String>,
    /// The configuration of `Control.StaticDecoupler2x2` for two members (C7; spec 37 §4.2), absent for a larger group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<StaticDecoupler2x2Design>,
}

impl GroupCandidate {
    /// The candidate with each member's channels and the decoupler's rows named by the plant signals the loop binds to them (L19), so
    /// that the page resolves no signal of its own.
    #[must_use]
    pub fn named_by(&self, loop_definition: &LoopDefinition) -> Self {
        let mut named = self.clone();
        for member in &mut named.members {
            member.input_name = Some(channel_name(
                loop_definition,
                Side::Input,
                &member.input_channel_id,
            ));
            member.output_name = Some(channel_name(
                loop_definition,
                Side::Output,
                &member.output_channel_id,
            ));
        }
        if let Some(decoupler) = &mut named.decoupler {
            decoupler.input_names = Some(
                decoupler
                    .input_channel_ids
                    .iter()
                    .map(|id| channel_name(loop_definition, Side::Input, id))
                    .collect(),
            );
        }
        named
    }

    /// The rules that a stored PID group candidate holds (B4.1, B10.4): it is unapplied, and every number is finite.
    ///
    /// # Errors
    /// The candidate is marked applied, or holds a value that is not finite; the text names which.
    pub fn check_stored(&self) -> Result<(), &'static str> {
        if self.applied {
            return Err("its PID group candidate is marked applied, but the study never writes to the controller (B4.1)");
        }
        if !self.is_finite() {
            return Err("its PID group candidate holds a value that is not finite");
        }
        Ok(())
    }

    /// Whether every number of the candidate is finite, as a stored candidate must be (B10.4).
    #[must_use]
    pub fn is_finite(&self) -> bool {
        let member_numbers = self.members.iter().flat_map(|member| {
            [
                member.path_gain,
                member.relative_gain,
                member.effective_gain,
                member.time_constant_seconds,
                member.delay_seconds,
                member.gains.proportional_gain,
                member.gains.integral_gain,
                member.gains.derivative_gain,
            ]
        });
        let decoupler_numbers = self
            .decoupler
            .iter()
            .flat_map(|decoupler| decoupler.matrix.iter().flatten().copied());
        member_numbers
            .chain(decoupler_numbers)
            .chain(self.values.values().copied())
            .all(f64::is_finite)
    }
}

/// The side of the model a channel is on.
#[derive(Clone, Copy)]
enum Side {
    /// A model input.
    Input,
    /// A model output.
    Output,
}

/// The name a channel goes by in an answer (L19): the symbol of the plant signal that the loop binds to the channel, else the channel's
/// identifier when the loop declares no such channel or signal.
fn channel_name(loop_definition: &LoopDefinition, side: Side, channel_id: &str) -> String {
    let signal_id = loop_definition
        .channels
        .as_ref()
        .and_then(|channels| match side {
            Side::Input => channels
                .inputs
                .iter()
                .find(|input| input.model_channel_id == channel_id)
                .map(|input| input.signal_id.as_str()),
            Side::Output => channels
                .outputs
                .iter()
                .find(|output| output.model_channel_id == channel_id)
                .map(|output| output.signal_id.as_str()),
        });
    signal_id
        .and_then(|id| loop_signal_of(loop_definition, id))
        .map_or_else(|| channel_id.to_owned(), |signal| signal.symbol)
}

/// The member's design as the candidate keeps it: its declared instance, channels and gain symbols, and its design.
///
/// # Errors
/// The member's parameter symbols do not include its proportional or its integral gain.
pub fn member_design(
    declared: &PidGroupMember,
    design: &DecentralizedMember,
) -> Result<GroupMemberDesign, Failure> {
    Ok(GroupMemberDesign {
        instance: declared.instance.clone(),
        input_channel_id: declared.input_channel_id.clone(),
        output_channel_id: declared.output_channel_id.clone(),
        input_name: None,
        output_name: None,
        proportional_symbol: gain_symbol(declared, "ProportionalGain")?,
        integral_symbol: gain_symbol(declared, "IntegralGain")?,
        path_gain: design.path_gain,
        relative_gain: design.relative_gain,
        effective_gain: design.effective_gain,
        time_constant_seconds: design.time_constant_seconds,
        delay_seconds: design.delay_seconds,
        gains: design.gains,
    })
}

/// The values that Apply writes for each member's gains (C7): its proportional gain, its integral gain, and its derivative gain, which is
/// zero, each under the symbol of the member's own parameter record. A member that binds no such symbol is refused by name.
///
/// # Errors
/// A member's parameter symbols do not include one of the three gains.
pub fn member_values(
    group: &PidControllerGroup,
    designs: &[DecentralizedMember],
) -> Result<BTreeMap<String, f64>, Failure> {
    let mut values = BTreeMap::new();
    for (declared, design) in group.members.iter().zip(designs) {
        values.insert(
            gain_symbol(declared, "ProportionalGain")?,
            design.gains.proportional_gain,
        );
        values.insert(
            gain_symbol(declared, "IntegralGain")?,
            design.gains.integral_gain,
        );
        values.insert(gain_symbol(declared, "DerivativeGain")?, 0.0);
    }
    Ok(values)
}

/// The values of the decoupler's variables (C7): each entry `D[i][j]` under the symbol that the program names for plant input `i` and
/// member `j`.
#[must_use]
pub fn decoupler_values(symbols: &[Vec<String>], matrix: &[Vec<f64>]) -> BTreeMap<String, f64> {
    symbols
        .iter()
        .zip(matrix)
        .flat_map(|(row, values)| row.iter().zip(values))
        .map(|(symbol, value)| (symbol.clone(), *value))
        .collect()
}

/// The parameter symbol of one gain of a member, by its suffix (`projectController.ts`: `endsWith(".ProportionalGain")` and so on).
fn gain_symbol(member: &PidGroupMember, name: &str) -> Result<String, Failure> {
    let suffix = format!(".{name}");
    member
        .parameter_symbols
        .iter()
        .find(|symbol| symbol.ends_with(suffix.as_str()))
        .cloned()
        .ok_or_else(|| {
            Failure::new(
                &format!("{} binds no {name}.", member.instance),
                Cause::Settings,
                NextAction::OpenLoopSetup,
            )
        })
}

#[cfg(test)]
#[path = "group_candidate_tests.rs"]
mod tests;
