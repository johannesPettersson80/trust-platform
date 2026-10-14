//! The recovery of an interrupted identification (`connectedIdentificationRecovery.ts`, and `resumeCampaign` of
//! `connectedIdentification.ts`; specification 35, A6.2, A8.5, A8.6 and A10.4; runtime protocol annex E11.4 to E11.6).
//!
//! A campaign journals its step before the start is sent, so after a restart of VS Code the loop's journal may hold a step
//! whose experiment still runs. Reopening reattaches it ([`resume_identification`]): the recording is recovered, the
//! experiment is read by its identity, and the hold is followed to its end without starting or aborting anything. The campaign
//! then completes as it would have: the restoration is confirmed, the recording is kept, the journal is cleared and the
//! recording is fitted. A journal that holds no step to resume is cancelled instead ([`cancel_identification_recovery`]): a
//! running experiment is followed to its end with status reads only, never aborted, and then the original recording is stopped
//! and kept and the journal cleared (E11.5). An experiment whose status cannot be read is left to the runtime with its journal,
//! and no start is ever sent again (E11.4).
//!
//! The engineer's Cancel of a running resume is the operation's cancellation: the hold aborts the experiment through the
//! runtime's restoration path, as the reattached client's Cancel does (E11.4, B3.4). Closing the panel does not cancel the
//! operation (E11.5, E11.11). The engine keeps no client between operations, so the release of a closed panel
//! (`release`), the attempt it shows (`snapshot`) and the refusal of a Cancel while a resume reattaches have no counterpart
//! here.
//!
//! A mode that the interrupted identification changed returns with it (A6.2, A8.6): once nothing of the campaign is left
//! running, a resume returns the loop to the mode it was found in before the fit, and a Cancel returns it after the recording is
//! kept. A journal that holds only the mode checkpoint is completed by returning the mode. The completion is part of the
//! journaled operation, not a new one.

use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use trust_control::experiment::{ControlExperimentRecovery, RecordingRecoveryOwner};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::automatic::AutomaticIdentificationResult;
use trust_control::identification::campaign::journal::{
    IdentificationCampaignJournal, NO_RUNNING_EXPERIMENT_TEXT,
};
use trust_control::identification::connected_run::{
    fit_window, hold_end, is_provisional, not_journaled, stopped_hold_outcome, HoldEnd,
};
use trust_control::identification::independent_validation::RunEvaluation;
use trust_control::identification::reconcile::ExperimentIdentity;
use trust_control::identification::texts::apply_cancel_refused;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;

use super::identify::cancel::{cancel_journaled, end_experiment};
use super::identify::run::{clear_entries, connected_record, fit_records, retain_recording};
use super::identify::{
    cancel_identification, ConnectedRuntime, IdentifyRequest, IdentifyRuntime, RecordingRetention,
};
use super::identify_campaign::{CampaignClock, CampaignReport, IdentifyCampaign, SystemClock};
use super::identify_mode::{mode_journaled, IdentificationMode};
use super::reconcile::{follow_until_ended, read_experiment_identity};
use crate::journal::{Journal, JournalEntry};
use crate::operation::OperationContext;
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::runtime_client::RuntimeClient;

/// How often a cancelled recovery reads its running experiment, in ms of wall time (`followUntilEnded`'s `intervalMs`).
const FOLLOW_INTERVAL_MS: u64 = 500;
/// The refusal of a recovery on a runtime that is not the original simulator (`connect`).
const SIMULATOR_TEXT: &str = "Reconnect the original simulator for identification recovery.";
/// Why a cancelled recovery waits (`RUNNING_REASON`): the runtime still runs the interrupted experiment, which only status
/// reads follow.
const RUNNING_TEXT: &str = "The interrupted experiment is still running in the runtime. The original mode returns when it ends; nothing is started or aborted meanwhile.";

/// What a resumed campaign gave (`resume` of `IdentificationRecovery`): the fit of the recording of its one held step, as a
/// campaign that was never interrupted gives it.
#[derive(Debug, Clone, PartialEq)]
pub struct ResumedIdentification<E> {
    /// The model and the record it was fitted from.
    pub result: AutomaticIdentificationResult<E>,
    /// Whether the model is provisional: a fit from the campaign's own recording is never validated evidence (specification
    /// "Single experiment and provisional model"), so it can be tuned but cannot authorize Apply.
    pub provisional: bool,
    /// The runtime recording the result came from: the run's identifier in its record.
    pub recording_id: String,
}

/// The ports a resume reads and writes: the runtime, where the recording is kept, the clock that paces the polls, the
/// operation's cancellation and progress, and the loop's journal.
pub(crate) struct RecoveryPorts<'a, T, K, C, P> {
    pub(crate) runtime: &'a T,
    pub(crate) retention: &'a mut K,
    pub(crate) clock: &'a C,
    pub(crate) report: &'a P,
    pub(crate) journal: &'a Mutex<Journal>,
}

/// Resumes the identification campaign journaled for the loop of `request` (`resume` of `connectedIdentificationRecovery.ts`,
/// and `resumeCampaign`; A8.5, A8.6). The journal in `storage_folder` must hold a dispatched step of the loop's revision. The
/// campaign's recording is recovered on the runtime of `connection`, and its experiment is read by its identity. A running or
/// ended experiment is followed to the end of its hold without a start or an abort, and finished once it has settled, as the
/// hold rule says (B3.4). Once nothing of the campaign is left running, its recording is kept through `retention` before the
/// runtime buffer is released and the journal is cleared, the loop returns to the mode the identification found it in when the
/// journal holds its mode checkpoint (A6.2, A8.6), and then the recording of a completed step is fitted. The operation of
/// `context` shows the progress, and its cancellation is the engineer's Cancel: the hold aborts the experiment through the
/// runtime's restoration path (E11.4).
///
/// # Errors
/// The runtime cannot be reached or is not simulated (reconnect the original simulator); the journal cannot be read or holds
/// no step to resume; the recording cannot be recovered; the experiment's status cannot be read, its hold fails or its
/// restoration is not confirmed, and the experiment, its recording and the journal are then left to the runtime (A8.6); the
/// experiment never started, stopped before its hold ended, or covered one of several plant inputs; or the recording cannot be
/// kept, its record or its fit fails, the journal cannot be cleared, or the mode cannot be returned (whose failure replaces only
/// a record that succeeded).
pub fn resume_identification<K, E>(
    context: &OperationContext,
    connection: &RuntimeConnection,
    pending: &Arc<PendingReleases>,
    storage_folder: &Path,
    request: &IdentifyRequest<'_, E>,
    retention: &mut K,
) -> Result<ResumedIdentification<E>, Failure>
where
    K: RecordingRetention,
    E: RunEvaluation + Clone,
{
    require_simulator(connection)?;
    let loop_definition = request.loop_definition;
    let journal = Mutex::new(Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?);
    let runtime = ConnectedRuntime::new(connection, pending);
    let clock = SystemClock::new();
    resume(
        RecoveryPorts {
            runtime: &runtime,
            retention,
            clock: &clock,
            report: context,
            journal: &journal,
        },
        request,
    )
}

/// Cancels the recovery of the identification journaled for `loop_definition` in `storage_folder` (`cancel` of
/// `connectedIdentificationRecovery.ts`, E11.5): the recovery of a journal that holds no step to resume, such as a campaign
/// still in its baseline or one of another loop revision. A running experiment of the campaign is followed on the runtime of
/// `connection` with status reads only, one every half second, while the operation of `context` shows why it waits; it is
/// never aborted. Once no experiment of the campaign is left running, the identification's Cancel finishes the recovery: the
/// ended experiment's restoration is confirmed, the original recording is stopped and kept through `retention` before its
/// buffer is released, and the journal is cleared. Then the loop returns to the mode the identification found it in when the
/// journal holds its mode checkpoint, with or without a campaign (A6.2). Without a journaled campaign or mode checkpoint
/// nothing is cancelled.
///
/// # Errors
/// The runtime cannot be reached or is not simulated (reconnect the original simulator); the journal cannot be read; the
/// experiment's status cannot be read, and nothing is restored; the operation was cancelled while the experiment still ran,
/// and the journal stays; the identification's Cancel fails (see [`cancel_identification`]); or the mode cannot be returned,
/// and its checkpoint stays journaled.
pub fn cancel_identification_recovery<K: RecordingRetention>(
    context: &OperationContext,
    connection: &RuntimeConnection,
    pending: &Arc<PendingReleases>,
    storage_folder: &Path,
    loop_definition: &LoopDefinition,
    retention: &mut K,
) -> Result<(), Failure> {
    require_simulator(connection)?;
    let journal = Mutex::new(Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?);
    let runtime = ConnectedRuntime::new(connection, pending);
    let clock = SystemClock::new();
    cancel_recovery(&runtime, &clock, context, &journal, || {
        cancel_identification(
            connection,
            pending,
            storage_folder,
            loop_definition,
            retention,
        )
    })?;
    // The Cancel wrote the journal through a journal of its own, so the mode's return reads it afresh.
    let journal = Mutex::new(Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?);
    return_found_mode(&runtime, &journal, loop_definition)
}

/// Cancels the journaled operation `operation_id` of the loop (§0.1b `cancel`, E5.5, E11.6), by what the operation is: an
/// identification campaign is cancelled as the identification's Cancel does (`cancel_identification`): a running experiment is
/// aborted through the runtime's restoration path, its restoration is confirmed, and its recording is kept. A lone recording
/// owner is stopped through the runtime, its recording is kept in the study before its buffer is released, and its owner is
/// cleared. A journaled Test experiment is cancelled as a campaign's experiment is (`cancel_test_experiment`). The experiment of
/// the journaled step belongs to its campaign, so naming it cancels the campaign. An identification's Cancel then returns the
/// loop to the mode the identification found it in (A6.2), and naming its mode checkpoint is the same Cancel. A journaled Apply
/// is not cancelled: Restore previous settings ends it (B6.4). The other operations of the journal are left as they are.
///
/// # Errors
/// The operation is not journaled ([`not_journaled`]), or it is an Apply; the campaign's Cancel fails (see
/// [`cancel_identification`]); the mode cannot be returned, and its checkpoint stays journaled; the Test experiment's
/// restoration cannot be confirmed; or the recording of the owner cannot be recovered, stopped, kept or released, and its owner
/// stays journaled.
pub fn cancel_operation<K: RecordingRetention>(
    connection: &RuntimeConnection,
    pending: &Arc<PendingReleases>,
    storage_folder: &Path,
    loop_definition: &LoopDefinition,
    operation_id: &str,
    retention: &mut K,
) -> Result<(), Failure> {
    let journal = Mutex::new(Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?);
    cancel_operation_on(
        &ConnectedRuntime::new(connection, pending),
        &SystemClock::new(),
        &journal,
        loop_definition,
        operation_id,
        retention,
    )
}

/// The Cancel of [`cancel_operation`] on `runtime`, with the wall clock `clock` that supervises an experiment's restoration
/// (E11.4, E11.6). The kind of the journaled operation decides, as there.
///
/// # Errors
/// See [`cancel_operation`].
pub(crate) fn cancel_operation_on<T, K, C>(
    runtime: &T,
    clock: &C,
    journal: &Mutex<Journal>,
    loop_definition: &LoopDefinition,
    operation_id: &str,
    retention: &mut K,
) -> Result<(), Failure>
where
    T: IdentifyRuntime,
    K: RecordingRetention,
    C: CampaignClock,
{
    let entry = journal
        .lock()
        .entries()
        .iter()
        .find(|entry| entry.operation_id() == operation_id)
        .cloned();
    // The experiment of the journaled step belongs to its campaign: naming it cancels the campaign (§0.1b, E5.5).
    let step_experiment = journaled_campaign(&journal.lock())
        .and_then(|campaign| campaign.plan)
        .is_some_and(|plan| plan.id == operation_id);
    match entry {
        Some(JournalEntry::RecordingOwner { owner }) => {
            cancel_recording_owner(runtime, journal, loop_definition, &owner, retention)
        }
        Some(JournalEntry::Experiment { recovery }) if !step_experiment => cancel_test_experiment(
            runtime,
            clock,
            journal,
            loop_definition,
            &recovery,
            retention,
        ),
        // The identification's Cancel returns the loop to the mode it was found in once no experiment of the campaign is left
        // running (A6.2). The mode checkpoint belongs to its identification: naming it is the same Cancel, which finds no
        // campaign when only the mode is left.
        Some(
            JournalEntry::IdentificationCampaign { .. }
            | JournalEntry::Experiment { .. }
            | JournalEntry::IdentificationMode { .. },
        ) => {
            cancel_journaled(runtime, clock, journal, loop_definition, retention)?;
            return_found_mode(runtime, journal, loop_definition)
        }
        // An Apply is ended by Restore previous settings, which returns the settings and the mode found before it (B6.4).
        Some(JournalEntry::Commissioning { checkpoint }) => Err(Failure::new(
            &apply_cancel_refused(&checkpoint.id),
            Cause::Unclassified,
            NextAction::Restore,
        )),
        // An isolated test run of Validate runs in a disposable simulation that never writes to the running controller and
        // ends within its wall budget (E8.4); an unfinished validation is never kept (B5.4). Cancelling it forgets it.
        Some(JournalEntry::IsolatedJob { job }) => {
            clear_entries(journal, std::slice::from_ref(&job.job_id))
        }
        None => Err(not_journaled(&loop_definition.id, operation_id)),
    }
}

/// The Cancel of a lone recording owner (E11.6): its recording is recovered by its identity, stopped, and kept in the study
/// before its buffer is released (E4.7), and then the owner is cleared from the journal. A recording the runtime does not stop
/// keeps its owner journaled, and the failure is the runtime's.
fn cancel_recording_owner<T: IdentifyRuntime, K: RecordingRetention>(
    runtime: &T,
    journal: &Mutex<Journal>,
    loop_definition: &LoopDefinition,
    owner: &RecordingRecoveryOwner,
    retention: &mut K,
) -> Result<(), Failure> {
    let mut recording = runtime.recover_recording(loop_definition, owner)?;
    retain_recording(&mut recording, retention)?;
    clear_entries(journal, std::slice::from_ref(&owner.recording_id))
}

/// The Cancel of a journaled Test experiment (§0.1b, E5.5, E11.6), as a campaign's experiment is cancelled: it is reattached by
/// its identifier, a running one is aborted through the restoration path and its restoration is confirmed, the recording it
/// owns is recovered, stopped and kept, and its journal entry, which holds the owner, is cleared.
///
/// # Errors
/// The experiment's restoration cannot be confirmed, or its recording cannot be recovered, stopped, kept or released. The
/// journal then stays.
fn cancel_test_experiment<T, K, C>(
    runtime: &T,
    clock: &C,
    journal: &Mutex<Journal>,
    loop_definition: &LoopDefinition,
    recovery: &ControlExperimentRecovery,
    retention: &mut K,
) -> Result<(), Failure>
where
    T: IdentifyRuntime,
    K: RecordingRetention,
    C: CampaignClock,
{
    end_experiment(runtime, clock, &recovery.experiment_id)?;
    if let Some(owner) = &recovery.recording_owner {
        let mut recording = runtime.recover_recording(loop_definition, owner)?;
        retain_recording(&mut recording, retention)?;
    }
    clear_entries(journal, std::slice::from_ref(&recovery.experiment_id))
}

/// Resumes the journaled campaign on `ports` (`resumeCampaign`); see [`resume_identification`].
pub(crate) fn resume<T, K, C, P, E>(
    ports: RecoveryPorts<'_, T, K, C, P>,
    request: &IdentifyRequest<'_, E>,
) -> Result<ResumedIdentification<E>, Failure>
where
    T: IdentifyRuntime,
    K: RecordingRetention,
    C: CampaignClock,
    P: CampaignReport,
    E: RunEvaluation + Clone,
{
    let RecoveryPorts {
        runtime,
        retention,
        clock,
        report,
        journal,
    } = ports;
    let loop_definition = request.loop_definition;
    let campaign = journaled_campaign(&journal.lock()).ok_or_else(no_running_experiment)?;
    let (plan, baseline) = campaign.resumable_step(&loop_definition.revision)?;
    let step_seconds = campaign.step_start_seconds.unwrap_or(0.0);
    let mut recording = runtime.recover_recording(loop_definition, &campaign.recording)?;
    let ended = match read_experiment_identity(|id| runtime.experiment_status(id), &plan.id) {
        // The status cannot be read: the experiment, its recording and the journal stay the runtime's, and reopening
        // reconciles them. Nothing is aborted, kept or cleared here (A8.6, E11.5).
        ExperimentIdentity::Unresolved { reason } => return Err(left_to_runtime(&reason)),
        ExperimentIdentity::Absent { reason } => Err(never_started(&reason)),
        ExperimentIdentity::Running { .. } | ExperimentIdentity::Terminal { .. } => {
            let mut experiment = runtime.experiment();
            // A hold that fails leaves the experiment to the runtime: nothing is aborted, kept or cleared (A8.6).
            let state = IdentifyCampaign::new(
                loop_definition,
                clock,
                &mut recording,
                &mut experiment,
                report,
                false,
                request.expected_settling_seconds,
            )
            .hold(plan, baseline, step_seconds)?;
            match hold_end(&state) {
                HoldEnd::Unrestored(failure) => return Err(failure),
                HoldEnd::Stopped(failure) => {
                    Err(stopped_hold_outcome(report.is_cancelled(), failure))
                }
                HoldEnd::Completed => all_inputs_covered(campaign.channels_remaining),
            }
        }
    };
    // Nothing of the campaign is left running. The recording of a completed step is kept and becomes its record
    // (`completeRecording`); a step is journaled only after its recording's start was acknowledged (E11.3), so the record
    // carries no start failure. Otherwise, or when that keep failed, the recording is kept now (`settleRecording`); a keep
    // that fails here leaves the buffer and the journal for a retry, and its failure replaces the step's.
    let mut kept = false;
    let record = ended.and_then(|()| {
        let run = retain_recording(&mut recording, retention)?;
        kept = true;
        connected_record(
            loop_definition,
            &run,
            None,
            plan,
            fit_window(step_seconds, baseline.window_seconds),
            request.evaluate,
        )
    });
    if !kept {
        retain_recording(&mut recording, retention)?;
    }
    clear_entries(
        journal,
        &[campaign.recording.recording_id.clone(), plan.id.clone()],
    )?;
    // The loop's original mode returns here, once the experiment has ended and before the fit (A6.2, A8.6). A failed return
    // replaces only a record that succeeded; its checkpoint stays journaled for the next reconciliation.
    let returned = return_found_mode(runtime, journal, loop_definition);
    let record = record?;
    returned?;
    let result = fit_records(loop_definition, report, std::slice::from_ref(&record))?;
    Ok(ResumedIdentification {
        provisional: is_provisional(&result),
        recording_id: record.run.id,
        result,
    })
}

/// Follows the experiment of the journaled campaign until nothing of it is left running, then lets `finish` restore the
/// original (`cancel` of `connectedIdentificationRecovery.ts`, E11.5). A running experiment is followed with status reads
/// only, one every [`FOLLOW_INTERVAL_MS`], while the operation shows why it waits; it is never aborted here, and the
/// operation's cancellation ends the follow with nothing restored. An experiment that has ended or that the runtime does not
/// hold lets `finish` run at once, as does a campaign that dispatched no step. Without a journaled campaign nothing is
/// cancelled.
///
/// `finish` is the identification's Cancel of the journaled campaign (`cancel_identification`): it confirms the ended
/// experiment's restoration, stops and keeps the original recording, and clears the journal.
fn cancel_recovery<T, C, P>(
    runtime: &T,
    clock: &C,
    report: &P,
    journal: &Mutex<Journal>,
    finish: impl FnOnce() -> Result<(), Failure>,
) -> Result<(), Failure>
where
    T: IdentifyRuntime,
    C: CampaignClock,
    P: CampaignReport,
{
    let Some(campaign) = journaled_campaign(&journal.lock()) else {
        return Ok(());
    };
    if let Some(plan) = &campaign.plan {
        let read = || read_experiment_identity(|id| runtime.experiment_status(id), &plan.id);
        match read() {
            ExperimentIdentity::Unresolved { reason } => return Err(nothing_restored(&reason)),
            ExperimentIdentity::Running { .. } => {
                report.progress(RUNNING_TEXT);
                if follow_until_ended(read, clock, || report.is_cancelled(), FOLLOW_INTERVAL_MS)
                    .is_none()
                {
                    return Err(Failure::new(
                        RUNNING_TEXT,
                        Cause::Unclassified,
                        NextAction::None,
                    ));
                }
            }
            ExperimentIdentity::Terminal { .. } | ExperimentIdentity::Absent { .. } => {}
        }
    }
    // The original recording is kept now; the caller returns the original mode after it, once no experiment of the campaign is
    // left running (A6.2).
    finish()
}

/// Returns the loop to the mode an interrupted identification found it in, when `journal` holds its mode checkpoint (A6.2,
/// A8.6): the return of [`IdentificationMode::restore`], on the runtime's transfer connection. Without a checkpoint nothing is
/// read or written.
///
/// # Errors
/// The runtime takes no transfers, the loop's controller has no transfer, or the return fails (see
/// [`IdentificationMode::restore`]); the checkpoint then stays journaled.
fn return_found_mode<T: IdentifyRuntime>(
    runtime: &T,
    journal: &Mutex<Journal>,
    loop_definition: &LoopDefinition,
) -> Result<(), Failure> {
    if !mode_journaled(&journal.lock()) {
        return Ok(());
    }
    IdentificationMode::new(loop_definition, runtime.transfer_connection()?, journal)?.restore()
}

/// The identification campaign the journal holds, when it holds one.
fn journaled_campaign(journal: &Journal) -> Option<IdentificationCampaignJournal> {
    journal.entries().iter().find_map(|entry| {
        if let JournalEntry::IdentificationCampaign { campaign } = entry {
            Some(campaign.as_ref().clone())
        } else {
            None
        }
    })
}

/// Refuses a recovery on any runtime but a simulated one (`connect`): the journaled campaign ran on the original simulator.
fn require_simulator(connection: &RuntimeConnection) -> Result<(), Failure> {
    if RuntimeClient::connect(connection)?.status()?.source() == EvidenceSource::SimulatedPlant {
        return Ok(());
    }
    Err(Failure::new(
        SIMULATOR_TEXT,
        Cause::Runtime,
        NextAction::ConnectRuntime,
    ))
}

/// The refusal of a resume that finds no journaled campaign at all: no running experiment to resume, as the record of a
/// campaign that dispatched no step says it (`resumable_step`).
fn no_running_experiment() -> Failure {
    Failure::new(
        NO_RUNNING_EXPERIMENT_TEXT,
        Cause::Unclassified,
        NextAction::Identify,
    )
}

/// The failure of a resume whose experiment's status cannot be read (`reason`): the experiment is left to the runtime.
fn left_to_runtime(reason: &str) -> Failure {
    Failure::new(
        &format!("{reason} The experiment is left to the runtime; reopening reconciles it."),
        Cause::Unclassified,
        NextAction::None,
    )
}

/// The failure of a resume whose experiment the runtime does not hold (`reason`): it never started, so nothing was applied.
fn never_started(reason: &str) -> Failure {
    Failure::new(
        &format!(
            "The interrupted experiment never started, so nothing was applied and the original command is restored. {reason}"
        ),
        Cause::Unclassified,
        NextAction::None,
    )
}

/// The refusal of a resumed campaign whose plant inputs were not all stepped: it finishes only the input it was stepping, and
/// the others are identified again.
fn all_inputs_covered(channels_remaining: u64) -> Result<(), Failure> {
    if channels_remaining == 0 {
        return Ok(());
    }
    Err(Failure::new(
        &format!(
            "The interrupted identification covered one of {} plant inputs. Identify again to cover the others.",
            channels_remaining.saturating_add(1)
        ),
        Cause::Unclassified,
        NextAction::Identify,
    ))
}

/// The failure of a cancelled recovery whose experiment's status cannot be read (`reason`): nothing was restored.
fn nothing_restored(reason: &str) -> Failure {
    Failure::new(
        &format!("{reason} Nothing was restored: reconnect the original simulator and try again."),
        Cause::Runtime,
        NextAction::ConnectRuntime,
    )
}

#[cfg(test)]
#[path = "identify_recovery_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "identify_cancel_tests.rs"]
mod cancel_tests;
