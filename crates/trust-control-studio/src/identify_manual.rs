//! The manual identification of `controlStudio/identify`: the Recorded-data fit of specification 35 (B3.3, B3.6, B3.7 and A8.6). The
//! page's form, the check of a recording the study keeps, the fit (an operation that a Cancel stops), the sensor-lag structure, the
//! acceptance and the reopened state. Every rule is trust-control's: the workflow (`identification::workflow`), the section
//! (`study::manual_identification`) and the page's view (`identification::workflow::manual_answer`). This module reads the recording
//! files, replays the saved workflow from them, keeps its state in the study and answers the page's view.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::acquisition_settings::AcquisitionMethod;
use trust_control::identification::form_draft::{
    check_held_step_model, default_form, excluded_intervals_of, held_step_settings_of,
    valid_form_draft, IdentificationFormDraft,
};
use trust_control::identification::policy::project_scripted;
use trust_control::identification::project_test::project_test_outcome;
use trust_control::identification::refusal::Refusal;
use trust_control::identification::sensor_lag_comparison::StructureChoice;
use trust_control::identification::state::{
    restore_identification_input, IdentificationPhase, SavedIdentificationInput,
    SavedIdentificationState,
};
use trust_control::identification::workflow::{IdentificationWorkflow, QualifyRequest};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::IdentifyAction;
use trust_control::messages::{CampaignRestoration, ManualIdentificationAnswer, ManualStatus};
use trust_control::recording::opened::opened_recording;
use trust_control::recording::RecordingFile;
use trust_control::study::manual_identification::{
    accepted_manual_section, error_limit_of, manual_input, ManualIdentificationSection,
    ManualRecording,
};
use trust_control::study::{Study, StudySection};

use super::identify_recording::last_opened_recording_if_any;
use super::{refuse_while_journaled, started, working_study, LoopWork};
use crate::application_state::state_now;
use crate::busy_loops::LoopClaim;
use crate::commands::to_value;
use crate::journal::Journal;
use crate::operation::{OperationContext, Operations};
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::storage;

/// The refusal of a fit, an edit or a check that needs a recording to have been checked first (the page's "Qualify" sentence).
const QUALIFY_FIRST: &str = "Qualify a suitable recording before fitting a model.";
/// The refusal of an acceptance that the page's gate does not allow: the validation or the kept structure does not qualify.
const NOT_ACCEPTABLE: &str =
    "Independent validation or model-structure evidence is not qualified for acceptance.";
/// The refusal of a form that the decoder refuses.
const FORM_MALFORMED: &str = "The identification form is malformed.";
/// The refusal of a check that names no recording while the study keeps no opened recording either (B3.6).
const NO_RECORDING_TO_CHECK: &str = "Open a recording before checking it.";
/// The refusal of an acceptance whose section names no estimation or validation recording.
const NO_RECORDING: &str = "The manual identification names no recording to accept.";
/// The sentence a project test's answer carries while the manual identification awaits its independent recording (B3.3).
const PROJECT_TEST_NOTE: &str =
    "The recording is the manual identification's independent recording; the model is not refitted.";

/// Answers a manual identification action (B3.3, B3.6, B3.7, A8.6): the edit, the check, the structure and the acceptance answer at
/// once, and the fit and the restoration start operations that hold the loop's `claim`. A journaled operation of the loop refuses
/// every action but the restoration (A8.5).
pub(super) fn answer(
    operations: &Operations,
    claim: LoopClaim,
    journal: &Journal,
    work: LoopWork,
    action: IdentifyAction,
) -> Result<Value, Failure> {
    if !matches!(action, IdentifyAction::ManualRestore) {
        refuse_while_journaled(journal)?;
    }
    match action {
        IdentifyAction::ManualEdit { form } => edit(&work, form),
        IdentifyAction::ManualQualify { recording_id, form } => {
            qualify(&work, recording_id.as_deref(), form)
        }
        IdentifyAction::ManualFit => started(operations, claim, move |context| fit(context, &work)),
        IdentifyAction::ManualChooseStructure { choice } => choose(&work, choice),
        IdentifyAction::ManualAccept => accept(&work),
        IdentifyAction::ManualRestore => started(operations, claim, move |_| restore(&work)),
        IdentifyAction::ManualImport { json } => import(&work, &json),
        _ => Err(Failure::new(
            "This is not a manual identification action.",
            Cause::Settings,
            NextAction::None,
        )),
    }
}

/// `edit`: stores the page's form (A8.4). The check, the fit and any accepted model are cleared; no fit runs, unless the model was
/// accepted, whose acceptance is replayed first.
pub(super) fn edit(work: &LoopWork, form: IdentificationFormDraft) -> Result<Value, Failure> {
    check_form(&form)?;
    let study = working_study(work)?;
    let section = section_of(&study)?;
    let mut workflow = workflow_for_inputs(work, section.as_ref())?;
    workflow.edit_form(form.clone());
    to_value(&keep(work, study, &workflow, Some(form))?)
}

/// `qualify`: checks the recording the study keeps with the page's form (B3.3, B3.6). The form's held-step settings and excluded
/// interval are read as the page read them, and the recording becomes the estimation, or the validation recording after a preview.
/// A check that names no recording reads the study's last opened recording, as `openRecording` does; a study that keeps none has
/// nothing to check.
pub(super) fn qualify(
    work: &LoopWork,
    recording_id: Option<&str>,
    form: IdentificationFormDraft,
) -> Result<Value, Failure> {
    let recording_id = match recording_id {
        Some(recording_id) => recording_id.to_owned(),
        None => last_opened_recording_if_any(work)?.ok_or_else(|| {
            Failure::new(NO_RECORDING_TO_CHECK, Cause::Settings, NextAction::None)
        })?,
    };
    check_form(&form)?;
    let study = working_study(work)?;
    if !study.recordings.contains(&recording_id) {
        return Err(refused(&Refusal::new(format!(
            "The study keeps no recording {recording_id}."
        ))));
    }
    let section = section_of(&study)?;
    let file =
        storage::read_recording(&work.project_root, &work.loop_definition.id, &recording_id)?;
    let request = request_of(
        work,
        &form,
        &recording_id,
        &file,
        limit_of(work, section.as_ref()),
    )?;
    let mut workflow = workflow_for_inputs(work, section.as_ref())?;
    workflow.qualify(request);
    to_value(&keep(work, study, &workflow, Some(form))?)
}

/// `import` (B3.3, B3.6): keeps the recording file the engineer opens as a recording of the study, and selects it for the role the
/// stage expects next (`select_recording`): the estimation, or the validation recording after a preview. A file the check refuses
/// keeps nothing and answers the check's sentence.
pub(super) fn import(work: &LoopWork, json: &str) -> Result<Value, Failure> {
    let (_check, file) =
        opened_recording(json, &work.loop_definition).map_err(|refusal| refused(&refusal))?;
    let study = working_study(work)?;
    let study = storage::save_recording(&work.project_root, &study, &file)?;
    let recording_id = file.id()?;
    let section = section_of(&study)?;
    let form = form_or_default(section.as_ref());
    let request = request_of(
        work,
        &form,
        &recording_id,
        &file,
        limit_of(work, section.as_ref()),
    )?;
    let mut workflow = workflow_for_inputs(work, section.as_ref())?;
    workflow.select_recording(request);
    to_value(&keep(work, study, &workflow, Some(form))?)
}

/// The project test the manual identification awaits (B3.3, C12): its recording, which the run kept, becomes the manual identification's
/// independent recording and is selected for the validation role. The test's own model is not kept, so the current model stays as it is,
/// and the answer names the recording, says that the model is not refitted, and says how the test ended, from the `restoration` its
/// campaign confirmed.
pub(super) fn keep_project_test_recording(
    work: &LoopWork,
    study: &Study,
    recording_id: &str,
    restoration: Option<CampaignRestoration>,
) -> Result<Value, Failure> {
    let file = storage::read_recording(&work.project_root, &work.loop_definition.id, recording_id)?;
    let section = require_section(study)?;
    let form = form_or_default(Some(&section));
    let request = request_of(
        work,
        &form,
        recording_id,
        &file,
        section.maximum_normalized_error,
    )?;
    let mut workflow = workflow_for_inputs(work, Some(&section))?;
    workflow.select_recording(request);
    let mut study = study.clone();
    put_section(&mut study, &workflow, Some(form.clone()))?;
    storage::save_study(&work.project_root, &study, &[])?;
    let mut answer = answer_of(work, &workflow, Some(form))?;
    answer.message = Some(PROJECT_TEST_NOTE.to_string());
    answer.test_outcome = Some(project_test_outcome(restoration));
    to_value(&answer)
}

/// Whether a run with `method` is the project test of the loop: a loop whose experiment is project-scripted runs the project's own
/// scenario whatever the method, and the page's project test asks for it with the Automatic method (C12), as it may with the
/// declared one (`Configured`).
#[must_use]
pub(crate) fn is_project_test(loop_definition: &LoopDefinition, method: AcquisitionMethod) -> bool {
    matches!(
        method,
        AcquisitionMethod::Automatic | AcquisitionMethod::Configured
    ) && project_scripted(loop_definition).is_some()
}

/// Whether a run with `method` takes the manual identification's independent recording (B3.3, C12): it is the project test, and the
/// manual identification's stage is the preview that awaits its recording. Such a run records and fits nothing.
pub(super) fn takes_the_manual_recording(
    work: &LoopWork,
    method: AcquisitionMethod,
) -> Result<bool, Failure> {
    if !is_project_test(&work.loop_definition, method) {
        return Ok(false);
    }
    let study = working_study(work)?;
    Ok(section_of(&study)?.is_some_and(|section| section.phase == IdentificationPhase::Preview))
}

/// The check request of a recording with `form`: the settings and the excluded interval the form holds, and the samples the recording
/// file yields as an observation (B3.6).
fn request_of(
    work: &LoopWork,
    form: &IdentificationFormDraft,
    recording_id: &str,
    file: &RecordingFile,
    limit: f64,
) -> Result<QualifyRequest, Failure> {
    let recording = ManualRecording {
        recording_id: recording_id.to_string(),
        requested_kind: form.model,
        step_response_settings: held_step_settings_of(form)?,
        first_order_step_response_settings: None,
        excluded: excluded_intervals_of(form)?,
    };
    let saved = manual_input(&recording, file, &work.loop_definition, limit)
        .map_err(|refusal| refused(&refusal))?;
    let input = restore_identification_input(&saved);
    Ok(QualifyRequest {
        samples: input.estimation,
        excluded: input.excluded,
        source: input.source,
        requested_kind: input.requested_kind,
        recording_id: Some(recording_id.to_string()),
        step_response_settings: input.step_response_settings,
        recording: None,
        first_order_step_response_settings: None,
    })
}

/// The section's form, or the default form a new identification starts with (B3.3).
fn form_or_default(section: Option<&ManualIdentificationSection>) -> IdentificationFormDraft {
    section
        .and_then(|section| section.form.clone())
        .unwrap_or_else(default_form)
}

/// `fit`, as an operation that a Cancel stops (B3.3): the first fit gives the provisional model, the second scores it on the
/// validation recording. A refused fit is the answer's failure; a cancelled one keeps nothing.
pub(super) fn fit(context: &OperationContext, work: &LoopWork) -> Result<Value, Failure> {
    fit_unless_cancelled(work, &|| context.is_cancelled())
}

/// The fit with its cancellation: `cancelled` is asked before the fit and again before the result is kept, so a cancelled fit leaves
/// the stored state as it was.
fn fit_unless_cancelled(work: &LoopWork, cancelled: &dyn Fn() -> bool) -> Result<Value, Failure> {
    let study = working_study(work)?;
    let section = require_section(&study)?;
    let mut workflow = replayed(work, Some(&section))?;
    stop_if(cancelled)?;
    let fitted = workflow.fit().map(|_| ());
    if let Err(refusal) = fitted {
        // A refusal of a fit is kept by the workflow as its failed stage; any other refusal is the answer's failure.
        if workflow.current_state().phase != IdentificationPhase::Failed {
            return Err(refused(&refusal));
        }
    }
    stop_if(cancelled)?;
    to_value(&keep(work, study, &workflow, section.form.clone())?)
}

/// `chooseStructure`: keeps the sensor-lag structure the comparison recommends (C4 comparison rules, N3.3).
pub(super) fn choose(work: &LoopWork, choice: StructureChoice) -> Result<Value, Failure> {
    let study = working_study(work)?;
    let section = require_section(&study)?;
    let mut workflow = replayed(work, Some(&section))?;
    workflow
        .choose_structure(choice)
        .map(|_| ())
        .map_err(|refusal| refused(&refusal))?;
    to_value(&keep(work, study, &workflow, section.form.clone())?)
}

/// `accept` (B3.7): the validated model becomes the study's current model, an Identify section with `model_source` Identify, so that
/// Tune designs on it. The manual section is kept as accepted, so that reopening restores it.
pub(super) fn accept(work: &LoopWork) -> Result<Value, Failure> {
    let mut study = working_study(work)?;
    let section = require_section(&study)?;
    let mut workflow = replayed(work, Some(&section))?;
    if !workflow.acceptance_ready() {
        return Err(Failure::new(
            NOT_ACCEPTABLE,
            Cause::ModelOrData,
            NextAction::None,
        ));
    }
    let accepted = workflow.accept().map_err(|refusal| refused(&refusal))?;
    let (Some(estimation), Some(validation)) = (&section.estimation, &section.current) else {
        return Err(Failure::new(
            NO_RECORDING,
            Cause::Settings,
            NextAction::None,
        ));
    };
    let estimation_file = storage::read_recording(
        &work.project_root,
        &work.loop_definition.id,
        &estimation.recording_id,
    )?;
    let identify = accepted_manual_section(
        &work.loop_definition,
        &accepted,
        estimation_file.source,
        &estimation.recording_id,
        &validation.recording_id,
    )
    .map_err(|refusal| refused(&refusal))?;
    put_section(&mut study, &workflow, section.form.clone())?;
    // The accepted model is the manual section's own, so the section stays in the same write (B3.7).
    storage::save_identify_keeping_manual(&work.project_root, &study, &identify, &[])?;
    to_value(&answer_of(work, &workflow, section.form.clone())?)
}

/// `restore` (A8.6): replays the saved manual identification from the recording files and answers the page's view. Nothing is written.
/// The answer carries the form the study keeps, or the default form of a study that keeps none, so the page shows the engine's numbers
/// (B3.3).
pub(super) fn restore(work: &LoopWork) -> Result<Value, Failure> {
    to_value(&restored(work)?)
}

/// The Recorded-data fit the study keeps, as the page shows it (A8.6, B3.3), for the answers that carry it: the saved stage replayed
/// from the kept recordings, or the default form of a study that keeps none. A saved stage that cannot be replayed (a recording the
/// study lists is gone, say) is answered as a failed stage that says why and offers to open a recording, so that an open answer never
/// fails for it and the sentence is not lost. Nothing is written.
pub(super) fn kept_state(work: &LoopWork) -> ManualIdentificationAnswer {
    restored(work).unwrap_or_else(|failure| {
        let workflow = IdentificationWorkflow::new(error_limit_of(&work.loop_definition));
        let mut answer = workflow.manual_answer(&work.loop_definition, Some(default_form()));
        let sentence = failure.detail.unwrap_or(failure.message);
        answer.status = ManualStatus::Failed;
        answer.reason = Some(sentence.clone());
        answer.message = Some(sentence);
        answer
    })
}

/// [`kept_state`] of the loop in `project_root`, for `controlStudio/open`, which holds no operation's work.
pub(crate) fn kept_state_of(
    project_root: &Path,
    loop_definition: &LoopDefinition,
) -> ManualIdentificationAnswer {
    kept_state(&LoopWork {
        project_root: project_root.to_path_buf(),
        storage_folder: PathBuf::new(),
        pending: Arc::new(PendingReleases::new()),
        loop_definition: loop_definition.clone(),
    })
}

/// The saved manual identification replayed from the recording files, as the page's view.
fn restored(work: &LoopWork) -> Result<ManualIdentificationAnswer, Failure> {
    let study = working_study(work)?;
    let section = section_of(&study)?;
    let workflow = replayed(work, section.as_ref())?;
    answer_of(work, &workflow, Some(form_or_default(section.as_ref())))
}

/// The manual section of the study, if it keeps one.
fn section_of(study: &Study) -> Result<Option<ManualIdentificationSection>, Failure> {
    study.section::<ManualIdentificationSection>()
}

/// The section of the study, which a fit or an acceptance needs (its check came first).
fn require_section(study: &Study) -> Result<ManualIdentificationSection, Failure> {
    section_of(study)?.ok_or_else(|| Failure::new(QUALIFY_FIRST, Cause::Settings, NextAction::None))
}

/// The error limit an input is checked with: the section's own, or the loop's policy for a new section (N3.2).
fn limit_of(work: &LoopWork, section: Option<&ManualIdentificationSection>) -> f64 {
    section.map_or_else(
        || error_limit_of(&work.loop_definition),
        |section| section.maximum_normalized_error,
    )
}

/// The workflow with the section's inputs and stage, without replaying its fits: the edit and the check clear every review, so they
/// need no fit. An accepted model is replayed instead, because its acceptance cannot be restored without the fit.
fn workflow_for_inputs(
    work: &LoopWork,
    section: Option<&ManualIdentificationSection>,
) -> Result<IdentificationWorkflow, Failure> {
    match section {
        Some(section) if section.phase == IdentificationPhase::Accepted => {
            replayed(work, Some(section))
        }
        Some(section) => Ok(IdentificationWorkflow::from_inputs(&saved_state(
            work, section,
        )?)),
        None => Ok(IdentificationWorkflow::new(error_limit_of(
            &work.loop_definition,
        ))),
    }
}

/// The workflow replayed from the section: its recordings are read from their files and the stages after them are fitted again.
fn replayed(
    work: &LoopWork,
    section: Option<&ManualIdentificationSection>,
) -> Result<IdentificationWorkflow, Failure> {
    let mut workflow = IdentificationWorkflow::new(error_limit_of(&work.loop_definition));
    if let Some(section) = section {
        workflow
            .restore_saved(&saved_state(work, section)?)
            .map_err(|refusal| refused(&refusal))?;
    }
    Ok(workflow)
}

/// The saved state of a section, with each recording's samples read from its file.
fn saved_state(
    work: &LoopWork,
    section: &ManualIdentificationSection,
) -> Result<SavedIdentificationState, Failure> {
    let estimation = section
        .estimation
        .as_ref()
        .map(|recording| input_of(work, section, recording))
        .transpose()?;
    let current = section
        .current
        .as_ref()
        .map(|recording| input_of(work, section, recording))
        .transpose()?;
    Ok(section.saved_state(estimation, current))
}

/// The saved input of one recording of a section: the recording's file, read as an observation (B3.6).
fn input_of(
    work: &LoopWork,
    section: &ManualIdentificationSection,
    recording: &ManualRecording,
) -> Result<SavedIdentificationInput, Failure> {
    let file = storage::read_recording(
        &work.project_root,
        &work.loop_definition.id,
        &recording.recording_id,
    )?;
    manual_input(
        recording,
        &file,
        &work.loop_definition,
        section.maximum_normalized_error,
    )
    .map_err(|refusal| refused(&refusal))
}

/// Puts the workflow's state into the study's manual section, or removes the section when the workflow holds no state.
fn put_section(
    study: &mut Study,
    workflow: &IdentificationWorkflow,
    form: Option<IdentificationFormDraft>,
) -> Result<(), Failure> {
    if let Some(saved) = workflow.snapshot() {
        let section = ManualIdentificationSection::from_saved(&saved, form)
            .map_err(|refusal| refused(&refusal))?;
        study.set_section(&section)
    } else {
        study
            .sections
            .remove(<ManualIdentificationSection as StudySection>::KEY);
        Ok(())
    }
}

/// Keeps the workflow's state in the study, writes the study, and answers the page's view.
fn keep(
    work: &LoopWork,
    mut study: Study,
    workflow: &IdentificationWorkflow,
    form: Option<IdentificationFormDraft>,
) -> Result<ManualIdentificationAnswer, Failure> {
    put_section(&mut study, workflow, form.clone())?;
    storage::save_study(&work.project_root, &study, &[])?;
    answer_of(work, workflow, form)
}

/// The page's view of the workflow (`manual_answer`), with the form the page holds. The stage evidence is read from the study as it
/// is saved now, and the stages are computed from it and the loop's journal as the open answer computes them, so that the page's gates
/// follow the study: an accepted model is the current model, and Tune is then available (B3.7, A8.4), and the Apply stage can start
/// exactly when Apply is offered (B7).
fn answer_of(
    work: &LoopWork,
    workflow: &IdentificationWorkflow,
    form: Option<IdentificationFormDraft>,
) -> Result<ManualIdentificationAnswer, Failure> {
    let mut answer = workflow.manual_answer(&work.loop_definition, form);
    let (application, evidence) = state_now(
        &work.project_root,
        &work.storage_folder,
        &work.loop_definition,
    )?;
    answer.stages = Some(application.stages(&evidence));
    answer.stage_evidence = Some(evidence);
    Ok(answer)
}

/// Stops the work when the operation was cancelled, before it keeps anything.
fn stop_if(cancelled: &dyn Fn() -> bool) -> Result<(), Failure> {
    if cancelled() {
        Err(crate::operation::cancelled())
    } else {
        Ok(())
    }
}

/// Refuses a form the decoder refuses, or a held step with a model it does not allow (N2.3a), before anything is read or written.
fn check_form(form: &IdentificationFormDraft) -> Result<(), Failure> {
    if !valid_form_draft(form) {
        return Err(Failure::new(
            FORM_MALFORMED,
            Cause::Settings,
            NextAction::None,
        ));
    }
    check_held_step_model(form)
}

/// A refusal of the identification's rules, as a failure of the data it checks (`identify_import::refused`).
fn refused(refusal: &Refusal) -> Failure {
    Failure::new(&refusal.0, Cause::ModelOrData, NextAction::None)
}

#[cfg(test)]
#[path = "identify_manual_recording_tests.rs"]
mod recording_tests;

#[cfg(test)]
#[path = "identify_manual_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "identify_manual_tests.rs"]
mod tests;
