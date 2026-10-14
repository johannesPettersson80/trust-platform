//! The commissioning checkpoint in the operation journal (specification 35, A8.3, A8.5, B6.4; V4 decision 5): read, written before
//! each dispatch, cleared once a Restore confirms or a later Apply replaces it, and retired as history when the runtime was replaced.
//! The journal file belongs to the engine; these functions address only its `Commissioning` entry and its recording owners.
//!
//! An operation holds the journal through [`SharedJournal`] for its whole run, so that the recording session's owner persistence and
//! the checkpoint writes share one borrow of the journal (E11.2: the identity is journaled before its request is dispatched).

use std::cell::RefCell;

use trust_control::commissioning::checkpoint::{CheckpointStage, CommissioningCheckpoint};
use trust_control::experiment::RecordingRecoveryOwner;
use trust_control::failure::Failure;

use crate::journal::{Journal, JournalEntry};
use crate::runtime_client::runtime_recording::RecordingOwnerPersistence;

/// The journal of one operation: the loop's journal, borrowed for the operation's run, through which the checkpoint and the owners
/// of its recordings are written (E11.2, E11.6). Every method takes `&self`, so a recording session and the checkpoint writes can
/// both use it at once.
pub struct SharedJournal<'j> {
    journal: RefCell<&'j mut Journal>,
}

impl<'j> SharedJournal<'j> {
    /// Shares the loop's journal for one operation's run.
    #[must_use]
    pub fn new(journal: &'j mut Journal) -> Self {
        Self {
            journal: RefCell::new(journal),
        }
    }

    /// The journaled commissioning checkpoint, a copy of it, when one is kept (A8.5). The first one found is the loop's: an Apply
    /// journals at most one checkpoint at a time.
    #[must_use]
    pub fn checkpoint(&self) -> Option<CommissioningCheckpoint> {
        self.journal
            .borrow()
            .entries()
            .iter()
            .find_map(|entry| match entry {
                JournalEntry::Commissioning { checkpoint } => Some(checkpoint.as_ref().clone()),
                _ => None,
            })
    }

    /// Journals the checkpoint, or rewrites the entry of the same identifier, before the operation it records is dispatched (E11.2).
    ///
    /// # Errors
    /// The journal cannot be written; nothing is dispatched then.
    pub fn write_checkpoint(&self, checkpoint: &CommissioningCheckpoint) -> Result<(), Failure> {
        self.journal
            .borrow_mut()
            .persist(JournalEntry::Commissioning {
                checkpoint: Box::new(checkpoint.clone()),
            })
    }

    /// Clears the checkpoint of operation `id` once a Restore has confirmed the previous settings, or a later Apply replaces it (A8.5).
    ///
    /// # Errors
    /// The journal cannot be written.
    pub fn clear_checkpoint(&self, id: &str) -> Result<(), Failure> {
        self.journal.borrow_mut().clear(id)
    }

    /// Retires a checkpoint whose runtime was replaced since its Apply (B6.4): it is no longer journaled, and the returned record,
    /// marked `retired`, is the history the caller keeps in the study's Apply section.
    ///
    /// # Errors
    /// The journal cannot be written.
    pub fn retire_checkpoint(
        &self,
        checkpoint: &CommissioningCheckpoint,
    ) -> Result<CommissioningCheckpoint, Failure> {
        let mut history = checkpoint.clone();
        history.stage = CheckpointStage::Retired;
        self.clear_checkpoint(&checkpoint.id)?;
        Ok(history)
    }
}

impl RecordingOwnerPersistence for SharedJournal<'_> {
    fn persist(&self, owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        self.journal
            .borrow_mut()
            .persist(JournalEntry::RecordingOwner {
                owner: owner.clone(),
            })
    }

    fn clear(&self, owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        self.journal.borrow_mut().clear(&owner.recording_id)
    }
}
