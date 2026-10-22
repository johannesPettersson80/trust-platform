//! A runtime recording of Control Studio (runtime protocol annex, E4; specification 35, A4, A8.5 and A8.6): its start
//! with its record period (A4.2, E4.4), its status pages, its stop and its release (`runtimeRecording.ts`).
//!
//! The start journals the recording's owner before the start is dispatched (E11.2). A refusal of the runtime comes before
//! any buffer exists, so the owner is cleared. A lost acknowledgement keeps the owner, and the recording is stopped at once
//! (E11.4). A symbolic start that the runtime refuses for want of a coherent inspection is inspected again and started again,
//! as trust-control allows. A stop keeps its samples before the buffer is released (E11.6). trust-control holds the limits,
//! the record period, the evidence and the checks of the answers; this module makes the calls in the order the annex requires.
//!
//! An identification campaign follows the running recording through `RecordingFollower`: the session turns each status page
//! into the samples the campaign reads, and keeps the origin of their process time across pages (A3.2).

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::experiment::plan::recording_task;
use trust_control::experiment::{
    check_recording_owner_scope, ControlEvidenceSource, RecordingRecoveryBinding,
    RecordingRecoveryOwner,
};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::campaign::FollowSample;
use trust_control::limits::{
    record_period_executions, sample_period_ms, RECORDING_DURATION_LIMIT_MS, RECORDING_SAMPLE_LIMIT,
};
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::channels::follow_samples;
use trust_control::recording::evidence::{
    recording_evidence, RecordingEvidence, UNCONFIRMED_START_RECOVERED,
};
use trust_control::recording::page::{check_status_page, FollowCursor};
use trust_control::recording::start::{may_retry_start, START_ATTEMPT_LIMIT};
use trust_control::recording::{EvidenceSource, RecordedRun, RecordingRequest, RecordingStatus};
use trust_control::runtime::{RuntimeConnection, RuntimeStatus};
use trust_control::signals::{loop_inspection_bindings, Inspection, SignalBinding};

use super::experiment::whole_milliseconds;
use super::recording_coordinator::RecordingSession;
use super::RuntimeClient;
use crate::journal::{Journal, JournalEntry};
use crate::workflow::identify_campaign::RecordingFollower;
use trust_runtime_client::ControlError;

/// The recording of a plan without a planned duration covers one hour of process time (`DEFAULT_RECORDING_DURATION_MS`).
pub const DEFAULT_RECORDING_DURATION_MS: u64 = 3_600_000;

/// Where a recording's owner is journaled (E11.2). It is written before the start is dispatched, written again when the
/// start's acknowledgement is lost, and cleared when the runtime refused the start before any buffer existed (E11.6).
pub trait RecordingOwnerPersistence {
    /// Writes the owner, replacing the entry of the same recording.
    ///
    /// # Errors
    /// The owner cannot be written; nothing is dispatched.
    fn persist(&self, owner: &RecordingRecoveryOwner) -> Result<(), Failure>;

    /// Clears the owner of a recording that needs no recovery any more.
    ///
    /// # Errors
    /// The owner cannot be cleared.
    fn clear(&self, owner: &RecordingRecoveryOwner) -> Result<(), Failure>;
}

impl RecordingOwnerPersistence for Mutex<Journal> {
    fn persist(&self, owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        self.lock().persist(JournalEntry::RecordingOwner {
            owner: owner.clone(),
        })
    }

    fn clear(&self, owner: &RecordingRecoveryOwner) -> Result<(), Failure> {
        self.lock().clear(&owner.recording_id)
    }
}

/// What a recording start asks for: its identifier, journaled before the start (E11.2); its planned duration, which fixes
/// the runtime window (A7) and the record period (A4.2); and the expected settling time of the response it records, when
/// the study declares one (A4.2), in ms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingStartPlan {
    /// The recording's identifier, 1 to 128 bytes.
    pub recording_id: String,
    /// The planned duration in ms of process time.
    pub planned_duration_ms: u64,
    /// The expected settling time in ms, when the study declares one.
    pub expected_settling_ms: Option<u64>,
}

/// A new recording identifier: `recording-` and 32 hex digits drawn from the random seeds of the standard hash map (E0.2).
#[must_use]
pub fn new_recording_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let state = RandomState::new();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "recording-{:016x}{:016x}",
        state.hash_one(count),
        state.hash_one(!count)
    )
}

/// Recordings that are stopped but not released (E4.7). A release that fails keeps its recording here, and the next start
/// of the same endpoint releases it again (`pendingReleases` of `runtimeRecording.ts`).
#[derive(Debug, Default)]
pub struct PendingReleases {
    entries: Mutex<Vec<PendingRelease>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingRelease {
    endpoint: String,
    recording_id: String,
    loop_revision: String,
}

impl PendingReleases {
    /// No recording waits for its release.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn keep(&self, release: PendingRelease) {
        let mut entries = self.entries.lock();
        entries.retain(|entry| entry.recording_id != release.recording_id);
        entries.push(release);
    }

    fn settle(&self, recording_id: &str) {
        self.entries
            .lock()
            .retain(|entry| entry.recording_id != recording_id);
    }

    fn waiting_on(&self, endpoint: &str) -> Vec<PendingRelease> {
        self.entries
            .lock()
            .iter()
            .filter(|entry| entry.endpoint == endpoint)
            .cloned()
            .collect()
    }
}

/// A recording start whose acknowledgement was lost, or was out of protocol (`RecordingStartUnconfirmed`). The start may
/// have reached the runtime, so its owner is kept: its session stops the recording, and the stopped samples are the
/// runtime's (E11.4). When the stop failed too, the owner stays unresolved.
pub struct RecordingStartUnconfirmed<S: RecordingSession> {
    /// The session of the recording.
    pub session: S,
    /// The recording that the stop returned, when it did.
    pub recording: Option<S::Recording>,
    /// Why the start's acknowledgement was not confirmed.
    pub start_error: Failure,
    /// Why the stop failed, when it did.
    pub stop_error: Option<Failure>,
}

impl<S: RecordingSession> RecordingStartUnconfirmed<S> {
    /// The message the panel shows (`RecordingStartUnconfirmed.message`).
    #[must_use]
    pub fn message(&self) -> String {
        let outcome = match (&self.recording, &self.stop_error) {
            (Some(_), _) => {
                "The existing recording was stopped and its actual samples retained.".to_string()
            }
            (None, Some(stop_error)) => format!(
                "Stop remains unconfirmed: {}. Retry Stop for this recording.",
                text(stop_error)
            ),
            (None, None) => "Stop remains unconfirmed. Retry Stop for this recording.".to_string(),
        };
        format!(
            "Recording Start acknowledgement was not confirmed: {}. {outcome}",
            text(&self.start_error)
        )
    }
}

/// Why a recording did not start.
pub enum RecordingStartError<S: RecordingSession> {
    /// The start's acknowledgement was lost, so its owner is kept (E11.4).
    Unconfirmed(Box<RecordingStartUnconfirmed<S>>),
    /// The start did not reach the runtime, or the runtime refused it before any buffer was allocated (E11.6).
    Failed(Failure),
}

/// A runtime recording of one loop, on the endpoint it started on (E11.1). Stop, release and status go to that endpoint and
/// name the recording by its identity.
pub struct RuntimeRecordingSession {
    connection: RuntimeConnection,
    loop_definition: LoopDefinition,
    /// The recording's identity. Stop, status and release send only the identity; the start's limits are not sent again.
    request: RecordingRequest,
    source: ControlEvidenceSource,
    /// Why the start's acknowledgement was not confirmed, when it was not.
    start_failure: Option<String>,
    stopped: Option<RecordedRun>,
    released: bool,
    /// Where the in-order follow continues (E4.6).
    cursor: FollowCursor,
    /// The runtime time of the recording's first sample in ns: the origin of the process time that the campaign's follow
    /// reports (A3.2). The first sample that follow converts sets it, and it is kept across pages.
    first_time_ns: Option<i64>,
    pending: Arc<PendingReleases>,
}

impl RuntimeRecordingSession {
    /// Starts a recording of `loop_definition` on `connection` (E4.2, E11.2 to E11.4).
    ///
    /// The owner is journaled through `persistence` before the start is dispatched. A refusal clears it (E11.6). A lost
    /// acknowledgement keeps it, and the recording is stopped at once (E11.4). A symbolic recording waits for a coherent
    /// inspection of its signals first, and names the program it inspected. A refusal for want of a coherent inspection is
    /// followed by a fresh inspection and a new start, at most [`START_ATTEMPT_LIMIT`] starts in all.
    ///
    /// # Errors
    /// The runtime cannot be reached or refuses, the request breaks a limit of E4.2, the journal cannot be written, or the
    /// start's acknowledgement is lost (see [`RecordingStartError::Unconfirmed`]).
    pub fn start(
        connection: &RuntimeConnection,
        loop_definition: &LoopDefinition,
        plan: &RecordingStartPlan,
        persistence: Option<&dyn RecordingOwnerPersistence>,
        pending: &Arc<PendingReleases>,
    ) -> Result<Self, RecordingStartError<Self>> {
        let mut client = RuntimeClient::connect(connection).map_err(RecordingStartError::Failed)?;
        let status = client.status().map_err(RecordingStartError::Failed)?;
        sweep_pending_releases(&mut client, connection, pending);

        let bindings = loop_inspection_bindings(loop_definition);
        // A loop's values change when its task runs, so a scheduled recording samples that task (A4.1). A background
        // recording samples every resource cycle, so its record period counts cycles (A4.2).
        let task = recording_task(loop_definition);
        let period_ms = task_period_ms(loop_definition).map_err(RecordingStartError::Failed)?;
        let record_every = record_period_executions(
            plan.planned_duration_ms,
            sample_period_ms(task.is_some(), period_ms, status.cycle_interval_ms),
            RECORDING_SAMPLE_LIMIT,
            plan.expected_settling_ms,
        );
        let symbolic = bindings.iter().any(|binding| binding.symbol.is_some());
        // A recording names the program it runs on, and a connected record needs that name (A4.4). A symbolic recording takes it
        // from its first coherent inspection, which every later inspection and start names (E11.4). Any other recording takes the
        // runtime's revision from the status read above: the runtime refuses its start when the program has changed since
        // (`start_io_recording_for_program`, E4.2), so the name stays true.
        let mut program_revision: Option<String> = (!symbolic)
            .then(|| status.program_revision.clone())
            .filter(|revision| !revision.is_empty());
        for attempt in 0..START_ATTEMPT_LIMIT {
            if symbolic {
                let revision = inspect_coherently(
                    &mut client,
                    loop_definition,
                    program_revision.as_deref(),
                    &bindings,
                )
                .map_err(RecordingStartError::Failed)?;
                if program_revision.is_none() {
                    program_revision = Some(revision);
                }
            }

            let request = RecordingRequest {
                recording_id: plan.recording_id.clone(),
                loop_revision: loop_definition.revision.clone(),
                program_revision: program_revision.clone(),
                bindings: bindings.clone(),
                maximum_samples: RECORDING_SAMPLE_LIMIT,
                maximum_duration_ms: plan.planned_duration_ms,
                task: task.clone(),
                record_every,
            };
            request.check().map_err(RecordingStartError::Failed)?;
            let params = serde_json::to_value(&request).map_err(|error| {
                RecordingStartError::Failed(Failure::invalid_request(&error.to_string()))
            })?;
            let session = Self {
                connection: connection.clone(),
                loop_definition: loop_definition.clone(),
                request,
                source: source_of(&status),
                start_failure: None,
                stopped: None,
                released: false,
                cursor: FollowCursor::default(),
                first_time_ns: None,
                pending: Arc::clone(pending),
            };
            let owner = session.recovery_owner();
            if let Some(journal) = persistence {
                journal
                    .persist(&owner)
                    .map_err(RecordingStartError::Failed)?;
            }
            match send_start(&mut client, params, &plan.recording_id) {
                Ok(()) => return Ok(session),
                // No coherent inspection of the symbols is current: a fresh inspection and a new start follow (E11.4).
                Err(ControlError::Refused { error, code })
                    if may_retry_start(
                        attempt,
                        symbolic && program_revision.is_some(),
                        code.as_deref(),
                        &error,
                    ) => {}
                Err(ControlError::Refused { error, code }) => {
                    // The refusal came before any buffer was allocated, so nothing is left to recover (E11.6).
                    if let Some(journal) = persistence {
                        journal.clear(&owner).map_err(RecordingStartError::Failed)?;
                    }
                    return Err(RecordingStartError::Failed(Failure::from_runtime(
                        code.as_deref().unwrap_or_default(),
                        &error,
                    )));
                }
                // Never repeat an uncertain start: keep the owner, and stop the recording it may have begun (E11.4).
                Err(lost) => {
                    return Err(unconfirmed_start(session, lost_failure(lost), persistence));
                }
            }
        }
        Err(RecordingStartError::Failed(Failure::new(
            "Recording could not obtain a coherent symbolic selection.",
            Cause::Runtime,
            NextAction::Wait,
        )))
    }

    /// Rebuilds the session of a persisted owner, so that only its recording can be stopped or released (E11.6). The endpoint
    /// is the one given, and a start that was not acknowledged stays unacknowledged (`fromRecoveryOwner`).
    ///
    /// # Errors
    /// The owner does not belong to `loop_definition`: another project, runtime, loop or revision, or no binding of 1 to 32.
    pub fn from_recovery_owner(
        connection: &RuntimeConnection,
        loop_definition: &LoopDefinition,
        owner: &RecordingRecoveryOwner,
        pending: &Arc<PendingReleases>,
    ) -> Result<Self, Failure> {
        check_recording_owner_scope(owner, loop_definition)?;
        let bindings: Vec<SignalBinding> = owner.bindings.iter().map(signal_binding).collect();
        Ok(Self {
            connection: connection.clone(),
            loop_definition: loop_definition.clone(),
            request: identity_request(
                &owner.recording_id,
                &owner.loop_revision,
                owner.program_revision.as_deref(),
                bindings,
            ),
            source: owner.source,
            start_failure: owner
                .start_acknowledgement_unconfirmed
                .filter(|unconfirmed| *unconfirmed)
                .map(|_| UNCONFIRMED_START_RECOVERED.to_string()),
            stopped: None,
            released: false,
            cursor: FollowCursor::default(),
            first_time_ns: None,
            pending: Arc::clone(pending),
        })
    }

    /// The owner the journal keeps: the recording's identity, with no credential (E11.2, E11.6).
    #[must_use]
    pub fn recovery_owner(&self) -> RecordingRecoveryOwner {
        RecordingRecoveryOwner {
            schema_version: 1,
            recording_id: self.request.recording_id.clone(),
            project_id: self.loop_definition.project_id.clone(),
            runtime_id: self.loop_definition.runtime_id.clone(),
            loop_id: self.loop_definition.id.clone(),
            loop_revision: self.loop_definition.revision.clone(),
            source: self.source,
            program_revision: self.request.program_revision.clone(),
            start_acknowledgement_unconfirmed: self.start_failure.is_some().then_some(true),
            bindings: self.request.bindings.iter().map(recovery_binding).collect(),
        }
    }

    /// The recording's identifier on the runtime.
    #[must_use]
    pub fn recording_id(&self) -> &str {
        &self.request.recording_id
    }

    /// Stops the recording and returns its run. A stop is idempotent: a repeated stop returns the same run without asking
    /// the runtime again (E4.7, E11.4). The run is kept by the caller before [`RuntimeRecordingSession::release`].
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers with a run that does not answer the request.
    pub fn stop(&mut self) -> Result<RecordedRun, Failure> {
        if let Some(run) = &self.stopped {
            return Ok(run.clone());
        }
        let mut client = RuntimeClient::connect(&self.connection)?;
        let run = client.stop_recording(&self.request)?;
        self.stopped = Some(run.clone());
        if !self.released {
            self.pending.keep(PendingRelease {
                endpoint: self.connection.endpoint.clone(),
                recording_id: self.request.recording_id.clone(),
                loop_revision: self.request.loop_revision.clone(),
            });
        }
        Ok(run)
    }

    /// The evidence of a stopped run of this recording (E4.5, E11.8): complete only when its acquisition is complete, it holds
    /// at least two samples at one steady record period, and its start was acknowledged.
    #[must_use]
    pub fn evidence(&self, run: &RecordedRun) -> RecordingEvidence {
        recording_evidence(run, self.start_failure.as_deref())
    }

    /// Frees the runtime buffer of the stopped recording. Call it only after the run that [`RuntimeRecordingSession::stop`]
    /// returned is kept (E4.7, E11.6). A release whose reply is lost keeps the buffer pending, and the next start or release
    /// retries it.
    ///
    /// # Errors
    /// The recording has not been stopped.
    pub fn release(&mut self) -> Result<(), Failure> {
        if self.stopped.is_none() {
            return Err(Failure::new(
                "Stop the recording before releasing it.",
                Cause::Implementation,
                NextAction::None,
            ));
        }
        if self.released {
            return Ok(());
        }
        let released = RuntimeClient::connect(&self.connection)
            .and_then(|mut client| client.release_recording(&self.request));
        if released.is_ok() {
            self.released = true;
            self.pending.settle(&self.request.recording_id);
        }
        Ok(())
    }

    /// The next page of the running recording, from the sample after the last page read (E4.6).
    ///
    /// # Errors
    /// The runtime cannot be reached or refuses, or its answer does not describe this recording and the samples after the
    /// last one read.
    pub fn follow(&mut self) -> Result<RecordingStatus, Failure> {
        let from_sample = self.cursor.next_sample;
        let mut client = RuntimeClient::connect(&self.connection)?;
        let page = client.recording_status(&self.request, Some(from_sample))?;
        check_status_page(&page, &self.cursor, &self.request.bindings)?;
        self.cursor = self.cursor.advance(&page);
        Ok(page)
    }
}

impl RecordingSession for RuntimeRecordingSession {
    type Recording = RecordedRun;

    fn loop_definition(&self) -> &LoopDefinition {
        &self.loop_definition
    }

    fn stop(&mut self) -> Result<RecordedRun, Failure> {
        RuntimeRecordingSession::stop(self)
    }

    fn release(&mut self) -> Result<(), Failure> {
        RuntimeRecordingSession::release(self)
    }

    fn recovery_owner(&self) -> Option<RecordingRecoveryOwner> {
        Some(RuntimeRecordingSession::recovery_owner(self))
    }
}

/// The session as an identification campaign follows it: each call reads the next status page of the running recording and
/// returns its samples as the campaign reads them (`follow_samples`). The session's own `follow` returns the page as the runtime
/// sent it; it is called here by its type, because this method has the same name.
impl RecordingFollower for RuntimeRecordingSession {
    fn follow(&mut self) -> Result<Vec<FollowSample>, Failure> {
        let page = RuntimeRecordingSession::follow(self)?;
        follow_samples(
            &self.loop_definition,
            &self.request.bindings,
            &page.samples,
            &mut self.first_time_ns,
        )
    }
}

/// Releases the recordings of the same endpoint that a failed release left pending. A failure keeps them pending; the start
/// reports the runtime's retained-buffer limit if it cannot free one.
fn sweep_pending_releases(
    client: &mut RuntimeClient,
    connection: &RuntimeConnection,
    pending: &PendingReleases,
) {
    for waiting in pending.waiting_on(&connection.endpoint) {
        let request = identity_request(
            &waiting.recording_id,
            &waiting.loop_revision,
            None,
            Vec::new(),
        );
        if client.release_recording(&request).is_ok() {
            pending.settle(&waiting.recording_id);
        }
    }
}

/// Sends `control.recording.start` and accepts only the answer that E4.2 names. Any other answer is never confirmation
/// (E11.8).
fn send_start(
    client: &mut RuntimeClient,
    params: Value,
    recording_id: &str,
) -> Result<(), ControlError> {
    let answer = client
        .client
        .request("control.recording.start", Some(params))?;
    if answer == json!({ "recording_id": recording_id, "status": "recording" }) {
        Ok(())
    } else {
        Err(ControlError::Malformed(
            "the answer to control.recording.start is not the one E4.2 names".to_string(),
        ))
    }
}

/// Inspects `bindings` until a coherent scan answers ([`RuntimeClient::inspect_until_scanned`]), pinned to the program revision
/// `expected` names, when it does; otherwise to the revision of the first answer (E11.4). Returns the program revision of the
/// coherent scan.
fn inspect_coherently(
    client: &mut RuntimeClient,
    loop_definition: &LoopDefinition,
    expected: Option<&str>,
    bindings: &[SignalBinding],
) -> Result<String, Failure> {
    match client.inspect_until_scanned(loop_definition, expected, bindings)? {
        Inspection::Available(sample) => Ok(sample.program_revision),
        Inspection::Pending { .. } => Err(Failure::new(
            "Wait for a current coherent symbolic scan, then retry recording.",
            Cause::Runtime,
            NextAction::Wait,
        )),
    }
}

/// The identity of a recording, for the calls that name it alone: stop, status and release (E4.6, E4.7). Their limits are
/// those of the start, which only the start sends, so the placeholders below are never sent.
fn identity_request(
    recording_id: &str,
    loop_revision: &str,
    program_revision: Option<&str>,
    bindings: Vec<SignalBinding>,
) -> RecordingRequest {
    RecordingRequest {
        recording_id: recording_id.to_string(),
        loop_revision: loop_revision.to_string(),
        program_revision: program_revision.map(str::to_string),
        bindings,
        maximum_samples: RECORDING_SAMPLE_LIMIT,
        maximum_duration_ms: RECORDING_DURATION_LIMIT_MS,
        task: None,
        record_every: 1,
    }
}

fn task_period_ms(loop_definition: &LoopDefinition) -> Result<u64, Failure> {
    let period = loop_definition.controller.task_period_ms;
    whole_milliseconds(period).ok_or_else(|| {
        Failure::invalid_request(&format!(
            "The loop's task period {period} ms is not a whole number of milliseconds."
        ))
    })
}

fn source_of(status: &RuntimeStatus) -> ControlEvidenceSource {
    match status.source() {
        EvidenceSource::SimulatedPlant => ControlEvidenceSource::Simulation,
        EvidenceSource::PhysicalPlant => ControlEvidenceSource::Physical,
    }
}

/// Keeps the owner of a start whose acknowledgement was lost, and stops the recording the start may have begun, so that its
/// samples are the runtime's (E11.4). A stop that fails too leaves the owner unresolved.
fn unconfirmed_start(
    mut session: RuntimeRecordingSession,
    start_error: Failure,
    persistence: Option<&dyn RecordingOwnerPersistence>,
) -> RecordingStartError<RuntimeRecordingSession> {
    session.start_failure = Some(text(&start_error).to_string());
    if let Some(journal) = persistence {
        if let Err(error) = journal.persist(&session.recovery_owner()) {
            return RecordingStartError::Failed(error);
        }
    }
    let (recording, stop_error) = match session.stop() {
        Ok(run) => (Some(run), None),
        Err(error) => (None, Some(error)),
    };
    RecordingStartError::Unconfirmed(Box::new(RecordingStartUnconfirmed {
        session,
        recording,
        start_error,
        stop_error,
    }))
}

fn lost_failure(error: ControlError) -> Failure {
    match error {
        ControlError::Transport(reason) => Failure::unreachable(&reason),
        ControlError::Refused { error, code } => {
            Failure::from_runtime(code.as_deref().unwrap_or_default(), &error)
        }
        ControlError::Malformed(_) => Failure::invalid_response("control.recording.start"),
    }
}

fn signal_binding(binding: &RecordingRecoveryBinding) -> SignalBinding {
    SignalBinding {
        id: binding.id.clone(),
        address: binding.address.clone(),
        symbol: binding.symbol.clone(),
        iec_type: binding.iec_type.clone(),
        unit: None,
    }
}

fn recovery_binding(binding: &SignalBinding) -> RecordingRecoveryBinding {
    RecordingRecoveryBinding {
        id: binding.id.clone(),
        address: binding.address.clone(),
        symbol: binding.symbol.clone(),
        iec_type: binding.iec_type.clone(),
    }
}

/// The words of a failure as the engineer reads them in a longer message: the whole text, when it says more.
/// A failure's text as a clause inside another sentence: its detail, or else its message, without its full stop.
fn text(failure: &Failure) -> &str {
    let text = failure.detail.as_deref().unwrap_or(&failure.message);
    text.strip_suffix('.').unwrap_or(text)
}

#[cfg(test)]
#[path = "runtime_recording_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "runtime_recording_follower_tests.rs"]
mod follower_tests;
