//! The guards of the Test experiments (`testExperimentGuards.ts`; specification 35, A2.7, A8.5 and A8.6). An identity
//! is journaled before anything is sent, and the recording is journaled through the Test journal. A start whose
//! outcome is unknown is reconciled by status and is never sent again. Live requests go only to the target pinned
//! when the loop opened, and nothing is sent without that target and the journal.

use parking_lot::Mutex;
use rustc_hash::FxHashSet;
use trust_control::experiment::RecordingRecoveryOwner;
use trust_control::failure::{Cause, Failure, NextAction};

use super::runtime_recording::RecordingOwnerPersistence;

/// The message of a Test start without the runtime the loop was pinned to (A2.7).
pub const UNPINNED_RUNTIME_MESSAGE: &str = "Control Studio is not connected to this loop's runtime. Start the runtime and reopen Control Studio to connect it.";

/// The message of a Test start without its journal: no request is sent (E11.2).
pub const JOURNAL_REQUIRED_MESSAGE: &str =
    "The Test recording needs its journal before Start is sent. No runtime request was made.";

/// Writes the journal entry, then dispatches. A failed journal write never dispatches (E11.2).
///
/// # Errors
/// The journal cannot be written, so nothing is dispatched; or the dispatch fails.
pub fn journal_before_dispatch<T>(
    journal: impl FnOnce() -> Result<(), Failure>,
    dispatch: impl FnOnce() -> Result<T, Failure>,
) -> Result<T, Failure> {
    journal()?;
    dispatch()
}

/// The recording journal of a Test experiment (`experimentRecordingJournal`). The owner is written with the
/// experiment's record, which is what reopening restores into the Test recorder. Clearing the owner keeps the identity
/// until the experiment is terminal (A8.5). Its two ports write the experiment's entry of the journal.
pub struct ExperimentRecordingJournal<P, C> {
    persist_owner: P,
    clear_owner: C,
}

/// The Test journal of the two ports (`experimentRecordingJournal`).
pub fn experiment_recording_journal<P, C>(
    persist_owner: P,
    clear_owner: C,
) -> ExperimentRecordingJournal<P, C>
where
    P: Fn(&RecordingRecoveryOwner) -> Result<(), Failure>,
    C: Fn() -> Result<(), Failure>,
{
    ExperimentRecordingJournal {
        persist_owner,
        clear_owner,
    }
}

impl<P, C> RecordingOwnerPersistence for ExperimentRecordingJournal<P, C>
where
    P: Fn(&RecordingRecoveryOwner) -> Result<(), Failure>,
    C: Fn() -> Result<(), Failure>,
{
    fn persist(&self, owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        (self.persist_owner)(owner)
    }

    fn clear(&self, _owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        (self.clear_owner)()
    }
}

/// The arguments of a Test start: the runtime the loop was pinned to when it opened, and the Test journal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestStartPorts<T, J> {
    /// The pinned runtime target.
    pub target: T,
    /// The Test journal.
    pub journal: J,
}

/// The arguments of a Test start. Without the pinned target, or without the journal, nothing is sent.
///
/// # Errors
/// The target is not pinned, or no journal is given.
pub fn test_start_ports<T, J>(
    target: Option<T>,
    journal: Option<J>,
) -> Result<TestStartPorts<T, J>, Failure> {
    let Some(target) = target else {
        return Err(Failure::new(
            UNPINNED_RUNTIME_MESSAGE,
            Cause::Runtime,
            NextAction::ConnectRuntime,
        ));
    };
    let Some(journal) = journal else {
        return Err(Failure::new(
            JOURNAL_REQUIRED_MESSAGE,
            Cause::Implementation,
            NextAction::None,
        ));
    };
    Ok(TestStartPorts { target, journal })
}

/// The experiment identities whose start may have reached the runtime. Such an identity is never sent again (A8.5).
#[derive(Debug, Default)]
pub struct UnconfirmedTestStarts {
    identities: Mutex<FxHashSet<String>>,
}

impl UnconfirmedTestStarts {
    /// No identity is unconfirmed.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records that the start of `experiment_id` may have reached the runtime.
    pub fn mark_unconfirmed(&self, experiment_id: &str) {
        self.identities.lock().insert(experiment_id.to_string());
    }

    /// Status has confirmed the identity, either as running or as never having been accepted.
    pub fn confirm(&self, experiment_id: &str) {
        self.identities.lock().remove(experiment_id);
    }

    /// Whether the start of `experiment_id` is still unconfirmed.
    #[must_use]
    pub fn is_unconfirmed(&self, experiment_id: &str) -> bool {
        self.identities.lock().contains(experiment_id)
    }

    /// # Errors
    /// The start of `experiment_id` is unconfirmed, so it is not sent again.
    pub fn assert_sendable(&self, experiment_id: &str) -> Result<(), Failure> {
        if self.is_unconfirmed(experiment_id) {
            return Err(Failure::new(
                "This Test start was not confirmed. Its identity is reconciled by status and is not sent again; review the experiment to create a new identity.",
                Cause::Runtime,
                NextAction::None,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "test_experiment_guards_tests.rs"]
mod tests;
