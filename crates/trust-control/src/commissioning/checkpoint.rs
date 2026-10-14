//! The commissioning checkpoint and the mode checkpoint of an identification (specification 35, A6.2, A8.3 and A8.5; annex E11.2): the
//! records that Control Studio journals before it dispatches a commissioning write or changes an identification's mode, so that a
//! restart can find the operations it owns. A record is data. `decode_checkpoint` decides whether a stored record belongs to the loop,
//! and `CommissioningCheckpoint::view` gives the completion rule its typed parts. The journal file is transport
//! (`workflow/commissioning_journal.rs`). From `commissioningCheckpoint.ts` and `connectedIdentificationMode.ts`.

// The fixed type names of the V4 contract begin with the name of this module.
#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::commissioning_result::{CheckpointView, TransferOwner, TransferStatus};
use super::found_mode::FoundMode;
use super::observation_assessment::JournalStage;
use super::parameter_application::ParameterOwner;
use super::parameter_status::ParameterStatus;
use super::texts;
use super::transfer::TransferBinding;
use super::transfer_tracking::TransferTracking;
use crate::experiment::status::RecordingRecoveryOwner;
use crate::failure::{Cause, Failure, NextAction};
use crate::json::decode_value;
use crate::loop_definition::{LoopDefinition, RealType};
use crate::model::general::{decode_general_model, GeneralModel};
use crate::recording::RecordedRun;
use crate::validation::engineer_verification_plan::{engineer_requirements, EngineerRequirements};

/// The largest stored checkpoint, in bytes of its JSON text (the 16 MiB limit of `decodeCommissioningCheckpoint`).
const MAXIMUM_CHECKPOINT_BYTES: usize = 16 * 1024 * 1024;

/// The stage of a commissioning checkpoint (`CommissioningCheckpoint.stage`): how far the journalled operation got (A8.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckpointStage {
    /// The reviewed settings are prepared, and nothing is written yet.
    Prepared,
    /// The runtime records the observation.
    Recording,
    /// The parameter record is being written.
    Parameters,
    /// The controller transfer is being made.
    Transfer,
    /// The settings are applied and the commissioning completed; Restore still works from this record.
    Completed,
    /// The previous settings are restored and confirmed.
    Recovered,
    /// The runtime was replaced, so the record is kept as history and grants nothing.
    Retired,
}

impl CheckpointStage {
    /// The journal stage that the completion rule names for this stage (`JournalStage`, A8.5).
    #[must_use]
    pub fn journal_stage(self) -> JournalStage {
        match self {
            Self::Prepared => JournalStage::Prepared,
            Self::Recording => JournalStage::Recording,
            Self::Parameters => JournalStage::Parameters,
            Self::Transfer => JournalStage::Transfer,
            Self::Completed => JournalStage::Completed,
            Self::Recovered => JournalStage::Recovered,
            Self::Retired => JournalStage::Retired,
        }
    }
}

/// The owner of the transfer that a mode checkpoint holds, with the binding it was made on (`transfer` of `IdentificationModeCheckpoint`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModeTransferOwner {
    /// The binding of the mode transfer.
    pub binding: TransferBinding,
    /// The identity of the mode transfer.
    pub owner: TransferOwner,
}

/// The journal record of an identification's mode change (`IdentificationModeCheckpoint`, A6.2, A8.6): the mode the loop was found in,
/// and the transfer that changed it, so that the loop returns to that mode when the campaign ends, fails or is cancelled, or after a
/// restart of the engine. It records the identity only; the restore is the caller's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentificationModeCheckpoint {
    /// The revision of the loop the mode belongs to.
    pub loop_revision: String,
    /// The transfer status the loop was found in.
    pub original: TransferStatus,
    /// The mode transfer that is owned, once one was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<ModeTransferOwner>,
}

/// The journal record of a commissioning run (`CommissioningCheckpoint`, A8.3, A8.5, E11.2). It holds what a recovery needs and no
/// authority: a recovery restores the previous settings by a fresh review, and never replays a write from this record (E11.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommissioningCheckpoint {
    /// The journal's identifier of the commissioning run.
    pub id: String,
    /// The revision of the loop the run was reviewed on.
    pub loop_revision: String,
    /// How far the run got.
    pub stage: CheckpointStage,
    /// The general model the run was reviewed on.
    pub model: GeneralModel,
    /// The requirements the run was reviewed against.
    pub task: EngineerRequirements,
    /// The parameter record before Apply: the origin that Restore returns to.
    pub origin: ParameterStatus,
    /// The candidate values that Apply wrote.
    pub candidate: BTreeMap<String, f64>,
    /// The identity of the parameter transaction that wrote the candidate.
    pub parameter_operation: ParameterOwner,
    /// The reference values the loop held before Apply.
    pub references: BTreeMap<String, f64>,
    /// The transfer binding of the controller.
    pub binding: TransferBinding,
    /// The tracking that a recovery to Manual holds.
    pub recovery_tracking: TransferTracking,
    /// The reviewed output change allowed by the recovery's transfers (E6.9).
    pub tolerance: f64,
    /// The program revision after a replacement of the runtime, when one was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_program_revision: Option<String>,
    /// The transfer that the run owns, once one was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<TransferOwner>,
    /// The recording that the run owns, once one was started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording: Option<RecordingRecoveryOwner>,
    /// The recording as the runtime returned it, once it was stopped and kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_result: Option<RecordedRun>,
    /// The mode the loop was found in at review (A6.2). A checkpoint without one has no mode, and its restore stays in Manual.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub found_mode: Option<FoundMode>,
    /// The measured output's noise that Apply's Auto entry allowed for (A6.4), so that a restore's return to Auto allows the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_noise: Option<f64>,
    /// The identity of the validation that the run's review relied on (A8.4).
    pub validation_identity: String,
    /// The transfers the runtime confirmed before the target change, journaled before it (B6.3): a reattached observation's result
    /// carries them, with the target change as the runtime still reports it under its journaled identity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transfers: Vec<TransferStatus>,
}

impl CommissioningCheckpoint {
    /// The typed parts of this checkpoint that the completion rule reads (`completedCommissioningMatchesCheckpoint`, A8.5).
    #[must_use]
    pub fn view(&self) -> CheckpointView {
        CheckpointView {
            stage: self.stage.journal_stage(),
            model_revision: self.model.identity().revision.clone(),
            origin_program: self.origin.program_revision.clone(),
            origin_binding: self.origin.binding_revision.clone(),
            parameter_operation_id: self.parameter_operation.id.clone(),
            candidate: self.candidate.clone(),
            transfer: self.transfer.clone(),
        }
    }
}

/// Decodes a stored checkpoint and checks that it belongs to `loop_definition` (`decodeCommissioningCheckpoint`, A8.5): its revision is
/// the loop's, each part decodes, the parameter values and the candidate hold the loop's declared parameters within their bounds (a REAL
/// value in single precision), and every owner names the origin's program and binding. A record that fails a rule grants no authority.
///
/// # Errors
/// The record is larger than the limit, does not decode, or does not belong to this loop. The failure says so and names no value.
pub fn decode_checkpoint(
    value: &Value,
    loop_definition: &LoopDefinition,
) -> Result<CommissioningCheckpoint, Failure> {
    let mismatch = || {
        Failure::new(
            &texts::checkpoint_does_not_match_loop(),
            Cause::Implementation,
            NextAction::None,
        )
    };
    let within_limit =
        serde_json::to_string(value).is_ok_and(|text| text.len() <= MAXIMUM_CHECKPOINT_BYTES);
    if !within_limit {
        return Err(mismatch());
    }
    let checkpoint: CommissioningCheckpoint =
        decode_value(value.clone()).map_err(|_| mismatch())?;
    if !belongs_to_loop(&checkpoint, loop_definition)
        || !checkpoint_values_hold(&checkpoint, loop_definition)
        || value
            .get("task")
            .is_none_or(|task| engineer_requirements(task).is_err())
        || value
            .get("model")
            .is_none_or(|model| decode_general_model(model).is_none())
    {
        return Err(mismatch());
    }
    Ok(checkpoint)
}

/// Whether the record names this loop, its revision and its runtime, and its owners name the origin's program and binding.
fn belongs_to_loop(checkpoint: &CommissioningCheckpoint, loop_definition: &LoopDefinition) -> bool {
    let origin = &checkpoint.origin;
    let origin_set = [
        &origin.program_revision,
        &origin.binding_revision,
        &origin.active_revision,
    ]
    .iter()
    .all(|revision| !revision.is_empty());
    let owners_match = checkpoint.parameter_operation.program == origin.program_revision
        && checkpoint.parameter_operation.binding == origin.binding_revision
        && !checkpoint.parameter_operation.id.is_empty()
        && checkpoint.transfer.as_ref().is_none_or(|owner| {
            owner.program == origin.program_revision
                && !owner.binding.is_empty()
                && !owner.id.is_empty()
        });
    let recording_belongs = checkpoint.recording.as_ref().is_none_or(|recording| {
        recording.loop_revision == loop_definition.revision
            && recording.loop_id == loop_definition.id
            && recording.project_id == loop_definition.project_id
            && recording.runtime_id == loop_definition.runtime_id
            && !recording.recording_id.is_empty()
    });
    checkpoint.loop_revision == loop_definition.revision
        && checkpoint.binding.loop_id == loop_definition.id
        && origin_set
        && owners_match
        && recording_belongs
        && !checkpoint.id.is_empty()
        && !checkpoint.validation_identity.is_empty()
}

/// Whether the record's values, its references, its tolerance and its process noise are of the form the rules allow.
fn checkpoint_values_hold(
    checkpoint: &CommissioningCheckpoint,
    loop_definition: &LoopDefinition,
) -> bool {
    let origin = &checkpoint.origin;
    parameter_values_hold(loop_definition, &origin.values)
        && origin
            .previous_values
            .as_ref()
            .is_none_or(|values| parameter_values_hold(loop_definition, values))
        && parameter_values_hold(loop_definition, &checkpoint.candidate)
        && !checkpoint.references.is_empty()
        && checkpoint.tolerance >= 0.0
        && checkpoint.process_noise.is_none_or(|noise| noise >= 0.0)
}

/// Whether `values` holds exactly the loop's declared parameters, each finite and within its bounds, and each REAL value exact in single
/// precision (`validValues` of `decodeCommissioningCheckpoint`; C0).
fn parameter_values_hold(loop_definition: &LoopDefinition, values: &BTreeMap<String, f64>) -> bool {
    loop_definition
        .parameter_binding
        .as_ref()
        .is_some_and(|binding| {
            values.len() == binding.parameters.len()
                && binding.parameters.iter().all(|parameter| {
                    values.get(&parameter.symbol).is_some_and(|&candidate| {
                        candidate.is_finite()
                            && candidate >= parameter.minimum
                            && candidate <= parameter.maximum
                            && (parameter.iec_type != RealType::Real
                                || is_single_precision(candidate))
                    })
                })
        })
}

/// Whether `value` survives the rounding to single precision unchanged (`Math.fround(value) === value`).
#[allow(clippy::cast_possible_truncation, clippy::float_cmp)] // The rounding is the test: the value must come through it unchanged.
fn is_single_precision(value: f64) -> bool {
    f64::from(value as f32) == value
}

#[cfg(test)]
#[path = "checkpoint_tests.rs"]
mod tests;
