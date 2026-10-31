//! The connected run (`ConnectedIdentification` of `connectedIdentification.ts`; specification 35, A6.2, A8.5, B3.4 and C7, C8; runtime
//! protocol annex E11.2 to E11.6 and E11.9): the sample and the method, the mode the method needs, the recording and its journal, the
//! steady baseline (a project test that declares a baseline waits for none: its recording starts with its plan, C12), the steps of the
//! loop's plant inputs in that one recording (each journaled before its start is sent once, held until it settles, and restored), the
//! kept recording and its record, the fit, the settling that ends an owned experiment and clears the journal once nothing of the run is
//! left running, and then the return of the loop to the mode it was found in.
//!
//! The helpers marked `pub(crate)` are the steps a resumed campaign shares (`connectedIdentificationRecovery.ts`).

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::Mutex;
use serde_json::Value;
use trust_control::experiment::{
    ControlExperimentPlan, ExperimentSample, RecordingRecoveryOwner, RestorationState,
};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::automatic::AutomaticIdentificationResult;
use trust_control::identification::campaign::baseline::BaselineSummary;
use trust_control::identification::campaign::journal::IdentificationCampaignJournal;
use trust_control::identification::campaign::{
    campaign_maximum_ms, campaign_recording_ms, campaign_settling_ms, require_campaign_limits,
    POLL_MILLISECONDS,
};
use trust_control::identification::connected_record::{
    connected_identification_record, RuntimeVerificationResult,
};
use trust_control::identification::connected_run::{
    campaign_hold_ms, campaign_method, campaign_refusal, ended_and_restored, evaluation_refusal,
    experiment_recovery, fit_failure, hold_end, hold_refusal, identification_cancelled,
    input_steps, is_provisional, offers_setpoint_step, own_steps_check, restoration_allowance_ms,
    several_channels, start_outcome, start_reconciliation, step_hold_ms, step_plan, step_sample,
    stopped_hold_outcome, FitWindow, HoldEnd, InputStep, StartOutcome, StartReconciliation,
    CHECKING_TEXT, FITTING_TEXT, IDENTIFYING_TEXT, RESTORATION_POLLS,
};
use trust_control::identification::identify_result::recorded_chart_samples;
use trust_control::identification::independent_validation::RunEvaluation;
use trust_control::identification::mode::{needs_mode_change, unreadable_mode_refusal};
use trust_control::identification::multichannel_fit::fit_multichannel_model;
use trust_control::identification::policy::{identification_method, IdentificationMethod};
use trust_control::identification::project_baseline::{
    campaign_fit_window, declared_baseline, declared_baseline_noise,
};
use trust_control::identification::recorded_model::{fit_recorded_model, RecordedFitError};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{CampaignRestoration, RecordedChartSample};
use trust_control::recording::context::ExperimentContext;
use trust_control::recording::{EvidenceSource, RecordedRun};

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
use crate::workflow::identify_mode::{mode_journaled, IdentificationMode};
use crate::workflow::identify_rest::RestWait;
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

/// The steps a campaign held: the plans of the plant inputs it stepped, in input order, and the window its recording is fitted from.
struct Step {
    plans: Vec<ControlExperimentPlan>,
    window: FitWindow,
}

/// What a campaign steps and how: its inputs in input order, the method and the validation of its steps, the baseline they are judged
/// against, and the durations that bound every hold (B3.4 steps 3 and 4). A campaign and a resumed campaign share it.
pub(crate) struct Schedule<'b> {
    /// The steps of the plant inputs, in input order.
    pub(crate) steps: &'b [InputStep],
    /// The method of the steps without an own step, which the campaign runs under (`campaign_method`).
    pub(crate) method: IdentificationMethod,
    /// Whether the steps are the independent model check's.
    pub(crate) validation: bool,
    /// The steady baseline the steps are judged against; a project test that declares a baseline carries that one instead (C12).
    pub(crate) baseline: &'b BaselineSummary,
    /// The largest duration of the campaign, in ms.
    pub(crate) maximum_ms: u64,
    /// The duration of the recording, in ms.
    pub(crate) recording_ms: u64,
    /// Where the steady baseline ends, in process time of the recording: the maximum duration counts from here. A project test that
    /// declares a baseline waits for no steady baseline, so it is the recording's start (C12).
    pub(crate) baseline_end_seconds: f64,
}

/// Where a campaign stands when a step is dispatched, which the journal keeps: where the step starts, how many inputs follow it, and the
/// plans of the inputs stepped before it.
struct Progress<'p> {
    step_seconds: f64,
    remaining_after: u64,
    stepped: &'p [ControlExperimentPlan],
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
    /// The runtime identities the run journaled: its recording and its experiments.
    journaled: Vec<String>,
    /// The mode change of a method that needs the other mode, which returns the loop once nothing of the run is left running (`mode`).
    mode: Option<IdentificationMode<'a>>,
    /// The restoration the last completed step's hold confirmed (C12); none for a campaign that completed no step.
    restoration: Option<CampaignRestoration>,
    /// The samples of the kept recording, which the answer hands the page (`recorded_chart_samples`).
    recorded_samples: Vec<RecordedChartSample>,
    /// The fit's refusal of a project test (C12), in full: the note the test keeps. None when the fit gave a model.
    fit_refusal: Option<String>,
    /// The value the runtime applied to the loop's hold parameter when the campaign began, which a project step test's plan records (C12,
    /// Hold parameter). None for a loop that declares none. A continuation of a journaled campaign plans no hold of its own: a project
    /// step test is one observation (C12), so no input follows its journaled step.
    hold_seconds: Option<f64>,
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
        mode: None,
        restoration: None,
        recorded_samples: Vec::new(),
        fit_refusal: None,
        hold_seconds: None,
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
    /// The sample, the method and the mode decision, then the work, the settling and the return of the mode (`execute` and
    /// `conclude`).
    fn execute(mut self, purpose: RunPurpose) -> Result<IdentifyOutcome<E>, Failure> {
        let loop_definition = self.request.loop_definition;
        let sample = self.ports.sampler.sample()?;
        // An available sample whose mode cannot be read refuses the run before a method is chosen (B3.4 step 1, B3.9).
        let mode = sample.as_ref().map(|sample| sample.controller_mode);
        if let Some(refusal) = unreadable_mode_refusal(loop_definition, mode) {
            return Err(refusal);
        }
        let loop_method = identification_method(loop_definition, sample.as_ref())
            .map_err(|error| error.failure())?;
        // The inputs' own steps are checked before any mode changes or anything is recorded (C7, C8); the campaign then runs its method.
        own_steps_check(loop_definition, loop_method)?;
        let method = campaign_method(loop_definition, loop_method);
        // A method that needs the other mode enters it through a transfer in the work, and a mode change that an earlier run left
        // journaled is returned first (`needsMode`, B3.4 step 1, A8.6). An open-loop loop has no mode to change, and no sample has
        // no mode to compare.
        let journaled = mode_journaled(&self.ports.journal.lock());
        if journaled || mode.is_some_and(|mode| needs_mode_change(loop_definition, method, mode)) {
            let connection = self.ports.runtime.transfer_connection()?;
            self.mode = Some(IdentificationMode::new(
                loop_definition,
                connection,
                self.ports.journal,
                self.ports.clock,
            )?);
        }
        self.note_run_plant(sample.as_ref());
        let worked = self.work(method, purpose, sample.as_ref());
        // The settling runs whatever the work gave; its own failure replaces the work's (`conclude`'s `finally`).
        self.settle()?;
        // Then the loop returns to the mode it was found in (A6.2). A failed return replaces only a work that succeeded: its
        // checkpoint stays journaled, so a Cancel or a reopened panel returns the mode (A8.6).
        let returned = self.return_mode();
        let (recording_id, result) = worked?;
        returned?;
        Ok(IdentifyOutcome {
            result,
            recording_id,
            method,
            plant_input: loop_definition.signals.manipulated_variable.symbol.clone(),
            measured_output: loop_definition.signals.process_variable.symbol.clone(),
            restoration: self.restoration,
            recorded_samples: std::mem::take(&mut self.recorded_samples),
            fit_refusal: self.fit_refusal.take(),
        })
    }

    /// Notes the simulated plant the run ran against with the retention, when the live sample comes from a simulated target and the
    /// runtime proves which configuration the loop's plant runs under (B10.2). A plant the runtime cannot prove is left out, and
    /// never refuses the run.
    fn note_run_plant(&mut self, sample: Option<&ExperimentSample>) {
        let loop_definition = self.request.loop_definition;
        if sample.is_some_and(|sample| sample.source == EvidenceSource::SimulatedPlant)
            && let Some(plant) = self.ports.runtime.plant_context(loop_definition)
        {
            self.ports.retention.note_plant(plant);
        }
    }

    /// The acquisition, then the fit or the recorded model check.
    fn work(
        &mut self,
        method: IdentificationMethod,
        purpose: RunPurpose,
        sample: Option<&ExperimentSample>,
    ) -> Result<(String, IdentifyResult<E>), Failure> {
        // The loop enters the mode the method needs before anything is recorded (B3.4 step 1). A loop found in Auto that a Manual
        // method takes to Manual first rests there, so the transfer holds the output at which the controller keeps it.
        if let Some(mode) = self.mode.as_mut() {
            if method.manual {
                let loop_definition = self.request.loop_definition;
                let mut rest = RestWait::new(
                    loop_definition,
                    campaign_maximum_ms(loop_definition)?,
                    self.request.expected_settling_seconds,
                    offers_setpoint_step(loop_definition, sample),
                    self.ports.report,
                    self.ports.clock,
                );
                mode.enter_at_rest(true, &mut rest)?;
            } else {
                mode.enter(false)?;
            }
        }
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
        let result = match fit_outcome(
            self.request.loop_definition,
            self.ports.report,
            std::slice::from_ref(&record),
        )? {
            FitOutcome::Model(result) => *result,
            FitOutcome::Refused(refusal) if purpose != RunPurpose::ProjectTest => {
                return Err(refusal);
            }
            FitOutcome::Refused(refusal) => {
                // The test completes with its recording and the refusal as its note: the record is not fitted, so no model is kept
                // (C12, B3.5).
                self.fit_refusal = Some(refusal.detail.unwrap_or(refusal.message));
                return Ok((recording_id, IdentifyResult::Recorded(Box::new(record))));
            }
        };
        let provisional = is_provisional(&result);
        Ok((
            recording_id,
            IdentifyResult::Identified {
                result,
                provisional,
            },
        ))
    }

    /// The campaign (`acquire`): the checks before anything is recorded, the steps of the plant inputs in one recording, the kept
    /// recording and its record.
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
        // The hold of a project step test is read, and refused as its review refuses it, before anything is recorded (C12).
        self.hold_seconds = self.applied_hold()?;
        // The plant inputs are read before anything is recorded, as `connectedIdentificationChannels` reads them (C7, C8).
        let steps = input_steps(loop_definition, &self.io_list()?)?;
        let recording_ms =
            campaign_recording_ms(loop_definition, self.request.expected_settling_seconds)?;
        let mut recording = self.start_recording(recording_ms, maximum_ms)?;
        let held = self.campaign(&mut recording, method, validation, sample, &steps);
        let (step, mut recording) = self.held(recording, held)?;
        let kept = retain_recording(&mut recording, self.ports.retention);
        let (run, _released) = self.held(recording, kept)?;
        self.recorded_samples = recorded_chart_samples(loop_definition, &run);
        self.note_declared_noise();
        campaign_record(
            loop_definition,
            &run,
            step.plans,
            step.window,
            self.request.evaluate,
        )
    }

    /// The runtime's I/O list, read when the loop has several plant inputs, which gives the addresses of their Manual commands
    /// (`connectedIdentificationChannels`). A loop with one plant input reads none.
    fn io_list(&self) -> Result<Value, Failure> {
        let several_inputs = self
            .request
            .loop_definition
            .channels
            .as_ref()
            .is_some_and(|channels| channels.inputs.len() > 1);
        if several_inputs {
            self.ports.runtime.io_list()
        } else {
            Ok(Value::Null)
        }
    }

    /// The noise that the first output of a return to Auto is allowed to move by, from the samples of a project test's declared
    /// baseline (A6.4, N1.3, C12): the test waited for no steady baseline, so its kept recording gives the noise. A test that declares
    /// no baseline gives none, and a campaign took its noise from the steady baseline it waited for.
    fn note_declared_noise(&mut self) {
        let noise = declared_baseline_noise(self.request.loop_definition, &self.recorded_samples);
        if let (Some(mode), Some(noise)) = (self.mode.as_mut(), noise) {
            mode.set_process_noise(noise);
        }
    }

    /// The value the runtime applies to the loop's hold parameter (C12, Hold parameter): a project step test records its baseline, that
    /// hold and one restoration sample. It is refused as the test's review refuses it: unreadable, outside the parameter's bounds, or
    /// recorded longer than the declared maximum duration. `None` for a loop that declares no hold parameter, which reads nothing.
    fn applied_hold(&self) -> Result<Option<f64>, Failure> {
        let loop_definition = self.request.loop_definition;
        let hold = self.ports.runtime.applied_hold_seconds(loop_definition)?;
        campaign_hold_ms(loop_definition, hold)?;
        Ok(hold)
    }

    /// Starts the recording the campaign follows, with its owner journaled before the start is sent (E11.2). A start whose
    /// acknowledgement was lost keeps its owner, and its recording stays here for the settling to keep (E11.4).
    fn start_recording(
        &mut self,
        recording_ms: u64,
        maximum_ms: u64,
    ) -> Result<T::Recording, Failure> {
        // A project test records every execution of its hold (C12): the record period follows the process only for a campaign that steps
        // the plant, whose settling time is not known (A4.2). The project's own scenario is not stepped by the campaign, and its
        // recording is read at every scan.
        let expected_settling_ms = campaign_settling_ms(
            self.request.loop_definition,
            maximum_ms,
            self.request.expected_settling_seconds,
        );
        let plan = RecordingStartPlan {
            recording_id: new_recording_id(),
            planned_duration_ms: recording_ms,
            expected_settling_ms,
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

    /// The baseline, then the steps of the plant inputs on the campaign's shared watch (A3.6): the journal before the baseline, the
    /// baseline, and each input's plan, dispatch, hold and restoration in input order.
    fn campaign(
        &mut self,
        recording: &mut T::Recording,
        method: IdentificationMethod,
        validation: bool,
        sample: Option<&ExperimentSample>,
        steps: &[InputStep],
    ) -> Result<Step, Failure> {
        let loop_definition = self.request.loop_definition;
        let maximum_ms = campaign_maximum_ms(loop_definition)?;
        let recording_ms =
            campaign_recording_ms(loop_definition, self.request.expected_settling_seconds)?;
        let owner = recording.owner();
        self.persist(JournalEntry::IdentificationCampaign {
            campaign: Box::new(IdentificationCampaignJournal::baseline_phase(
                &loop_definition.revision,
                owner.clone(),
                count_u64(steps.len()),
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
        // A project test that declares a baseline waits for no steady baseline (C12): its recording starts with its plan, which holds the
        // activation off for that baseline, and its maximum duration, which includes it, counts from the recording's start. A project
        // test that declares none waits for the steady baseline as a campaign does, so that its fit has an operating point.
        let (baseline, baseline_end_seconds) =
            if let Some(declared) = declared_baseline(loop_definition, sample) {
                (declared, 0.0)
            } else {
                let baseline = campaign.baseline(maximum_ms)?;
                // The baseline's half range is the measured noise that a return to Auto allows its first output to move by (A6.4).
                if let Some(mode) = self.mode.as_mut() {
                    mode.set_process_noise(baseline.half_range);
                }
                (baseline, campaign.seconds())
            };
        self.check_cancelled()?;
        let schedule = Schedule {
            steps,
            method,
            validation,
            baseline: &baseline,
            maximum_ms,
            recording_ms,
            baseline_end_seconds,
        };
        let plans = self.step_inputs(&mut campaign, &owner, &schedule, 0, Vec::new())?;
        Ok(Step {
            plans,
            window: campaign_fit_window(loop_definition, baseline_end_seconds, &baseline),
        })
    }

    /// Steps the plant inputs from input `first` on, in input order, each in its own interval of the recording: planned, journaled and
    /// started once, and held until it settles and is restored (B3.4 steps 3 to 6). `stepped` holds the plans of the inputs before
    /// `first`; the result holds them with the plans of these steps.
    fn step_inputs(
        &mut self,
        campaign: &mut IdentifyCampaign<'_, C, T::Recording, T::Experiment, P>,
        owner: &RecordingRecoveryOwner,
        schedule: &Schedule<'_>,
        first: usize,
        stepped: Vec<ControlExperimentPlan>,
    ) -> Result<Vec<ControlExperimentPlan>, Failure> {
        let loop_definition = self.request.loop_definition;
        let count = schedule.steps.len();
        let mut plans = stepped;
        for (index, step) in schedule.steps.iter().enumerate().skip(first) {
            self.check_cancelled()?;
            let remaining = count - index;
            let step_seconds = campaign.seconds();
            let hold_ms = step_hold_ms(
                schedule.maximum_ms,
                schedule.recording_ms,
                restoration_allowance_ms(loop_definition, schedule.baseline),
                schedule.baseline_end_seconds,
                step_seconds,
                count_u32(remaining),
            );
            let plan = self.prepare_plan(step, schedule, hold_ms)?;
            let progress = Progress {
                step_seconds,
                remaining_after: count_u64(remaining - 1),
                stepped: &plans,
            };
            self.dispatch(&plan, owner, schedule, &progress)?;
            let state = campaign.hold(&plan, schedule.baseline, step_seconds)?;
            // Whatever the hold ended with, the experiment's context is noted for the recording that is kept next: a recording from an
            // aborted test keeps its stop reason and its restoration (B10.2, B10.3).
            if let Some(context) = ExperimentContext::of(&plan, &state) {
                self.ports.retention.note_experiment(context);
            }
            match hold_end(&state) {
                HoldEnd::Unrestored(failure) => return Err(failure),
                HoldEnd::Stopped(failure) => {
                    self.active = None;
                    return Err(stopped_hold_outcome(
                        self.ports.report.is_cancelled(),
                        failure,
                    ));
                }
                HoldEnd::Completed => {
                    self.active = None;
                    if let Some(restoration) = hold_restoration(&state) {
                        self.restoration = Some(restoration);
                    }
                    plans.push(plan);
                }
            }
        }
        Ok(plans)
    }

    /// The plan of one input's step (`preparePlan`), with a fresh identifier: planned on the input's loop, under the input's own method
    /// or else the campaign's, and on the sample the input reads.
    fn prepare_plan(
        &mut self,
        step: &InputStep,
        schedule: &Schedule<'_>,
        hold_ms: f64,
    ) -> Result<ControlExperimentPlan, Failure> {
        if let Some(refusal) = hold_refusal(hold_ms) {
            return Err(refusal);
        }
        let sample = step_sample(step, self.ports.sampler.sample()?)?;
        let method = step.own_method.unwrap_or(schedule.method);
        let mut plan = step_plan(
            &step.loop_definition,
            sample.as_ref(),
            method,
            schedule.baseline,
            hold_ms,
            schedule.validation,
            self.hold_seconds,
        )?;
        plan.id = new_experiment_id();
        Ok(plan)
    }

    /// Journals the step and its experiment, then sends the start once (`dispatch`). A refused start ends the run; a start whose
    /// outcome is unknown is reconciled by its identity and never sent again (E11.4).
    fn dispatch(
        &mut self,
        plan: &ControlExperimentPlan,
        owner: &RecordingRecoveryOwner,
        schedule: &Schedule<'_>,
        progress: &Progress<'_>,
    ) -> Result<(), Failure> {
        let loop_definition = self.request.loop_definition;
        let campaign = IdentificationCampaignJournal::step_phase(
            &loop_definition.revision,
            owner.clone(),
            plan.clone(),
            *schedule.baseline,
            progress.step_seconds,
            progress.remaining_after,
        )
        .with_progress(schedule.baseline_end_seconds, progress.stepped.to_vec());
        self.persist(JournalEntry::IdentificationCampaign {
            campaign: Box::new(campaign),
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
        // The loop's mode returns after the settling, once no live experiment is left (`return_mode`, A6.2).
        clear_entries(self.ports.journal, &self.journaled)
    }

    /// Returns the loop to the mode it was found in once no experiment of the run is left running (`settle`'s
    /// `mode.restore()`, A6.2, E11.9). An experiment still owned here keeps the mode it needs, and the mode checkpoint stays
    /// journaled with it for the reconciliation on reopening (A8.6).
    ///
    /// # Errors
    /// The return fails (see [`IdentificationMode::restore`]).
    fn return_mode(&mut self) -> Result<(), Failure> {
        if self.active.is_some() {
            return Ok(());
        }
        self.mode
            .as_mut()
            .map_or(Ok(()), IdentificationMode::restore)
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
        let ended = if experiment.abort(&plan.id).is_ok() {
            confirmed_status(&mut experiment, self.ports.clock, &plan.id)
                .ok()
                .flatten()
        } else {
            None
        };
        if let Some(status) = ended {
            // The experiment this run ended itself is noted as it ended, for the recording that is kept next: a test that a failure
            // aborted keeps its stop reason and its restoration (B10.2, B10.3).
            if let Some(context) = ExperimentContext::of(plan, &status) {
                self.ports.retention.note_experiment(context);
            }
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

/// Steps the plant inputs a resumed campaign still has to step, from input `first` on, in the recording it holds (`resumeCampaign`,
/// C7, C8): each input is planned, journaled, started and held as a campaign's own step is. `stepped` holds the plans already stepped,
/// and the result holds them with the plans of these steps. The flag says whether an experiment of the run is still owned here after
/// a failure: an owned experiment is left to the runtime with its journal (A8.6), and nothing is aborted, kept or cleared.
pub(crate) fn step_remaining<T, S, K, C, P, E>(
    ports: RunPorts<'_, T, S, K, C, P>,
    request: &IdentifyRequest<'_, E>,
    campaign: &mut IdentifyCampaign<'_, C, T::Recording, T::Experiment, P>,
    owner: &RecordingRecoveryOwner,
    schedule: &Schedule<'_>,
    first: usize,
    stepped: Vec<ControlExperimentPlan>,
) -> (Result<Vec<ControlExperimentPlan>, Failure>, bool)
where
    T: IdentifyRuntime,
    S: LoopSampler,
    K: RecordingRetention,
    C: CampaignClock,
    P: CampaignReport,
    E: RunEvaluation + Clone,
{
    let mut run = Run {
        ports,
        request,
        active: None,
        may_abort: true,
        recording: None,
        journaled: Vec::new(),
        mode: None,
        restoration: None,
        recorded_samples: Vec::new(),
        fit_refusal: None,
        hold_seconds: None,
    };
    let plans = run.step_inputs(campaign, owner, schedule, first, stepped);
    let still_active = run.active.is_some();
    (plans, still_active)
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

/// The connected record of a campaign's kept recording, with the plan of every plant input it stepped (`completeRecording`): the
/// record's experiment is the first of them, and its request keeps all of them in input order (C7, C8).
///
/// # Errors
/// As [`connected_record`]; and a campaign that stepped no plant input, which is the engine's own wiring failure.
pub(crate) fn campaign_record<E: RunEvaluation>(
    loop_definition: &LoopDefinition,
    run: &RecordedRun,
    plans: Vec<ControlExperimentPlan>,
    window: FitWindow,
    evaluate: RunEvaluator<'_, E>,
) -> Result<RuntimeVerificationResult<E>, Failure> {
    let Some(first) = plans.first() else {
        return Err(Failure::new(
            "The campaign stepped no plant input, so it has no experiment to record.",
            Cause::Implementation,
            NextAction::None,
        ));
    };
    let mut record = connected_record(loop_definition, run, None, first, window, evaluate)?;
    record.request.experiments = Some(plans);
    Ok(record)
}

/// Fits the records of a campaign (`fit`): the fit runs inside the operation, and a fit that ends after a cancellation is not
/// the answer. A loop with several plant inputs or measured outputs is fitted as one multichannel state model (C7, C8).
///
/// # Errors
/// The operation is cancelled, or the fit gives no model.
pub(crate) fn fit_records<E: Clone>(
    loop_definition: &LoopDefinition,
    report: &impl CampaignReport,
    runs: &[RuntimeVerificationResult<E>],
) -> Result<AutomaticIdentificationResult<E>, Failure> {
    match fit_outcome(loop_definition, report, runs)? {
        FitOutcome::Model(result) => Ok(*result),
        FitOutcome::Refused(refusal) => Err(refusal),
    }
}

/// What the fit of a campaign's records gave: the model, or the refusal that says the records cannot support one (C12).
enum FitOutcome<E> {
    /// The fit gave a model from the records.
    Model(Box<AutomaticIdentificationResult<E>>),
    /// The fit refused the records; the failure says why, in full.
    Refused(Failure),
}

/// The fit of a campaign's records, as [`fit_records`] runs it, with a refusal of the fit kept apart from the operation's cancellation:
/// a cancelled operation fails with `Err`, and a fit that refuses answers [`FitOutcome::Refused`].
///
/// # Errors
/// The operation is cancelled before or after the fit.
fn fit_outcome<E: Clone>(
    loop_definition: &LoopDefinition,
    report: &impl CampaignReport,
    runs: &[RuntimeVerificationResult<E>],
) -> Result<FitOutcome<E>, Failure> {
    check_cancelled(report)?;
    report.progress(FITTING_TEXT);
    let fitted = if several_channels(loop_definition) {
        fit_multichannel_model(loop_definition, runs)
            .map(|identification| AutomaticIdentificationResult::StateModel {
                model: Box::new(identification.model),
                runs: identification.runs,
                fit: identification.fit,
            })
            .map_err(|refusal| fit_failure(loop_definition, &RecordedFitError::Refused(refusal)))
    } else {
        fit_recorded_model(loop_definition, runs)
            .map_err(|error| fit_failure(loop_definition, &error))
    };
    check_cancelled(report)?;
    Ok(match fitted {
        Ok(result) => FitOutcome::Model(Box::new(result)),
        Err(refusal) => FitOutcome::Refused(refusal),
    })
}

/// The restoration a held step's status confirmed (C12): restored once the runtime confirmed the restoring write, or not
/// required when the experiment wrote none. A pending or failed restoration is no outcome to report.
pub(crate) fn hold_restoration(state: &Value) -> Option<CampaignRestoration> {
    let restoration: RestorationState =
        serde_json::from_value(state.get("restoration")?.clone()).ok()?;
    match restoration {
        RestorationState::Restored => Some(CampaignRestoration::Restored),
        RestorationState::NotRequired => Some(CampaignRestoration::NotRequired),
        RestorationState::Pending | RestorationState::Failed => None,
    }
}

/// Polls until the experiment has ended and its original command is restored, within [`RESTORATION_POLLS`] reads one poll
/// apart (`confirmRestored`). Gives the status that confirmed it: the runtime's last status of the experiment, which says how it
/// ended (B10.2). `None` when the restoration was not confirmed within the polls.
///
/// # Errors
/// A status read fails.
pub(crate) fn confirmed_status(
    experiment: &mut impl ExperimentClient,
    clock: &impl CampaignClock,
    experiment_id: &str,
) -> Result<Option<Value>, Failure> {
    for _ in 0..RESTORATION_POLLS {
        let status = experiment.status(experiment_id)?;
        if ended_and_restored(&status) {
            return Ok(Some(status));
        }
        clock.wait(POLL_MILLISECONDS);
    }
    Ok(None)
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

/// A count of steps as the journal's `u32` (saturating, though a loop has at most sixteen inputs).
fn count_u32(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// A count of steps as the journal's `u64` (saturating).
fn count_u64(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
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
