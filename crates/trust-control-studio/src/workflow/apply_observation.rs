//! The observation of an Apply (specification 35, B6.3, A3.2 to A3.6, A8.5, A8.6 and A10.4; annex E4.2, E4.6, E4.7, E11.2, E11.5 and
//! E11.6; numerics N8): from `applyObservation.ts` (`observeTargetChange`, `followAppliedResponse`, `resumeApplyObservation`) and the
//! reattachment of `engineerActions/recovery.ts`.
//!
//! The runtime's recording starts before the validated target change, its owner journaled in the checkpoint before the start is
//! dispatched. The recording is followed in process time; every sample of every measured output is checked against its permitted range,
//! and a departure ends the observation at once. The response starts at the first scan in Auto at the reviewed target, and the recording
//! is stopped as soon as the response covers the reviewed duration. A reopened engine reattaches to an observation by its recording's
//! journaled owner, follows it to its end and assesses it with the same refusals. Every decision is trust-control's
//! (`observation_assessment`, `observation`, `pid_commissioning::response_starts`); this module follows, stops and reads.

mod samples;

use std::sync::Arc;
use std::thread::sleep;

use trust_control::commissioning::adapters::controller_adapter;
use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::application_records::ApplyOutcome;
use trust_control::commissioning::apply_texts;
use trust_control::commissioning::checkpoint::CommissioningCheckpoint;
use trust_control::commissioning::commissioning_result::{
    CommissioningChannel, CommissioningResult, CommissioningStatus, TransferStatus,
};
use trust_control::commissioning::found_mode::FoundMode;
use trust_control::commissioning::observation::commissioning_observation;
use trust_control::commissioning::observation_assessment::{
    check_permitted_ranges, connection_lost, observation_in_progress, observed_ranges_of,
    planned_recording_ms, response_covers_duration, ObservedRecording,
};
use trust_control::commissioning::parameter_binding::runtime_binding_of;
use trust_control::commissioning::pid_commissioning::response_starts;
use trust_control::commissioning::proportional_commissioning::{
    assess_run, check_held_values, expected_settling_seconds, run_channels, run_criteria,
    settle_run, Recovery, RunAttempt,
};
use trust_control::experiment::plan::recording_task;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::connected_record::RunEvidence;
use trust_control::identification::connected_run::recording_settling_ms;
use trust_control::limits::{record_period_executions, sample_period_ms, RECORDING_SAMPLE_LIMIT};
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::{
    Completeness, EvidenceSource, RecordedRun, RecordingState, RecordingStatus,
};
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::SignalBinding;

use crate::operation::{cancelled, OperationContext};
use crate::runtime_client::parameters::ParameterTransactions;
use crate::runtime_client::runtime_recording::{
    new_recording_id, PendingReleases, RecordingOwnerPersistence, RecordingStartError,
    RecordingStartPlan, RecordingStartUnconfirmed, RuntimeRecordingSession,
};
use crate::runtime_client::transfers::TransferSession;
use crate::runtime_client::RuntimeClient;
use crate::workflow::apply::{
    checkpoint_stage, keep_recording, law_target, outcome_of, release_kept, restore_at_once,
    stop_unfinished, wall_clock, ApplyContext, ApplyTiming, CheckpointWriter,
};
use crate::workflow::apply_texts as engine_texts;
use crate::workflow::commissioning_journal::SharedJournal;
use crate::workflow::identify_campaign::CampaignReport;

pub(crate) use samples::{page_samples, run_samples};

/// What an observation follows of a runtime recording (E4.6, E4.7): its pages in order, its stop, and the bindings its samples are read
/// against. The runtime's recording session is one; the tests follow scripted pages.
pub(crate) trait AppliedRecording {
    /// The next page of the recording, from the sample after the last page read (E4.6).
    ///
    /// # Errors
    /// The runtime cannot be reached or refuses, or its answer does not describe the recording.
    fn page(&mut self) -> Result<RecordingStatus, Failure>;

    /// Stops the recording and returns its run (E4.7).
    ///
    /// # Errors
    /// The runtime cannot be reached or refuses, or its answer does not answer the recording.
    fn stop(&mut self) -> Result<RecordedRun, Failure>;

    /// The recording's signals, in the order of every sample's arrays.
    fn bindings(&self) -> Vec<SignalBinding>;
}

impl AppliedRecording for RuntimeRecordingSession {
    fn page(&mut self) -> Result<RecordingStatus, Failure> {
        RuntimeRecordingSession::follow(self)
    }

    fn stop(&mut self) -> Result<RecordedRun, Failure> {
        RuntimeRecordingSession::stop(self)
    }

    fn bindings(&self) -> Vec<SignalBinding> {
        self.recovery_owner()
            .bindings
            .into_iter()
            .map(|binding| SignalBinding {
                id: binding.id,
                address: binding.address,
                symbol: binding.symbol,
                iec_type: binding.iec_type,
                unit: None,
            })
            .collect()
    }
}

/// How one recording is followed (B6.3, A3.4): the loop whose measured outputs are checked, the reviewed target whose first scan in Auto
/// starts the response, the reviewed duration, and the record period that sets the connection-loss timeout (A7). Without a target the
/// recording is followed to its own end, as a reattachment follows it (`resumeApplyObservation`).
pub(crate) struct FollowPlan<'a> {
    /// The loop being applied.
    pub(crate) loop_definition: &'a LoopDefinition,
    /// The reviewed target; `None` follows the recording to its own end.
    pub(crate) target: Option<f64>,
    /// The reviewed duration of the response, in ms of process time.
    pub(crate) duration_ms: f64,
    /// The record period of the recording, in ms (A4.2); 0 when it is not known.
    pub(crate) record_period_ms: f64,
}

/// How a followed recording ended (B6.3).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Followed {
    /// The response was followed to its end and the recording stopped, every sample inside its permitted range.
    Stopped(RecordedRun),
    /// A measured output left its permitted range (B6.3, B6.7): the departure, and the stopped recording when the departure was found
    /// in it.
    Departed {
        /// The departure, which names the output, its value and its range.
        failure: Failure,
        /// The recording, when the departure was found once it was stopped.
        stopped: Option<RecordedRun>,
    },
    /// The operation was cancelled while the recording ran; the recording still runs.
    Cancelled,
}

/// Follows a running recording in process time (`followAppliedResponse`; B6.3, A3.2, A3.6). Every sample is checked against the
/// permitted range of each measured output as it arrives, so a departure ends the observation at once and no further page is read. With a
/// target, the response starts at the first scan in Auto at it, and the recording is stopped as soon as the response covers the reviewed
/// duration from there, so the time the target change took never shortens it. The stopped recording is checked again before it is
/// returned whole.
///
/// # Errors
/// A page or the stop fails; the runtime ended the recording (state `failed`, A6.7); or no newer runtime value arrived within the
/// connection-loss timeout (A3.4).
pub(crate) fn follow_applied_response(
    recording: &mut dyn AppliedRecording,
    plan: &FollowPlan<'_>,
    report: &dyn CampaignReport,
    timing: &ApplyTiming<'_>,
) -> Result<Followed, Failure> {
    let ranges = observed_ranges_of(&plan.loop_definition.signals);
    let bindings = recording.bindings();
    let mut origin = None;
    let mut newest_elapsed_ms: Option<u64> = None;
    let mut newest_progress_ms = (timing.wall_ms)();
    let mut response_start: Option<f64> = None;
    let mut newest_sample_seconds = f64::NEG_INFINITY;
    loop {
        if report.is_cancelled() {
            return Ok(Followed::Cancelled);
        }
        let page = recording.page()?;
        if page.state == RecordingState::Failed {
            return Err(apply_texts::recording_ended(page.reason.as_deref()));
        }
        let samples = page_samples(plan.loop_definition, &bindings, &page.samples, &mut origin)?;
        if let Err(departure) = check_permitted_ranges(&samples, &ranges) {
            return Ok(Followed::Departed {
                failure: departure.failure().clone(),
                stopped: None,
            });
        }
        for sample in &samples {
            if response_start.is_none()
                && plan
                    .target
                    .is_some_and(|target| response_starts(sample, target))
            {
                response_start = Some(sample.elapsed_seconds);
            }
            newest_sample_seconds = newest_sample_seconds.max(sample.elapsed_seconds);
        }
        let now_ms = (timing.wall_ms)();
        if newest_elapsed_ms.is_none_or(|newest| page.elapsed_ms > newest) {
            newest_elapsed_ms = Some(page.elapsed_ms);
            newest_progress_ms = now_ms;
        } else if connection_lost(
            exact_ms(newest_progress_ms),
            exact_ms(now_ms),
            plan.record_period_ms,
        ) {
            return Err(Failure::stopped_sending());
        }
        if response_covers_duration(response_start, newest_sample_seconds, plan.duration_ms) {
            break;
        }
        if page.state != RecordingState::Recording && page.next_sample >= page.sample_count {
            break;
        }
        if page.state == RecordingState::Recording {
            let elapsed_ms = match (plan.target, response_start) {
                (Some(_), Some(start)) => (newest_sample_seconds - start) * 1000.0,
                (Some(_), None) => 0.0,
                (None, _) => exact_ms(page.elapsed_ms),
            };
            report.progress(&engine_texts::recording_progress(
                elapsed_ms,
                plan.duration_ms,
            ));
            sleep(timing.follow_poll);
        }
    }
    let stopped = recording.stop()?;
    if let Err(departure) =
        check_permitted_ranges(&run_samples(plan.loop_definition, &stopped)?, &ranges)
    {
        return Ok(Followed::Departed {
            failure: departure.failure().clone(),
            stopped: Some(stopped),
        });
    }
    Ok(Followed::Stopped(stopped))
}

/// The recorded response as the assessment reads it (B6.3, A9): the recording's completeness as its evidence says, its scans, the
/// applied output's unit, and its source, a recording of the target.
///
/// # Errors
/// A sample carries no runtime time.
pub(crate) fn observed_recording(
    loop_definition: &LoopDefinition,
    run: &RecordedRun,
    completeness: Completeness,
) -> Result<ObservedRecording, Failure> {
    Ok(ObservedRecording {
        completeness,
        samples: run_samples(loop_definition, run)?,
        output_unit: loop_definition.signals.manipulated_variable.unit.clone(),
        evidence: RunEvidence::PhysicalRecorded,
    })
}

/// What an Apply's observation is started with (B6.3, A4.2, A7): the loop and its runtime, the engine's list of recordings that wait for
/// their release, the reviewed target and duration, the settling time the record period follows, and the runtime's resource cycle.
pub(crate) struct ObservationPlan<'a> {
    /// The loop being applied.
    pub(crate) loop_definition: &'a LoopDefinition,
    /// The runtime the Apply writes to (E11.1).
    pub(crate) connection: &'a RuntimeConnection,
    /// The recordings that are stopped but not released (E4.7).
    pub(crate) pending: &'a Arc<PendingReleases>,
    /// The reviewed target of the response.
    pub(crate) target: f64,
    /// The reviewed duration of the response, in ms of process time.
    pub(crate) duration_ms: f64,
    /// The settling time the record period follows, in s (A4.2); absent for an ON/OFF output.
    pub(crate) expected_settling_seconds: Option<f64>,
    /// The runtime's resource cycle interval, in ms (E1.3).
    pub(crate) cycle_interval_ms: u64,
}

/// An observation as it ended: the recording session, once it started (its owner, its stop and its release), and how it ended.
pub(crate) struct Observation {
    /// The recording session, once the start was dispatched.
    pub(crate) session: Option<RuntimeRecordingSession>,
    /// How the observation ended.
    pub(crate) end: ObservationEnd,
}

/// How an Apply's observation ended (B6.3, B6.7).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ObservationEnd {
    /// The response was followed to its end and the recording stopped.
    Recorded(RecordedRun),
    /// A measured output left its permitted range: the departure, and the recording when it was already stopped.
    Departed {
        /// The departure.
        failure: Failure,
        /// The stopped recording, when the departure was found in it.
        recording: Option<RecordedRun>,
    },
    /// The operation was cancelled.
    Cancelled,
    /// The recording did not start, the target change failed, or the follow failed: the failure, and the recording a stop after an
    /// unconfirmed start returned (E11.4).
    Failed {
        /// Why the observation failed.
        failure: Failure,
        /// The recording an unconfirmed start's stop returned, when it did.
        recording: Option<RecordedRun>,
    },
}

/// Runs the validated target change under the runtime's recording (`observeTargetChange`; B6.3, A4.2, E11.2 to E11.4): the recording
/// starts first, with its owner journaled through `persistence` before its start is dispatched, for the reviewed duration, the
/// allowance of the target change and three record periods (`planned_recording_ms`); then `apply_target` changes the target; then the recording is followed
/// to the end of the response. A start whose acknowledgement was lost keeps its owner, and its stopped samples are the runtime's.
pub(crate) fn observe_target_change(
    plan: &ObservationPlan<'_>,
    persistence: &dyn RecordingOwnerPersistence,
    apply_target: impl FnOnce() -> Result<(), Failure>,
    report: &dyn CampaignReport,
    timing: &ApplyTiming<'_>,
) -> Observation {
    let loop_definition = plan.loop_definition;
    let planned_ms = whole_ms(planned_recording_ms(
        plan.duration_ms,
        loop_definition.controller.task_period_ms,
    ));
    let settling_ms = plan
        .expected_settling_seconds
        .and_then(|seconds| recording_settling_ms(planned_ms, Some(seconds)));
    let start = RecordingStartPlan {
        recording_id: new_recording_id(),
        planned_duration_ms: planned_ms,
        expected_settling_ms: settling_ms,
    };
    let mut session = match RuntimeRecordingSession::start(
        plan.connection,
        loop_definition,
        &start,
        Some(persistence),
        plan.pending,
    ) {
        Ok(session) => session,
        Err(RecordingStartError::Failed(failure)) => {
            return Observation {
                session: None,
                end: ObservationEnd::Failed {
                    failure,
                    recording: None,
                },
            }
        }
        Err(RecordingStartError::Unconfirmed(unconfirmed)) => {
            let failure = Failure::new(
                &unconfirmed.message(),
                Cause::Unclassified,
                NextAction::None,
            );
            let RecordingStartUnconfirmed {
                session, recording, ..
            } = *unconfirmed;
            return Observation {
                session: Some(session),
                end: ObservationEnd::Failed { failure, recording },
            };
        }
    };
    if report.is_cancelled() {
        return Observation {
            session: Some(session),
            end: ObservationEnd::Cancelled,
        };
    }
    if let Err(failure) = apply_target() {
        return Observation {
            session: Some(session),
            end: ObservationEnd::Failed {
                failure,
                recording: None,
            },
        };
    }
    let follow = FollowPlan {
        loop_definition,
        target: Some(plan.target),
        duration_ms: plan.duration_ms,
        record_period_ms: record_period_ms(
            loop_definition,
            planned_ms,
            settling_ms,
            plan.cycle_interval_ms,
        ),
    };
    let end = match follow_applied_response(&mut session, &follow, report, timing) {
        Ok(Followed::Stopped(run)) => ObservationEnd::Recorded(run),
        Ok(Followed::Departed { failure, stopped }) => ObservationEnd::Departed {
            failure,
            recording: stopped,
        },
        Ok(Followed::Cancelled) => ObservationEnd::Cancelled,
        Err(failure) => ObservationEnd::Failed {
            failure,
            recording: None,
        },
    };
    Observation {
        session: Some(session),
        end,
    }
}

/// The record period of an observation's recording, in ms (A4.2, E4.4): the executions per sample that the start chooses for the
/// planned duration and the settling time, times the period of what it samples, the loop's task or the resource cycle.
fn record_period_ms(
    loop_definition: &LoopDefinition,
    planned_ms: u64,
    settling_ms: Option<u64>,
    cycle_interval_ms: u64,
) -> f64 {
    let sampled = sample_period_ms(
        recording_task(loop_definition).is_some(),
        whole_ms(loop_definition.controller.task_period_ms),
        cycle_interval_ms,
    );
    let executions =
        record_period_executions(planned_ms, sampled, RECORDING_SAMPLE_LIMIT, settling_ms);
    exact_ms(executions.saturating_mul(sampled))
}

/// Reattaches a reopened engine to the Apply observation that the loop's journal holds (A8.6, A10.4, B6.3; E11.5): the recording is
/// rebuilt from its journaled owner, never started again, followed to its own end and assessed with the same refusals as an
/// uninterrupted Apply. A response that passed or failed a requirement keeps the settings applied, with Restore and Keep in project
/// offered; a departure from the permitted range runs the one Restore at once, which on a physical target writes only with the approval
/// that `context` carries (A2.5). Its samples are kept in the checkpoint before the runtime's buffer is released, and no write of the
/// Apply is replayed. A cancel stops following and leaves the observation to the runtime, as closing the panel does (E11.11).
///
/// # Errors
/// The journal holds no observation that the runtime may still record; the loop's adapter or the target cannot be built; the runtime
/// cannot be reached or refuses, ends the recording or stops sending values; the cancel (`operation::cancelled`); or the journal cannot be
/// written.
pub fn reattach_observation(
    context: &mut ApplyContext<'_>,
    operation: &OperationContext,
) -> Result<ApplyOutcome, Failure> {
    let clock = wall_clock();
    reattach_reporting(context, operation, &ApplyTiming::production(&clock))
}

/// [`reattach_observation`], reporting to `report` with the pauses and the clock of `timing`.
pub(crate) fn reattach_reporting(
    context: &mut ApplyContext<'_>,
    report: &dyn CampaignReport,
    timing: &ApplyTiming<'_>,
) -> Result<ApplyOutcome, Failure> {
    let loop_definition = context.loop_definition;
    let connection = context.connection;
    let physical_approved = context.physical_approved;
    let journal = SharedJournal::new(&mut *context.journal);
    let checkpoint = journal
        .checkpoint()
        .filter(observing)
        .ok_or_else(apply_texts::no_observation_to_resume)?;
    let owner = checkpoint
        .recording
        .clone()
        .ok_or_else(apply_texts::no_observation_to_resume)?;
    let adapter = controller_adapter(loop_definition)?;
    let target = law_target(
        &adapter,
        loop_definition,
        &checkpoint.model,
        &checkpoint.task,
        &checkpoint.binding,
    )?;
    report.progress(engine_texts::REATTACHING);
    let runtime = RuntimeClient::connect(connection)?.status()?;
    let pending = Arc::new(PendingReleases::new());
    let mut session = RuntimeRecordingSession::from_recovery_owner(
        connection,
        loop_definition,
        &owner,
        &pending,
    )?;
    let duration_ms = checkpoint.task.duration_seconds * 1000.0;
    let planned_ms = whole_ms(planned_recording_ms(
        duration_ms,
        loop_definition.controller.task_period_ms,
    ));
    let settling_ms = expected_settling_seconds(loop_definition, &checkpoint.task)
        .and_then(|seconds| recording_settling_ms(planned_ms, Some(seconds)));
    let follow = FollowPlan {
        loop_definition,
        target: None,
        duration_ms,
        record_period_ms: record_period_ms(
            loop_definition,
            planned_ms,
            settling_ms,
            runtime.cycle_interval_ms,
        ),
    };
    let followed = follow_applied_response(&mut session, &follow, report, timing)?;
    let reattachment = Reattachment {
        loop_definition,
        connection,
        source: match runtime.source() {
            EvidenceSource::SimulatedPlant => ApplySource::Simulation,
            EvidenceSource::PhysicalPlant => ApplySource::Physical,
        },
        physical_approved,
        target,
    };
    let writer = CheckpointWriter::new(journal, checkpoint);
    match followed {
        Followed::Cancelled => Err(cancelled()),
        Followed::Stopped(run) => settle_reattached(&reattachment, &writer, &mut session, run),
        Followed::Departed { failure, stopped } => restore_reattached(
            &reattachment,
            &writer,
            &mut session,
            failure,
            stopped,
            report,
        ),
    }
}

/// Whether a journaled checkpoint holds an observation the runtime may still record (A8.5): the checkpoint journals the observation's
/// recording owner only while the observation runs, so the owner stands for both the observation and its recording.
fn observing(checkpoint: &CommissioningCheckpoint) -> bool {
    let recorded = checkpoint.recording.is_some();
    observation_in_progress(checkpoint.stage.journal_stage(), recorded, recorded)
}

/// What a reattachment works with besides its checkpoint: the loop, its runtime and target, the approval that covers a physical target's
/// restoration (A2.5), and the reviewed target of the response.
struct Reattachment<'a> {
    loop_definition: &'a LoopDefinition,
    connection: &'a RuntimeConnection,
    source: ApplySource,
    physical_approved: bool,
    target: f64,
}

/// The ending of a reattached observation that was followed to its end (`settlePass` and `reportAppliedFailure` of
/// `engineerActions/recovery.ts`; B6.3): the samples are kept and the buffer released, the response is assessed against the reviewed
/// criteria, and a passed response is checked against the running record. The settings stay applied, so the checkpoint completes.
fn settle_reattached(
    reattachment: &Reattachment<'_>,
    writer: &CheckpointWriter<'_>,
    session: &mut RuntimeRecordingSession,
    run: RecordedRun,
) -> Result<ApplyOutcome, Failure> {
    let loop_definition = reattachment.loop_definition;
    let completeness = session.evidence(&run).completeness;
    keep_recording(writer, Some(&run))?;
    release_kept(Some(session), Some(&run));
    let checkpoint = writer.checkpoint();
    let criteria = trust_control::commissioning::on_off_adapter::with_switching(
        &run_criteria(
            loop_definition,
            &checkpoint.binding,
            &checkpoint.task,
            reattachment.target,
        ),
        loop_definition,
        &checkpoint.candidate,
    )?;
    let observation = commissioning_observation(
        loop_definition,
        &checkpoint.model,
        &checkpoint.task,
        &checkpoint.candidate,
    )?;
    let mut attempt = assess_run(
        &observed_recording(loop_definition, &run, completeness)?,
        &criteria,
        &observation,
    );
    let mut maximum_error = None;
    let mut parameters = None;
    if let RunAttempt::Passed {
        maximum_error: error,
    } = attempt
    {
        maximum_error = Some(error);
        let mut client = RuntimeClient::connect(reattachment.connection)?;
        let read = ParameterTransactions::new(
            &mut client,
            runtime_binding_of(loop_definition)?,
            reattachment.source,
        )
        .status()?;
        if let Err(changed) = check_held_values(
            &checkpoint.origin.program_revision,
            &checkpoint.candidate,
            &read,
        ) {
            // The settings stay applied, and the engineer decides on Restore previous settings (B6.3).
            attempt = RunAttempt::RequirementFailed(Failure::from_parts(
                changed.message,
                changed.detail,
                Cause::Settings,
                NextAction::Restore,
            ));
        }
        parameters = Some(read);
    }
    let transfers = observed_transfers(reattachment.connection, &checkpoint)?;
    let unit = &loop_definition.signals.process_variable.unit;
    let mut ending = settle_run(
        attempt,
        false,
        Recovery::NotAttempted,
        checkpoint.found_mode == Some(FoundMode::Auto),
        unit,
        None,
    )
    .ok_or_else(no_ending)?;
    if ending.status == CommissioningStatus::Passed {
        ending.reason = apply_texts::reattach_passed(maximum_error.unwrap_or_default(), unit);
    }
    writer.save(checkpoint_stage(ending.stage))?;
    let result = CommissioningResult {
        status: ending.status,
        reason: ending.reason,
        recording: Some(run),
        transfers,
        recovery_confirmed: ending.recovery_confirmed,
        maximum_error,
        target: reattachment.target,
        parameters,
        model_revision: checkpoint.model.identity().revision.clone(),
        channels: reattached_channels(loop_definition)?,
        criteria,
        gain_recovery: None,
        failure: ending.failure,
        found_mode: checkpoint.found_mode,
    };
    Ok(outcome_of(result, writer, false))
}

/// The ending of a reattached observation whose measured output left its permitted range (`restoreAfterDeparture` of
/// `engineerActions/recovery.ts`; B6.3, B6.7): the recording is stopped and kept, and the one Restore runs at once.
fn restore_reattached(
    reattachment: &Reattachment<'_>,
    writer: &CheckpointWriter<'_>,
    session: &mut RuntimeRecordingSession,
    departure: Failure,
    stopped: Option<RecordedRun>,
    report: &dyn CampaignReport,
) -> Result<ApplyOutcome, Failure> {
    let loop_definition = reattachment.loop_definition;
    let mut recording = stopped;
    let recording_stop = stop_unfinished(Some(&mut *session), &mut recording);
    keep_recording(writer, recording.as_ref())?;
    release_kept(Some(session), recording.as_ref());
    let restoration = restore_at_once(
        reattachment.connection,
        loop_definition,
        writer,
        reattachment.physical_approved,
        &|text: &str| report.progress(text),
    );
    let checkpoint = writer.checkpoint();
    let ending = settle_run(
        RunAttempt::PermittedRangeLeft(departure),
        false,
        restoration.recovery,
        checkpoint.found_mode == Some(FoundMode::Auto),
        &loop_definition.signals.process_variable.unit,
        recording_stop.as_deref(),
    )
    .ok_or_else(no_ending)?;
    let result = CommissioningResult {
        status: ending.status,
        reason: ending.reason,
        recording,
        transfers: Vec::new(),
        recovery_confirmed: ending.recovery_confirmed,
        maximum_error: None,
        target: reattachment.target,
        parameters: restoration.parameters,
        model_revision: checkpoint.model.identity().revision.clone(),
        channels: reattached_channels(loop_definition)?,
        criteria: trust_control::commissioning::on_off_adapter::with_switching(
            &run_criteria(
                loop_definition,
                &checkpoint.binding,
                &checkpoint.task,
                reattachment.target,
            ),
            loop_definition,
            &checkpoint.candidate,
        )?,
        gain_recovery: None,
        failure: ending.failure,
        found_mode: checkpoint.found_mode,
    };
    Ok(outcome_of(result, writer, restoration.restored))
}

/// The confirmed transfers a reattached observation can show (B6.3, A8.6; `observedTransfers` of `engineerActions/recovery.ts`): those
/// journaled before the target change, and the target change as the runtime still reports it under its journaled identity. Nothing is
/// invented: another last transaction adds none.
fn observed_transfers(
    connection: &RuntimeConnection,
    checkpoint: &CommissioningCheckpoint,
) -> Result<Vec<TransferStatus>, Failure> {
    let mut journaled_transfers = checkpoint.transfers.clone();
    let Some(owner) = &checkpoint.transfer else {
        return Ok(journaled_transfers);
    };
    let mut client = RuntimeClient::connect(connection)?;
    let status = TransferSession::new(
        &mut client,
        checkpoint.binding.clone(),
        checkpoint.origin.program_revision.clone(),
    )
    .status()?;
    let journaled = status
        .transaction
        .as_ref()
        .is_some_and(|transaction| transaction.id == owner.id);
    let already_shown = journaled_transfers.iter().any(|shown| {
        shown
            .transaction
            .as_ref()
            .is_some_and(|transaction| transaction.id == owner.id)
    });
    if journaled && !already_shown {
        journaled_transfers.push(status);
    }
    Ok(journaled_transfers)
}

/// The channels a reattached observation's result names (`observationResult` of `engineerActions/recovery.ts`; C1): the setpoint, the
/// measurement and the manipulated variable, then every additional signal of the loop.
fn reattached_channels(
    loop_definition: &LoopDefinition,
) -> Result<Vec<CommissioningChannel>, Failure> {
    let mut channels = run_channels(loop_definition)?;
    channels.extend(
        loop_definition
            .signals
            .additional
            .iter()
            .flatten()
            .map(|signal| CommissioningChannel {
                id: signal.symbol.clone(),
                unit: signal.unit.clone(),
            }),
    );
    Ok(channels)
}

/// The failure of an ending the rules did not give: a defect of the engine (A11.3).
fn no_ending() -> Failure {
    Failure::new(
        engine_texts::NO_ENDING,
        Cause::Implementation,
        NextAction::None,
    )
}

/// A whole number of ms from a duration in ms, rounded up so that the recording covers it (A7). The durations converted here are finite,
/// not negative and at most a day of process time plus the confirmation window, far inside `u64`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn whole_ms(milliseconds: f64) -> u64 {
    milliseconds.ceil() as u64
}

/// A count of ms as a float. The counts converted here are process or wall times far below 2^53 ms, so the conversion is exact.
#[allow(clippy::cast_precision_loss)]
fn exact_ms(milliseconds: u64) -> f64 {
    milliseconds as f64
}

#[cfg(test)]
#[path = "apply_observation_tests.rs"]
mod tests;
