//! The active recording transaction of Control Studio (`recordingCoordinator.ts`; specification 35, A8.5; annex E4.7,
//! E11.3 and E11.6): one start, one stop, and the owners a reopened panel reattaches to. A stop that arrives while a
//! start still waits for its acknowledgement waits for it, then stops the same recording (E11.3).
//!
//! The coordinator is shared by the threads of Control Studio's operations, so its state is behind a mutex, and the
//! calls to the runtime run outside that mutex. A waiting stop is woken when the start settles.

use parking_lot::{Condvar, Mutex};
use trust_control::experiment::RecordingRecoveryOwner;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;

use super::runtime_recording::{
    RecordingOwnerPersistence, RecordingStartError, RecordingStartUnconfirmed,
};

/// The words shown while a start waits for its acknowledgement.
const STARTING: &str = "Starting recording…";
/// The words shown once a start is acknowledged and until its stop retains the runtime's scans.
const RECORDING_AT_RUNTIME: &str =
    "Recording at runtime — Start acknowledged; waiting for Stop to retain runtime scans.";

/// A recording's session: it stops the recording, and it releases the runtime buffer once the stopped samples are kept
/// (E4.7, E11.6).
pub trait RecordingSession: Send {
    /// What a stop returns: the recording's run.
    type Recording: Send;

    /// The loop the recording belongs to.
    fn loop_definition(&self) -> &LoopDefinition;

    /// Stops the recording.
    ///
    /// # Errors
    /// The stop is refused, cannot be reached, or answers out of protocol.
    fn stop(&mut self) -> Result<Self::Recording, Failure>;

    /// Frees the stopped runtime buffer. It is called only after the stopped samples are kept.
    ///
    /// # Errors
    /// The buffer cannot be released; it stays for the next release.
    fn release(&mut self) -> Result<(), Failure> {
        Ok(())
    }

    /// The owner that a reopened panel reattaches to, when the session has one.
    fn recovery_owner(&self) -> Option<RecordingRecoveryOwner> {
        None
    }
}

/// Whether the panel shows a recording as starting, running, or unconfirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingNoticeState {
    /// The start waits for its acknowledgement.
    Starting,
    /// The start is acknowledged; the recording runs.
    Recording,
    /// The state cannot be resolved until its owner is stopped.
    Unconfirmed,
}

/// The state of a recording, as the panel shows it (`RecordingStateNotice`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingStateNotice {
    /// The state.
    pub state: RecordingNoticeState,
    /// What the engineer reads.
    pub reason: String,
}

/// What a start or a stop did (`RecordingTransition`).
pub enum RecordingTransition<S: RecordingSession> {
    /// The start was acknowledged.
    Started,
    /// The recording stopped and its run is returned.
    Stopped(S::Recording),
    /// The start's acknowledgement was lost. The message says why, and the owner is kept for a stop.
    StartUnconfirmed(String),
}

/// Keeps a stopped recording in the study before its runtime buffer is freed (`RecordingRetention`).
pub type RecordingKeeper<'a, R> = &'a dyn Fn(&R) -> Result<(), Failure>;

/// Starts the recording of a loop, given the journal that keeps its owner and the planned duration in milliseconds.
type StartRecording<S> = Box<
    dyn Fn(
            &LoopDefinition,
            Option<&dyn RecordingOwnerPersistence>,
            Option<u64>,
        ) -> Result<S, RecordingStartError<S>>
        + Send
        + Sync,
>;

/// The coordinator of one engine's recording (`RecordingCoordinator`).
pub struct RecordingCoordinator<S: RecordingSession> {
    start: StartRecording<S>,
    state: Mutex<State<S>>,
    settled: Condvar,
}

/// The state of the coordinator. A session that is stopping is taken out of `session`, and its loop is kept in
/// `session_loop` until the stop settles.
struct State<S: RecordingSession> {
    session: Option<S>,
    session_loop: Option<LoopDefinition>,
    pending_loop: Option<LoopDefinition>,
    start_failure: Option<Failure>,
    unconfirmed: Option<Unconfirmed<S>>,
    recovered_reason: Option<String>,
    recovered_loop: Option<LoopDefinition>,
    stopping: bool,
}

/// A start whose acknowledgement was lost. When its stop returned the recording, the session and the recording are held
/// until they are kept and released.
struct Unconfirmed<S: RecordingSession> {
    message: String,
    held: Option<(S, S::Recording)>,
}

impl<S: RecordingSession> State<S> {
    fn empty() -> Self {
        Self {
            session: None,
            session_loop: None,
            pending_loop: None,
            start_failure: None,
            unconfirmed: None,
            recovered_reason: None,
            recovered_loop: None,
            stopping: false,
        }
    }

    /// Whether a recording is bound: starting, acknowledged, recovered, or stopping (`recording`).
    fn recording(&self) -> bool {
        self.pending_loop.is_some()
            || self.session_loop.is_some()
            || self.recovered_loop.is_some()
            || self.stopping
    }

    /// The loop of the bound recording (`loop`).
    fn bound_loop(&self) -> Option<&LoopDefinition> {
        self.session_loop
            .as_ref()
            .or(self.pending_loop.as_ref())
            .or(self.recovered_loop.as_ref())
    }

    /// Keeps a lost start: with its recording it is held until it is kept (`retainUnconfirmedStart`); without one its
    /// session stays the owner, so the next Stop can stop it.
    fn retain_unconfirmed(&mut self, unconfirmed: RecordingStartUnconfirmed<S>) {
        let message = unconfirmed.message();
        let RecordingStartUnconfirmed {
            session, recording, ..
        } = unconfirmed;
        if let Some(recording) = recording {
            self.session = None;
            self.session_loop = None;
            self.unconfirmed = Some(Unconfirmed {
                message,
                held: Some((session, recording)),
            });
        } else {
            self.session_loop = Some(session.loop_definition().clone());
            self.session = Some(session);
            self.unconfirmed = Some(Unconfirmed {
                message,
                held: None,
            });
        }
    }
}

impl<S: RecordingSession> RecordingCoordinator<S> {
    /// A coordinator that starts recordings with `start`.
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
            start: Box::new(start),
            state: Mutex::new(State::empty()),
            settled: Condvar::new(),
        }
    }

    /// Whether a recording is bound here: starting, acknowledged, recovered, or stopping (`recording`).
    #[must_use]
    pub fn recording(&self) -> bool {
        self.state.lock().recording()
    }

    /// The loop of the bound recording (`loop`).
    #[must_use]
    pub fn loop_definition(&self) -> Option<LoopDefinition> {
        self.state.lock().bound_loop().cloned()
    }

    /// The message of a start whose acknowledgement was lost and is not resolved yet.
    #[must_use]
    pub fn unresolved_start(&self) -> Option<String> {
        self.state
            .lock()
            .unconfirmed
            .as_ref()
            .map(|unconfirmed| unconfirmed.message.clone())
    }

    /// What the panel shows of the recording's state (`currentUpdate`).
    #[must_use]
    pub fn current_update(&self) -> Option<RecordingStateNotice> {
        let state = self.state.lock();
        if let Some(unconfirmed) = &state.unconfirmed {
            return Some(RecordingStateNotice {
                state: RecordingNoticeState::Unconfirmed,
                reason: unconfirmed.message.clone(),
            });
        }
        if let Some(reason) = &state.recovered_reason {
            return Some(RecordingStateNotice {
                state: RecordingNoticeState::Unconfirmed,
                reason: reason.clone(),
            });
        }
        if state.pending_loop.is_some() {
            return Some(RecordingStateNotice {
                state: RecordingNoticeState::Starting,
                reason: STARTING.to_string(),
            });
        }
        state.session_loop.is_some().then(|| RecordingStateNotice {
            state: RecordingNoticeState::Recording,
            reason: RECORDING_AT_RUNTIME.to_string(),
        })
    }

    /// The owner of the acknowledged session, for the journal.
    #[must_use]
    pub fn session_recovery_owner(&self) -> Option<RecordingRecoveryOwner> {
        self.state
            .lock()
            .session
            .as_ref()
            .and_then(RecordingSession::recovery_owner)
    }

    /// Adopts a recovered owner. It may only stop its original recording, so the reason says why it is blocked
    /// (`adoptRecoveredOwner`).
    ///
    /// # Errors
    /// Another recording is bound here, or the reason is empty.
    pub fn adopt_recovered_owner(&self, session: S, reason: &str) -> Result<(), Failure> {
        let mut state = self.state.lock();
        let loop_definition = session.loop_definition().clone();
        let retrying_unavailable_owner = state.session_loop.is_none()
            && state
                .recovered_loop
                .as_ref()
                .is_some_and(|recovered| same_recording_scope(recovered, &loop_definition));
        if (!retrying_unavailable_owner && state.recording()) || reason.trim().is_empty() {
            return Err(already_active());
        }
        state.session_loop = Some(loop_definition.clone());
        state.recovered_loop = Some(loop_definition);
        state.recovered_reason = Some(reason.to_string());
        state.session = Some(session);
        Ok(())
    }

    /// Records that the owner of `loop_definition` is unavailable until its runtime is reachable again, so only that
    /// owner can be retried (`adoptUnavailableRecovery`).
    ///
    /// # Errors
    /// Another recording is bound here, or the reason is empty.
    pub fn adopt_unavailable_recovery(
        &self,
        loop_definition: &LoopDefinition,
        reason: &str,
    ) -> Result<(), Failure> {
        let mut state = self.state.lock();
        let retrying_unavailable_owner = state.session_loop.is_none()
            && state
                .recovered_loop
                .as_ref()
                .is_some_and(|recovered| same_recording_scope(recovered, loop_definition));
        if (!retrying_unavailable_owner && state.recording()) || reason.trim().is_empty() {
            return Err(already_active());
        }
        state.recovered_loop = Some(loop_definition.clone());
        state.recovered_reason = Some(reason.to_string());
        Ok(())
    }

    /// Shows a failed stop of the recovered owner, which stays blocked (`reportRecoveredStopFailure`). Returns whether
    /// the failure belongs to that owner.
    pub fn report_recovered_stop_failure(
        &self,
        loop_definition: &LoopDefinition,
        error: &Failure,
    ) -> bool {
        let mut state = self.state.lock();
        let belongs = state.session_loop.is_some()
            && state
                .recovered_loop
                .as_ref()
                .is_some_and(|recovered| same_recording_scope(recovered, loop_definition));
        if belongs {
            state.recovered_reason = Some(format!(
                "Recording recovery unconfirmed — {}",
                error.detail.as_deref().unwrap_or(&error.message)
            ));
        }
        belongs
    }

    /// Starts a recording of `loop_definition`, with its planned duration when it has one. Returns `None` when a
    /// recording is already bound here (`execute("start")`).
    ///
    /// # Errors
    /// The start failed without reaching the runtime, or the runtime refused it (E11.6).
    pub fn start(
        &self,
        loop_definition: &LoopDefinition,
        persistence: Option<&dyn RecordingOwnerPersistence>,
        planned_duration_ms: Option<u64>,
    ) -> Result<Option<RecordingTransition<S>>, Failure> {
        {
            let mut state = self.state.lock();
            if state.recording() {
                return Ok(None);
            }
            state.unconfirmed = None;
            state.start_failure = None;
            state.pending_loop = Some(loop_definition.clone());
        }
        let outcome = (self.start)(loop_definition, persistence, planned_duration_ms);
        let mut state = self.state.lock();
        state.pending_loop = None;
        let transition = match outcome {
            Ok(session) => {
                state.session_loop = Some(session.loop_definition().clone());
                state.session = Some(session);
                Ok(Some(RecordingTransition::Started))
            }
            Err(RecordingStartError::Unconfirmed(unconfirmed)) => {
                let message = unconfirmed.message();
                state.retain_unconfirmed(*unconfirmed);
                Ok(Some(RecordingTransition::StartUnconfirmed(message)))
            }
            Err(RecordingStartError::Failed(failure)) => {
                state.start_failure = Some(failure.clone());
                Err(failure)
            }
        };
        drop(state);
        self.settled.notify_all();
        transition
    }

    /// Stops the recording of `loop_definition`. The stopped samples are kept by `retain` first; the runtime buffer is
    /// released only after that (E4.7, E11.6). Returns `None` when a stop is already running.
    ///
    /// # Errors
    /// The loop is not the bound one, the stop or the keeping fails, or the owner was recovered without a session.
    pub fn stop(
        &self,
        loop_definition: &LoopDefinition,
        retain: Option<RecordingKeeper<'_, S::Recording>>,
    ) -> Result<Option<RecordingTransition<S>>, Failure> {
        let mut state = self.state.lock();
        let matches_bound_loop = state
            .bound_loop()
            .is_some_and(|bound| same_recording_scope(bound, loop_definition));
        if !matches_bound_loop {
            return Err(Failure::new(
                "No matching runtime recording is active for this scope.",
                Cause::Implementation,
                NextAction::None,
            ));
        }
        // A start still waiting for its acknowledgement is waited for; then this Stop stops the same recording (E11.3).
        let waited_for_start = state.pending_loop.is_some();
        self.settled
            .wait_while(&mut state, |state| state.pending_loop.is_some());
        if waited_for_start {
            if let Some(failure) = state.start_failure.clone() {
                return Err(failure);
            }
        }
        // A lost start that holds its recording: the samples are kept, then the buffer is released.
        let held = state
            .unconfirmed
            .as_mut()
            .and_then(|unconfirmed| unconfirmed.held.take());
        if let Some((mut session, recording)) = held {
            state.stopping = true;
            drop(state);
            let kept = keep_and_release(&mut session, &recording, retain);
            let mut state = self.state.lock();
            state.stopping = false;
            let outcome = match kept {
                Ok(()) => {
                    state.unconfirmed = None;
                    Ok(Some(RecordingTransition::Stopped(recording)))
                }
                Err(error) => {
                    if let Some(unconfirmed) = state.unconfirmed.as_mut() {
                        unconfirmed.held = Some((session, recording));
                    }
                    Err(error)
                }
            };
            drop(state);
            self.settled.notify_all();
            return outcome;
        }
        // A recovered owner without a live session may only be reported (`recoveredReason`).
        if state.session_loop.is_none() {
            if let Some(reason) = state.recovered_reason.clone() {
                return Err(Failure::new(&reason, Cause::Runtime, NextAction::None));
            }
        }
        if state.stopping {
            return Ok(None);
        }
        let Some(mut session) = state.session.take() else {
            return Ok(None);
        };
        state.stopping = true;
        drop(state);

        let outcome = match session.stop() {
            Ok(recording) => keep_and_release(&mut session, &recording, retain).map(|()| recording),
            Err(error) => Err(error),
        };
        let mut state = self.state.lock();
        state.stopping = false;
        let transition = match outcome {
            Ok(recording) => {
                // The owner is settled, so none of it remains (`this.session === session`).
                state.session_loop = None;
                state.unconfirmed = None;
                state.recovered_reason = None;
                state.recovered_loop = None;
                Ok(Some(RecordingTransition::Stopped(recording)))
            }
            Err(error) => {
                // A failed stop or keep leaves the owner in place, so the next Stop retries the keep before any release.
                state.session = Some(session);
                Err(error)
            }
        };
        drop(state);
        self.settled.notify_all();
        transition
    }
}

/// Keeps the stopped samples, then frees the runtime buffer. Without a keeper the buffer stays for the next start's sweep.
fn keep_and_release<S: RecordingSession>(
    session: &mut S,
    recording: &S::Recording,
    retain: Option<RecordingKeeper<'_, S::Recording>>,
) -> Result<(), Failure> {
    let Some(retain) = retain else {
        return Ok(());
    };
    retain(recording)?;
    session.release()
}

/// Whether two loops are the same recording scope: the same project, runtime, loop and revision (`sameRecordingScope`).
#[must_use]
pub fn same_recording_scope(left: &LoopDefinition, right: &LoopDefinition) -> bool {
    left.project_id == right.project_id
        && left.runtime_id == right.runtime_id
        && left.id == right.id
        && left.revision == right.revision
}

fn already_active() -> Failure {
    Failure::new(
        "A recording owner is already active or recovery reason is invalid.",
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "recording_coordinator_tests.rs"]
mod tests;
