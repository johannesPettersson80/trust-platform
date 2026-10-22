//! The method `controlStudio/export` (specification 35, §0.1a and B10.5): the loop's workflow result as JSON, with the run context of
//! each recording the study keeps (B10.2), or the signal table of one recording as CSV. The engine writes the text; the extension asks
//! the engineer where to save it. Every rule is trust-control's (`trust_control::export`); this module only reads the study the export
//! names.

use std::path::Path;

use serde_json::Value;
use trust_control::export::{
    recording_context, samples_csv, workflow_json_with_recordings, ExportFormat, ExportRequest,
    ExportResult, RecordingContextEntry,
};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::identify_result::recorded_chart_samples;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{OpenRequest, OpenResult};
use trust_control::recording::RecordingFile;

use crate::commands::to_value;
use crate::open_action;
use crate::{loops, storage};

/// Answers `controlStudio/export`: the loop's workflow result as JSON, as `controlStudio/open` answers it, or the signal table of the
/// requested recording (or of the recording the current Identify result was fitted from) as CSV. It writes nothing and starts nothing.
///
/// # Errors
/// The loop cannot be opened; the workflow result or the study cannot be read; the CSV has no recording to show; or the export cannot
/// be written as text.
pub fn answer(request: ExportRequest) -> Result<Value, Failure> {
    let ExportRequest {
        project_root,
        runtime_id,
        loop_id,
        format,
        recording_id,
    } = request;
    let project = Path::new(&project_root);
    let loop_definition = loops::open(project, &runtime_id, &loop_id)?;
    let folder = storage::folder_name(&loop_id);
    let result = match format {
        ExportFormat::Json => {
            // The journal folder is the engine's own (an empty one), which the page's open uses as well (A8.5, A8.6).
            let mut opened = open_action::open(&OpenRequest {
                project_root: project_root.clone(),
                runtime_id: runtime_id.clone(),
                loop_id: Some(loop_id.clone()),
                storage_folder: String::new(),
            })?;
            with_recorded_samples(project, &loop_definition, &mut opened)?;
            let contexts = kept_contexts(project, &loop_definition)?;
            ExportResult {
                format,
                suggested_file_name: format!("{folder}-results.json"),
                text: workflow_json_with_recordings(&opened, &contexts)?,
            }
        }
        ExportFormat::Csv => {
            let file = selected_recording(project, &loop_definition, recording_id)?;
            ExportResult {
                format,
                suggested_file_name: format!("{folder}-samples.csv"),
                text: samples_csv(&loop_definition, file.source, &file.run),
            }
        }
    };
    to_value(&result)
}

/// The identification of the open result with the samples of its recording (B10.3): the samples the Identify chart draws, each with its
/// additional channels, so that the JSON export keeps them as the page shows them. An identification with no recording keeps none.
///
/// # Errors
/// The recording the identification names cannot be read.
fn with_recorded_samples(
    project: &Path,
    loop_definition: &LoopDefinition,
    opened: &mut OpenResult,
) -> Result<(), Failure> {
    if let OpenResult::Opened {
        identification: Some(identification),
        ..
    } = opened
    {
        if let Some(id) = identification.recording_id.clone() {
            let file = storage::read_recording(project, &loop_definition.id, &id)?;
            identification.recorded_samples =
                Some(recorded_chart_samples(loop_definition, &file.run));
        }
    }
    Ok(())
}

/// The run contexts of the recordings the loop's working study lists (B10.2), in the order it lists them: the JSON export reads them
/// from the recording files, so that a recording from an aborted test is exported with its stop reason and its restoration (B10.3).
/// The files are read one at a time, so that a study with many recordings is never held in memory at once.
///
/// # Errors
/// The study cannot be read, or a recording it lists cannot.
fn kept_contexts(
    project: &Path,
    loop_definition: &LoopDefinition,
) -> Result<Vec<RecordingContextEntry>, Failure> {
    let Some(study) =
        storage::current_study(project, &loop_definition.id, &loop_definition.revision)?
    else {
        return Ok(Vec::new());
    };
    let mut contexts = Vec::new();
    for id in &study.recordings {
        let file = storage::read_recording(project, &loop_definition.id, id)?;
        contexts.extend(recording_context(id, &file));
    }
    Ok(contexts)
}

/// The recording the CSV table holds: the one the request names, or else the recording the study's current Identify result was fitted
/// from (B10.2, A8.3).
fn selected_recording(
    project: &Path,
    loop_definition: &LoopDefinition,
    requested: Option<String>,
) -> Result<RecordingFile, Failure> {
    if let Some(id) = requested {
        return storage::read_recording(project, &loop_definition.id, &id);
    }
    let study = storage::current_study(project, &loop_definition.id, &loop_definition.revision)?;
    let stored = match study {
        Some(study) => storage::read_identify(project, &study)?,
        None => None,
    };
    stored.map(|stored| stored.estimation).ok_or_else(|| {
        Failure::new(
            "There is no recording to export: identify the plant first.",
            Cause::Settings,
            NextAction::Identify,
        )
    })
}
