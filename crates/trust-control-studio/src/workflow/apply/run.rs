//! The run of a confirmed Apply (specification 35, B6.1 to B6.3, B6.7, A6.2 to A6.4 and A8.5; annex E6.5, E7.3, E7.4, E11.2 and E11.10):
//! `execute` of `pidCommissioning.ts` and `proportionalCommissioning.ts`, with the decisions of trust-control.
//!
//! After the review is recomputed and its identity matches, the run waits for the starting point without writing, journals its
//! checkpoint (stage `prepared`), checks the reviewed record, mode and reference again, stages Manual when the loop is out of it,
//! applies the candidate's record and confirms it, enters Auto with the aligning initialization, and observes the validated target
//! change. A passed response or a failed requirement keeps the settings applied: the checkpoint completes, and Restore and Keep are
//! offered. A departure from the permitted range, a cancel or any other failure after the first write runs the one Restore at once.

use std::sync::Arc;

use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::application_records::ApplyOutcome;
use trust_control::commissioning::application_texts::{physical_approval_needed, review_changed};
use trust_control::commissioning::checkpoint::{CheckpointStage, CommissioningCheckpoint};
use trust_control::commissioning::commissioning_result::TransferStatus;
use trust_control::commissioning::parameter_application::ParameterOwner;
use trust_control::commissioning::parameter_status::ParameterStatus;
use trust_control::commissioning::pid_ending::RunOutcome;
use trust_control::commissioning::pid_recovery::recovery_needed;
use trust_control::commissioning::proportional_commissioning::{
    Recovery, RunAttempt, TransferStep,
};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::{Completeness, RecordedRun};
use trust_control::runtime::RuntimeConnection;

use super::ending::{
    keep_recording, outcome_of, release_kept, restore_at_once, stop_unfinished, whole_text,
};
use super::journaling::{checkpoint_stage, new_checkpoint_id, CheckpointWriter};
use super::law::{Law, LawSetup, LawStart};
use super::review::{prepare, ApplyInputs, Clients, Prepared};
use super::wait::await_starting_point;
use super::{ApplyContext, ApplyTiming};
use crate::runtime_client::parameters::ParameterTransactions;
use crate::runtime_client::runtime_recording::{PendingReleases, RuntimeRecordingSession};
use crate::runtime_client::transfers::{TransferSession, TransferWrite};
use crate::workflow::apply_observation::{
    observe_target_change, observed_recording, ObservationEnd, ObservationPlan,
};
use crate::workflow::apply_texts as engine_texts;
use crate::workflow::commissioning_journal::SharedJournal;
use crate::workflow::identify_campaign::CampaignReport;

/// Runs the reviewed Apply whose review identity is `identity` (B6.1 to B6.3, §0.1b Confirmation): the review is recomputed from the
/// live runtime and refused when it changed, the starting point is waited for, and the run writes from its checkpoint on. The checkpoint
/// an earlier Apply left is removed before this one is journaled, so the journal holds one checkpoint per loop (A8.5; V4 decision 5): an
/// unresolved one of a replaced runtime is retired (B6.4), a completed, recovered or retired one is replaced, and an unresolved one of the
/// running program has refused the review (`review::checkpoint_to_retire`, E11.7).
///
/// # Errors
/// The review changed since `identity` was given; the target is physical and the confirmation carries no approval (A2.5); or a refusal
/// before the first write: a reading fails, the wait is cancelled or refused, the starting point does not hold, or the journal cannot be
/// written. A failure after the checkpoint is journaled is reported in the outcome's result instead, with the Restore it ran.
pub(crate) fn run_confirmed(
    context: &mut ApplyContext<'_>,
    inputs: &ApplyInputs,
    identity: &str,
    report: &dyn CampaignReport,
    timing: &ApplyTiming<'_>,
) -> Result<ApplyOutcome, Failure> {
    let loop_definition = context.loop_definition;
    let connection = context.connection;
    let mut clients = Clients::connect(connection)?;
    let prepared = prepare(context, inputs, &mut clients)?;
    if prepared.identity != identity {
        return Err(review_changed());
    }
    if prepared.source == ApplySource::Physical && !context.physical_approved {
        return Err(physical_approval_needed());
    }
    let Prepared {
        runtime,
        source,
        kind,
        binding,
        runtime_binding,
        review,
        first,
        retired,
        ..
    } = prepared;
    let program_revision = review.observed.program_revision.clone();
    let setup = LawSetup {
        loop_definition,
        model: &inputs.model,
        task: &inputs.task,
        process_noise: inputs.process_noise,
    };
    let start = LawStart::begin(kind, setup, binding, review, &first)?;
    let Clients {
        transfer: mut transfer_client,
        parameters: mut parameter_client,
    } = clients;
    let mut transfers = TransferSession::new(
        &mut transfer_client,
        start.binding().clone(),
        program_revision,
    );
    let settled = await_starting_point(
        &mut || transfers.status(),
        &|status: &TransferStatus, process_ms: f64, inside_since_ms: Option<f64>| {
            start.wait_step(status, process_ms, inside_since_ms)
        },
        report,
        timing,
    )?;
    let law = start.accept(&settled)?;
    report.progress(engine_texts::COMMISSIONING);
    let journal = SharedJournal::new(&mut *context.journal);
    replace_earlier(&journal, retired.as_ref())?;
    let writer = CheckpointWriter::new(journal, checkpoint_of(&law, inputs, loop_definition)?);
    writer.save(CheckpointStage::Prepared)?;
    let mut run = Run {
        loop_definition,
        connection,
        target: WriteTarget {
            source,
            physical_approved: context.physical_approved,
        },
        cycle_interval_ms: runtime.cycle_interval_ms,
        pending: Arc::new(PendingReleases::new()),
        law,
        transfers,
        parameters: ParameterTransactions::new(&mut parameter_client, runtime_binding, source),
        writer,
        parameters_applied: false,
    };
    run.execute(report, timing)
}

/// Removes the checkpoint an earlier Apply left, before this Apply journals its own, so that the journal holds one checkpoint per loop
/// (A8.5; V4 decision 5): an unresolved checkpoint of a replaced runtime, which the review let pass, is retired (B6.4: it is no longer
/// journaled); any other is completed, recovered or retired, and this Apply replaces it by clearing it by its identifier.
///
/// # Errors
/// The journal cannot be written; nothing is dispatched then.
fn replace_earlier(
    journal: &SharedJournal<'_>,
    retired: Option<&CommissioningCheckpoint>,
) -> Result<(), Failure> {
    if let Some(earlier) = retired {
        // The history record it returns is for the study's Apply section (A8.1), which this workflow does not write.
        journal.retire_checkpoint(earlier)?;
    }
    if let Some(earlier) = journal.checkpoint() {
        journal.clear_checkpoint(&earlier.id)?;
    }
    Ok(())
}

/// The checkpoint an Apply journals before its first write (A8.3, A8.5; `CommissioningJournal`): the model the run is judged on, the
/// requirements, the record before Apply and the candidate, the parameter transaction's identity, the references and the mode found, the
/// binding with the tracking and the allowance of a recovery, the measured noise, and the validation the review relied on.
///
/// # Errors
/// The recovery's tracking or allowance cannot be built.
fn checkpoint_of(
    law: &Law<'_>,
    inputs: &ApplyInputs,
    loop_definition: &LoopDefinition,
) -> Result<CommissioningCheckpoint, Failure> {
    let review = law.review();
    Ok(CommissioningCheckpoint {
        id: new_checkpoint_id(),
        loop_revision: loop_definition.revision.clone(),
        stage: CheckpointStage::Prepared,
        model: law.model().clone(),
        task: inputs.task.clone(),
        origin: review.observed.clone(),
        candidate: review.values.clone(),
        parameter_operation: ParameterOwner {
            id: review.transaction_id.clone(),
            program: review.observed.program_revision.clone(),
            binding: review.observed.binding_revision.clone(),
        },
        references: law.references(),
        binding: law.binding().clone(),
        recovery_tracking: law.recovery_tracking()?,
        tolerance: law.tolerance()?,
        replacement_program_revision: None,
        transfer: None,
        recording: None,
        recording_result: None,
        found_mode: law.found_mode(),
        process_noise: Some(inputs.process_noise),
        validation_identity: inputs.validation_identity.clone(),
        transfers: Vec::new(),
    })
}

/// Where the run's writes go (A2.4, A2.5): the target the runtime reported, and whether the engineer approved a physical write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WriteTarget {
    source: ApplySource,
    physical_approved: bool,
}

/// Why a run stopped before its response was judged.
enum Halt {
    /// The engineer cancelled the run.
    Cancelled,
    /// A step was refused or failed.
    Refused(Failure),
}

impl From<Failure> for Halt {
    fn from(failure: Failure) -> Self {
        Self::Refused(failure)
    }
}

/// What a run keeps besides its attempt: the recording session, the stopped recording, the largest error of a passed response, the
/// record the runtime last reported, and the error of a record read after a failed requirement.
#[derive(Default)]
struct Kept {
    session: Option<RuntimeRecordingSession>,
    recording: Option<RecordedRun>,
    maximum_error: Option<f64>,
    parameters: Option<ParameterStatus>,
    settings_unread: Option<String>,
}

/// One Apply after its checkpoint is journaled: the law it runs, its transfer and parameter clients, and its checkpoint.
struct Run<'r, 'c, 'j> {
    loop_definition: &'r LoopDefinition,
    connection: &'r RuntimeConnection,
    target: WriteTarget,
    cycle_interval_ms: u64,
    pending: Arc<PendingReleases>,
    law: Law<'r>,
    transfers: TransferSession<'c>,
    parameters: ParameterTransactions<'c>,
    writer: CheckpointWriter<'j>,
    parameters_applied: bool,
}

impl Run<'_, '_, '_> {
    /// Runs the steps, then ends the run (`execute`): its attempt, the recovery it needs, the ending the rules give, the result and the
    /// checkpoint's final stage.
    fn execute(
        &mut self,
        report: &dyn CampaignReport,
        timing: &ApplyTiming<'_>,
    ) -> Result<ApplyOutcome, Failure> {
        let mut kept = Kept::default();
        let attempt = match self.attempt(report, timing, &mut kept) {
            Ok(attempt) => attempt,
            Err(Halt::Cancelled) => RunAttempt::Cancelled,
            Err(Halt::Refused(failure)) => RunAttempt::Refused(failure),
        };
        self.finish(attempt, kept, report)
    }

    /// The steps of the run, up to the judgement of the recorded response (`execute`; B6.2, B6.3, A6.3, A6.8, E11.10).
    fn attempt(
        &mut self,
        report: &dyn CampaignReport,
        timing: &ApplyTiming<'_>,
        kept: &mut Kept,
    ) -> Result<RunAttempt, Halt> {
        halt_if_cancelled(report)?;
        let boundary = self.transfers.status()?;
        let current = self.parameters.status()?;
        self.law.check_before_manual(&boundary, &current)?;
        if self.law.stages_manual(&boundary) {
            let staging = self.law.manual_step(&boundary)?;
            self.transfer(staging, true, Some(boundary))?;
        }
        halt_if_cancelled(report)?;
        self.writer.save(CheckpointStage::Parameters)?;
        self.parameters_applied = true;
        self.parameters
            .apply(self.law.review(), self.target.physical_approved)?;
        halt_if_cancelled(report)?;
        let before_auto = self.transfers.status()?;
        self.law.check_before_auto(&before_auto)?;
        let entry = self.law.auto_entry_step(&before_auto)?;
        let reviewed = self.law.reviewed_for(&before_auto);
        self.transfer(entry, false, reviewed)?;
        halt_if_cancelled(report)?;
        let entered = self.transfers.status()?;
        // B6.3: the confirmed transfers are journaled before the target change, so that a reattached observation shows them.
        let confirmed = self.transfers.outcomes().to_vec();
        self.writer
            .save_with(None, |checkpoint| checkpoint.transfers = confirmed)?;
        let change = self.law.target_step(&entered)?;
        let reviewed = self.law.reviewed_for(&entered);
        let plan = ObservationPlan {
            loop_definition: self.loop_definition,
            connection: self.connection,
            pending: &self.pending,
            target: self.law.target()?,
            duration_ms: self.law.duration_ms(),
            expected_settling_seconds: self.law.expected_settling_seconds(),
            cycle_interval_ms: self.cycle_interval_ms,
        };
        let transfers = &mut self.transfers;
        let writer = &self.writer;
        let target = self.target;
        let observation = observe_target_change(
            &plan,
            writer,
            || write_transfer(transfers, writer, change, false, reviewed, target).map(|_| ()),
            report,
            timing,
        );
        kept.session = observation.session;
        match observation.end {
            ObservationEnd::Recorded(run) => {
                let completeness = kept
                    .session
                    .as_ref()
                    .map(|session| session.evidence(&run).completeness);
                let recorded = observed_recording(
                    self.loop_definition,
                    &run,
                    completeness.unwrap_or(Completeness::Incomplete),
                )?;
                kept.recording = Some(run);
                let attempt = self.law.assess(&recorded)?;
                self.read_after(&attempt, kept)?;
                Ok(attempt)
            }
            ObservationEnd::Departed { failure, recording } => {
                kept.recording = recording;
                Ok(RunAttempt::PermittedRangeLeft(failure))
            }
            ObservationEnd::Cancelled => Err(Halt::Cancelled),
            ObservationEnd::Failed { failure, recording } => {
                kept.recording = recording;
                Err(Halt::Refused(failure))
            }
        }
    }

    /// Reads the record after the response was judged (`execute`; B6.3, A6.8): a passed response must find the reviewed program and
    /// values still in force, or the run is refused and restored; a failed requirement reads the settings that stay applied, and a read
    /// that fails leaves the recovery unconfirmed.
    fn read_after(&mut self, attempt: &RunAttempt, kept: &mut Kept) -> Result<(), Halt> {
        match attempt {
            RunAttempt::Passed { maximum_error } => {
                kept.maximum_error = Some(*maximum_error);
                let parameters = self.parameters.status()?;
                kept.parameters = Some(parameters.clone());
                self.law.held_after(&parameters)?;
            }
            RunAttempt::RequirementFailed(_) => match self.parameters.status() {
                Ok(parameters) => kept.parameters = Some(parameters),
                Err(failure) => kept.settings_unread = Some(whole_text(&failure)),
            },
            _ => {}
        }
        Ok(())
    }

    /// Ends the run (`execute`'s catch, finally and return; B6.3, B6.7, A8.5): a failure after the first write stops and keeps the
    /// recording and runs the one Restore at once; the rules give the ending, the result is recorded, and the checkpoint takes its final
    /// stage: kept while the settings are applied, cleared once the previous settings are back.
    fn finish(
        &mut self,
        attempt: RunAttempt,
        mut kept: Kept,
        report: &dyn CampaignReport,
    ) -> Result<ApplyOutcome, Failure> {
        let cancelled = report.is_cancelled();
        let mut recording_stop = None;
        let mut restored = false;
        let mut restore_ran = false;
        let recovery = match &attempt {
            RunAttempt::Passed { .. } | RunAttempt::Detached => Recovery::NotAttempted,
            RunAttempt::RequirementFailed(_) => kept
                .settings_unread
                .take()
                .map_or(Recovery::NotAttempted, Recovery::Unconfirmed),
            RunAttempt::Refused(_) | RunAttempt::Cancelled | RunAttempt::PermittedRangeLeft(_) => {
                recording_stop = stop_unfinished(kept.session.as_mut(), &mut kept.recording);
                keep_recording(&self.writer, kept.recording.as_ref())?;
                if recovery_needed(
                    self.parameters_applied,
                    self.transfers.has_owned_operation(),
                ) {
                    restore_ran = true;
                    let restoration = restore_at_once(
                        self.connection,
                        self.loop_definition,
                        &self.writer,
                        self.target.physical_approved,
                        &|text: &str| report.progress(text),
                    );
                    restored = restoration.restored;
                    if restoration.parameters.is_some() {
                        kept.parameters = restoration.parameters;
                    }
                    restoration.recovery
                } else {
                    // Nothing was written, so the previous settings are back at once (`recover`).
                    Recovery::Confirmed
                }
            }
        };
        let ending = self
            .law
            .settle(attempt, cancelled, recovery, recording_stop.as_deref())
            .ok_or_else(|| {
                Failure::new(
                    engine_texts::NO_ENDING,
                    Cause::Implementation,
                    NextAction::None,
                )
            })?;
        let stage = checkpoint_stage(ending.stage);
        let result = self.law.result(
            ending,
            RunOutcome {
                recording: kept.recording.clone(),
                transfers: self.transfers.outcomes().to_vec(),
                maximum_error: kept.maximum_error,
                parameters: kept.parameters,
            },
        )?;
        if !restore_ran {
            self.close_checkpoint(stage, kept.recording.as_ref())?;
        }
        release_kept(kept.session.as_mut(), kept.recording.as_ref());
        Ok(outcome_of(result, &self.writer, restored))
    }

    /// Writes the checkpoint's final stage when no Restore wrote it (A8.5): a completed checkpoint keeps the settings' recording and
    /// stays journaled for Restore and Keep; a recovered one is cleared. A Restore writes and clears its own checkpoint.
    fn close_checkpoint(
        &self,
        stage: CheckpointStage,
        recording: Option<&RecordedRun>,
    ) -> Result<(), Failure> {
        if stage == CheckpointStage::Recovered {
            return self.writer.clear_checkpoint();
        }
        self.writer.save_with(Some(stage), |checkpoint| {
            if let Some(run) = recording {
                checkpoint.recording_result = Some(run.clone());
                checkpoint.recording = None;
            }
        })
    }

    /// Writes one transfer of the run, its owner journaled in the checkpoint before it is dispatched (E11.2).
    fn transfer(
        &mut self,
        step: TransferStep,
        manual: bool,
        reviewed: Option<TransferStatus>,
    ) -> Result<TransferStatus, Failure> {
        write_transfer(
            &mut self.transfers,
            &self.writer,
            step,
            manual,
            reviewed,
            self.target,
        )
    }
}

/// Writes one transfer (`transfer.apply`; A6.3, E6.5): the step's states, the mode it leaves, its allowance and its tracking, on the
/// status it was reviewed on, to the target with its approval. The transfer session retains its owner through the checkpoint before it
/// dispatches the apply, and confirms the outcome.
fn write_transfer(
    transfers: &mut TransferSession<'_>,
    writer: &CheckpointWriter<'_>,
    step: TransferStep,
    manual: bool,
    reviewed: Option<TransferStatus>,
    target: WriteTarget,
) -> Result<TransferStatus, Failure> {
    let write = TransferWrite {
        values: step.values,
        manual,
        tolerance: step.tolerance,
        tracking: Some(step.tracking),
        reviewed,
        source: target.source,
        physical_approved: target.physical_approved,
    };
    transfers.apply(&write, &mut writer.transfer_sink())
}

/// Stops a run that the engineer cancelled, between its steps (`checkCancelled`).
fn halt_if_cancelled(report: &dyn CampaignReport) -> Result<(), Halt> {
    if report.is_cancelled() {
        Err(Halt::Cancelled)
    } else {
        Ok(())
    }
}
