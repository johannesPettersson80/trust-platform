//! The import of a saved Control Studio result or recording file, and of a supplied plant model, answered by
//! `controlStudio/identify` with `importRecording` and `importModel` (specification 35, §0.1a, B3.6, B3.7, B10.2, B10.4 and N4.1).
//! The rules are trust-control's: `recording::opened::opened_recording` checks a file with one recording and turns it into the file
//! the study keeps; `identification::recording_import::archive` reads a file's archived runs, keeps them and pairs the two that are
//! independent (B5.2); and `model::decode_general_model` with `SuppliedModelSection` checks a model. This module keeps what they
//! accept through `storage` and answers the result. A refused file or model keeps nothing and writes nothing; a file whose runs
//! need a second independent recording keeps its runs and answers what it still needs. `openRecording` of an opened recording the
//! study keeps answers at once when that recording is part of an independent pair (`open_kept_recording`).

use std::path::Path;

use serde_json::{json, Value};
use trust_control::experiment::ControlExperimentKind;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::connected_run::fit_failure;
use trust_control::identification::identify_result::{
    identify_answer, identify_signals, study_evidence,
};
use trust_control::identification::recording_import::archive::{
    archived_recording, archived_runs, check_fitted_gain, independent_pair, opened_archived_run,
    ArchivedRun, ArchivedRuns,
};
use trust_control::identification::recording_import::ImportCheck;
use trust_control::identification::refusal::Refusal;
use trust_control::identification::selected_recording::{
    fit_selected_recording_model, SelectedRun,
};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{AnswerKind, IdentifyResult, ImportedRecording};
use trust_control::model::decode_general_model;
use trust_control::recording::opened::opened_recording;
use trust_control::recording::opened_channels::{
    archive_catalog, configured_recording_selection, opened_recording_catalog, workflow_units,
    CatalogChannel, RecordingSignalCatalog, RecordingSignalSelection,
};
use trust_control::recording::{EvidenceSource, RecordingFile};
use trust_control::study::current_model::{current_model, CurrentModel};
use trust_control::study::general_model_persistence::SuppliedModelSection;
use trust_control::study::identify::{IdentifyMethod, IdentifySection, IdentifySignals};
use trust_control::study::inputs::{stage_evidence_of, InputsSection};
use trust_control::study::Study;

use crate::commands::to_value;
use crate::storage;

/// The largest supplied model file an import reads, in bytes of its text (B3.7; the page checks the same limit).
pub const MAXIMUM_MODEL_BYTES: usize = 256_000;

/// The sentence of a file whose archived runs do not yet hold an independent pair (`openedRecordingIdentification.ts`).
const NEEDS_SECOND_RECORDING: &str = "Open an independent recording to check the plant model.";
/// The sentence of a model fitted with independent validation: it was checked against an independent recording (B5.2).
const CHECKED_AGAINST_RECORDING: &str = "The model was checked against an independent recording.";
/// The sentence of a supplied model that reuses the current model's revision with other content (B3.7, A8.4).
const CHANGED_MODEL: &str = "A changed model needs a new revision.";

/// Answers `importRecording`: the file is checked against the loop, and a file that passes is kept as recordings of the study
/// with their source and the opened marker (B10.2, A9). A file of archived runs keeps them and pairs them at once (B3.6, B5.2); a
/// file with one recording keeps it and answers its channel catalogue (B3.6). The answer names the kept recording and the channels
/// it offers.
///
/// # Errors
/// The file is refused by the check (B10.4, B3.6, B3.9, A9), with the sentence it gives and no next action, and keeps nothing; the
/// two recordings do not fit (the fit's sentence); or the study cannot be read or written.
pub fn import_recording(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    json: &str,
) -> Result<ImportedRecording, Failure> {
    match archived_runs(json, loop_definition).map_err(|refusal| refused(&refusal))? {
        Some(archive) => import_archive(project_root, loop_definition, archive),
        None => import_single(project_root, loop_definition, json),
    }
}

/// Keeps a checked file's one recording as the study's recording, with the channels it offers (B3.6, B10.2). The selection is the one
/// the study keeps when the file holds its channels (`kept_selection`).
fn import_single(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    json: &str,
) -> Result<ImportedRecording, Failure> {
    let (check, file) =
        opened_recording(json, loop_definition).map_err(|refusal| refused(&refusal))?;
    let recording_id = file.id()?;
    let units = workflow_units(loop_definition);
    let runs = std::slice::from_ref(&file.run);
    let offered = opened_recording_catalog(&units, loop_definition, runs, None)?;
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let catalog = match kept_selection(&study, &offered.channels)? {
        Some(requested) => {
            opened_recording_catalog(&units, loop_definition, runs, Some(&requested))?
        }
        None => offered,
    };
    let study = with_selection(study, &catalog.selection)?;
    storage::save_recording(project_root, &study, &file)?;
    Ok(ImportedRecording {
        kind: AnswerKind::new(),
        check,
        recording_id,
        recording_catalog: catalog,
        identification: None,
        reason: None,
        validation: None,
        operation_id: None,
    })
}

/// What a pair fit reads and where it keeps its result: the study and its project folder, the source of the recordings, and the
/// plant input and measured output the engineer selected (B5.2, B10.2).
struct PairFit<'a> {
    project_root: &'a Path,
    loop_definition: &'a LoopDefinition,
    study: &'a Study,
    source: EvidenceSource,
    selection: &'a RecordingSignalSelection,
}

/// Keeps the archived runs of a saved result as recordings of the study, and pairs the two that are independent (B3.6, B5.2, B10.2).
/// The candidates are the study's kept opened recordings of the same source, then the file's runs, as
/// `openedRecordingIdentification.ts` takes them. The pair is read under the selection the study keeps when the file holds those
/// channels (`kept_selection`). A file whose runs need a second recording keeps them; a pair that gives no model (a refused fit or a
/// gain the loop's action contradicts) keeps nothing, as a refused file does.
fn import_archive(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    archive: ArchivedRuns,
) -> Result<ImportedRecording, Failure> {
    let ArchivedRuns {
        check,
        source,
        runs,
        channels,
    } = archive;
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let kept = kept_selection(&study, &channels)?;
    let catalog = archive_catalog(channels, loop_definition, kept.as_ref())?;
    let selection = catalog.selection.clone();
    let files = runs
        .iter()
        .map(|run| archived_recording_file(loop_definition, source, run))
        .collect::<Result<Vec<RecordingFile>, Refusal>>()
        .map_err(|refusal| refused(&refusal))?;
    let study = with_selection(study, &selection)?;
    let Some(last) = files.last() else {
        return Err(refused(&Refusal::new(NEEDS_SECOND_RECORDING)));
    };
    let earlier = kept_opened_runs(project_root, &study, loop_definition, source)?;
    let mut candidates: Vec<(String, &ArchivedRun)> =
        earlier.iter().map(|(id, run)| (id.clone(), run)).collect();
    for (file, run) in files.iter().zip(&runs) {
        candidates.push((file.id()?, run));
    }
    let refs: Vec<&ArchivedRun> = candidates.iter().map(|(_, run)| *run).collect();
    let pair = independent_pair(&refs, &selected_units(&catalog));
    if pair.len() < 2 {
        keep_files(project_root, &study, &files)?;
        return Ok(ImportedRecording {
            kind: AnswerKind::new(),
            check: ImportCheck {
                status: "recording-needed",
                ..check
            },
            recording_id: last.id()?,
            recording_catalog: catalog,
            identification: None,
            reason: Some(NEEDS_SECOND_RECORDING.to_string()),
            validation: None,
            operation_id: None,
        });
    }
    let fit = PairFit {
        project_root,
        loop_definition,
        study: &study,
        source,
        selection: &selection,
    };
    let (estimation_id, identification) = fit_pair(&fit, &candidates, &pair, &files)?;
    Ok(ImportedRecording {
        kind: AnswerKind::new(),
        check,
        recording_id: estimation_id,
        recording_catalog: catalog,
        identification: Some(identification),
        reason: None,
        validation: Some(CHECKED_AGAINST_RECORDING.to_string()),
        operation_id: None,
    })
}

/// Fits the independent pair `pair` (two indices into `candidates`) with the selected signals and independent validation, and keeps
/// the Identify section with the recordings `files` (B5.2, B10.2). The section keeps the loop's signals, with the selected channels
/// beside them (B3.6). The fit of `importRecording` and the fit of `openRecording` are this one function. Returns the identifier of
/// the estimation recording and the identification the study now answers.
///
/// # Errors
/// The fit refuses the pair (`fit_failure`), the fitted gain contradicts the loop's controller action (B3.9, N6.3), or the study
/// cannot be written.
fn fit_pair(
    fit: &PairFit<'_>,
    candidates: &[(String, &ArchivedRun)],
    pair: &[usize],
    files: &[RecordingFile],
) -> Result<(String, IdentifyResult), Failure> {
    let loop_definition = fit.loop_definition;
    let (estimation_id, estimation) = (candidates[pair[0]].0.clone(), candidates[pair[0]].1);
    let (validation_id, validation) = (candidates[pair[1]].0.clone(), candidates[pair[1]].1);
    let pair_runs = [selected_run(estimation), selected_run(validation)];
    let fitted = fit_selected_recording_model(loop_definition, &pair_runs, fit.selection)
        .map_err(|error| fit_failure(loop_definition, &error))?;
    if *fit.selection == configured_recording_selection(loop_definition) {
        // The loop's own signals are a controller's plant: their gain must agree with the controller action (B3.9, N6.3).
        check_fitted_gain(loop_definition, &fitted.model).map_err(|refusal| refused(&refusal))?;
    }
    let kind = pair_experiment_kind(estimation);
    let section = IdentifySection::new(
        IdentifyMethod::Opened(kind),
        fit.source,
        identify_signals(loop_definition, kind),
        &estimation_id,
        Some(validation_id.as_str()),
        fitted.model,
        fitted.presentation,
    )
    .with_recorded_channels(IdentifySignals {
        inputs: fit.selection.inputs.clone(),
        outputs: fit.selection.outputs.clone(),
        setpoint: None,
    });
    let kept = storage::save_identify(fit.project_root, fit.study, &section, files)?;
    let current = CurrentModel::identified(section);
    let evidence = study_evidence(loop_definition, Some(&current), !kept.recordings.is_empty());
    let identification = identify_answer(&kept, loop_definition, &current, evidence)?;
    Ok((estimation_id, identification))
}

/// Answers `openRecording` of an opened recording the study keeps, at once, when that recording pairs or needs a second recording
/// (B3.6, B5.2). The study first keeps the selection the request names. When the study keeps an independent pair of opened
/// recordings that holds this recording under that selection, the pair is fitted as `importRecording` fits it. When there is no such
/// pair and the recording is an isolated simulation's run, the answer says that a second recording is needed. `None` for any other
/// recording: an opened connected recording alone is fitted by the operation.
///
/// # Errors
/// The study cannot be read or written, or the pair does not fit (`fit_pair`).
pub fn open_kept_recording(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    recording_id: &str,
    file: &RecordingFile,
    catalog: &RecordingSignalCatalog,
) -> Result<Option<Value>, Failure> {
    let study = with_selection(
        storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?,
        &catalog.selection,
    )?;
    storage::save_study(project_root, &study, &[])?;
    let kept = kept_opened_runs(project_root, &study, loop_definition, file.source)?;
    let candidates: Vec<(String, &ArchivedRun)> =
        kept.iter().map(|(id, run)| (id.clone(), run)).collect();
    let refs: Vec<&ArchivedRun> = candidates.iter().map(|(_, run)| *run).collect();
    let pair = independent_pair(&refs, &selected_units(catalog));
    if pair.len() == 2
        && pair
            .iter()
            .any(|&index| candidates[index].0 == recording_id)
    {
        let fit = PairFit {
            project_root,
            loop_definition,
            study: &study,
            source: file.source,
            selection: &catalog.selection,
        };
        let (estimation_id, identification) = fit_pair(&fit, &candidates, &pair, &[])?;
        return to_value(&ImportedRecording {
            kind: AnswerKind::new(),
            check: opened_check(loop_definition, file, "checked"),
            recording_id: estimation_id,
            recording_catalog: catalog.clone(),
            identification: Some(identification),
            reason: None,
            validation: Some(CHECKED_AGAINST_RECORDING.to_string()),
            operation_id: None,
        })
        .map(Some);
    }
    if file.model_revision.is_some() {
        return to_value(&ImportedRecording {
            kind: AnswerKind::new(),
            check: opened_check(loop_definition, file, "recording-needed"),
            recording_id: recording_id.to_string(),
            recording_catalog: catalog.clone(),
            identification: None,
            reason: Some(NEEDS_SECOND_RECORDING.to_string()),
            validation: None,
            operation_id: None,
        })
        .map(Some);
    }
    Ok(None)
}

/// The plant input and measured output the study keeps for opened recordings, as the request of a catalogue, when the file holds
/// every one of them (B3.6). A file that lacks one of them takes the loop's own signals instead, so the answer is `None`.
fn kept_selection(study: &Study, channels: &[CatalogChannel]) -> Result<Option<Value>, Failure> {
    let Some(kept) = study
        .section::<InputsSection>()?
        .and_then(|inputs| inputs.recording_selection)
    else {
        return Ok(None);
    };
    let holds_all = !kept.inputs.is_empty()
        && !kept.outputs.is_empty()
        && kept
            .inputs
            .iter()
            .chain(&kept.outputs)
            .all(|id| channels.iter().any(|channel| channel.id == *id));
    Ok(holds_all.then(|| json!({ "inputs": kept.inputs, "outputs": kept.outputs })))
}

/// The study's kept opened recordings of `source` that read as runs of the loop, with their identifiers, in the study's order: the
/// earlier runs of `openedRecordingIdentification.ts`. A recording kept from an isolated simulation reads back as its isolated run,
/// and a connected recording as its connected record (`opened_archived_run`); a recording that reads as neither is not a candidate.
fn kept_opened_runs(
    project_root: &Path,
    study: &Study,
    loop_definition: &LoopDefinition,
    source: EvidenceSource,
) -> Result<Vec<(String, ArchivedRun)>, Failure> {
    let mut kept = Vec::new();
    for id in &study.recordings {
        let file = storage::read_recording(project_root, &loop_definition.id, id)?;
        if !file.opened || file.source != source {
            continue;
        }
        if let Some(run) = opened_archived_run(loop_definition, &file) {
            kept.push((id.clone(), run));
        }
    }
    Ok(kept)
}

/// Keeps each file as a recording of the study, and returns the study as it was written (B10.2).
fn keep_files(
    project_root: &Path,
    study: &Study,
    files: &[RecordingFile],
) -> Result<Study, Failure> {
    let mut kept = study.clone();
    for file in files {
        kept = storage::save_recording(project_root, &kept, file)?;
    }
    Ok(kept)
}

/// The file an archived run is kept as: `archived_recording`'s file, with the model revision of an isolated simulation's run, which no
/// other field of the file keeps (B3.6). A connected record's revision is derived from its program revision, so none is kept for it.
fn archived_recording_file(
    loop_definition: &LoopDefinition,
    source: EvidenceSource,
    run: &ArchivedRun,
) -> Result<RecordingFile, Refusal> {
    let mut file = archived_recording(loop_definition, source, run)?;
    if let ArchivedRun::Isolated(isolated) = run {
        file.model_revision = Some(isolated.run.conditions.model_revision.clone());
    }
    Ok(file)
}

/// The run of an archived recording as the fit reads it: its program revision and its run (B5.2).
fn selected_run(run: &ArchivedRun) -> SelectedRun<'_> {
    SelectedRun {
        project_revision: run.project_revision(),
        run: run.run(),
    }
}

/// The experiment kind an opened pair's fit is shown with. A connected record's plan names its kind; an isolated simulation's run
/// names none, so its fit reads its own samples as an observation (B3.6, C12).
fn pair_experiment_kind(estimation: &ArchivedRun) -> ControlExperimentKind {
    match estimation {
        ArchivedRun::Connected(record) => record.request.experiment.kind,
        ArchivedRun::Isolated(_) => ControlExperimentKind::Observation,
    }
}

/// The selected channels of a catalogue with their units, for the eligibility of the archived runs (`openedRecordingIdentification.ts`).
fn selected_units(catalog: &RecordingSignalCatalog) -> Vec<(&str, &str)> {
    catalog
        .selection
        .inputs
        .iter()
        .chain(&catalog.selection.outputs)
        .filter_map(|id| {
            catalog
                .channels
                .iter()
                .find(|channel| channel.id == *id)
                .map(|channel| (channel.id.as_str(), channel.unit.as_str()))
        })
        .collect()
}

/// The study with its inputs recording `selection`, so that reopening shows the signals the engineer chose (B3.6, A8.6).
pub(crate) fn with_selection(
    mut study: Study,
    selection: &RecordingSignalSelection,
) -> Result<Study, Failure> {
    let mut inputs = study.section::<InputsSection>()?.unwrap_or_default();
    inputs.recording_selection = Some(selection.clone());
    study.set_section(&inputs)?;
    Ok(study)
}

/// The check an answer about an opened recording names: the recording itself, with its source and sample count. The study keeps no
/// revision of the saved result the recording came from, so the revision is `0` (B10.2).
fn opened_check(
    loop_definition: &LoopDefinition,
    file: &RecordingFile,
    status: &'static str,
) -> ImportCheck {
    ImportCheck {
        status,
        result_id: file.run.recording_id.clone(),
        result_revision: 0,
        project_id: loop_definition.project_id.clone(),
        source: source_word(file.source).to_string(),
        samples: u64::try_from(file.run.samples.len()).unwrap_or(u64::MAX),
    }
}

/// The word a recording's source takes in a check, as the file writes it: `Simulation` or `Physical` (A9).
fn source_word(source: EvidenceSource) -> &'static str {
    match source {
        EvidenceSource::SimulatedPlant => "Simulation",
        EvidenceSource::PhysicalPlant => "Physical",
    }
}

/// Answers `importModel`: the supplied model is checked as N4.1 requires and kept as the study's model (B3.7). It is provisional
/// unless it carries evidence of independent validation. The answer is the identification result of the current model, as
/// `controlStudio/open` gives it (B3.7), with the stage evidence after the import, so Tune is available on it. A model that reuses the
/// current model's revision is the same model, which changes nothing, or it is refused (A8.4).
///
/// # Errors
/// The text is larger than 256,000 bytes, is not a model of the model contract (N4.1), is a model fitted from a recording (B3.1), or
/// reuses the current model's revision with other content; the refusal has its sentence and no next action. The study cannot be read
/// or written.
pub fn import_model(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    json: &str,
) -> Result<Value, Failure> {
    let section = supplied_section(json).map_err(|refusal| refused(&refusal))?;
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    // The section's own rules come first (B3.1, B10.4): the study checks them when it keeps the model, and so does this copy.
    let mut checked = study.clone();
    checked.set_section(&section)?;
    let (kept, current) = match current_model(&study)? {
        Some(current) if current.model.identity().revision == section.model.identity().revision => {
            if current.model != section.model {
                return Err(Failure::new(
                    CHANGED_MODEL,
                    Cause::Settings,
                    NextAction::None,
                ));
            }
            (study, current)
        }
        _ => {
            let kept = storage::save_supplied_model(project_root, &study, &section)?;
            (kept, CurrentModel::supplied(section))
        }
    };
    let evidence = stage_evidence_of(loop_definition, &kept)?;
    to_value(&identify_answer(
        &kept,
        loop_definition,
        &current,
        evidence,
    )?)
}

/// The supplied model section of the model file `json`: the size limit, the JSON, and the model contract (N4.1). The section's own
/// rules (a model fitted from a recording is refused, B3.1) are applied when the study keeps it.
fn supplied_section(json: &str) -> Result<SuppliedModelSection, Refusal> {
    if json.len() > MAXIMUM_MODEL_BYTES {
        return Err(Refusal::new(
            "Choose a plant model JSON file of at most 256 kB.",
        ));
    }
    let value: Value = serde_json::from_str(json)
        .map_err(|_| Refusal::new("Choose a valid plant model JSON file."))?;
    let model = decode_general_model(&value).ok_or_else(|| {
        Refusal::new(
            "The plant model does not meet the model contract of numerical annex N4.1: check its channels, units, delay, sample time and coefficients.",
        )
    })?;
    Ok(SuppliedModelSection::new(model, None))
}

/// The failure of a refused file or model: its sentence, with the data as its cause and no next action, because the engineer
/// chooses another file.
fn refused(refusal: &Refusal) -> Failure {
    Failure::new(&refusal.0, Cause::ModelOrData, NextAction::None)
}
