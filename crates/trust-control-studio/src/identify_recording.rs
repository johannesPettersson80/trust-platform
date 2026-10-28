//! The recording paths of `controlStudio/identify` (specification 35, §0.1a, B3.6, B5.2 and B10.2 to B10.4): `openRecording` fits a
//! recording the study keeps, `importRecording` checks a file and fits what it kept, and a fitted model is kept as the study's Identify
//! section.

use serde_json::{json, Value};
use trust_control::experiment::ControlExperimentKind;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::automatic::AutomaticIdentificationResult;
use trust_control::identification::connected_run::{
    accepted_model_from_one_record, model_not_from_one_record, qualification, stored_fit_window,
    FitWindow,
};
use trust_control::identification::identify_result::{
    identify_answer, identify_signals, study_evidence,
};
use trust_control::identification::recorded_limits::{
    evaluate_recorded_limits, RecordedLimitsEvaluation,
};
use trust_control::identification::recording_import::archive::with_extra_channels;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::opened::opened_plan;
use trust_control::recording::opened_channels::{
    configured_recording_selection, opened_recording_catalog, selection_loop, workflow_units,
    RecordingSignalCatalog, RecordingSignalSelection,
};
use trust_control::recording::{EvidenceSource, RecordingFile};
use trust_control::setup::discovery::study_tune_methods;
use trust_control::study::current_model::CurrentModel;
use trust_control::study::identify::{
    refit_refusal, IdentifyMethod, IdentifySection, IdentifySignals, StoredRecordRequest,
};
use trust_control::study::Study;

use crate::busy_loops::LoopClaim;
use crate::commands::to_value;
use crate::identify_import;
use crate::operation::{OperationContext, Operations};
use crate::storage;
use crate::tune_action::study_relay_evidence;
use crate::workflow::identify::run::{connected_record, fit_records};
use crate::workflow::identify::RunEvaluator;

use super::{started, working_study, LoopWork};

/// A model fitted from one recording of the study, with how that recording was obtained.
pub(super) struct Fitted {
    /// How the recording was obtained (B3.3, B3.6).
    pub(super) method: IdentifyMethod,
    /// Where the recording's values come from (A9).
    pub(super) source: EvidenceSource,
    /// The recording file the model was fitted on (A8.3).
    pub(super) recording_file_id: String,
    /// The fit.
    pub(super) result: AutomaticIdentificationResult<RecordedLimitsEvaluation>,
    /// The value of the scheduling variable that the run read when it started (C10 Identify, D-21); `None` when the loop has no
    /// schedule, and when no run started with the fit (a resumed campaign, an opened recording).
    pub(super) scheduling_value: Option<f64>,
}

/// `openRecording` (B3.6, B5.2). `recording_id` names the recording; without it the study's last opened recording is fitted
/// (`last_opened_recording`). An opened recording the study keeps is answered from the study when it pairs or needs a second
/// recording; otherwise it is fitted from its own samples with the signals the engineer selected. Any other recording is fitted again
/// from the request its Identify result keeps.
pub(super) fn open_recording_answer(
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

/// `importRecording` (B3.6, B10.2): the file is checked and kept (`identify_import::import_recording`). A checked file is then fitted at
/// once, as `openRecording` fits the recording the import kept (`open_recording_answer`): the answer names the fit's operation, which the
/// page follows, or carries the answer the study gives that recording (a pair fitted from the study, or a recording that needs a second
/// one). A file that needs another recording, or a pair, is answered as the import answers it.
///
/// # Errors
/// As `identify_import::import_recording`, and as `openRecording` for the fit the import starts.
pub(super) fn import_recording_answer(
    operations: &Operations,
    claim: LoopClaim,
    work: LoopWork,
    json: &str,
) -> Result<Value, Failure> {
    let mut imported =
        identify_import::import_recording(&work.project_root, &work.loop_definition, json)?;
    if imported.check.status != "checked" || imported.identification.is_some() {
        return to_value(&imported);
    }
    let selection = imported.recording_catalog.selection.clone();
    let fit = open_recording_answer(
        operations,
        claim,
        work,
        Some(imported.recording_id.clone()),
        Some(selection.inputs),
        Some(selection.outputs),
    )?;
    let operation_id = fit
        .get("operationId")
        .and_then(Value::as_str)
        .map(str::to_owned);
    match operation_id {
        Some(operation_id) => {
            imported.operation_id = Some(operation_id);
            to_value(&imported)
        }
        None => Ok(fit),
    }
}

/// The recording `openRecording` fits when it names none: the study's last opened recording, the latest of the recordings the study
/// lists whose file carries the opened marker (B3.6). A study that keeps none refuses, and the engineer opens a recording first.
///
/// # Errors
/// The study keeps no opened recording, or a recording the study lists cannot be read.
fn last_opened_recording(work: &LoopWork) -> Result<String, Failure> {
    last_opened_recording_if_any(work)?.ok_or_else(|| {
        Failure::new(
            "The study keeps no opened recording to fit. Open a recording first.",
            Cause::Settings,
            NextAction::None,
        )
    })
}

/// The study's last opened recording (B3.6), when it keeps one: the latest of the recordings the study lists whose file carries the
/// opened marker. `openRecording` and the Recorded-data fit's check (`manualQualify`) read it when their request names no recording.
///
/// # Errors
/// The study cannot be read, or a recording the study lists cannot be read.
pub(super) fn last_opened_recording_if_any(work: &LoopWork) -> Result<Option<String>, Failure> {
    let study = working_study(work)?;
    for id in study.recordings.iter().rev() {
        let file = storage::read_recording(&work.project_root, &work.loop_definition.id, id)?;
        if file.opened {
            return Ok(Some(id.clone()));
        }
    }
    Ok(None)
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
            // The refit fits the recording of the stored section, so the value its start read is the one it keeps.
            scheduling_value: stored.section.scheduling_value,
        },
    )
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
            // An opened recording starts no run, so no scheduling value was read for it.
            scheduling_value: None,
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
pub(super) fn keep_result(
    work: &LoopWork,
    study: &Study,
    fitted: Fitted,
) -> Result<Value, Failure> {
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
        scheduling_value,
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
    })
    .with_scheduling_value(scheduling_value);
    let section = match read {
        Some(read) => section.with_recorded_channels(read),
        None => section,
    };
    let kept = storage::save_identify(&work.project_root, study, &section, &[])?;
    // The Tune page's methods after this identification (C4, N5.R): the loop's own, and the relay rule when the kept record is a qualified
    // Feedback relay. Only a relay identification reads its recording, and an unreadable one offers no relay rule (Tune still refuses it).
    let relay = study_relay_evidence(&work.project_root, loop_definition, Some(&section))
        .ok()
        .flatten();
    let (methods, method_titles) = study_tune_methods(loop_definition, relay.as_ref())
        .map_or((None, None), |(methods, titles)| (Some(methods), titles));
    let current = CurrentModel::identified(section);
    let evidence = study_evidence(loop_definition, Some(&current), !kept.recordings.is_empty());
    let mut answer = identify_answer(&kept, loop_definition, &current, evidence)?;
    answer.methods = methods;
    answer.method_titles = method_titles;
    to_value(&answer)
}

/// The experiment a recording was taken with.
fn experiment_kind(method: IdentifyMethod) -> ControlExperimentKind {
    match method {
        IdentifyMethod::Live(kind) | IdentifyMethod::Opened(kind) => kind,
    }
}
