//! The isolated test runs of Validate (specification 35, A2.6, A8.5, A10.5 and B5.3 to B5.5; runtime protocol annex E8.4,
//! E11.2, E11.4 and E11.5): each run is journaled, started, read until the runtime ends it and read with its results, and a run
//! that the engineer cancels is cancelled at the runtime and its end confirmed before the operation ends.
//!
//! Every decision is trust-control's: the requests (built by the caller from `validation::run_requests`), the checks of each
//! answer (`IsolatedRunStatus::check`) and the assessment of the evidence. This module makes the calls in their order and keeps
//! the journal, so that a lost acknowledgement is read by its identity and never started again (E11.4). The model check, the
//! first part of Validate (B5.2), is `workflow::validate_model_check`; the order of the two parts is the caller's.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::validation::engineer_verification_campaign::CampaignCase;
use trust_control::validation::isolated_run_protocol::{IsolatedRunRequest, IsolatedRunStatus};

use crate::journal::{Journal, JournalEntry};
use crate::operation::cancelled;
use crate::runtime_client::RuntimeClient;

/// The failure of a run whose end the runtime does not confirm in time (E8.4, E11.4). Its identity stays journaled.
const CANCEL_UNCONFIRMED: &str = "The runtime did not confirm that the isolated run ended, so it is not known to have stopped. Its identity stays journaled.";
/// The failure of a run that the runtime stopped answering for past its wall budget and margin (E8.4); the run is cancelled.
const MONITORING_TIMED_OUT: &str = "Verification monitoring timed out; cancellation requested.";
/// The failure of a campaign whose current and candidate halves name other scenarios: the engine's own wiring.
const CAMPAIGNS_DIFFER: &str = "The current and the candidate runs name other scenarios.";

/// The purpose of an isolated job (E8.4, E11.2): a test run, which Validate starts for a scenario's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IsolatedJobPurpose {
    /// An isolated test run (E8.4).
    Test,
}

/// The identity of an isolated job, journaled before its start is dispatched (E11.2, A8.5): the runtime's job, the project and
/// the runtime it runs on, the loop it validates, the project revision it runs on and its purpose. It holds no credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IsolatedJobOwner {
    /// The job's identifier: the request's `request_id`, which the runtime answers as its `job_id` (E8.4).
    pub job_id: String,
    /// The project the job runs in (A8.2).
    pub project_id: String,
    /// The runtime the job runs on (E11.1).
    pub runtime_id: String,
    /// The loop the job validates (A8.2).
    pub loop_id: String,
    /// The project revision the job runs on (E8.2, E8.4).
    pub project_revision: String,
    /// What the job is for.
    pub purpose: IsolatedJobPurpose,
}

/// The identity the test runs of one Validate share: the project, the runtime and the loop they validate, and the project
/// revision they run on (E8.2). Each job takes it with its own identifier (E11.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunIdentity {
    /// The project the runs are in.
    pub project_id: String,
    /// The runtime the runs start on (E11.1).
    pub runtime_id: String,
    /// The loop the runs validate.
    pub loop_id: String,
    /// The project revision the runs run on, as the runtime's readiness answer gives it (E8.2).
    pub project_revision: String,
}

impl RunIdentity {
    /// The journal identity of the job `job_id` of these runs (E11.2).
    #[must_use]
    pub fn owner(&self, job_id: &str) -> IsolatedJobOwner {
        IsolatedJobOwner {
            job_id: job_id.to_string(),
            project_id: self.project_id.clone(),
            runtime_id: self.runtime_id.clone(),
            loop_id: self.loop_id.clone(),
            project_revision: self.project_revision.clone(),
            purpose: IsolatedJobPurpose::Test,
        }
    }
}

/// How a job is waited for (E8.4, A7): the pause between two reads, the time the engine waits past the job's wall budget for
/// the runtime to report its end, and the time a cancellation may take to be confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobTiming {
    /// The pause between two reads of a job that has not ended.
    pub poll: Duration,
    /// The time past the job's wall budget that the engine waits for the runtime to report the job's end.
    pub margin: Duration,
    /// The time a cancellation may take before the runtime confirms the job's end.
    pub confirmation: Duration,
}

impl JobTiming {
    /// The engine's timing: a read every 100 ms, and five seconds each for the margin and for a confirmed cancellation.
    pub const PRODUCTION: Self = Self {
        poll: Duration::from_millis(100),
        margin: Duration::from_secs(5),
        confirmation: Duration::from_secs(5),
    };
}

/// The runtime side of the isolated runs of one Validate: the connection, the journal, the identity the jobs share, how the
/// operation reports and whether it was cancelled, and how the runs are waited for.
pub struct RunSession<'a> {
    /// The connection the runs are started and read on (E11.1).
    pub client: &'a mut RuntimeClient,
    /// The operation journal, where each job's identity is written before its start (E11.2).
    pub journal: &'a mut Journal,
    /// The identity every job of the session carries.
    pub identity: RunIdentity,
    /// Whether the operation was cancelled.
    pub is_cancelled: &'a dyn Fn() -> bool,
    /// Reports what the operation is doing.
    pub progress: &'a dyn Fn(&str),
    /// How the jobs are waited for.
    pub timing: JobTiming,
}

/// Which settings an isolated run of a scenario runs with (B5.3): the program's own compiled settings, or the candidate's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settings {
    /// The current settings: the program's own compiled record, with no parameter overrides.
    Current,
    /// The candidate settings: the candidate record as parameter overrides.
    Candidate,
}

/// The two isolated runs of one scenario (B5.3): the run with the current settings and the run with the candidate, each as the
/// runtime ended it. Trust-control decides what each one proves (E8.4, B5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioRuns {
    /// The scenario's identity: `target-response`, an envelope case's identity, or a declared scenario's id.
    pub id: String,
    /// The run with the current settings, with its results once it ended.
    pub current: IsolatedRunStatus,
    /// The run with the candidate settings, with its results once it ended.
    pub candidate: IsolatedRunStatus,
}

/// A new identifier for an isolated job: `isolated-` and 32 hex digits from the random seeds of the standard hash map, as the
/// recording and experiment identifiers are drawn (E0.2). The runtime accepts letters, digits and hyphens (E8.4).
#[must_use]
pub fn new_job_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let state = RandomState::new();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "isolated-{:016x}{:016x}",
        state.hash_one(count),
        state.hash_one(!count)
    )
}

/// Runs every scenario of the two campaigns as isolated test runs, in the campaigns' order (B5.3): for each scenario the run with
/// the current settings, then the run with the candidate. `current` and `candidate` are the campaigns built from the current and
/// the candidate settings; they name the same scenarios in the same order. `build` makes the request of a case with the settings
/// it runs with and the identifier the job takes (trust-control's request builder).
///
/// # Errors
/// The two campaigns name other scenarios; `build` refuses a request; or a run cannot be run to its end (see [`run_job`]). The
/// runs that ended before the failure keep their journal entries cleared, and the failing run keeps its own (E11.4).
pub fn run_scenarios(
    session: &mut RunSession<'_>,
    current: &[CampaignCase],
    candidate: &[CampaignCase],
    build: &dyn Fn(&CampaignCase, Settings, &str) -> Result<IsolatedRunRequest, Failure>,
) -> Result<Vec<ScenarioRuns>, Failure> {
    let same_scenarios = current.len() == candidate.len()
        && current
            .iter()
            .zip(candidate)
            .all(|(one, other)| one.id == other.id);
    if !same_scenarios {
        return Err(Failure::new(
            CAMPAIGNS_DIFFER,
            Cause::Implementation,
            NextAction::None,
        ));
    }
    let mut runs = Vec::with_capacity(candidate.len());
    for (current_case, candidate_case) in current.iter().zip(candidate) {
        (session.progress)(&format!("Running scenario {}.", candidate_case.id));
        let current_run = run_case(session, current_case, Settings::Current, build)?;
        let candidate_run = run_case(session, candidate_case, Settings::Candidate, build)?;
        runs.push(ScenarioRuns {
            id: candidate_case.id.clone(),
            current: current_run,
            candidate: candidate_run,
        });
    }
    Ok(runs)
}

/// Builds the request of one case with one settings and runs it to its end.
fn run_case(
    session: &mut RunSession<'_>,
    case: &CampaignCase,
    settings: Settings,
    build: &dyn Fn(&CampaignCase, Settings, &str) -> Result<IsolatedRunRequest, Failure>,
) -> Result<IsolatedRunStatus, Failure> {
    let job_id = new_job_id();
    let request = build(case, settings, &job_id)?;
    if request.request_id != job_id {
        return Err(Failure::new(
            "The request of an isolated run names another identifier than its job.",
            Cause::Implementation,
            NextAction::None,
        ));
    }
    run_job(session, &request)
}

/// Runs one isolated test run to its end (E8.4, E11.2): its identity is journaled, the run is started, read until the runtime
/// ends it, and read again with its results. The terminal status is returned whatever the runtime ended the run with: a run that
/// completed, failed or was cancelled by the runtime is evidence of what it recorded, and trust-control decides what that proves
/// (E8.4, B5.4). The identity leaves the journal once that status is read.
///
/// A run cancelled before its start is never dispatched, so nothing is journaled. A cancelled run is cancelled at the runtime,
/// and its end is confirmed before the operation ends (E8.4).
///
/// # Errors
/// - the operation was cancelled before the start ([`cancelled`]);
/// - the identity cannot be journaled (nothing is started);
/// - the start is not acknowledged and the runtime does not hold the job: the start's error is returned and the identity stays
///   journaled, because a start is never repeated to resolve an unknown outcome (E11.4);
/// - a read fails, or the wall budget and its margin pass without the runtime reporting an end: the run is cancelled, and that
///   failure is returned once the runtime confirms the end;
/// - the operation was cancelled while the run ran ([`cancelled`]), once the runtime confirms the end;
/// - the runtime does not confirm the end in time: the identity stays journaled (E11.4, E11.5).
pub fn run_job(
    session: &mut RunSession<'_>,
    request: &IsolatedRunRequest,
) -> Result<IsolatedRunStatus, Failure> {
    if (session.is_cancelled)() {
        return Err(cancelled());
    }
    let owner = session.identity.owner(&request.request_id);
    session.journal.record(JournalEntry::IsolatedJob {
        job: Box::new(owner),
    })?;
    if let Err(error) = session.client.start_isolated_run(request) {
        // A lost acknowledgement is read by the job's identity and never started again (E11.4). A job the runtime does not
        // hold means the start did not happen; its identity stays journaled until the engineer resolves it.
        if session.client.isolated_run_status(request, None).is_err() {
            return Err(error);
        }
    }
    let deadline =
        Instant::now() + Duration::from_millis(request.wall_budget_ms) + session.timing.margin;
    loop {
        if (session.is_cancelled)() {
            return Err(stop(session, request, cancelled()));
        }
        match session.client.isolated_run_status(request, None) {
            Ok(status) if status.status.is_terminal() => {
                // The ended run is read with its results. Its identity leaves the journal only once they are read.
                let ended = session.client.isolated_run_status(request, Some(true))?;
                session.journal.clear(&request.request_id)?;
                return Ok(ended);
            }
            Ok(_) if Instant::now() < deadline => thread::sleep(session.timing.poll),
            Ok(_) => {
                let timed_out =
                    Failure::new(MONITORING_TIMED_OUT, Cause::Runtime, NextAction::None);
                return Err(stop(session, request, timed_out));
            }
            Err(error) => return Err(stop(session, request, error)),
        }
    }
}

/// Stops a started run the engine has to end: the run is cancelled at the runtime and its end confirmed, then its identity leaves
/// the journal. The returned failure is `reason`. When the end is not confirmed, the identity stays journaled and the failure is
/// the unconfirmed cancellation (E11.4, E11.5).
fn stop(session: &mut RunSession<'_>, request: &IsolatedRunRequest, reason: Failure) -> Failure {
    if let Err(unconfirmed) = confirm_end(session, request) {
        return unconfirmed;
    }
    match session.journal.clear(&request.request_id) {
        Ok(()) => reason,
        Err(error) => error,
    }
}

/// Cancels the started run at the runtime and reads its status until the runtime reports the end (E8.4): `cancelled`, `failed` or
/// `completed`. The runtime has the session's confirmation time to report it.
///
/// # Errors
/// The cancellation or a read is refused or fails, or the end is not reported within the confirmation time; the failure names the
/// reason and keeps the identity journaled.
fn confirm_end(session: &mut RunSession<'_>, request: &IsolatedRunRequest) -> Result<(), Failure> {
    let timing = session.timing;
    let deadline = Instant::now() + timing.confirmation;
    let mut status = session
        .client
        .cancel_isolated_run(request)
        .map_err(|error| unconfirmed(&error.message))?;
    loop {
        if status.status.is_terminal() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(unconfirmed("The runtime still reports the run as running."));
        }
        thread::sleep(timing.poll);
        status = session
            .client
            .isolated_run_status(request, None)
            .map_err(|error| unconfirmed(&error.message))?;
    }
}

/// The failure of a run whose end the runtime did not confirm: [`CANCEL_UNCONFIRMED`], with the reason it gives.
fn unconfirmed(reason: &str) -> Failure {
    Failure::new(
        &format!("{CANCEL_UNCONFIRMED} {reason}"),
        Cause::Runtime,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
