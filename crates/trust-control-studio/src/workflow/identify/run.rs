//! The connected run (`ConnectedIdentification` of `connectedIdentification.ts`; specification 35, A6.2, A8.5, B3.4; runtime
//! protocol annex E11.2 to E11.6): the sample and the method, the recording and its journal, the steady baseline, the one step
//! with its journal written before the start is sent once, the hold, the restoration, the kept recording and its record, the
//! fit, and the settling that ends an owned experiment and clears the journal once nothing of the run is left running.
//!
//! The helpers marked `pub(crate)` are the steps a resumed campaign shares (`connectedIdentificationRecovery.ts`).

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use trust_control::experiment::{ControlExperimentPlan, ExperimentSample, RecordingRecoveryOwner};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::automatic::AutomaticIdentificationResult;
use trust_control::identification::campaign::baseline::BaselineSummary;
use trust_control::identification::campaign::journal::IdentificationCampaignJournal;
use trust_control::identification::campaign::{
    campaign_maximum_ms, campaign_recording_ms, require_campaign_limits, POLL_MILLISECONDS,
};
use trust_control::identification::connected_record::{
    connected_identification_record, RuntimeVerificationResult,
};
use trust_control::identification::connected_run::{
    campaign_refusal, ended_and_restored, evaluation_refusal, experiment_recovery, fit_failure,
    fit_window, hold_end, hold_refusal, identification_cancelled, is_provisional,
    offers_setpoint_step, recording_settling_ms, start_outcome, start_reconciliation, step_hold_ms,
    step_plan, stopped_hold_outcome, FitWindow, HoldEnd, StartOutcome, StartReconciliation,
    CHECKING_TEXT, FITTING_TEXT, IDENTIFYING_TEXT, RESTORATION_POLLS,
};
use trust_control::identification::independent_validation::RunEvaluation;
use trust_control::identification::mode::{
    mode_change_refusal, needs_mode_change, unreadable_mode_refusal,
};
use trust_control::identification::policy::{identification_method, IdentificationMethod};
use trust_control::identification::recorded_model::fit_recorded_model;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::RecordedRun;

use super::{
    IdentifyOutcome, IdentifyRecording, IdentifyRequest, IdentifyResult, IdentifyRuntime,
    LoopSampler, RecordingRetention, RunEvaluator, RunPurpose,
};
use crate::journal::{Journal, JournalEntry};
use crate::runtime_client::runtime_recording::{
    new_recording_id, RecordingStartError, RecordingStartPlan,
};
use crate::workflow::identify_campaign::{
    CampaignClock, CampaignReport, ExperimentClient, IdentifyCampaign,
};
use crate::workflow::reconcile::{read_experiment_identity, release_recording};

/// The ports of one run (`ConnectedIdentification`'s request, ports and options).
pub(crate) struct RunPorts<'a, T, S, K, C, P> {
    /// The runtime.
    pub(crate) runtime: &'a T,
    /// The loop's live sample.
    pub(crate) sampler: &'a mut S,
    /// Where the recording is kept.
    pub(crate) retention: &'a mut K,
    /// The clock that paces the polls.
    pub(crate) clock: &'a C,
    /// The operation's cancellation and progress.
    pub(crate) report: &'a P,
    /// The loop's journal.
    pub(crate) journal: &'a Mutex<Journal>,
}

/// The step a campaign held: its plan, and the window its recording is fitted from.
struct Step {
    plan: ControlExperimentPlan,
    window: FitWindow,
}

/// One run and what it owns while it runs (`ConnectedIdentification`'s fields).
struct Run<'a, T: IdentifyRuntime, S, K, C, P, E> {
    ports: RunPorts<'a, T, S, K, C, P>,
    request: &'a IdentifyRequest<'a, E>,
    /// The live experiment owned here (`active`).
    active: Option<ControlExperimentPlan>,
    /// False once the experiment must be left to the runtime: an unreconciled start is never aborted here (`mayAbort`).
    may_abort: bool,
    /// The recording, until it is kept and released (`recorder`).
    recording: Option<T::Recording>,
    /// The runtime identities the run journaled: its recording and its experiment.
    journaled: Vec<String>,
}

/// Runs one connected identification, or records the independent model check, and then settles (`run`, `recordValidation`).
///
/// # Errors
/// As [`run_identification`](super::run_identification), without the journal's opening.
pub(crate) fn run<T, S, K, C, P, E>(
    ports: RunPorts<'_, T, S, K, C, P>,
    request: &IdentifyRequest<'_, E>,
    purpose: RunPurpose,
) -> Result<IdentifyOutcome<E>, Failure>
where
    T: IdentifyRuntime,
    S: LoopSampler,
    K: RecordingRetention,
    C: CampaignClock,
    P: CampaignReport,
    E: RunEvaluation + Clone,
{
    Run {
        ports,
        request,
        active: None,
        may_abort: true,
        recording: None,
        journaled: Vec::new(),
    }
    .execute(purpose)
}

impl<T, S, K, C, P, E> Run<'_, T, S, K, C, P, E>
where
    T: IdentifyRuntime,
    S: LoopSampler,
    K: RecordingRetention,
    C: CampaignClock,
    P: CampaignReport,
    E: RunEvaluation + Clone,
{
    /// The sample, the method and the mode decision, then the work and the settling (`execute` and `conclude`).
    fn execute(mut self, purpose: RunPurpose) -> Result<IdentifyOutcome<E>, Failure> {
        let loop_definition = self.request.loop_definition;
        let sample = self.ports.sampler.sample()?;
        // An available sample whose mode cannot be read refuses the run before a method is chosen (B3.4 step 1, B3.9).
        let mode = sample.as_ref().map(|sample| sample.controller_mode);
        if let Some(refusal) = unreadable_mode_refusal(loop_definition, mode) {
            return Err(refusal);
        }
        let method = identification_method(loop_definition, sample.as_ref())
            .map_err(|error| error.failure())?;
        // Mode transfer: moves with V4 (runtimeTransfer.ts). Until then a method that needs the other mode is refused before
        // anything is recorded or written; an open-loop loop has no mode to change, and no sample has no mode to compare.
        if mode.is_some_and(|mode| needs_mode_change(loop_definition, method, mode)) {
            return Err(mode_change_refusal(method));
        }
        let worked = self.work(method, purpose, sample.as_ref());
        // The settling runs whatever the work gave; its own failure replaces the work's (`conclude`'s `finally`).
        self.settle()?;
        let (recording_id, result) = worked?;
        Ok(IdentifyOutcome {
            result,
            recording_id,
            method,
            plant_input: loop_definition.signals.manipulated_variable.symbol.clone(),
            measured_output: loop_definition.signals.process_variable.symbol.clone(),
        })
    }

    /// The acquisition, then the fit or the recorded model check.
    fn work(
        &mut self,
        method: IdentificationMethod,
        purpose: RunPurpose,
        sample: Option<&ExperimentSample>,
    ) -> Result<(String, IdentifyResult<E>), Failure> {
        // Mode transfer: moves with V4 (runtimeTransfer.ts). The method's mode would be entered here.
        self.check_cancelled()?;
        let validation = purpose == RunPurpose::RecordValidation;
        self.ports.report.progress(if validation {
            CHECKING_TEXT
        } else {
            IDENTIFYING_TEXT
        });
        let record = self.acquire(method, validation, sample)?;
        let recording_id = record.run.id.clone();
        if validation {
            return Ok((recording_id, IdentifyResult::Recorded(Box::new(record))));
        }
        let result = fit_records(
            self.request.loop_definition,
            self.ports.report,
            std::slice::from_ref(&record),
        )?;
        let provisional = is_provisional(&result);
        Ok((
            recording_id,
            IdentifyResult::Identified {
                result,
                provisional,
            },
        ))
    }

    /// The campaign (`acquire`): the checks before anything is recorded, the recording, the baseline and the held step, then the
    /// kept recording and its record.
    fn acquire(
        &mut self,
        method: IdentificationMethod,
        validation: bool,
        sample: Option<&ExperimentSample>,
    ) -> Result<RuntimeVerificationResult<E>, Failure> {
        let loop_definition = self.request.loop_definition;
        require_campaign_limits(loop_definition)?;
        let maximum_ms = campaign_maximum_ms(loop_definition)?;
        if let Some(refusal) = campaign_refusal(loop_definition, method, sample) {
            return Err(refusal);
        }
        // One plant input: the loop is the one channel (connectedIdentificationChannels.ts is parked with V7).
        let recording_ms = campaign_recording_ms(loop_definition)?;
        let mut recording = self.start_recording(recording_ms, maximum_ms)?;
        let held = self.campaign(
            &mut recording,
            method,
            validation,
            sample,
            maximum_ms,
            recording_ms,
        );
        let (step, mut recording) = self.held(recording, held)?;
        let kept = retain_recording(&mut recording, self.ports.retention);
        let (run, _released) = self.held(recording, kept)?;
        connected_record(
            loop_definition,
            &run,
            None,
            &step.plan,
            step.window,
            self.request.evaluate,
        )
    }

    /// Starts the recording the campaign follows, with its owner journaled before the start is sent (E11.2). A start whose
    /// acknowledgement was lost keeps its owner, and its recording stays here for the settling to keep (E11.4).
    fn start_recording(
        &mut self,
        recording_ms: u64,
        maximum_ms: u64,
    ) -> Result<T::Recording, Failure> {
        let plan = RecordingStartPlan {
            recording_id: new_recording_id(),
            planned_duration_ms: recording_ms,
            expected_settling_ms: recording_settling_ms(
                maximum_ms,
                self.request.expected_settling_seconds,
            ),
        };
        self.journaled.push(plan.recording_id.clone());
        let started = self.ports.runtime.start_recording(
            self.request.loop_definition,
            &plan,
            self.ports.journal,
        );
        match started {
            Ok(recording) => Ok(recording),
            Err(RecordingStartError::Failed(failure)) => Err(failure),
            Err(RecordingStartError::Unconfirmed(unconfirmed)) => {
                let failure = Failure::new(
                    &unconfirmed.message(),
                    Cause::Unclassified,
                    NextAction::None,
                );
                self.recording = Some(unconfirmed.session);
                Err(failure)
            }
        }
    }

    /// The baseline and the one step on the campaign's shared watch (A3.6): the journal before the baseline, the plan, the
    /// dispatch, the hold until it settles, and the restoration's confirmation.
    fn campaign(
        &mut self,
        recording: &mut T::Recording,
        method: IdentificationMethod,
        validation: bool,
        sample: Option<&ExperimentSample>,
        maximum_ms: u64,
        recording_ms: u64,
    ) -> Result<Step, Failure> {
        let loop_definition = self.request.loop_definition;
        let owner = recording.owner();
        self.persist(JournalEntry::IdentificationCampaign {
            campaign: Box::new(IdentificationCampaignJournal::baseline_phase(
                &loop_definition.revision,
                owner.clone(),
                1,
            )),
        })?;
        let mut experiment = self.ports.runtime.experiment();
        let mut campaign = IdentifyCampaign::new(
            loop_definition,
            self.ports.clock,
            recording,
            &mut experiment,
            self.ports.report,
            offers_setpoint_step(loop_definition, sample),
            self.request.expected_settling_seconds,
        );
        let baseline = campaign.baseline(maximum_ms)?;
        let baseline_end_seconds = campaign.seconds();
        // Mode transfer: moves with V4 (runtimeTransfer.ts). The transfer's process noise is the baseline's half range.
        self.check_cancelled()?;
        let step_seconds = campaign.seconds();
        let hold_ms = step_hold_ms(
            maximum_ms,
            recording_ms,
            &baseline,
            baseline_end_seconds,
            step_seconds,
        );
        let plan = self.prepare_plan(method, validation, &baseline, hold_ms)?;
        self.dispatch(&plan, &baseline, step_seconds, &owner)?;
        let state = campaign.hold(&plan, &baseline, step_seconds)?;
        match hold_end(&state) {
            HoldEnd::Unrestored(failure) => Err(failure),
            HoldEnd::Stopped(failure) => {
                self.active = None;
                Err(stopped_hold_outcome(
                    self.ports.report.is_cancelled(),
                    failure,
                ))
            }
            HoldEnd::Completed => {
                self.active = None;
                Ok(Step {
                    plan,
                    window: fit_window(baseline_end_seconds, baseline.window_seconds),
                })
            }
        }
    }

    /// The plan of the step (`preparePlan`), with a fresh identifier.
    fn prepare_plan(
        &mut self,
        method: IdentificationMethod,
        validation: bool,
        baseline: &BaselineSummary,
        hold_ms: f64,
    ) -> Result<ControlExperimentPlan, Failure> {
        if let Some(refusal) = hold_refusal(hold_ms) {
            return Err(refusal);
        }
        let sample = self.ports.sampler.sample()?;
        let mut plan = step_plan(
            self.request.loop_definition,
            sample.as_ref(),
            method,
            baseline,
            hold_ms,
            validation,
        )?;
        plan.id = new_experiment_id();
        Ok(plan)
    }

    /// Journals the step and its experiment, then sends the start once (`dispatch`). A refused start ends the run; a start
    /// whose outcome is unknown is reconciled by its identity and never sent again (E11.4).
    fn dispatch(
        &mut self,
        plan: &ControlExperimentPlan,
        baseline: &BaselineSummary,
        step_seconds: f64,
        owner: &RecordingRecoveryOwner,
    ) -> Result<(), Failure> {
        let loop_definition = self.request.loop_definition;
        self.persist(JournalEntry::IdentificationCampaign {
            campaign: Box::new(IdentificationCampaignJournal::step_phase(
                &loop_definition.revision,
                owner.clone(),
                plan.clone(),
                *baseline,
                step_seconds,
                0,
            )),
        })?;
        let recovery = experiment_recovery(loop_definition, plan, owner)?;
        self.journaled.push(plan.id.clone());
        self.persist(JournalEntry::Experiment {
            recovery: Box::new(recovery),
        })?;
        self.check_cancelled()?;
        self.active = Some(plan.clone());
        let read = self.ports.runtime.start_experiment(plan);
        match start_outcome(&plan.id, &read) {
            StartOutcome::Started => Ok(()),
            StartOutcome::Refused(refusal) => {
                self.active = None;
                Err(refusal)
            }
            StartOutcome::Unknown => self.reconcile_start(&plan.id),
        }
    }

    /// Reads the identity of a start whose outcome is unknown (`reconcileStart`). Nothing is sent but the status request.
    fn reconcile_start(&mut self, experiment_id: &str) -> Result<(), Failure> {
        let runtime = self.ports.runtime;
        let identity = read_experiment_identity(|id| runtime.experiment_status(id), experiment_id);
        match start_reconciliation(&identity) {
            StartReconciliation::Follow => Ok(()),
            StartReconciliation::Absent(failure) => {
                self.active = None;
                Err(failure)
            }
            StartReconciliation::Unresolved(failure) => {
                self.may_abort = false;
                Err(failure)
            }
        }
    }

    /// Ends the owned live experiment, then keeps the recording and clears the journal once no live experiment is left
    /// (`settle`, A6.2). A failed keep leaves the recording's buffer and the journal for a retry.
    fn settle(&mut self) -> Result<(), Failure> {
        self.release_active();
        if self.active.is_some() {
            return Ok(());
        }
        if let Some(recording) = self.recording.as_mut() {
            retain_recording(recording, self.ports.retention)?;
            self.recording = None;
        }
        // Mode transfer: moves with V4 (runtimeTransfer.ts). The loop's mode returns once no live experiment is left (A6.2).
        clear_entries(self.ports.journal, &self.journaled)
    }

    /// Ends a live experiment owned here through the runtime's restoration path (`releaseActive`). An abort that fails, or a
    /// restoration that is not confirmed, keeps the experiment owned; an unreconciled start is never aborted here.
    fn release_active(&mut self) {
        let Some(plan) = &self.active else {
            return;
        };
        if !self.may_abort {
            return;
        }
        let mut experiment = self.ports.runtime.experiment();
        let restored = experiment.abort(&plan.id).is_ok()
            && matches!(
                confirm_restored(&mut experiment, self.ports.clock, &plan.id),
                Ok(true)
            );
        if restored {
            self.active = None;
        }
    }

    /// Hands `recording` back with the value of `result`. On a failure the recording stays here, so the settling keeps it.
    fn held<V>(
        &mut self,
        recording: T::Recording,
        result: Result<V, Failure>,
    ) -> Result<(V, T::Recording), Failure> {
        match result {
            Ok(value) => Ok((value, recording)),
            Err(failure) => {
                self.recording = Some(recording);
                Err(failure)
            }
        }
    }

    fn persist(&self, entry: JournalEntry) -> Result<(), Failure> {
        self.ports.journal.lock().persist(entry)
    }

    fn check_cancelled(&self) -> Result<(), Failure> {
        check_cancelled(self.ports.report)
    }
}

/// Stops the recording, keeps its run with its evidence, then releases the runtime buffer (`retainRecording`; E4.7, E11.6).
/// A failed keep leaves the buffer for a retry.
///
/// # Errors
/// The recording cannot be stopped, kept or released.
pub(crate) fn retain_recording<R: IdentifyRecording, K: RecordingRetention>(
    recording: &mut R,
    retention: &mut K,
) -> Result<RecordedRun, Failure> {
    let run = recording.stop()?;
    retention.retain(&run, &recording.evidence(&run))?;
    release_recording(recording)?;
    Ok(run)
}

/// The connected record of a kept recording, from the start of its steady baseline window (`completeRecording`; B3.4, N1.3).
/// Only complete runtime evidence whose evaluation passes is a record. `start_failure` is why the recording's start was not
/// acknowledged, when it was not.
///
/// # Errors
/// The recording is not complete runtime evidence, the evaluation refuses the run, or the evaluation does not pass.
pub(crate) fn connected_record<E: RunEvaluation>(
    loop_definition: &LoopDefinition,
    run: &RecordedRun,
    start_failure: Option<&str>,
    plan: &ControlExperimentPlan,
    window: FitWindow,
    evaluate: RunEvaluator<'_, E>,
) -> Result<RuntimeVerificationResult<E>, Failure> {
    let record = connected_identification_record(
        loop_definition,
        run,
        start_failure,
        plan,
        window.fit_from_seconds,
        Some(window.baseline_end_seconds),
        evaluate,
    )?;
    match evaluation_refusal(&record) {
        Some(refusal) => Err(refusal),
        None => Ok(record),
    }
}

/// Fits the records of a campaign (`fit`): the fit runs inside the operation, and a fit that ends after a cancellation is not
/// the answer.
///
/// # Errors
/// The operation is cancelled, or the fit gives no model.
pub(crate) fn fit_records<E: Clone>(
    loop_definition: &LoopDefinition,
    report: &impl CampaignReport,
    runs: &[RuntimeVerificationResult<E>],
) -> Result<AutomaticIdentificationResult<E>, Failure> {
    check_cancelled(report)?;
    report.progress(FITTING_TEXT);
    let fitted = fit_recorded_model(loop_definition, runs)
        .map_err(|error| fit_failure(loop_definition, &error));
    check_cancelled(report)?;
    fitted
}

/// Polls until the experiment has ended and its original command is restored, within [`RESTORATION_POLLS`] reads one poll
/// apart (`confirmRestored`). Returns whether it was confirmed.
///
/// # Errors
/// A status read fails.
pub(crate) fn confirm_restored(
    experiment: &mut impl ExperimentClient,
    clock: &impl CampaignClock,
    experiment_id: &str,
) -> Result<bool, Failure> {
    for _ in 0..RESTORATION_POLLS {
        if ended_and_restored(&experiment.status(experiment_id)?) {
            return Ok(true);
        }
        clock.wait(POLL_MILLISECONDS);
    }
    Ok(false)
}

/// Clears the journal entries of the given runtime identities (`ports.clear()`).
///
/// # Errors
/// The journal cannot be written.
pub(crate) fn clear_entries(journal: &Mutex<Journal>, ids: &[String]) -> Result<(), Failure> {
    let mut entries = journal.lock();
    for id in ids {
        entries.clear(id)?;
    }
    Ok(())
}

/// The failure of a run cancelled between its steps (`checkCancelled`).
///
/// # Errors
/// The operation was cancelled.
pub(crate) fn check_cancelled(report: &impl CampaignReport) -> Result<(), Failure> {
    if report.is_cancelled() {
        return Err(identification_cancelled());
    }
    Ok(())
}

/// A new experiment identifier (`randomUUID` of `preparePlan`): `experiment-` and 32 hex digits drawn from the random seeds of
/// the standard hash map, as a recording identifier is drawn (E0.2).
fn new_experiment_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let state = RandomState::new();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "experiment-{:016x}{:016x}",
        state.hash_one(count),
        state.hash_one(!count)
    )
}
