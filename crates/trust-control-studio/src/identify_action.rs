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
//! `identify`, `openRecording` and `reset`), the method of an `identify` start is one the loop can run (the Method list's own
//! decision, B3.2, B3.3), the engineer's acquisition settings, and the recording a refit names. `identify`
//! and `resume` reach the runtime, so they are refused at once without a connection (E11.1). The review and the approval of an
//! `identify` start need the runtime's sample and, for a project step test, the hold the runtime applies (C12), so they are checked
//! inside its operation, and a refusal ends that operation (§0.1a). Every rule and refusal is trust-control's; the workflow makes
//! the runtime calls, and the storage writes the files.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use trust_control::experiment::ControlExperimentKind;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::acquisition_settings::{
    configured_acquisition, unreadable_duration_failure, AcquisitionMethod, AcquisitionSettings,
};
use trust_control::identification::campaign::{campaign_maximum_ms, require_campaign_limits};
use trust_control::identification::campaign_review::campaign_settings;
use trust_control::identification::connected_run::{
    checked_acquisition_settings, model_check_instead_of_model, not_journaled, qualification,
    recording_not_kept, resumed_without_experiment, runtime_required, unfinished_operation_refusal,
};
use trust_control::identification::identify_result::project_test_result;
use trust_control::identification::method_offers::identify_method_offers;
use trust_control::identification::policy::project_scripted;
use trust_control::identification::project_test::{project_test_outcome, settings_released};
use trust_control::identification::recorded_limits::{
    evaluate_recorded_limits, RecordedLimitsEvaluation,
};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{
    AnswerKind, CampaignRestoration, CancelResult, IdentifyAction, IdentifyRequest,
    OperationStarted, RecordedChartSample, ResetResult,
};
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::simulator_start::runtime_not_running;
use trust_control::stages::stage_availability;
use trust_control::study::identify::IdentifyMethod;
use trust_control::study::inputs::stage_evidence_of;
use trust_control::study::Study;

use crate::application_state::{refuse_while_restore_waits, ApplicationState};
use crate::busy_loops::{LoopClaim, LoopClaims};
use crate::commands::to_value;
use crate::identify_adapters::{RuntimeSampler, StudyRetention};
use crate::identify_import;
use crate::identify_schedule;
use crate::journal::{Journal, JournalEntry};
use crate::operation::{OperationContext, Operations};
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::runtime_client::RuntimeClient;
use crate::workflow::identify::{
    identify_plant, record_for_manual_check, IdentifyOutcome, IdentifyPorts,
    IdentifyRequest as RunRequest, IdentifyResult as RunResult,
};
use crate::workflow::identify_recovery::{cancel_operation, resume_identification};
use crate::{loops, storage};

/// The command of the operations that `identify`, `openRecording` and `resume` start (§0.1a).
const IDENTIFY_COMMAND: &str = "identify";
/// The command of the operation that cancels a loop's journaled identification (§0.1b, `cancel`).
const CANCEL_COMMAND: &str = "cancel";
/// The sentence of the Identify page's status line after a reset (A8.4, B9.9).
const NEW_STUDY_TEXT: &str = "New identification study started.";

/// The manual identification (the Recorded-data fit of B3.3, B3.6 and B3.7): its checks, fit, structure, acceptance and reopening.
#[path = "identify_manual.rs"]
pub(crate) mod identify_manual;

/// The opened and imported recording paths (B3.6, B5.2, B10.4) and the keeping of a fitted model as the study's Identify section.
#[path = "identify_recording.rs"]
mod identify_recording;

/// The review of a campaign and the check of a start against it (§0.1a, C12), which read the runtime.
#[path = "identify_review.rs"]
mod identify_review;

use identify_recording::{import_recording_answer, keep_result, open_recording_answer, Fitted};
use identify_review::{confirm_start, review};

/// What an operation of the loop works with: owned, so that it moves to the operation's thread. The runtime connection is not
/// part of it: an action that reaches the runtime passes its own connection (E11.1).
struct LoopWork {
    project_root: PathBuf,
    storage_folder: PathBuf,
    pending: Arc<PendingReleases>,
    loop_definition: LoopDefinition,
}

/// Answers `controlStudio/identify`. `identify`, `openRecording` and `resume` start an operation on `operations` and answer
/// with its identifier at once; `reset` answers with the stages of the new working study. Each action first claims the loop
/// in `claims` (one operation per loop at a time); an operation holds the claim until it ends, and a refused request lets it
/// go at once. The runtime recordings the operations stop are released through `pending`, the engine's one list (E4.7).
///
/// # Errors
/// The loop cannot be opened, an operation of this engine works on it, or its journal cannot be read; `identify`,
/// `openRecording` or `reset` while the loop's journal holds an operation; `identify` with a method the loop cannot run
/// (`require_applicable_method`); `identify` or `resume` without a runtime connection
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
        runtime_mode,
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
            // A project test carries no settings: it runs the loop's declared experiment (C12).
            let settings = campaign_settings(&work.loop_definition, settings)?;
            require_applicable_method(&work.loop_definition, &settings)?;
            let connection =
                connection.ok_or_else(|| runtime_not_running(runtime_mode.as_deref()))?;
            let settings =
                checked_acquisition_settings(&work.loop_definition, &to_value(&settings)?)?;
            // Settings that cannot configure the loop are refused before anything starts; the run configures it again.
            configured_acquisition(&work.loop_definition, Some(&settings))
                .map_err(|error| error.failure())?;
            // The review and the approval read the runtime's sample, so they are checked inside the operation (§0.1a): the
            // answer comes at once, and a refused start ends the operation with its refusal before anything is written.
            started(operations, claim, move |context| {
                let scheduling_value = confirm_start(
                    &work.loop_definition,
                    &connection,
                    &settings,
                    review.as_deref(),
                    physical_approved,
                )?;
                run_identify(context, &work, &connection, &settings, scheduling_value)
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
            let settings = campaign_settings(&work.loop_definition, settings)?;
            // The review does not check the other settings' ranges, but a time that no duration reads is refused under its field.
            if settings.duration_seconds.is_nan() {
                return Err(unreadable_duration_failure());
            }
            review(
                &work.loop_definition,
                connection.as_ref(),
                runtime_mode.as_deref(),
                &settings,
            )
        }
        // The check answers at once, with its claim: a file that passes is kept as a recording of the study (B3.6, B10.2, B10.4). A
        // checked recording is fitted at once too, and the answer names the fit (B3.6).
        IdentifyAction::ImportRecording { json } => {
            refuse_while_journaled(&journal)?;
            import_recording_answer(operations, claim, work, &json)
        }
        // The model check answers at once, with its claim; a model that passes is kept as the study's model (B3.7, N4.1).
        IdentifyAction::ImportModel { json } => {
            refuse_while_journaled(&journal)?;
            identify_import::import_model(&work.project_root, &work.loop_definition, &json)
        }
        // The manual identification (B3.3, B3.6, B3.7) has its own dispatch: its checks answer at once, and its fit and restoration
        // are operations.
        manual @ (IdentifyAction::ManualEdit { .. }
        | IdentifyAction::ManualQualify { .. }
        | IdentifyAction::ManualFit
        | IdentifyAction::ManualChooseStructure { .. }
        | IdentifyAction::ManualAccept
        | IdentifyAction::ManualRestore
        | IdentifyAction::ManualImport { .. }) => {
            identify_manual::answer(operations, claim, &journal, work, manual)
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
        IdentifyAction::ImportRecording { .. } | IdentifyAction::ManualImport { .. } => {
            "Import recording"
        }
        IdentifyAction::ImportModel { .. } => "Import model",
        IdentifyAction::ManualEdit { .. } => "Edit identification",
        IdentifyAction::ManualQualify { .. } => "Check recording",
        IdentifyAction::ManualFit => "Fit identification",
        IdentifyAction::ManualChooseStructure { .. } => "Choose model structure",
        IdentifyAction::ManualAccept => "Accept identification",
        IdentifyAction::ManualRestore => "Restore identification",
    }
}

/// The identification stage of a run (B8): the live campaign of `controlStudio/identify` with `settings`, inside the run's own
/// operation. It refuses what `identify` refuses while the loop's journal holds an operation (A8.5), and it claims nothing, because
/// the run holds the loop. Its Cancel reaches the campaign through `context`. What needs no runtime is checked first, as `identify`
/// checks it: a method the loop cannot run and a missing limit are named before the missing `connection` (E11.1), so a run on a
/// stopped runtime still asks for the limits or names the method's prerequisite.
///
/// # Errors
/// The journal cannot be read or holds an unfinished operation (A8.5); the loop cannot run the settings' method
/// ([`require_applicable_method`]); the limits refuse the settings (B3.4 step 1); there is no runtime `connection` (E11.1); or the
/// campaign fails, or the scheduling variable is refused (see `run_identify` and `identify_schedule::check_start`).
pub(crate) fn run_stage(
    context: &OperationContext,
    project_root: &Path,
    storage_folder: &Path,
    pending: &Arc<PendingReleases>,
    loop_definition: &LoopDefinition,
    connection: Option<&RuntimeConnection>,
    settings: &AcquisitionSettings,
) -> Result<Value, Failure> {
    let journal = Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    refuse_while_journaled(&journal)?;
    require_applicable_method(loop_definition, settings)?;
    // The campaign's own limits are checked before anything starts (B3.4 step 1): a missing limit is the run's input to ask for.
    let configured =
        configured_acquisition(loop_definition, Some(settings)).map_err(|error| error.failure())?;
    require_campaign_limits(&configured)?;
    campaign_maximum_ms(&configured)?;
    // The checks above need no runtime; the campaign does (E11.1).
    let connection = connection.ok_or_else(runtime_required)?;
    let work = LoopWork {
        project_root: project_root.to_path_buf(),
        storage_folder: storage_folder.to_path_buf(),
        pending: Arc::clone(pending),
        loop_definition: loop_definition.clone(),
    };
    // The run's identify stage starts a campaign too, so the scheduling variable is checked before it (C10 Identify, D-21).
    let source = runtime_source(connection)?;
    let scheduling_value = identify_schedule::check_start(connection, loop_definition, source)?;
    run_identify(context, &work, connection, settings, scheduling_value)
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
    scheduling_value: Option<f64>,
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
    if identify_manual::takes_the_manual_recording(work, settings.method)? {
        // A project test that the manual identification awaits records its recording and fits nothing: the recording becomes the
        // independent recording of the manual identification, and the test's own model is not kept (B3.3, C12).
        let outcome = record_for_manual_check(
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
        let file_id = kept_file(&retention, &outcome.recording_id)?;
        return identify_manual::keep_project_test_recording(
            work,
            retention.study(),
            &file_id,
            outcome.restoration,
        );
    }
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
    keep_campaign(work, &retention, source, scheduling_value, outcome)
}

/// Keeps what a live campaign gave (B3.4, C12): a fit is kept as the study's Identify section, with the campaign's restoration and the
/// samples of its kept recording beside it, and for the project test of a project-scripted loop the sentence of its end. A project
/// test whose fit its recording cannot support keeps the recording and no model, and finishes with the fit's refusal as its note.
fn keep_campaign(
    work: &LoopWork,
    retention: &StudyRetention<'_>,
    source: EvidenceSource,
    scheduling_value: Option<f64>,
    outcome: IdentifyOutcome<RecordedLimitsEvaluation>,
) -> Result<Value, Failure> {
    let IdentifyOutcome {
        result,
        recording_id,
        method,
        restoration,
        recorded_samples,
        fit_refusal,
        ..
    } = outcome;
    let recording_file_id = kept_file(retention, &recording_id)?;
    match result {
        RunResult::Identified { result, .. } => {
            let mut answer = keep_result(
                work,
                retention.study(),
                Fitted {
                    method: IdentifyMethod::Live(method.kind),
                    source,
                    recording_file_id,
                    result,
                    scheduling_value,
                },
            )?;
            add_campaign_facts(
                &mut answer,
                &work.loop_definition,
                restoration,
                &recorded_samples,
            )?;
            Ok(answer)
        }
        RunResult::Recorded(_) => {
            // A project test whose fit the recording cannot support keeps its recording and no model; its answer is the completed test,
            // with the refusal as its note (C12, B3.5).
            let refusal = fit_refusal.ok_or_else(model_check_instead_of_model)?;
            let mut answer = project_test_result(
                &work.loop_definition,
                retention.study(),
                &recording_file_id,
                restoration,
                recorded_samples,
                refusal,
            )?;
            // The study's model is unchanged, so its validation may stay current: the Apply stage can start exactly when Apply is
            // offered, as the loop's journal allows it (B7, B9.4).
            let journal = Journal::open(
                &work.storage_folder,
                &work.loop_definition.project_id,
                &work.loop_definition.id,
            )?;
            answer.stages =
                ApplicationState::of(&journal, retention.study(), &answer.stage_evidence, false)?
                    .stages(&answer.stage_evidence);
            to_value(&answer)
        }
    }
}

/// Adds to `answer` the facts of a campaign that the stored section does not keep: its restoration (C12), the sentence that says the
/// confirmed restoration releases the experiment settings, the samples of the kept recording, which the page draws its recorded
/// signals from (`control-studio.recording.evidence`), and the sentence of a project test's end. A fresh campaign and a resumed one
/// answer them alike.
fn add_campaign_facts(
    answer: &mut Value,
    loop_definition: &LoopDefinition,
    restoration: Option<CampaignRestoration>,
    recorded_samples: &[RecordedChartSample],
) -> Result<(), Failure> {
    if let Some(fields) = answer.as_object_mut() {
        fields.insert("restoration".into(), to_value(&restoration)?);
        if let Some(note) = settings_released(restoration) {
            fields.insert("settingsReleased".into(), Value::String(note));
        }
        fields.insert("recordedSamples".into(), to_value(&recorded_samples)?);
        if let Some(outcome) = test_outcome(loop_definition, restoration) {
            fields.insert("testOutcome".into(), Value::String(outcome));
        }
    }
    Ok(())
}

/// The sentence of a project test's end (C12), for the campaign of a project-scripted loop, which is its project test; `None` for any
/// other loop's campaign.
fn test_outcome(
    loop_definition: &LoopDefinition,
    restoration: Option<CampaignRestoration>,
) -> Option<String> {
    project_scripted(loop_definition).map(|_| project_test_outcome(restoration))
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
    let mut answer = keep_result(
        work,
        retention.study(),
        Fitted {
            method: IdentifyMethod::Live(kind),
            source,
            recording_file_id,
            result: resumed.result,
            // The resumed campaign's start was checked when it began, and its value is not journaled, so none is kept.
            scheduling_value: None,
        },
    )?;
    add_campaign_facts(
        &mut answer,
        loop_definition,
        resumed.restoration,
        &resumed.recorded_samples,
    )?;
    Ok(answer)
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
    // The new study keeps no Recorded-data fit: the answer carries the engine's start state, so the page shows it without asking (B3.3).
    to_value(&ResetResult {
        kind: AnswerKind::new(),
        status_sentence: NEW_STUDY_TEXT.to_owned(),
        stages: stage_availability(&evidence),
        stage_evidence: evidence,
        manual_identification: Some(identify_manual::kept_state(work)),
    })
}

/// Refuses an action while the loop's journal holds an operation that runs or whose outcome is unconfirmed (A8.5). The parameter tool's
/// reviews and writes are refused by it too, so a project test that is not restored keeps the experiment settings frozen (C12). An
/// Apply's checkpoint that waits for its Restore is refused with the block's reason and the next action Restore (B6.4, B6.7): Identify,
/// Open recording, Reset and the imports wait for it.
pub(crate) fn refuse_while_journaled(journal: &Journal) -> Result<(), Failure> {
    refuse_while_restore_waits(journal)?;
    unfinished_operation_refusal(&journaled_ids(journal)).map_or(Ok(()), Err)
}

/// Refuses a start whose method the loop cannot run (B3.2, B3.3). The refusal is the sentence that the open answer's `identifyMethods`
/// gives that method (`identify_method_offers`), with the next action Choose another method, so the page checks no method itself. A
/// method with no offer, such as the observation an `Automatic` or a declared observation stands for, always runs.
///
/// # Errors
/// The loop cannot run the settings' method.
fn require_applicable_method(
    loop_definition: &LoopDefinition,
    settings: &AcquisitionSettings,
) -> Result<(), Failure> {
    let Some(word) = offered_word(loop_definition, settings.method) else {
        return Ok(());
    };
    let reason = identify_method_offers(loop_definition)
        .into_iter()
        .find(|offer| offer.value == word)
        .and_then(|offer| offer.reason);
    reason.map_or(Ok(()), |sentence| {
        Err(Failure::new(
            &sentence,
            Cause::Settings,
            NextAction::ChooseMethod,
        ))
    })
}

/// The word the Method list offers `method` under: its own, or for `Configured` the method the loop's experiment declares. `None` for a
/// `Configured` method of a loop that declares no experiment.
fn offered_word(loop_definition: &LoopDefinition, method: AcquisitionMethod) -> Option<String> {
    let word = match method {
        AcquisitionMethod::Configured => {
            serde_json::to_value(loop_definition.experiment.as_ref()?.kind)
        }
        explicit => serde_json::to_value(explicit),
    };
    word.ok()?.as_str().map(str::to_owned)
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

#[cfg(test)]
#[path = "identify_method_tests.rs"]
mod method_tests;

#[cfg(test)]
#[path = "identify_hold_tests.rs"]
mod hold_tests;

#[cfg(test)]
#[path = "identify_project_test_tests.rs"]
mod project_test_tests;
