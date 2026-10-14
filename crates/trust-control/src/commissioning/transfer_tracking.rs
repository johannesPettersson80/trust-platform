//! The tracking of a transfer (specification 35, A6.3 and A6.4; annex E6.4): the controller states that a transfer's
//! binding tracks, and the law that starts from them. Ported from `proportionalTransfer.ts` (`proportionalTracking`,
//! `proportionalTrackingReference`) and `pidTransfer.ts` (`parallelPidTracking`, `pidTransferStates`). The
//! `declared-state-v1` tracking belongs to the PID initialization layer, which builds it from its own declarations.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::declared_initialization::DeclaredStateTracking;
use super::texts;
use super::transfer::{
    parallel_pid_contract, proportional_parameter, rounded_transfer_value, TransferBinding,
    TransferMember, TransferValue,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::json::decode_value;
use crate::loop_definition::{ControllerLaw, LoopDefinition};

/// The kind word of a proportional tracking record: always `proportional`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ProportionalKind {
    /// `proportional`.
    #[serde(rename = "proportional")]
    Proportional,
}

/// The kind word of a parallel PID tracking record: always `parallel-pid-v1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ParallelPidKind {
    /// `parallel-pid-v1`.
    #[serde(rename = "parallel-pid-v1")]
    ParallelPidV1,
}

/// The kind word of a declared-state tracking record: always `declared-state-v1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DeclaredStateKind {
    /// `declared-state-v1`.
    #[serde(rename = "declared-state-v1")]
    DeclaredStateV1,
}

/// The tracking of a proportional transfer (annex E6.4, `proportional`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProportionalTracking {
    /// The record kind.
    pub kind: ProportionalKind,
    /// The reference state.
    pub reference_symbol: String,
    /// The Manual command state.
    pub manual_command_symbol: String,
    /// The measured process value.
    pub process_variable: String,
    /// The gain, a guard.
    pub gain_symbol: String,
    /// Whether the reference is aligned to the held output at entry.
    pub align_reference: bool,
}

/// The tracking of a parallel PID transfer (annex E6.4, `parallel-pid-v1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ParallelPidTracking {
    /// The record kind.
    pub kind: ParallelPidKind,
    /// The reference state.
    pub reference_symbol: String,
    /// The Manual command state.
    pub manual_command_symbol: String,
    /// The measured process value.
    pub process_variable: String,
    /// The proportional gain, a guard.
    pub proportional_gain_symbol: String,
    /// The integral gain, a guard.
    pub integral_gain_symbol: String,
    /// The derivative gain, a guard.
    pub derivative_gain_symbol: String,
    /// The integral state.
    pub integral_state_symbol: String,
    /// The derivative state.
    pub derivative_state_symbol: String,
    /// β of the law, in [0, 1].
    pub proportional_setpoint_weight: f64,
    /// γ of the law, in [0, 1].
    pub derivative_setpoint_weight: f64,
    /// The task period in s.
    pub cycle_seconds: f64,
    /// Whether the output is aligned to the held output at entry.
    pub align_output: bool,
}

/// The tracking a transfer request carries (annex E6.4): one of the kinds the runtime supports. Serialized as the bare
/// record, which carries its own `kind`. Decoded by reading `kind` first, through `crate::json`, so no fraction of a
/// record is read through a tagged or untagged enum.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum TransferTracking {
    /// A proportional transfer.
    Proportional(ProportionalTracking),
    /// A parallel PID transfer.
    ParallelPid(ParallelPidTracking),
    /// A declared-state transfer, built by the PID initialization layer.
    DeclaredState(DeclaredStateTracking),
}

impl<'de> Deserialize<'de> for TransferTracking {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let value = serde_json::Value::deserialize(deserializer)?;
        let kind = value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let tracking = match kind.as_deref() {
            Some("proportional") => decode_value(value).map(Self::Proportional),
            Some("parallel-pid-v1") => decode_value(value).map(Self::ParallelPid),
            Some("declared-state-v1") => decode_value(value).map(Self::DeclaredState),
            _ => {
                return Err(D::Error::custom(
                    "the tracking kind is not one the runtime supports",
                ))
            }
        };
        tracking.map_err(D::Error::custom)
    }
}

/// The reference and the Manual command of a binding: the first two states, which every commissioning binding declares in
/// that order (A6.3).
fn state_pair(binding: &TransferBinding) -> Result<(&TransferMember, &TransferMember), Failure> {
    match (binding.states.first(), binding.states.get(1)) {
        (Some(reference), Some(manual)) => Ok((reference, manual)),
        _ => Err(Failure::new(
            &texts::transfer_configure_binding(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )),
    }
}

/// The tracking of a proportional transfer (`proportionalTracking`, A6.3): the reference and the Manual command are the
/// states the transfer writes, the process value and the gain are read, and the reference is aligned at entry or not.
///
/// # Errors
/// The binding lacks its reference and Manual command, or the loop has no proportional gain.
pub fn proportional_tracking(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    align_reference: bool,
) -> Result<ProportionalTracking, Failure> {
    let (reference, manual) = state_pair(binding)?;
    Ok(ProportionalTracking {
        kind: ProportionalKind::Proportional,
        reference_symbol: reference.symbol.clone(),
        manual_command_symbol: manual.symbol.clone(),
        process_variable: loop_definition.signals.process_variable.symbol.clone(),
        gain_symbol: proportional_parameter(loop_definition)?.symbol.clone(),
        align_reference,
    })
}

/// The reference that an aligned proportional Auto entry starts from, so that its first output equals the held output:
/// `r = PV + MV / Kp`, rounded as the reference is typed (`proportionalTrackingReference`, A6.3, E6.4).
///
/// # Errors
/// The gain is zero or not finite, or the reference does not fit its type and bounds.
pub fn proportional_tracking_reference(
    binding: &TransferBinding,
    process_value: f64,
    manual_value: f64,
    gain: f64,
) -> Result<f64, Failure> {
    if !gain.is_finite() || gain == 0.0 {
        return Err(Failure::new(
            &texts::transfer_gain_cannot_align(),
            Cause::Settings,
            NextAction::Tune,
        ));
    }
    let (reference, _) = state_pair(binding)?;
    rounded_transfer_value(reference, process_value + manual_value / gain)
}

/// The tracking of a parallel PID transfer (`parallelPidTracking`, A6.3, E6.4): the reference, the Manual command, the
/// process value, the three gains of the law, the integral and derivative states of the contract, its setpoint weights,
/// and the task period in s.
///
/// # Errors
/// The loop has no `parallel-pid-v1` contract, or the binding lacks its reference and Manual command.
pub fn parallel_pid_tracking(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    align_output: bool,
) -> Result<ParallelPidTracking, Failure> {
    let contract = parallel_pid_contract(loop_definition)?;
    let Some(ControllerLaw::Pid(law)) = loop_definition.controller.law.as_ref() else {
        return Err(Failure::new(
            &texts::transfer_pid_law_unavailable(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    };
    let (reference, manual) = state_pair(binding)?;
    Ok(ParallelPidTracking {
        kind: ParallelPidKind::ParallelPidV1,
        reference_symbol: reference.symbol.clone(),
        manual_command_symbol: manual.symbol.clone(),
        process_variable: loop_definition.signals.process_variable.symbol.clone(),
        proportional_gain_symbol: law.proportional_gain_symbol.clone(),
        integral_gain_symbol: law.integral_gain_symbol.clone(),
        derivative_gain_symbol: law.derivative_gain_symbol.clone(),
        integral_state_symbol: contract.integral_state.symbol.clone(),
        derivative_state_symbol: contract.derivative_state.symbol.clone(),
        proportional_setpoint_weight: contract.proportional_setpoint_weight,
        derivative_setpoint_weight: contract.derivative_setpoint_weight,
        cycle_seconds: loop_definition.controller.task_period_ms / 1000.0,
        align_output,
    })
}

/// The states that a PID transfer reports back, each rounded as its member is typed (`pidTransferStates`). Only the values
/// the runtime reports as numbers are carried: no state is guessed.
///
/// # Errors
/// The runtime reported no number for a declared state, or a value does not fit its state.
pub fn pid_transfer_states(
    binding: &TransferBinding,
    observed: &BTreeMap<String, TransferValue>,
) -> Result<BTreeMap<String, f64>, Failure> {
    binding
        .states
        .iter()
        .map(|member| {
            let Some(TransferValue::Number(value)) = observed.get(&member.symbol).copied() else {
                return Err(Failure::new(
                    &texts::transfer_pid_state_missing(&member.symbol),
                    Cause::Runtime,
                    NextAction::None,
                ));
            };
            Ok((
                member.symbol.clone(),
                rounded_transfer_value(member, value)?,
            ))
        })
        .collect()
}

#[cfg(test)]
#[path = "transfer_tracking_tests.rs"]
mod tests;
