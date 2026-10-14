//! The commissioning checkpoint of one Apply, written through the loop's journal (specification 35, A8.3, A8.5 and B6.4; annex E11.2 and
//! E11.6). The checkpoint is journaled before the first write and again before each transaction and recording is dispatched, with the
//! owner the runtime knows it by: the transfer session retains its owner through [`CheckpointWriter::transfer_sink`], and the recording
//! session its owner through the writer's [`RecordingOwnerPersistence`]. The records are trust-control's (`commissioning::checkpoint`);
//! this module only writes them.

use std::cell::RefCell;
use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};

use trust_control::commissioning::checkpoint::{CheckpointStage, CommissioningCheckpoint};
use trust_control::commissioning::commissioning_result::TransferOwner;
use trust_control::commissioning::observation_assessment::JournalStage;
use trust_control::experiment::RecordingRecoveryOwner;
use trust_control::failure::Failure;

use crate::runtime_client::runtime_recording::RecordingOwnerPersistence;
use crate::runtime_client::transfers::TransferOwnerSink;
use crate::workflow::commissioning_journal::SharedJournal;

/// A new checkpoint identifier: `commissioning-` and 32 hex digits drawn from the random seeds of the standard hash map, as a recording
/// or transaction identifier is drawn (E0.2). It names the Apply's journal entry and its receipt (A8.5).
#[must_use]
pub(crate) fn new_checkpoint_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let state = RandomState::new();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "commissioning-{:016x}{:016x}",
        state.hash_one(count),
        state.hash_one(!count)
    )
}

/// The checkpoint stage a run's ending is written at (`execute`, A8.5): the journal stage that the commissioning rules decide.
#[must_use]
pub(crate) fn checkpoint_stage(stage: JournalStage) -> CheckpointStage {
    match stage {
        JournalStage::Prepared => CheckpointStage::Prepared,
        JournalStage::Recording => CheckpointStage::Recording,
        JournalStage::Parameters => CheckpointStage::Parameters,
        JournalStage::Transfer => CheckpointStage::Transfer,
        JournalStage::Completed => CheckpointStage::Completed,
        JournalStage::Recovered => CheckpointStage::Recovered,
        JournalStage::Retired => CheckpointStage::Retired,
    }
}

/// The word of a checkpoint stage, as the journal writes it.
#[must_use]
pub(crate) fn stage_word(stage: CheckpointStage) -> &'static str {
    match stage {
        CheckpointStage::Prepared => "prepared",
        CheckpointStage::Recording => "recording",
        CheckpointStage::Parameters => "parameters",
        CheckpointStage::Transfer => "transfer",
        CheckpointStage::Completed => "completed",
        CheckpointStage::Recovered => "recovered",
        CheckpointStage::Retired => "retired",
    }
}

/// Whether a journaled checkpoint's operations are still unresolved: it has not completed, recovered or retired (A8.5, B6.4), so its
/// writes may be in flight and only Restore resolves them.
#[must_use]
pub(crate) fn unresolved(stage: CheckpointStage) -> bool {
    !matches!(
        stage,
        CheckpointStage::Completed | CheckpointStage::Recovered | CheckpointStage::Retired
    )
}

/// The checkpoint of one Apply and the journal it is written to. Every method takes `&self`, so the transfer session and the recording
/// session can each retain their owner through it while the run holds it.
pub(crate) struct CheckpointWriter<'j> {
    journal: SharedJournal<'j>,
    current: RefCell<CommissioningCheckpoint>,
}

impl<'j> CheckpointWriter<'j> {
    /// The writer of `checkpoint` through `journal`. Nothing is written until [`CheckpointWriter::save`].
    #[must_use]
    pub(crate) fn new(journal: SharedJournal<'j>, checkpoint: CommissioningCheckpoint) -> Self {
        Self {
            journal,
            current: RefCell::new(checkpoint),
        }
    }

    /// The loop's journal, which a Restore writes through as well (B6.4).
    #[must_use]
    pub(crate) fn journal(&self) -> &SharedJournal<'j> {
        &self.journal
    }

    /// The checkpoint as this run last wrote it.
    #[must_use]
    pub(crate) fn checkpoint(&self) -> CommissioningCheckpoint {
        self.current.borrow().clone()
    }

    /// Journals the checkpoint at `stage` before the step it records is dispatched (E11.2).
    ///
    /// # Errors
    /// The journal cannot be written; nothing is dispatched then.
    pub(crate) fn save(&self, stage: CheckpointStage) -> Result<(), Failure> {
        self.save_with(Some(stage), |_| {})
    }

    /// Changes the checkpoint with `change`, moves it to `stage` when one is given, and journals it (E11.2).
    ///
    /// # Errors
    /// The journal cannot be written; nothing is dispatched then.
    pub(crate) fn save_with(
        &self,
        stage: Option<CheckpointStage>,
        change: impl FnOnce(&mut CommissioningCheckpoint),
    ) -> Result<(), Failure> {
        let snapshot = {
            let mut current = self.current.borrow_mut();
            change(&mut current);
            if let Some(stage) = stage {
                current.stage = stage;
            }
            current.clone()
        };
        self.journal.write_checkpoint(&snapshot)
    }

    /// Clears the checkpoint from the journal: its settings are recovered, or a later Apply replaces it (A8.5). Clearing one that is no
    /// longer journaled changes nothing.
    ///
    /// # Errors
    /// The journal cannot be written.
    pub(crate) fn clear_checkpoint(&self) -> Result<(), Failure> {
        let id = self.current.borrow().id.clone();
        self.journal.clear_checkpoint(&id)
    }

    /// The sink through which the transfer session retains each transfer's owner in the checkpoint, at the stage `transfer`, before the
    /// apply is dispatched (E11.2).
    #[must_use]
    pub(crate) fn transfer_sink(&self) -> TransferRetention<'_, 'j> {
        TransferRetention { writer: self }
    }
}

/// The recording's owner goes into the checkpoint, so that the observation and its recording are journaled together (A8.5, E11.2): at
/// the stage `recording`, before the start is dispatched. A start the runtime refused before any buffer existed drops the owner again
/// (E11.6); the stage stays the one the last dispatched write left, because a write may already have been made.
impl RecordingOwnerPersistence for CheckpointWriter<'_> {
    fn persist(&self, owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        self.save_with(Some(CheckpointStage::Recording), |checkpoint| {
            checkpoint.recording = Some(owner.clone());
        })
    }

    fn clear(&self, _owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        self.save_with(None, |checkpoint| checkpoint.recording = None)
    }
}

/// Retains each transfer's owner in the checkpoint before its apply is dispatched (E11.2): the stage `transfer` and the owner.
pub(crate) struct TransferRetention<'w, 'j> {
    writer: &'w CheckpointWriter<'j>,
}

impl TransferOwnerSink for TransferRetention<'_, '_> {
    fn retain(&mut self, owner: &TransferOwner) -> Result<(), Failure> {
        self.writer
            .save_with(Some(CheckpointStage::Transfer), |checkpoint| {
                checkpoint.transfer = Some(owner.clone());
            })
    }
}
