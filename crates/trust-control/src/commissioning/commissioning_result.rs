//! The result of a commissioning run (specification 35, B6.3 and B6.7, C13.8 and C0): what a run stores and the engineer reads.
//! `completed_matches_checkpoint` decides whether a stored completed record is the receipt of a journalled campaign (A8.5), and
//! `valid_channels` is the channel rule of a stored result (`validCommissioning`, `workflowResult.ts`). The record is plain data:
//! the run that builds it is in `proportional_commissioning`, and the journal that keeps it is transport.
//!
//! `Failure` (failure.rs) derives `Serialize` only, so the stored `failure` is decoded here from the names its enums serialize to
//! (`deserialize_failure`), and the result needs no change to failure.rs.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::found_mode::FoundMode;
use super::gain_recovery::GainRecoveryContext;
use super::observation_assessment::JournalStage;
use super::parameter_status::{same_parameter_values, ParameterStatus};
use super::transfer::TransferValue;
use crate::failure::{Cause, Failure, NextAction};
use crate::recording::RecordedRun;
use crate::validation::commissioning_assessment::CommissioningCriteria;

/// How a commissioning run ended (`CommissioningResult.status`, B6.3): the response passed, a step failed, or the engineer
/// cancelled the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CommissioningStatus {
    /// The response met every requirement, and the loop stays in Auto (B6.3).
    Passed,
    /// A step was refused or a requirement failed (B6.3, B6.7).
    Failed,
    /// The engineer cancelled the run (A10.4).
    Cancelled,
}

/// A channel the run recorded: a signal's identifier and unit (`channels`, C13.8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommissioningChannel {
    /// The signal's symbol.
    pub id: String,
    /// The signal's unit.
    pub unit: String,
}

/// The status of a transfer, or of its last transaction (annex E6.3, E6.8): `idle` only for a transfer with no transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TransferState {
    /// No transaction is recorded.
    Idle,
    /// The transaction waits for its confirmation (E6.7).
    Pending,
    /// The output image committed the transaction (E6.7).
    Applied,
    /// The runtime refused the transaction before any write (E6.6).
    Rejected,
    /// A write happened and a later check or commit failed (E6.8).
    Failed,
    /// The transaction was cancelled before its commit (E6.10).
    Cancelled,
}

/// How a transaction was confirmed (annex E6.7): by the post-execution check, or by the cycle's output image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TransferConfirmation {
    /// The post-execution check passed; the transaction is not yet applied.
    PostSt,
    /// The cycle's output image committed the transaction (E6.7).
    OutputImage,
}

/// The last transaction of a transfer (annex E6.8), in the runtime's `snake_case` field names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TransferTransaction {
    /// The transaction's identifier.
    pub id: String,
    /// The program revision the transaction was made on.
    pub program_revision: String,
    /// The binding revision the transaction was made on.
    pub binding_revision: String,
    /// The transaction's status.
    pub status: TransferState,
    /// Why the transaction was refused, failed or cancelled, when it was.
    pub reason: Option<String>,
    /// The values the runtime observed when it checked the transaction.
    pub observed_values: Option<BTreeMap<String, TransferValue>>,
    /// How the transaction was confirmed, once it is.
    pub confirmation: Option<TransferConfirmation>,
    /// The values before the transaction, when the runtime kept them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_values: Option<BTreeMap<String, TransferValue>>,
    /// The values the transaction applied, when the runtime kept them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_values: Option<BTreeMap<String, f64>>,
    /// Whether the runtime returned every member it wrote to its value before the transaction (E6.7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restored: Option<bool>,
}

/// One transfer's status as the runtime reports it (annex E6.3), in the runtime's `snake_case` field names. Its values are
/// observations, never write authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TransferStatus {
    /// The program revision the transfer binding belongs to.
    pub program_revision: String,
    /// The transfer binding's revision.
    pub binding_revision: String,
    /// One value per member and per mode: a number, or a BOOL for a mode.
    pub values: BTreeMap<String, TransferValue>,
    /// The runtime clock of the reported scan, when the runtime gives one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_time_ns: Option<u64>,
    /// The status of the last transaction, or `idle`.
    pub status: TransferState,
    /// Why the last transaction stopped, when it did.
    pub reason: Option<String>,
    /// The tracking kinds the runtime supports, when it reports them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_tracking: Option<Vec<String>>,
    /// The last transaction, or nothing when there is none.
    pub transaction: Option<TransferTransaction>,
}

/// The identity of a transfer that the journal owns (`TransferOwner`, A8.5): its transaction identifier, program and binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransferOwner {
    /// The transaction's identifier.
    pub id: String,
    /// The program revision the transfer was made on.
    pub program: String,
    /// The binding revision the transfer was made on.
    pub binding: String,
}

/// The stored result of a commissioning run (`CommissioningResult`, specification 35, B6.3, B6.7, C13.8): the outcome, the
/// recorded response, the transfers and parameter record the run left, and the criteria and gain set it was judged by. Its
/// fields use the `TypeScript`'s `camelCase` names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommissioningResult {
    /// How the run ended.
    pub status: CommissioningStatus,
    /// The sentence the engineer reads (A11.1).
    pub reason: String,
    /// The recorded response, as the runtime returned it (E4.7), when the run recorded one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording: Option<RecordedRun>,
    /// The transfers the run made, oldest first.
    pub transfers: Vec<TransferStatus>,
    /// Whether the previous settings are confirmed back (B6.4, B6.7).
    pub recovery_confirmed: bool,
    /// The largest error of the settled samples, in the output's unit, once the response has been assessed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_error: Option<f64>,
    /// The reviewed target the response holds.
    pub target: f64,
    /// The parameter record as the runtime last reported it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<ParameterStatus>,
    /// The revision of the model the run was reviewed on.
    pub model_revision: String,
    /// The channels the recording holds, in the order the run records them.
    pub channels: Vec<CommissioningChannel>,
    /// The criteria the response was judged by (B6.3).
    pub criteria: CommissioningCriteria,
    /// The original gain set and the candidate, which a later Restore returns to (B6.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain_recovery: Option<GainRecoveryContext>,
    /// The typed failure with its next action (A11.2), when the run failed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_failure"
    )]
    pub failure: Option<Failure>,
    /// The mode the loop was found in: Restore returns it there (A6.2, B6.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub found_mode: Option<FoundMode>,
}

/// A stored failure as its JSON names it (`Failure`, failure.rs): its message, its detail when it has one, and its cause and next
/// action by their serialized names.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredFailure {
    message: String,
    #[serde(default)]
    detail: Option<String>,
    cause: String,
    next_action: String,
    /// The words of the next action, which a stored failure carries as it was written; they follow from the next action.
    #[serde(default)]
    #[allow(dead_code)]
    // Read only to accept the stored field; the label is derived from the next action again.
    next_action_label: Option<String>,
}

/// Decodes the stored `failure` of a result. The cause and the next action decode by the names they serialize to, so a name that
/// no variant serializes to is refused.
fn deserialize_failure<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Failure>, D::Error> {
    use serde::de::Error as _;
    let Some(stored) = Option::<StoredFailure>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let cause = cause_named(&stored.cause)
        .ok_or_else(|| D::Error::custom("the failure has an unknown cause"))?;
    let next_action = next_action_named(&stored.next_action)
        .ok_or_else(|| D::Error::custom("the failure has an unknown next action"))?;
    Ok(Some(Failure::from_parts(
        stored.message,
        stored.detail,
        cause,
        next_action,
    )))
}

/// Every cause of a failure (failure.rs).
const CAUSES: [Cause; 8] = [
    Cause::ModelOrData,
    Cause::Settings,
    Cause::ControllerCapability,
    Cause::TaskInfeasible,
    Cause::Implementation,
    Cause::Runtime,
    Cause::Approval,
    Cause::Unclassified,
];

/// Every next action of a failure (failure.rs).
const NEXT_ACTIONS: [NextAction; 13] = [
    NextAction::ConnectRuntime,
    NextAction::OpenLoopSetup,
    NextAction::OpenLimits,
    NextAction::OpenRequirements,
    NextAction::OpenChannels,
    NextAction::ChooseMethod,
    NextAction::Identify,
    NextAction::Tune,
    NextAction::Validate,
    NextAction::Approve,
    NextAction::Restore,
    NextAction::Wait,
    NextAction::None,
];

/// The cause a serialized name stands for.
fn cause_named(name: &str) -> Option<Cause> {
    CAUSES
        .into_iter()
        .find(|cause| serde_json::to_value(cause).is_ok_and(|value| value == name))
}

/// The next action a serialized name stands for.
fn next_action_named(name: &str) -> Option<NextAction> {
    NEXT_ACTIONS
        .into_iter()
        .find(|action| serde_json::to_value(action).is_ok_and(|value| value == name))
}

/// The parts of a journalled commissioning checkpoint that the completion rule reads (`CommissioningCheckpoint`, A8.5). The
/// journal's own record is transport; this is its typed view for the rule.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointView {
    /// The journal's stage.
    pub stage: JournalStage,
    /// The revision of the model the campaign was reviewed on.
    pub model_revision: String,
    /// The program revision of the parameter record before Apply (the origin).
    pub origin_program: String,
    /// The binding revision of the parameter record before Apply (the origin).
    pub origin_binding: String,
    /// The identifier of the parameter transaction that wrote the candidate.
    pub parameter_operation_id: String,
    /// The candidate gain set the campaign wrote.
    pub candidate: BTreeMap<String, f64>,
    /// The Auto transfer the journal owns, once one was made.
    pub transfer: Option<TransferOwner>,
}

/// Whether a stored completed record is the receipt of this checkpoint (`completedCommissioningMatchesCheckpoint`, A8.5). Only a
/// completed checkpoint whose record passed and confirmed its recovery, that names the same model, parameter transaction, origin
/// and candidate, and that holds an output-image transfer under the checkpoint's own transfer identity, is authority for a
/// restored result. Every other receipt is refused.
#[must_use]
pub fn completed_matches_checkpoint(
    receipt: Option<&CommissioningResult>,
    checkpoint: &CheckpointView,
) -> bool {
    let Some(receipt) = receipt else {
        return false;
    };
    let Some(owner) = checkpoint.transfer.as_ref() else {
        return false;
    };
    let Some(parameters) = receipt.parameters.as_ref() else {
        return false;
    };
    checkpoint.stage == JournalStage::Completed
        && receipt.status == CommissioningStatus::Passed
        && receipt.recovery_confirmed
        && receipt.model_revision == checkpoint.model_revision
        && parameters.program_revision == checkpoint.origin_program
        && parameters.binding_revision == checkpoint.origin_binding
        && parameters.transaction_id.as_deref() == Some(checkpoint.parameter_operation_id.as_str())
        && same_parameter_values(&parameters.values, &checkpoint.candidate)
        && receipt.transfers.iter().any(|transfer| {
            transfer.transaction.as_ref().is_some_and(|transaction| {
                transaction.id == owner.id
                    && transaction.program_revision == owner.program
                    && transaction.binding_revision == owner.binding
                    && transaction.confirmation == Some(TransferConfirmation::OutputImage)
            })
        })
}

/// Whether a result's channels are usable (`validCommissioning`, `workflowResult.ts`; C13.8): three to 32 channels, each with an
/// identifier that is not blank, and no identifier listed twice. A recording of the loop's extra signals keeps them all.
#[must_use]
pub fn valid_channels(channels: &[CommissioningChannel]) -> bool {
    let identifiers: BTreeSet<&str> = channels.iter().map(|channel| channel.id.as_str()).collect();
    (3..=32).contains(&channels.len())
        && identifiers.len() == channels.len()
        && channels.iter().all(|channel| !channel.id.trim().is_empty())
}

/// A transfer value in the JSON of a status or a transaction: a finite number, or a BOOL for a mode (annex E6.3). The impls are
/// written by hand, not derived with `untagged`: an untagged number does not decode under `arbitrary_precision` (`crate::json`).
impl Serialize for TransferValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Self::Number(number) => serializer.serialize_f64(number),
            Self::Boolean(flag) => serializer.serialize_bool(flag),
        }
    }
}

impl<'de> Deserialize<'de> for TransferValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::Bool(flag) => Ok(Self::Boolean(flag)),
            serde_json::Value::Number(number) => number
                .as_f64()
                .filter(|value| value.is_finite())
                .map(Self::Number)
                .ok_or_else(|| D::Error::custom("a transfer value is a finite number or a BOOL")),
            _ => Err(D::Error::custom(
                "a transfer value is a finite number or a BOOL",
            )),
        }
    }
}

impl JsonSchema for TransferValue {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("TransferValue")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({ "anyOf": [{ "type": "number" }, { "type": "boolean" }] })
    }
}

#[cfg(test)]
#[path = "commissioning_result_tests.rs"]
mod tests;
