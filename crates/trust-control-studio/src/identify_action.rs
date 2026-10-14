//! The Identify method `controlStudio/identify` (specification 35, §0.1a, A8.3 to A8.6 and B3; Identify page specification)
//! and the engine's cancel of one journaled operation of a loop (§0.1b, `cancel`).
//!
//! - `identify` runs a live campaign with the engineer's method and limits (B3.2 to B3.4), `openRecording` fits the recording
//!   of the study's Identify result again without live excitation (B3.6), and `resume` follows the journaled campaign to its
//!   end (A8.5, A8.6). Each answers at once with its operation's identifier. Its final update carries trust-control's
//!   `IdentifyResult`, once the study keeps the Identify section with the request its recording was taken with.
//! - `reset` starts a new working study and keeps the previous one as history (A8.4), and answers at once with the stages.
//! - `cancel_loop` cancels the journaled operation the command line names (§0.1b, E5.5, E11.6): a campaign through its
//!   restoration path, a lone recording owner by stopping and keeping its recording. Its final update is a [`CancelResult`].
//!
//! What needs no runtime is checked before anything starts: the loop opens, the loop's journal holds no operation (for
//! `identify`, `openRecording` and `reset`), the engineer's acquisition settings, and the recording a refit names. `identify`
//! and `resume` reach the runtime, so they are refused at once without a connection (E11.1). Every rule and refusal is
//! trust-control's; the workflow makes the runtime calls, and the storage writes the files.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use trust_control::experiment::ControlExperimentKind;
use trust_control::failure::Failure;
use trust_control::identification::acquisition_settings::{
    configured_acquisition, AcquisitionSettings,
};
use trust_control::identification::automatic::AutomaticIdentificationResult;
use trust_control::identification::campaign::{campaign_maximum_ms, require_campaign_limits};
use trust_control::identification::campaign_review::campaign_review;
use trust_control::identification::connected_run::{
    accepted_model_from_one_record, checked_acquisition_settings, model_check_instead_of_model,
    model_not_from_one_record, not_journaled, qualification, recording_not_kept,
    resumed_without_experiment, runtime_required, stored_fit_window, unfinished_operation_refusal,
};
use trust_control::identification::identify_result::{
    identify_result, identify_signals, study_evidence,
};
use trust_control::identification::recorded_limits::{
    evaluate_recorded_limits, RecordedLimitsEvaluation,
};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{
    CancelResult, IdentifyAction, IdentifyRequest, OperationStarted, ResetResult,
};
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::stages::stage_availability;
use trust_control::study::identify::{
    refit_refusal, IdentifyMethod, IdentifySection, StoredRecordRequest,
};
use trust_control::study::Study;

use crate::busy_loops::{LoopClaim, LoopClaims};
use crate::commands::to_value;
use crate::identify_adapters::{RuntimeSampler, StudyRetention};
use crate::journal::{Journal, JournalEntry};
use crate::operation::{OperationContext, Operations};
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::runtime_client::RuntimeClient;
use crate::workflow::identify::run::{connected_record, fit_records};
use crate::workflow::identify::{
    identify_plant, IdentifyPorts, IdentifyRequest as RunRequest, IdentifyResult as RunResult,
    LoopSampler, RunEvaluator,
};
use crate::workflow::identify_recovery::{cancel_operation, resume_identification};
use crate::{loops, storage};

/// The command of the operations that `identify`, `openRecording` and `resume` start (§0.1a).
const IDENTIFY_COMMAND: &str = "identify";
/// The command of the operation that cancels a loop's journaled identification (§0.1b, `cancel`).
const CANCEL_COMMAND: &str = "cancel";

/// What an operation of the loop works with: owned, so that it moves to the operation's thread. The runtime connection is not
/// part of it: an action that reaches the runtime passes its own connection (E11.1).
struct LoopWork {
    project_root: PathBuf,
    storage_folder: PathBuf,
    pending: Arc<PendingReleases>,
    loop_definition: LoopDefinition,
}

/// A model fitted from one recording of the study, with how that recording was obtained.
struct Fitted {
    /// How the recording was obtained (B3.3, B3.6).
    method: IdentifyMethod,
    /// Where the recording's values come from (A9).
    source: EvidenceSource,
    /// The recording file the model was fitted on (A8.3).
    recording_file_id: String,
    /// The fit.
    result: AutomaticIdentificationResult<RecordedLimitsEvaluation>,
}

/// Answers `controlStudio/identify`. `identify`, `openRecording` and `resume` start an operation on `operations` and answer
/// with its identifier at once; `reset` answers with the stages of the new working study. Each action first claims the loop
/// in `claims` (one operation per loop at a time); an operation holds the claim until it ends, and a refused request lets it
/// go at once. The runtime recordings the operations stop are released through `pending`, the engine's one list (E4.7).
///
/// # Errors
/// The loop cannot be opened, an operation of this engine works on it, or its journal cannot be read; `identify`,
/// `openRecording` or `reset` while the loop's journal holds an operation; `identify` or `resume` without a runtime connection
/// (E11.1); acquisition settings that fail their checks or cannot configure the loop; a refit of a recording whose request the
/// study does not keep; or the study cannot be read, or written by `reset`.
pub fn answer(
    operations: &Operations,
    pending: &Arc<PendingReleases>,
    claims: &LoopClaims,
    request: IdentifyRequest,
) -> Result<Value, Failure> {
    let IdentifyRequest {
        project_root,
        runtime_id,
        loop_id,
        storage_folder,
        connection,
        action,
    } = request;
    let project_root = PathBuf::from(project_root);
    let storage_folder = PathBuf::from(storage_folder);
    let loop_definition = loops::open(&project_root, &runtime_id, &loop_id)?;
    let claim = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        operation_name(&action),
    )?;
    let journal = Journal::open(
        &storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    let work = LoopWork {
        project_root,
        storage_folder,
        pending: Arc::clone(pending),
        loop_definition,
    };
    match action {
        IdentifyAction::Identify { settings } => {
            refuse_while_journaled(&journal)?;
            let connection = connection.ok_or_else(runtime_required)?;
            let settings =
                checked_acquisition_settings(&work.loop_definition, &to_value(&settings)?)?;
            // Settings that cannot configure the loop are refused before anything starts; the run configures it again.
            configured_acquisition(&work.loop_definition, Some(&settings))
                .map_err(|error| error.failure())?;
            started(operations, claim, move |context| {
                run_identify(context, &work, &connection, &settings)
            })
        }
        IdentifyAction::OpenRecording { recording_id } => {
            refuse_while_journaled(&journal)?;
            let section = working_study(&work)?.section::<IdentifySection>()?;
            section
                .ok_or_else(|| refit_refusal(&recording_id))?
                .refit_request(&recording_id)?;
            started(operations, claim, move |context| {
                run_refit(context, &work, &recording_id)
            })
        }
        IdentifyAction::Resume => {
            let connection = connection.ok_or_else(runtime_required)?;
            // The campaign's method is read before the resume clears the journal.
            let kind = journaled_kind(&journal);
            started(operations, claim, move |context| {
                run_resume(context, &work, &connection, kind)
            })
        }
        // Reset answers at once; the loop's claim ends when this answer returns.
        IdentifyAction::Reset => reset(&work, &journal),
        // The review answers at once, with its claim, and starts nothing (B3.2 to B3.4, C12).
        IdentifyAction::Review { settings } => {
            review(&work.loop_definition, connection.as_ref(), &settings)
        }
    }
}

/// The name of the operation an identification request starts, as a refusal names the operation that holds the loop (A8.5).
fn operation_name(action: &IdentifyAction) -> &'static str {
    match action {
        IdentifyAction::Identify { .. } => "Identify plant",
        IdentifyAction::OpenRecording { .. } => "Open recording",
        IdentifyAction::Resume => "Resume",
        IdentifyAction::Reset => "Reset",
        IdentifyAction::Review { .. } => "Review",
    }
}

/// The review of the campaign that `identify` would run with `settings` (§0.1a, B3.2 to B3.4, C12): the loop's live sample is read
/// when a runtime is connected, so the mode Automatic chooses from is the controller's. Without a connection the sample is absent,
/// and the review says what the campaign needs. It starts nothing and writes nothing.
///
/// # Errors
/// The runtime cannot be reached for its sample; or the campaign's own checks refuse the settings (trust-control's
/// `campaign_review`, with the next action it names).
fn review(
    loop_definition: &LoopDefinition,
    connection: Option<&RuntimeConnection>,
    settings: &AcquisitionSettings,
) -> Result<Value, Failure> {
    let sample = match connection {
        Some(connection) => {
            let source = runtime_source(connection)?;
            RuntimeSampler::new(connection, loop_definition, source).sample()?
        }
        None => None,
    };
    to_value(&campaign_review(
        loop_definition,
        settings,
        sample.as_ref(),
    )?)
}

/// The identification stage of a run (B8): the live campaign of `controlStudio/identify` with `settings`, inside the run's own
/// operation. It refuses what `identify` refuses while the loop's journal holds an operation (A8.5), and it claims nothing, because
/// the run holds the loop. Its Cancel reaches the campaign through `context`.
///
/// # Errors
/// The journal cannot be read or holds an unfinished operation (A8.5); the limits refuse the settings (B3.4 step 1); or the campaign
/// fails (see `run_identify`).
pub(crate) fn run_stage(
    context: &OperationContext,
    project_root: &Path,
    storage_folder: &Path,
    pending: &Arc<PendingReleases>,
    loop_definition: &LoopDefinition,
    connection: &RuntimeConnection,
    settings: &AcquisitionSettings,
) -> Result<Value, Failure> {
    let journal = Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    refuse_while_journaled(&journal)?;
    // The campaign's own limits are checked before anything starts (B3.4 step 1): a missing limit is the run's input to ask for.
    let configured =
        configured_acquisition(loop_definition, Some(settings)).map_err(|error| error.failure())?;
    require_campaign_limits(&configured)?;
    campaign_maximum_ms(&configured)?;
    let work = LoopWork {
        project_root: project_root.to_path_buf(),
        storage_folder: storage_folder.to_path_buf(),
        pending: Arc::clone(pending),
        loop_definition: loop_definition.clone(),
    };
    run_identify(context, &work, connection, settings)
}

/// What `cancel_loop` cancels (§0.1b `cancel`): the runtime it reattaches through (E11.5), the project and the folder of the
/// journals, the loop, and the journaled operation, by its runtime identifier.
#[derive(Debug, Clone, Copy)]
pub struct CancelRequest<'a> {
    /// The runtime the operation reattaches through (E11.1).
    pub connection: &'a RuntimeConnection,
    /// The project folder, where the study of the loop is.
    pub project_root: &'a Path,
    /// The folder of the operation journals (A8.3).
    pub storage_folder: &'a Path,
    /// The loop whose journal holds the operation.
    pub loop_definition: &'a LoopDefinition,
    /// The runtime identifier of the operation to cancel, as `open --loop` lists it.
    pub operation_id: &'a str,
}

/// Starts the Cancel of one operation journaled for the loop (§0.1b `cancel`): an operation `cancel` on this process's
/// `operations`, whose work is [`cancel_operation`]. A running experiment of a campaign is aborted through the runtime's
/// restoration path and an ended one has its restoration confirmed (E5.5); a lone recording owner is stopped and its recording
/// kept (E11.6). The final update's result is a [`CancelResult`]. The other operations of the journal stay as they are. The
/// operation reattaches by its journaled identity and never cancels an operation of this process. The loop is claimed in
/// `claims` until the Cancel ends.
///
/// # Errors
/// The operation is not journaled for the loop, which is refused before anything starts; an operation of this engine works on
/// the loop; or the loop's journal or its working study cannot be read.
pub fn cancel_loop(
    operations: &Operations,
    pending: &Arc<PendingReleases>,
    claims: &LoopClaims,
    request: CancelRequest<'_>,
) -> Result<OperationStarted, Failure> {
    let CancelRequest {
        connection,
        project_root,
        storage_folder,
        loop_definition,
        operation_id,
    } = request;
    let claim = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        &format!("Cancel of {operation_id}"),
    )?;
    let journal = Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    if !journal
        .entries()
        .iter()
        .any(|entry| entry.operation_id() == operation_id)
    {
        return Err(not_journaled(&loop_definition.id, operation_id));
    }
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let work = LoopWork {
        project_root: project_root.to_path_buf(),
        storage_folder: storage_folder.to_path_buf(),
        pending: Arc::clone(pending),
        loop_definition: loop_definition.clone(),
    };
    let connection = connection.clone();
    let operation_id = operation_id.to_string();
    Ok(operations.start(
        CANCEL_COMMAND,
        claimed(claim, move |_| {
            run_cancel(&work, &connection, study, &operation_id)
        }),
    ))
}

/// Starts `work` as an Identify operation that holds `claim` until it ends, and answers with its identifier.
fn started(
    operations: &Operations,
    claim: LoopClaim,
    work: impl FnOnce(&OperationContext) -> Result<Value, Failure> + Send + 'static,
) -> Result<Value, Failure> {
    to_value(&operations.start(IDENTIFY_COMMAND, claimed(claim, work)))
}

/// `work` holding the loop's `claim` until it ends. The claim ends before the operation reports its end; a panic of the work
/// ends it too, because the claim is dropped with the work.
fn claimed(
    claim: LoopClaim,
    work: impl FnOnce(&OperationContext) -> Result<Value, Failure> + Send + 'static,
) -> impl FnOnce(&OperationContext) -> Result<Value, Failure> + Send + 'static {
    move |context| {
        let worked = work(context);
        drop(claim);
        worked
    }
}

/// The live campaign (B3.4): the run on the runtime with the production sampler and the study's retention, then the Identify
/// section the study keeps.
fn run_identify(
    context: &OperationContext,
    work: &LoopWork,
    connection: &RuntimeConnection,
    settings: &AcquisitionSettings,
) -> Result<Value, Failure> {
    let loop_definition = &work.loop_definition;
    let source = runtime_source(connection)?;
    let mut sampler = RuntimeSampler::new(connection, loop_definition, source);
    let mut retention = StudyRetention::new(
        &work.project_root,
        &loop_definition.id,
        source,
        working_study(work)?,
    );
    let outcome = identify_plant(
        context,
        connection,
        &work.pending,
        &work.storage_folder,
        &run_request(loop_definition),
        Some(settings),
        IdentifyPorts {
            sampler: &mut sampler,
            retention: &mut retention,
        },
    )?;
    let RunResult::Identified { result, .. } = outcome.result else {
        return Err(model_check_instead_of_model());
    };
    let recording_file_id = kept_file(&retention, &outcome.recording_id)?;
    keep_result(
        work,
        retention.study(),
        Fitted {
            method: IdentifyMethod::Live(outcome.method.kind),
            source,
            recording_file_id,
            result,
        },
    )
}

/// The journaled campaign followed to its end (A8.5, A8.6), its model qualified for the loop as an identified model is, then
/// the Identify section the study keeps. `kind` is the experiment the journal named before the resume.
fn run_resume(
    context: &OperationContext,
    work: &LoopWork,
    connection: &RuntimeConnection,
    kind: Option<ControlExperimentKind>,
) -> Result<Value, Failure> {
    let loop_definition = &work.loop_definition;
    let source = runtime_source(connection)?;
    let mut retention = StudyRetention::new(
        &work.project_root,
        &loop_definition.id,
        source,
        working_study(work)?,
    );
    let resumed = resume_identification(
        context,
        connection,
        &work.pending,
        &work.storage_folder,
        &run_request(loop_definition),
        &mut retention,
    )?;
    qualification(loop_definition, &resumed.result)?;
    let kind = kind.ok_or_else(resumed_without_experiment)?;
    let recording_file_id = kept_file(&retention, &resumed.recording_id)?;
    keep_result(
        work,
        retention.study(),
        Fitted {
            method: IdentifyMethod::Live(kind),
            source,
            recording_file_id,
            result: resumed.result,
        },
    )
}

/// The recording of the study's Identify result fitted again, without live excitation (B3.6): the kept file, checked against
/// the section (B10.3), becomes the connected record of the request the section keeps, from the fit window that request was
/// taken with; it meets the same limits check (B3.4 step 6), is fitted and qualified, and the section is kept again with the
/// same request.
fn run_refit(
    context: &OperationContext,
    work: &LoopWork,
    recording_id: &str,
) -> Result<Value, Failure> {
    let loop_definition = &work.loop_definition;
    let study = working_study(work)?;
    let Some(stored) = storage::read_identify(&work.project_root, &study)? else {
        return Err(refit_refusal(recording_id));
    };
    let request = &stored.section.refit_request(recording_id)?.request;
    let evaluate: RunEvaluator<'_, RecordedLimitsEvaluation> = &evaluate_recorded_limits;
    let record = connected_record(
        loop_definition,
        &stored.estimation.run,
        None,
        &request.experiment,
        stored_fit_window(request),
        evaluate,
    )?;
    let result = fit_records(loop_definition, context, std::slice::from_ref(&record))?;
    qualification(loop_definition, &result)?;
    keep_result(
        work,
        &study,
        Fitted {
            method: stored.section.method,
            source: stored.section.source,
            recording_file_id: stored.section.estimation_recording_id.clone(),
            result,
        },
    )
}

/// The Cancel of the journaled operation `operation_id`, keeping its recording in `study` before the runtime buffer is released
/// (E5.5, E11.6). Its result names the operation cancelled (§0.1b).
fn run_cancel(
    work: &LoopWork,
    connection: &RuntimeConnection,
    study: Study,
    operation_id: &str,
) -> Result<Value, Failure> {
    let loop_definition = &work.loop_definition;
    let source = runtime_source(connection)?;
    let mut retention = StudyRetention::new(&work.project_root, &loop_definition.id, source, study);
    cancel_operation(
        connection,
        &work.pending,
        &work.storage_folder,
        loop_definition,
        operation_id,
        &mut retention,
    )?;
    to_value(&CancelResult {
        cancelled: vec![operation_id.to_string()],
    })
}

/// Reset (A8.4): the working study moves to `history/` with every section and recording it holds, and a new working study
/// starts. The loop definition, with its signals and limits, is not touched.
fn reset(work: &LoopWork, journal: &Journal) -> Result<Value, Failure> {
    refuse_while_journaled(journal)?;
    let fresh = storage::start_new_study(&work.project_root, &working_study(work)?)?;
    let evidence = study_evidence(&work.loop_definition, None, !fresh.recordings.is_empty());
    to_value(&ResetResult {
        stages: stage_availability(&evidence),
    })
}

/// Keeps the Identify section of `fitted` in `study` (A8.1, A8.3; B3.5): the provisional model, its presentation, the signals
/// it used and the request its recording was taken with. Answers with the result the page shows and the stages after it.
fn keep_result(work: &LoopWork, study: &Study, fitted: Fitted) -> Result<Value, Failure> {
    let Fitted {
        method,
        source,
        recording_file_id,
        result,
    } = fitted;
    let loop_definition = &work.loop_definition;
    let AutomaticIdentificationResult::Model {
        model,
        runs,
        presentation,
        ..
    } = result
    else {
        return Err(accepted_model_from_one_record());
    };
    let [record] = runs.as_slice() else {
        return Err(model_not_from_one_record());
    };
    let section = IdentifySection::new(
        method,
        source,
        identify_signals(loop_definition, experiment_kind(method)),
        &recording_file_id,
        None,
        *model,
        *presentation,
    )
    .with_record_request(StoredRecordRequest {
        project_revision: record.project_revision.clone(),
        request: record.request.clone(),
    });
    let kept = storage::save_identify(&work.project_root, study, &section, &[])?;
    let evidence = study_evidence(loop_definition, Some(&section), !kept.recordings.is_empty());
    to_value(&identify_result(&section, stage_availability(&evidence)))
}

/// Refuses an action while the loop's journal holds an operation that runs or whose outcome is unconfirmed (A8.5).
fn refuse_while_journaled(journal: &Journal) -> Result<(), Failure> {
    unfinished_operation_refusal(&journaled_ids(journal)).map_or(Ok(()), Err)
}

/// The runtime identities of the operations the loop's journal holds.
fn journaled_ids(journal: &Journal) -> Vec<&str> {
    journal
        .entries()
        .iter()
        .filter(|entry| entry.holds_unfinished_operation())
        .map(JournalEntry::operation_id)
        .collect()
}

/// The experiment kind of the step the loop's journal holds: the method of the campaign a resume follows (B3.3).
fn journaled_kind(journal: &Journal) -> Option<ControlExperimentKind> {
    journal.entries().iter().find_map(|entry| {
        if let JournalEntry::IdentificationCampaign { campaign } = entry {
            campaign.plan.as_ref().map(|plan| plan.kind)
        } else {
            None
        }
    })
}

/// The experiment a recording was taken with.
fn experiment_kind(method: IdentifyMethod) -> ControlExperimentKind {
    match method {
        IdentifyMethod::Live(kind) | IdentifyMethod::Opened(kind) => kind,
    }
}

/// What a run identifies with in production: the loop, its declared expected settling time (B1.4; the baseline window is a tenth
/// of it, B3.4 step 2, when the loop declares one), and the limits check of B3.4 step 6.
fn run_request(loop_definition: &LoopDefinition) -> RunRequest<'_, RecordedLimitsEvaluation> {
    RunRequest {
        loop_definition,
        expected_settling_seconds: loop_definition.limits.expected_settling_seconds,
        evaluate: &evaluate_recorded_limits,
    }
}

/// Where the values of the runtime of `connection` come from (A2.4, A9), read when an operation starts.
fn runtime_source(connection: &RuntimeConnection) -> Result<EvidenceSource, Failure> {
    Ok(RuntimeClient::connect(connection)?.status()?.source())
}

/// The loop's working study (A8.2).
fn working_study(work: &LoopWork) -> Result<Study, Failure> {
    storage::open_study(
        &work.project_root,
        &work.loop_definition.id,
        &work.loop_definition.revision,
    )
}

/// The recording file the runtime recording `recording_id` became when the run kept it.
fn kept_file(retention: &StudyRetention<'_>, recording_id: &str) -> Result<String, Failure> {
    retention
        .kept_file_id(recording_id)
        .map(str::to_string)
        .ok_or_else(|| recording_not_kept(recording_id))
}

#[cfg(test)]
#[path = "identify_action_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "identify_review_tests.rs"]
mod review_tests;
