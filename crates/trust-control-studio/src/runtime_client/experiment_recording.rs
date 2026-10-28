//! The recording that pairs a Test experiment (`experimentRecording.ts`; specification 35, A8.5 and A8.6; annex E11.2 and
//! E11.3). The Test's runtime acquisition starts before its excitation is sent. The experiment is dispatched only after the
//! recording's start is acknowledged and its owner is written, and never without the Test journal. A Test whose recording
//! start was not confirmed keeps its owner, and a reopened Test can only stop the original buffer. The runtime window of the
//! recording is trust-control's (`experiment_recording_window_ms`).

use trust_control::experiment::RecordingRecoveryOwner;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;

pub use trust_control::experiment::experiment_recording_window_ms;

use super::recording_coordinator::{
    RecordingCoordinator, RecordingKeeper, RecordingSession, RecordingTransition,
};
use super::runtime_recording::{RecordingOwnerPersistence, RecordingStartError};
use super::test_experiment_guards::JOURNAL_REQUIRED_MESSAGE;

/// Journals a recording's owner once the runtime has acknowledged its start (E11.2).
pub type OwnerJournal<'a> = &'a dyn Fn(&RecordingRecoveryOwner) -> Result<(), Failure>;

/// Why an adopted Test recovery is blocked: only its original buffer may be stopped, to keep the terminal Test evidence.
pub const RECOVERED_TEST_REASON: &str = "Test recording recovery unconfirmed — Stop the original runtime buffer to retain terminal Test evidence.";

/// Owns the runtime acquisition paired with a bounded Test experiment (`ExperimentRecordingOwner`). It reuses the recording
/// protocol and its owner rules, and never makes Test samples from the editor's monitor observations.
pub struct ExperimentRecordingOwner<S: RecordingSession> {
    coordinator: RecordingCoordinator<S>,
}

impl<S: RecordingSession> ExperimentRecordingOwner<S> {
    /// An owner whose recordings start with `start`.
    pub fn new(
        start: impl Fn(
                &LoopDefinition,
                Option<&dyn RecordingOwnerPersistence>,
                Option<u64>,
            ) -> Result<S, RecordingStartError<S>>
            + Send
            + Sync
            + 'static,
    ) -> Self {
        Self {
            coordinator: RecordingCoordinator::new(start),
        }
    }

    /// Whether a Test recording is bound to this owner.
    #[must_use]
    pub fn recording(&self) -> bool {
        self.coordinator.recording()
    }

    /// The loop of the bound recording.
    #[must_use]
    pub fn loop_definition(&self) -> Option<LoopDefinition> {
        self.coordinator.loop_definition()
    }

    /// The message of a start whose acknowledgement was lost and is not resolved yet.
    #[must_use]
    pub fn unresolved_start(&self) -> Option<String> {
        self.coordinator.unresolved_start()
    }

    /// The owner of the acknowledged recording, for the journal.
    #[must_use]
    pub fn recovery_owner(&self) -> Option<RecordingRecoveryOwner> {
        self.coordinator.session_recovery_owner()
    }

    /// Adopts the persisted Test recording of a reopened panel, which may only be stopped (`adoptRecoveredOwner`).
    ///
    /// # Errors
    /// Another recording is bound here, or the reason is empty.
    pub fn adopt_recovered_owner(&self, session: S, reason: &str) -> Result<(), Failure> {
        self.coordinator.adopt_recovered_owner(session, reason)
    }

    /// Starts the Test recording of `loop_definition`.
    ///
    /// # Errors
    /// A Test recording is already active, or the start failed without reaching the runtime (see `start_experiment_with_recording`).
    pub fn start(
        &self,
        loop_definition: &LoopDefinition,
        persistence: Option<&dyn RecordingOwnerPersistence>,
        planned_duration_ms: Option<u64>,
    ) -> Result<RecordingTransition<S>, Failure> {
        if self.coordinator.recording() {
            return Err(Failure::new(
                "A Test recording is already active for this loop.",
                Cause::Implementation,
                NextAction::None,
            ));
        }
        self.coordinator
            .start(loop_definition, persistence, planned_duration_ms)?
            .ok_or_else(|| {
                Failure::new(
                    "The Test recording could not start.",
                    Cause::Implementation,
                    NextAction::None,
                )
            })
    }

    /// Stops the Test recording. `retain` keeps the stopped samples first; the runtime buffer is released only after that
    /// (E4.7, E11.6).
    ///
    /// # Errors
    /// No Test recording is active for the loop, or the stop or the keeping fails.
    pub fn stop(
        &self,
        loop_definition: &LoopDefinition,
        retain: Option<RecordingKeeper<'_, S::Recording>>,
    ) -> Result<RecordingTransition<S>, Failure> {
        self.coordinator
            .stop(loop_definition, retain)?
            .ok_or_else(|| {
                Failure::new(
                    "No matching Test recording is active for this loop.",
                    Cause::Implementation,
                    NextAction::None,
                )
            })
    }
}

/// Starts the Test recording, then the experiment (`startExperimentWithRecording`). The recording's owner is journaled
/// through `persistence` before its start is dispatched. Once the start is acknowledged, the acknowledged owner is written
/// through `persist_acknowledged`, and only then is the experiment dispatched (E11.2, E11.3). Without the journal nothing
/// is sent. When anything fails after the recording started, the recording is stopped.
///
/// # Errors
/// The journal is missing, the recording's start is not acknowledged, the acknowledged owner cannot be written, or the
/// experiment's start fails. A failed stop is reported together with the failure it followed.
pub fn start_experiment_with_recording<S: RecordingSession, T>(
    owner: &ExperimentRecordingOwner<S>,
    loop_definition: &LoopDefinition,
    start_experiment: impl FnOnce() -> Result<T, Failure>,
    persistence: Option<&dyn RecordingOwnerPersistence>,
    persist_acknowledged: Option<OwnerJournal<'_>>,
    planned_duration_ms: Option<u64>,
) -> Result<T, Failure> {
    let Some(persistence) = persistence else {
        return Err(Failure::new(
            JOURNAL_REQUIRED_MESSAGE,
            Cause::Implementation,
            NextAction::None,
        ));
    };
    match owner.start(loop_definition, Some(persistence), planned_duration_ms)? {
        RecordingTransition::Started => {}
        RecordingTransition::StartUnconfirmed(message) => {
            // The original owner stays recoverable, and it never grants Test authority.
            if owner.recording() {
                let _ = owner.stop(loop_definition, None);
            }
            return Err(Failure::new(
                &format!("Test recording Start acknowledgement was not confirmed. {message}"),
                Cause::Runtime,
                NextAction::None,
            ));
        }
        RecordingTransition::Stopped(_) => {
            return Err(Failure::new(
                "The Test recording could not start.",
                Cause::Implementation,
                NextAction::None,
            ));
        }
    }
    if let Some(persist) = persist_acknowledged {
        let persisted = owner
            .recovery_owner()
            .ok_or_else(|| {
                Failure::new(
                    "The acknowledged Test recording does not expose a recoverable owner.",
                    Cause::Implementation,
                    NextAction::None,
                )
            })
            .and_then(|recovered| persist(&recovered));
        if let Err(error) = persisted {
            return Err(stop_after(owner, loop_definition, error));
        }
    }
    start_experiment().map_err(|error| stop_after(owner, loop_definition, error))
}

/// Stops the Test recording after `error`. A failed stop is reported with the error it followed.
fn stop_after<S: RecordingSession>(
    owner: &ExperimentRecordingOwner<S>,
    loop_definition: &LoopDefinition,
    error: Failure,
) -> Failure {
    match owner.stop(loop_definition, None) {
        Ok(_) => error,
        Err(stop_error) => Failure::new(
            &format!(
                "{} Test recording Stop is unconfirmed: {}",
                error.detail.as_deref().unwrap_or(&error.message),
                stop_error.detail.as_deref().unwrap_or(&stop_error.message)
            ),
            error.cause,
            error.next_action,
        ),
    }
}

/// Stops the exact Test acquisition and returns only the runtime-owned record (`stopExperimentRecording`).
///
/// # Errors
/// The stop fails, or it did not produce the terminal evidence of a Test recording.
pub fn stop_experiment_recording<S: RecordingSession>(
    owner: &ExperimentRecordingOwner<S>,
    loop_definition: &LoopDefinition,
    retain: Option<RecordingKeeper<'_, S::Recording>>,
) -> Result<S::Recording, Failure> {
    match owner.stop(loop_definition, retain)? {
        RecordingTransition::Stopped(recording) => Ok(recording),
        _ => Err(Failure::new(
            "The Test recording did not produce terminal evidence.",
            Cause::Runtime,
            NextAction::None,
        )),
    }
}

/// Reconnects only the persisted Test buffer. It has no start path (`restoreExperimentRecordingOwner`).
///
/// # Errors
/// The persisted buffer cannot be reached, or another recording is already bound to the owner.
pub fn restore_experiment_recording_owner<S: RecordingSession>(
    owner: &ExperimentRecordingOwner<S>,
    loop_definition: &LoopDefinition,
    persisted: &RecordingRecoveryOwner,
    recover: impl FnOnce(&LoopDefinition, &RecordingRecoveryOwner) -> Result<S, Failure>,
) -> Result<(), Failure> {
    let session = recover(loop_definition, persisted)?;
    owner.adopt_recovered_owner(session, RECOVERED_TEST_REASON)
}

#[cfg(test)]
#[path = "experiment_recording_tests.rs"]
mod tests;
