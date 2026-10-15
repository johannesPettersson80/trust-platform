//! The Identify method `controlStudio/identify` (specification 35, §0.1a, A8.3 to A8.6 and B3; Identify page specification)
//! and the engine's cancel of one journaled operation of a loop (§0.1b, `cancel`).
//!
//! - `identify` runs a live campaign with the engineer's method and limits (B3.2 to B3.4), `openRecording` fits the recording
//!   of the study's Identify result again without live excitation (B3.6), and `resume` follows the journaled campaign to its
//!   end (A8.5, A8.6). Each answers at once with its operation's identifier, except an opened recording that pairs or needs a
//!   second recording, which is answered from the study (B5.2, `identify_import`). Its final update carries trust-control's
//!   `IdentifyResult`, once the study keeps the Identify section with the request its recording was taken with.
//! - `reset` starts a new working study and keeps the previous one as history (A8.4), and answers at once with the stages.
//! - `importRecording` checks a saved result or recording file and keeps it as a recording of the study with its source and the
//!   opened marker; `openRecording` fits such a recording from its own samples (B3.6, B10.2). `importModel` checks a supplied
//!   plant model and keeps it as the study's model (B3.7). Both answer at once (`identify_import`, B10.4).
//! - `cancel_loop` cancels the journaled operation the command line names (§0.1b, E5.5, E11.6): a campaign through its
//!   restoration path, a lone recording owner by stopping and keeping its recording. Its final update is a [`CancelResult`].
//!
//! What needs no runtime is checked before anything starts: the loop opens, the loop's journal holds no operation (for
//! `identify`, `openRecording` and `reset`), the engineer's acquisition settings, and the recording a refit names. `identify`
//! and `resume` reach the runtime, so they are refused at once without a connection (E11.1). The review and the approval of an
//! `identify` start need the runtime's sample, so they are checked inside its operation, and a refusal ends that operation
//! (§0.1a). Every rule and refusal is trust-control's; the workflow makes the runtime calls, and the storage writes the files.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::experiment::ControlExperimentKind;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::acquisition_settings::{
    configured_acquisition, AcquisitionSettings,
};
use trust_control::identification::automatic::AutomaticIdentificationResult;
use trust_control::identification::campaign::{campaign_maximum_ms, require_campaign_limits};
use trust_control::identification::campaign_review::{campaign_review, IdentifyReview};
use trust_control::identification::connected_run::{
    accepted_model_from_one_record, checked_acquisition_settings, live_write_refusal,
    model_check_instead_of_model, model_not_from_one_record, not_journaled, qualification,
    recording_not_kept, resumed_without_experiment, review_changed_refusal, runtime_required,
    stored_fit_window, unfinished_operation_refusal, FitWindow, IDENTIFY_OPERATION,
};
use trust_control::identification::identify_result::{
    identify_answer, identify_signals, study_evidence,
};
use trust_control::identification::recorded_limits::{
    evaluate_recorded_limits, RecordedLimitsEvaluation,
};
use trust_control::identification::recording_import::archive::with_extra_channels;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{
    CancelResult, IdentifyAction, IdentifyRequest, OperationStarted, ResetResult,
};
use trust_control::recording::opened::opened_plan;
use trust_control::recording::opened_channels::{
    configured_recording_selection, opened_recording_catalog, selection_loop, workflow_units,
    RecordingSignalCatalog, RecordingSignalSelection,
};
use trust_control::recording::{EvidenceSource, RecordingFile};
use trust_control::runtime::RuntimeConnection;
use trust_control::stages::stage_availability;
use trust_control::study::current_model::CurrentModel;
use trust_control::study::identify::{
    refit_refusal, IdentifyMethod, IdentifySection, IdentifySignals, StoredRecordRequest,
};
use trust_control::study::inputs::stage_evidence_of;
use trust_control::study::Study;

use crate::busy_loops::{LoopClaim, LoopClaims};
use crate::commands::to_value;
use crate::identify_adapters::{RuntimeSampler, StudyRetention};
use crate::identify_import;
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
        IdentifyAction::Identify {
            settings,
            review,
            physical_approved,
        } => {
            refuse_while_journaled(&journal)?;
            let connection = connection.ok_or_else(runtime_required)?;
            let settings =
                checked_acquisition_settings(&work.loop_definition, &to_value(&settings)?)?;
            // Settings that cannot configure the loop are refused before anything starts; the run configures it again.
            configured_acquisition(&work.loop_definition, Some(&settings))
                .map_err(|error| error.failure())?;
            // The review and the approval read the runtime's sample, so they are checked inside the operation (§0.1a): the
            // answer comes at once, and a refused start ends the operation with its refusal before anything is written.
            started(operations, claim, move |context| {
                confirm_start(
                    &work.loop_definition,
                    &connection,
                    &settings,
                    review.as_deref(),
                    physical_approved,
                )?;
                run_identify(context, &work, &connection, &settings)
            })
        }
        IdentifyAction::OpenRecording {
            recording_id,
            inputs,
            outputs,
        } => {
            refuse_while_journaled(&journal)?;
            open_recording_answer(operations, claim, work, recording_id, inputs, outputs)
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
        // The check answers at once, with its claim: a file that passes is kept as a recording of the study (B3.6, B10.2, B10.4).
        IdentifyAction::ImportRecording { json } => {
            refuse_while_journaled(&journal)?;
            identify_import::import_recording(&work.project_root, &work.loop_definition, &json)
        }
        // The model check answers at once, with its claim; a model that passes is kept as the study's model (B3.7, N4.1).
        IdentifyAction::ImportModel { json } => {
            refuse_while_journaled(&journal)?;
            identify_import::import_model(&work.project_root, &work.loop_definition, &json)
        }
    }
}

/// `openRecording` (B3.6, B5.2). `recording_id` names the recording; without it the study's last opened recording is fitted
/// (`last_opened_recording`). An opened recording the study keeps is answered from the study when it pairs or needs a second
/// recording; otherwise it is fitted from its own samples with the signals the engineer selected. Any other recording is fitted again
/// from the request its Identify result keeps.
fn open_recording_answer(
    operations: &Operations,
    claim: LoopClaim,
    work: LoopWork,
    recording_id: Option<String>,
    inputs: Option<Vec<String>>,
    outputs: Option<Vec<String>>,
) -> Result<Value, Failure> {
    let recording_id = match recording_id {
        Some(recording_id) => recording_id,
        None => last_opened_recording(&work)?,
    };
    if let Some(file) = opened_recording_of_study(&work, &recording_id)? {
        let catalog = opened_catalog(&work.loop_definition, &file, inputs, outputs)?;
        if let Some(answer) = identify_import::open_kept_recording(
            &work.project_root,
            &work.loop_definition,
            &recording_id,
            &file,
            &catalog,
        )? {
            return Ok(answer);
        }
        let selection = catalog.selection;
        return started(operations, claim, move |context| {
            run_opened_fit(context, &work, &file, &selection)
        });
    }
    let section = working_study(&work)?.section::<IdentifySection>()?;
    section
        .ok_or_else(|| refit_refusal(&recording_id))?
        .refit_request(&recording_id)?;
    started(operations, claim, move |context| {
        run_refit(context, &work, &recording_id)
    })
}

/// The recording `openRecording` fits when it names none: the study's last opened recording, the latest of the recordings the study
/// lists whose file carries the opened marker (B3.6). A study that keeps none refuses, and the engineer opens a recording first.
///
/// # Errors
/// The study keeps no opened recording, or a recording the study lists cannot be read.
fn last_opened_recording(work: &LoopWork) -> Result<String, Failure> {
    let study = working_study(work)?;
    for id in study.recordings.iter().rev() {
        let file = storage::read_recording(&work.project_root, &work.loop_definition.id, id)?;
        if file.opened {
            return Ok(id.clone());
        }
    }
    Err(Failure::new(
        "The study keeps no opened recording to fit. Open a recording first.",
        Cause::Settings,
        NextAction::None,
    ))
}

/// The name of the operation an identification request starts, as a refusal names the operation that holds the loop (A8.5).
fn operation_name(action: &IdentifyAction) -> &'static str {
    match action {
        IdentifyAction::Identify { .. } => "Identify plant",
        IdentifyAction::OpenRecording { .. } => "Open recording",
        IdentifyAction::Resume => "Resume",
        IdentifyAction::Reset => "Reset",
        IdentifyAction::Review { .. } => "Review",
        IdentifyAction::ImportRecording { .. } => "Import recording",
        IdentifyAction::ImportModel { .. } => "Import model",
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

/// The engine's check of a start that `controlStudio/identify` runs inside its operation (§0.1a, A2.5, E11.8): the campaign the
/// start would run now, on the live target, must be the one the engineer reviewed, and on a physical target the engineer must
/// have approved it. A refused start writes nothing, and its operation ends with the refusal.
///
/// # Errors
/// The runtime cannot be read for its sample; the campaign's own checks refuse the settings; or [`start_refusal`] refuses the start.
fn confirm_start(
    loop_definition: &LoopDefinition,
    connection: &RuntimeConnection,
    settings: &AcquisitionSettings,
    review: Option<&str>,
    physical_approved: bool,
) -> Result<(), Failure> {
    let source = runtime_source(connection)?;
    let sample = RuntimeSampler::new(connection, loop_definition, source).sample()?;
    let reviewed = campaign_review(loop_definition, settings, sample.as_ref())?;
    match start_refusal(&reviewed, review, physical_approved) {
        Some(refusal) => Err(refusal),
        None => Ok(()),
    }
}

/// The refusal of a start, from the review the engineer saw and the approval the start carries (§0.1a, A2.5, E11.8). A review
/// that names another identity than the campaign reviewed now is refused. On a physical target, which the reviewed campaign
/// shows with an approval block, the start needs both the review and the engineer's approval; the approval covers that identity
/// and nothing else. A start that names no review passes on a simulated target, where pressing the action is the confirmation.
/// `None` when the start may run.
fn start_refusal(
    reviewed: &IdentifyReview,
    review: Option<&str>,
    physical_approved: bool,
) -> Option<Failure> {
    if review.is_some_and(|identity| identity != reviewed.identity) {
        return Some(review_changed_refusal());
    }
    if physical_approved && review.is_some() {
        return None;
    }
    let source = if reviewed.approval.is_some() {
        EvidenceSource::PhysicalPlant
    } else {
        EvidenceSource::SimulatedPlant
    };
    live_write_refusal(source, IDENTIFY_OPERATION)
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
    let evidence = stage_evidence_of(&work.loop_definition, &fresh)?;
    to_value(&ResetResult {
        stages: stage_availability(&evidence),
        stage_evidence: evidence,
    })
}

/// The recording `recording_id` of the study, when the study keeps it as an opened recording (B3.6): a recording the study lists,
/// whose file carries the opened marker. `None` for any other recording.
///
/// # Errors
/// The study cannot be read, or a recording the study lists cannot be read.
fn opened_recording_of_study(
    work: &LoopWork,
    recording_id: &str,
) -> Result<Option<RecordingFile>, Failure> {
    let study = working_study(work)?;
    if !study.recordings.iter().any(|listed| listed == recording_id) {
        return Ok(None);
    }
    let file = storage::read_recording(&work.project_root, &work.loop_definition.id, recording_id)?;
    Ok(file.opened.then_some(file))
}

/// The fit of an opened recording the study keeps (B3.6, C12). Its samples are observed as `opened_plan` describes them: the
/// file is an observation, so nothing is written to the plant. The fit meets the same limits check as a live recording (B3.4
/// step 6), is fitted and qualified, and is kept with the recording's source. The Identify result keeps the observation it was
/// fitted with, so refitting the recording reads the same plan.
fn run_opened_fit(
    context: &OperationContext,
    work: &LoopWork,
    file: &RecordingFile,
    selection: &RecordingSignalSelection,
) -> Result<Value, Failure> {
    let loop_definition = &work.loop_definition;
    // The study keeps the selection, so that reopening shows the signals the fit used (B3.6).
    let study = identify_import::with_selection(working_study(work)?, selection)?;
    let (input, output) = single_channel_selection(selection)?;
    // The fit reads the selected plant input and measured output from the recording, as the view of the loop with those signals
    // (B3.6). A selected output that the loop does not bind is an extra channel of the recording, which the record must carry too.
    let view = selection_loop(loop_definition, &file.run, input, output)?;
    let plan = opened_plan(file, loop_definition);
    let window = FitWindow {
        fit_from_seconds: 0.0,
        baseline_end_seconds: 0.0,
    };
    let evaluate: RunEvaluator<'_, RecordedLimitsEvaluation> = &evaluate_recorded_limits;
    let mut record = connected_record(loop_definition, &file.run, None, &plan, window, evaluate)?;
    with_extra_channels(&mut record.run, file);
    let result = fit_records(&view, context, std::slice::from_ref(&record))?;
    // The model is qualified against the signals it was fitted to, which are the view's (B3.6).
    qualification(&view, &result)?;
    // The channels the fit read from the recording are kept beside the loop's signals (B3.6).
    let read = IdentifySignals {
        inputs: vec![input.to_string()],
        outputs: vec![output.to_string()],
        setpoint: None,
    };
    keep_fitted(
        work,
        &study,
        Fitted {
            method: IdentifyMethod::Opened(ControlExperimentKind::Observation),
            source: file.source,
            recording_file_id: file.id()?,
            result,
        },
        Some(read),
    )
}

/// The catalogue of an opened recording with the selection an `openRecording` names (B3.6): the plant input and measured output the
/// engineer chose, and the loop's own for a signal the request leaves out. A name the recording does not hold is refused
/// (`opened_recording_catalog`).
fn opened_catalog(
    loop_definition: &LoopDefinition,
    file: &RecordingFile,
    inputs: Option<Vec<String>>,
    outputs: Option<Vec<String>>,
) -> Result<RecordingSignalCatalog, Failure> {
    let requested = if inputs.is_none() && outputs.is_none() {
        None
    } else {
        let configured = configured_recording_selection(loop_definition);
        Some(json!({
            "inputs": inputs.unwrap_or(configured.inputs),
            "outputs": outputs.unwrap_or(configured.outputs),
        }))
    };
    opened_recording_catalog(
        &workflow_units(loop_definition),
        loop_definition,
        std::slice::from_ref(&file.run),
        requested.as_ref(),
    )
}

/// The plant input and measured output a fit of one recording takes (B3.6): one of each. Several signals wait for the state model
/// fit (V7), and a selection without either names nothing to fit.
fn single_channel_selection(selection: &RecordingSignalSelection) -> Result<(&str, &str), Failure> {
    match (selection.inputs.as_slice(), selection.outputs.as_slice()) {
        ([input], [output]) => Ok((input.as_str(), output.as_str())),
        ([], _) | (_, []) => Err(Failure::new(
            "Choose the recording's plant inputs and measured outputs.",
            Cause::Settings,
            NextAction::None,
        )),
        _ => Err(Failure::new(
            "Identify fits one plant input and one measured output of a recording; several signals wait for the state model fit (V7).",
            Cause::Settings,
            NextAction::None,
        )),
    }
}

/// Keeps the Identify section of `fitted` in `study` with the loop's own signals (A8.1, A8.3; B3.5): the provisional model, its
/// presentation and the request its recording was taken with.
fn keep_result(work: &LoopWork, study: &Study, fitted: Fitted) -> Result<Value, Failure> {
    keep_fitted(work, study, fitted, None)
}

/// Keeps the Identify section of `fitted` in `study`: the provisional model, its presentation and the request its recording was
/// taken with. Its signals are the loop's for the fit's experiment (A8.4, B3.2), and `read` names the recording channels the fit
/// read the plant input and measured output from, when it read other ones (B3.6). A state-space fit of several channels keeps its
/// model with its signals and no presentation (B10.2). Answers with the result the page shows and the stages after it.
fn keep_fitted(
    work: &LoopWork,
    study: &Study,
    fitted: Fitted,
    read: Option<IdentifySignals>,
) -> Result<Value, Failure> {
    let Fitted {
        method,
        source,
        recording_file_id,
        result,
    } = fitted;
    let loop_definition = &work.loop_definition;
    let (model, runs, presentation) = match result {
        AutomaticIdentificationResult::Model {
            model,
            runs,
            presentation,
            ..
        } => (model, runs, Some(presentation)),
        AutomaticIdentificationResult::StateModel { model, runs, .. } => (model, runs, None),
        AutomaticIdentificationResult::Accepted { .. } => {
            return Err(accepted_model_from_one_record());
        }
    };
    let [record] = runs.as_slice() else {
        return Err(model_not_from_one_record());
    };
    let signals = identify_signals(loop_definition, experiment_kind(method));
    let section = match presentation {
        Some(presentation) => IdentifySection::new(
            method,
            source,
            signals,
            &recording_file_id,
            None,
            *model,
            *presentation,
        ),
        None => {
            IdentifySection::new_state_model(method, source, signals, &recording_file_id, *model)
        }
    }
    .with_record_request(StoredRecordRequest {
        project_revision: record.project_revision.clone(),
        request: record.request.clone(),
    });
    let section = match read {
        Some(read) => section.with_recorded_channels(read),
        None => section,
    };
    let kept = storage::save_identify(&work.project_root, study, &section, &[])?;
    let current = CurrentModel::identified(section);
    let evidence = study_evidence(loop_definition, Some(&current), !kept.recordings.is_empty());
    let answer = identify_answer(&kept, loop_definition, &current, evidence)?;
    to_value(&answer)
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
