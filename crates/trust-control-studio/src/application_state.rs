//! What the loop's journal and study say about the Validate page's actions (specification 35, B5.4, B6.4, B6.5, B6.7, B7 and A8.6;
//! annex E11.7): the offers, the action that holds the page's position, and the block of the stages while an Apply's checkpoint waits
//! for its Restore. The rules are trust-control's (`commissioning::application_offers`, `commissioning::checkpoint_block` and
//! `stages::blocked_stage_availability`); this module reads the checkpoint the journal holds and the study's validation and receipts
//! (the receipts name the validation the newest Apply applied when the journal holds no checkpoint, B7) for every answer that states
//! the offers (open, the study inputs, Validate, Apply, Restore and Keep) and for the answers whose study may keep a validation current
//! (the run, the Recorded-data fit and a project test), and refuses the actions that the block holds.

use std::path::Path;

use trust_control::commissioning::application_offers::{
    newest_applied_validation, offered_stages, ApplicationOffers, JournaledApply, PrimaryAction,
};
use trust_control::commissioning::apply_texts::validation_already_applied;
use trust_control::commissioning::checkpoint::CommissioningCheckpoint;
use trust_control::commissioning::checkpoint_block::{block_reason, block_refusal};
use trust_control::failure::Failure;
use trust_control::loop_definition::LoopDefinition;
use trust_control::stages::{apply_available, StageAvailability, StageEvidence};
use trust_control::study::apply::ApplySection;
use trust_control::study::inputs::stage_evidence_of;
use trust_control::study::validate::ValidateSection;
use trust_control::study::Study;

use crate::journal::{Journal, JournalEntry};
use crate::storage;
use crate::workflow::apply::validation_identity;

/// What the Validate page offers, the action that holds its position, and the block of the stages, as the loop's journal and study say
/// them now (B6.4, B7, A8.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions)] // `ApplicationState` names the module's subject.
pub(crate) struct ApplicationState {
    /// The offers (`ApplicationOffers::of`).
    pub(crate) offers: ApplicationOffers,
    /// The reason of the block of a checkpoint that waits for its Restore (`checkpoint_block::block_reason`); `None` when nothing holds
    /// the loop.
    pub(crate) block: Option<&'static str>,
}

impl ApplicationState {
    /// The state of a loop whose journal is `journal` and whose study is `study`, for an answer whose stage evidence is `evidence`.
    /// `running` says that an operation of this engine works on the loop now (A8.6): an unresolved checkpoint is then that operation's,
    /// which the page follows to its end, and it holds nothing back.
    ///
    /// # Errors
    /// The study's Validate or Apply section cannot be read, or the identity of its validation cannot be written.
    pub(crate) fn of(
        journal: &Journal,
        study: &Study,
        evidence: &StageEvidence,
        running: bool,
    ) -> Result<Self, Failure> {
        let checkpoint = journaled_checkpoint(journal);
        let journaled = JournaledApply::of(checkpoint, running);
        let passed = passed_validation(study, evidence)?;
        let receipts = study.section::<ApplySection>()?.unwrap_or_default();
        let applied = newest_applied_validation(checkpoint, receipts.newest_validation());
        Ok(Self {
            offers: ApplicationOffers::of(
                journaled,
                passed.as_deref(),
                applied,
                receipts.keep_available(),
            ),
            block: block_reason(journaled),
        })
    }

    /// The action that holds the Validate page's position (B7).
    #[must_use]
    pub(crate) fn primary_action(&self) -> Option<PrimaryAction> {
        self.offers.primary_action()
    }

    /// The status of every stage of `evidence`, held by this state's block (B6.4, B6.7, B9.4), with the Apply stage, which can start
    /// exactly when Apply is offered (B7, `application_offers::offered_stages`).
    #[must_use]
    pub(crate) fn stages(&self, evidence: &StageEvidence) -> Vec<StageAvailability> {
        offered_stages(evidence, self.block, self.offers.apply)
    }
}

/// The state of the loop as its study and journal hold them now, with the stage evidence of the study, for the answer that ends an
/// operation of this engine: the operation has done its work, so a checkpoint it leaves unresolved waits for its Restore (B6.7).
///
/// # Errors
/// The study or the journal cannot be read (see [`ApplicationState::of`]).
pub(crate) fn state_now(
    project_root: &Path,
    storage_folder: &Path,
    loop_definition: &LoopDefinition,
) -> Result<(ApplicationState, StageEvidence), Failure> {
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let journal = Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    let evidence = stage_evidence_of(loop_definition, &study)?;
    Ok((
        ApplicationState::of(&journal, &study, &evidence, false)?,
        evidence,
    ))
}

/// The Apply checkpoint that the loop's journal holds (A8.5): an Apply journals at most one at a time.
#[must_use]
pub(crate) fn journaled_checkpoint(journal: &Journal) -> Option<&CommissioningCheckpoint> {
    journal.entries().iter().find_map(|entry| match entry {
        JournalEntry::Commissioning { checkpoint } => Some(&**checkpoint),
        _ => None,
    })
}

/// Refuses an Apply of `validation`, the identity of the study's passed validation, when the loop's newest Apply applied it (B7,
/// `application_offers::apply_offered`): whether that Apply's settings stand, were restored or were retired, Apply is offered again only
/// for a validation that passes after it, and the review keeps the rule the offers state, so a request that names the old validation is
/// refused as the page's offer is withheld.
///
/// # Errors
/// The study's Apply section cannot be read, or the newest Apply applied `validation`.
pub(crate) fn refuse_applied_validation(
    journal: &Journal,
    study: &Study,
    validation: &str,
) -> Result<(), Failure> {
    let receipts = study.section::<ApplySection>()?.unwrap_or_default();
    let applied =
        newest_applied_validation(journaled_checkpoint(journal), receipts.newest_validation());
    if applied == Some(validation) {
        return Err(validation_already_applied());
    }
    Ok(())
}

/// Refuses an action that the block holds (B6.4, B6.7; annex E11.7): Identify, Tune, Validate, Reset, the imports and a change of the
/// study inputs wait while the loop's journal holds an Apply's checkpoint that waits for its Restore, and the refusal names the block's
/// reason with the next action Restore. The caller has claimed the loop, so no operation of this engine works on it, and an unresolved checkpoint waits.
///
/// # Errors
/// The journal holds an Apply's checkpoint that waits.
pub(crate) fn refuse_while_restore_waits(journal: &Journal) -> Result<(), Failure> {
    block_refusal(JournaledApply::of(journaled_checkpoint(journal), false)).map_or(Ok(()), Err)
}

/// [`refuse_while_restore_waits`] for the journal of `loop_definition` in `storage_folder` (A8.3), for an action that reads no other
/// journal entry, such as Tune.
///
/// # Errors
/// The journal cannot be read, or it holds an Apply's checkpoint that waits.
pub(crate) fn refuse_while_restore_waits_in(
    storage_folder: &Path,
    loop_definition: &LoopDefinition,
) -> Result<(), Failure> {
    let journal = Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    refuse_while_restore_waits(&journal)
}

/// The identity of the study's validation of its current candidate when that validation passed (B5.4, `stages::apply_available`);
/// `None` when no validation of the current candidate passed.
fn passed_validation(study: &Study, evidence: &StageEvidence) -> Result<Option<String>, Failure> {
    if !apply_available(evidence) {
        return Ok(None);
    }
    study
        .section::<ValidateSection>()?
        .map(|section| validation_identity(&section))
        .transpose()
}

#[cfg(test)]
#[path = "application_state_tests.rs"]
mod tests;
