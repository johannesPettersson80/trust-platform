//! What the Validate page offers (specification 35, B5.4, B6.4, B6.5, B7 and A8.6; the Validate page specification, "Restore and
//! restart"): Apply, Restore previous settings, Keep in project, the Resume of an interrupted Apply observation, and the action that
//! holds the page's one position. Every answer that states them states them by these rules, from the Apply checkpoint the loop's journal
//! holds ([`JournaledApply`]), the study's validation, the validation the newest Apply applied ([`newest_applied_validation`]) and the
//! study's Apply receipts; the page shows the statement and keeps no rule of its own.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::application_records::ReviewedAction;
use super::checkpoint::{CheckpointStage, CommissioningCheckpoint};
use super::observation_assessment::observation_in_progress;
use crate::stages::{blocked_stage_availability, Stage, StageAvailability, StageEvidence};

/// The Apply checkpoint that a loop's journal holds (A8.5, B6.4, B7), as the offers and the stages read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournaledApply {
    /// No Apply's checkpoint is journaled, or the journaled one is closed: its previous settings are back, or it was retired.
    None,
    /// An Apply's settings stand: its checkpoint completed, and Restore previous settings and Keep in project start from it (B6.4, B7).
    Standing,
    /// An Apply or a Restore of this engine works on the unresolved checkpoint now (A8.5): its operation runs, and the page follows it.
    InProgress,
    /// An Apply or a Restore stopped before its end and nothing works on its checkpoint: it waits for its Restore, or for its Resume while
    /// its observation may still record (B6.4, B6.7, A8.6; annex E11.7).
    Waiting {
        /// The observation may still record, so the Apply can be resumed to its end (A8.5, A8.6).
        resumable: bool,
    },
}

impl JournaledApply {
    /// The state of `checkpoint`, the Apply checkpoint the loop's journal holds (`None` when it holds none). `running` says that an
    /// operation of this engine works on the loop now: for an unresolved checkpoint, that is its Apply or its Restore.
    #[must_use]
    pub fn of(checkpoint: Option<&CommissioningCheckpoint>, running: bool) -> Self {
        let Some(checkpoint) = checkpoint else {
            return Self::None;
        };
        match checkpoint.stage {
            CheckpointStage::Recovered | CheckpointStage::Retired => Self::None,
            CheckpointStage::Completed => Self::Standing,
            CheckpointStage::Prepared
            | CheckpointStage::Recording
            | CheckpointStage::Parameters
            | CheckpointStage::Transfer => {
                if running {
                    Self::InProgress
                } else {
                    let recorded = checkpoint.recording.is_some();
                    Self::Waiting {
                        resumable: observation_in_progress(
                            checkpoint.stage.journal_stage(),
                            recorded,
                            recorded,
                        ),
                    }
                }
            }
        }
    }
}

/// The action that holds the Validate page's position (B7; the Validate page specification: Apply and Restore never compete as primary
/// actions): `apply` puts Apply in the position, `restore` puts Restore previous settings there, and `keep` makes Keep in project the
/// page's primary action while Restore previous settings keeps the position beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PrimaryAction {
    /// Apply the validated candidate (B6.1).
    Apply,
    /// Restore previous settings (B6.4).
    Restore,
    /// Keep in project (B6.5).
    Keep,
}

/// What the Validate page offers (B6.4, B6.5, B7, A8.6), each as the engine states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // The four offers the page shows, each the engine's statement.
#[allow(clippy::module_name_repetitions)] // `ApplicationOffers` names the module's subject.
pub struct ApplicationOffers {
    /// Apply ([`apply_offered`]).
    pub apply: bool,
    /// Restore previous settings: the journal holds an Apply's checkpoint that is not closed, which Restore starts from (B6.4).
    pub restore: bool,
    /// Keep in project: an Apply's settings stand, and the study's receipts allow it (B6.5, `study::apply`).
    pub keep: bool,
    /// The Resume of an interrupted Apply observation, which the engine follows to its end (A8.5, A8.6).
    pub resume: bool,
}

impl ApplicationOffers {
    /// The offers of a loop whose journal holds `journaled`, whose study's validation of its current candidate passed with the identity
    /// `passed_validation` (`None` when no validation of the current candidate passed), whose newest Apply applied the validation
    /// `applied_validation` ([`newest_applied_validation`]; `None` when no Apply is recorded), and whose Apply receipts allow Keep in
    /// project (`keep_receipts`, `ApplySection::keep_available`).
    #[must_use]
    pub fn of(
        journaled: JournaledApply,
        passed_validation: Option<&str>,
        applied_validation: Option<&str>,
        keep_receipts: bool,
    ) -> Self {
        Self {
            apply: apply_offered(journaled, passed_validation, applied_validation),
            restore: !matches!(journaled, JournaledApply::None),
            keep: keep_receipts && matches!(journaled, JournaledApply::Standing),
            resume: matches!(journaled, JournaledApply::Waiting { resumable: true }),
        }
    }

    /// The action that holds the position for these offers ([`primary_action`]).
    #[must_use]
    pub const fn primary_action(self) -> Option<PrimaryAction> {
        primary_action(self.apply, self.restore, self.keep)
    }
}

/// Whether Apply is offered (B5.4, B7): the one rule of every answer that states it. The study's validation of the current candidate
/// passed (`passed_validation` names its identity), no Apply's checkpoint is unresolved, and the newest Apply applied another validation
/// (`applied_validation`, [`newest_applied_validation`]), whether its settings stand, were restored or were retired: Apply is not offered
/// again for the validation an Apply applied, and a candidate that passes validation after the Apply, or after its Restore, may be
/// applied (B7; the Validate page specification: "A new candidate must pass validation before this action can offer Apply for that
/// candidate"). So the answers after an Apply or a Restore state what its end stated ([`apply_offered_after`]).
#[must_use]
pub fn apply_offered(
    journaled: JournaledApply,
    passed_validation: Option<&str>,
    applied_validation: Option<&str>,
) -> bool {
    let Some(validation) = passed_validation else {
        return false;
    };
    match journaled {
        JournaledApply::None | JournaledApply::Standing => applied_validation != Some(validation),
        JournaledApply::InProgress | JournaledApply::Waiting { .. } => false,
    }
}

/// The identity of the validation that the newest Apply of the loop applied (A8.4, A8.5, B7), whatever became of its settings since.
/// The checkpoint the loop's journal holds (`journaled`), at any stage, is the newest Apply's, because an Apply replaces the checkpoint
/// an earlier one left before it journals its own; with none journaled, the study's newest Apply receipt names it (`newest_receipt`,
/// `ApplySection::newest_validation`). `None` when neither records an Apply.
#[must_use]
pub fn newest_applied_validation<'a>(
    journaled: Option<&'a CommissioningCheckpoint>,
    newest_receipt: Option<&'a str>,
) -> Option<&'a str> {
    journaled
        .map(|checkpoint| checkpoint.validation_identity.as_str())
        .or(newest_receipt)
}

/// The action that holds the position (B7): Apply while it is offered, then Keep in project once it is offered after an Apply, else
/// Restore previous settings while it is offered; none when nothing is offered. Keep in project is offered only while an Apply's settings
/// stand, and Restore previous settings is offered then too, so it keeps the position beside the primary Keep in project.
#[must_use]
pub const fn primary_action(apply: bool, restore: bool, keep: bool) -> Option<PrimaryAction> {
    if apply {
        Some(PrimaryAction::Apply)
    } else if keep {
        Some(PrimaryAction::Keep)
    } else if restore {
        Some(PrimaryAction::Restore)
    } else {
        None
    }
}

/// Whether Apply is offered once `ended` has ended (B7): never. After an Apply its settings stand and Restore previous settings holds
/// the position, or the Apply restored the previous settings at once; after a Restore the position is Apply's again. In each case the
/// candidate needs a validation that passes after the application before Apply is offered for it (the Validate page specification: "A
/// new candidate must pass validation before this action can offer Apply for that candidate"), and the next Validate answer that
/// passes offers it ([`apply_offered`]). Every later answer states the same until then, because the validation the ended Apply applied
/// is the newest Apply's ([`newest_applied_validation`]).
#[must_use]
pub const fn apply_offered_after(ended: ReviewedAction) -> bool {
    match ended {
        ReviewedAction::Apply | ReviewedAction::Restore => false,
    }
}

/// The status of every stage in an answer that states the offers (B7, B9.4): the stages as `block` holds them
/// (`stages::blocked_stage_availability`), and the Apply stage, which can start exactly when Apply is offered (`apply`, by
/// [`apply_offered`]). The page enables Apply from the Apply stage of the answer it received last; an answer that states no offers
/// (Identify, Tune, a reset) leaves no validation current, so its Apply stage cannot start, as Apply is then not offered.
#[must_use]
pub fn offered_stages(
    evidence: &StageEvidence,
    block: Option<&'static str>,
    apply: bool,
) -> Vec<StageAvailability> {
    let mut stages = blocked_stage_availability(evidence, block);
    for stage in stages
        .iter_mut()
        .filter(|stage| stage.stage == Stage::Apply)
    {
        stage.available &= apply;
    }
    stages
}

#[cfg(test)]
#[path = "application_offers_tests.rs"]
mod tests;
