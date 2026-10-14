//! Specification 35, A7 and A8.2 to A8.4: the study files of a project, with the Identify result and the supplied model
//! (`control-studio-study-store.test.ts` and `control-studio-engineer-study-persistence.test.ts`).

use std::collections::BTreeMap;

use serde_json::{json, Value};
use trust_control::experiment::ControlExperimentKind;
use trust_control::identification::model_comparison::{
    presentation_describes_model, ComparisonPresentation,
};
use trust_control::limits::STUDY_STORAGE_LIMIT_BYTES;
use trust_control::model::{decode_general_model, GeneralModel};
use trust_control::recording::evidence::{recording_evidence, UNCONFIRMED_START_RECOVERED};
use trust_control::recording::{Completeness, EvidenceSource, RecordedRun};
use trust_control::study::current_model::{current_model, CurrentModelSource};
use trust_control::study::general_model_persistence::{
    Capability, ChannelFrequencyResponse, FrequencyCapabilities, FrequencyPoint,
    GeneralModelFrequencyResult, ModelFrequencyGrid, ModelPole, SuppliedModelAnalysis,
};
use trust_control::study::identify::{IdentifyMethod, IdentifySignals};
use trust_control::study::StudySection;

use super::*;

fn project(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "trust-control-storage-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("project folder");
    root
}

/// A complete run of two consecutive samples; `first` makes the runs of different recordings differ.
fn recorded_run(first: f64) -> RecordedRun {
    serde_json::from_value(json!({
        "recording_id": "recording-1", "loop_revision": "sha256:loop-1",
        "bindings": [{ "id": "processVariable", "address": "%IL0", "iec_type": "LREAL" }],
        "stop_reason": "sample_limit",
        "samples": [
            { "scan": 1, "runtime_time_ns": 100_000_000, "captured_at_ms": 1, "values": [first], "forced": [false], "quality": ["good"] },
            { "scan": 2, "runtime_time_ns": 200_000_000, "captured_at_ms": 2, "values": [first + 0.5], "forced": [false], "quality": ["good"] }
        ],
        "acquisition": { "method": "runtime_scan_buffer", "completeness": "complete", "observedSampleCount": 2,
            "scanRange": { "first": 1, "last": 2 }, "missingScanRanges": [], "droppedObservedSamples": 0,
            "actualCycleTimingAvailable": true, "freshnessAvailable": false, "reason": "" }
    }))
    .expect("a recorded run")
}

/// The recording file of a run that the runtime stopped, with the verdict it earned (E4.5).
fn recording_of(loop_id: &str, source: EvidenceSource, first: f64) -> RecordingFile {
    let run = recorded_run(first);
    let evidence = recording_evidence(&run, None);
    RecordingFile::new(loop_id, source, run, evidence)
}

fn recording(loop_id: &str) -> RecordingFile {
    recording_of(loop_id, EvidenceSource::SimulatedPlant, 20.5)
}

/// A recording whose start acknowledgement was lost before the host reloaded: incomplete whatever the runtime claims.
fn unconfirmed_recording(loop_id: &str) -> RecordingFile {
    let run = recorded_run(20.5);
    let evidence = recording_evidence(&run, Some(UNCONFIRMED_START_RECOVERED));
    RecordingFile::new(loop_id, EvidenceSource::SimulatedPlant, run, evidence)
}

fn channel(id: &str, unit: &str, maximum: f64) -> Value {
    json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -10.0, "maximum": maximum } })
}

/// A fitted model `1 / (s + 1)` from a command in V to a position in m (the model of the `TypeScript` persistence test).
fn fitted_model(validation: &str, evidence: &[&str]) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "model:fitted:1", "source": "fitted",
        "validation": validation, "evidence": evidence, "delaySeconds": 0.0,
        "inputs": [channel("command", "V", 10.0)], "outputs": [channel("position", "m", 10.0)],
        "numerator": [1.0], "denominator": [1.0, 1.0]
    }))
    .expect("a fitted model")
}

/// What the page shows of `model`: the first-order fit of the `TypeScript` test, in the model's units.
fn comparison_of(model: &GeneralModel) -> ComparisonPresentation {
    let value = json!({
            "equation": "G(s) = 1 / (1s + 1)",
            "units": { "input": "V", "output": "m", "gain": "m/V", "time": "s" },
            "parameters": [
                { "name": "Gain", "value": 1.0, "unit": "m/V" },
                { "name": "Time constant 1", "value": 1.0, "unit": "s" },
                { "name": "Dead time", "value": 0.0, "unit": "s" }
            ],
            "poles": [-1.0], "zeros": [], "delaySeconds": 0.0,
            "uncertainty": { "label": "Sample-resolution envelope from the estimation record; not statistical confidence" },
            "estimation": { "normalizedError": 0.1 }, "validation": {},
            "predictedResponse": [
                { "timeSeconds": 0.0, "measured": 0.0, "predicted": 0.0, "residual": 0.0 },
                { "timeSeconds": 2.0, "measured": 1.0, "predicted": 0.9, "residual": 0.1 }
            ],
            "validityRange": { "inputMinimum": 0.0, "inputMaximum": 1.0, "outputMinimum": 0.0, "outputMaximum": 1.0 },
            "warnings": []
    });
    let comparison: ComparisonPresentation =
        trust_control::json::decode_value(value).expect("the presentation decodes");
    assert!(
        presentation_describes_model(&comparison, model),
        "the presentation describes its model"
    );
    comparison
}

/// The result of a live step fitted on `estimation`: provisional, or validated on the distinct `validation`.
fn identify_section(
    estimation: &RecordingFile,
    validation: Option<&RecordingFile>,
) -> IdentifySection {
    let model = match validation {
        None => fitted_model("unvalidated", &["sha256:recording"]),
        Some(_) => fitted_model("validated", &["sha256:estimation", "sha256:validation"]),
    };
    let comparison = comparison_of(&model);
    let id_of_validation = validation.map(|file| file.id().expect("identified"));
    IdentifySection::new(
        IdentifyMethod::Live(ControlExperimentKind::MvStep),
        EvidenceSource::SimulatedPlant,
        IdentifySignals {
            inputs: vec!["Plant.Command".into()],
            outputs: vec!["Plant.Position".into()],
            setpoint: Some("Plant.Target".into()),
        },
        &estimation.id().expect("identified"),
        id_of_validation.as_deref(),
        model,
        comparison,
    )
}

/// The supplied model of the session test: `2 / (s + 2)` from a drive in V to a speed in rpm.
fn supplied_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "m", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("drive", "V", 10.0)], "outputs": [channel("speed", "rpm", 100.0)],
        "numerator": [2.0], "denominator": [1.0, 2.0]
    }))
    .expect("the supplied model")
}

/// The analysis of the supplied model on the grid 1 to 10 rad/s with three points: `G(jω) = 2 / (2 + jω)`.
fn supplied_analysis(model: &GeneralModel) -> SuppliedModelAnalysis {
    let grid = ModelFrequencyGrid {
        minimum_frequency: 1.0,
        maximum_frequency: 10.0,
        points: 3,
    };
    let identity = model.identity();
    let points = (0..grid.points)
        .map(|index| {
            let omega = grid.frequency_at(index);
            let scale = 4.0 + omega * omega;
            let (real, imaginary) = (4.0 / scale, -2.0 * omega / scale);
            FrequencyPoint {
                frequency_rad_per_second: omega,
                real,
                imaginary,
                magnitude: real.hypot(imaginary),
                phase_radians: imaginary.atan2(real),
            }
        })
        .collect::<Vec<_>>();
    SuppliedModelAnalysis {
        grid,
        result: GeneralModelFrequencyResult {
            model: model.clone(),
            channels: vec![ChannelFrequencyResponse {
                input: identity.inputs[0].clone(),
                output: identity.outputs[0].clone(),
                points,
            }],
            poles: vec![ModelPole {
                real: -2.0,
                imaginary: 0.0,
            }],
            stable: true,
            pole_scope: "Model poles only.".into(),
            relative_gain_matrix: Some(vec![vec![1.0]]),
            state_structure: None,
            capabilities: FrequencyCapabilities {
                relative_gain: Capability {
                    available: true,
                    reason: None,
                },
                step_response: Capability {
                    available: false,
                    reason: Some("Select a time-response adapter.".into()),
                },
                transfer_conversion: Capability {
                    available: true,
                    reason: None,
                },
            },
        },
        response: None,
    }
}

/// Every file under a folder, keyed by its relative path, with its exact content (the `TypeScript` test's snapshot).
fn snapshot(directory: &Path) -> BTreeMap<String, String> {
    fn visit(root: &Path, current: &Path, files: &mut BTreeMap<String, String>) {
        for entry in fs::read_dir(current).expect("a folder") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                let name = path
                    .strip_prefix(root)
                    .expect("inside")
                    .display()
                    .to_string();
                files.insert(name, fs::read_to_string(&path).expect("a text file"));
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(directory, directory, &mut files);
    files
}

fn recording_path(root: &Path, file: &RecordingFile) -> PathBuf {
    study_directory(root, &file.loop_id)
        .join("recordings")
        .join(format!("{}.json", file.id().expect("identified")))
}

#[test]
fn a_saved_study_and_its_recording_are_read_back_unchanged() {
    let root = project("round-trip");
    let mut study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    assert!(study.sections.is_empty() && study.recordings.is_empty());
    let file = recording("loop:tank");
    let id = file.id().expect("identified");
    study.recordings.push(id.clone());
    study
        .sections
        .insert("identify".into(), json!({ "recording": id }));
    save_study(&root, &study, std::slice::from_ref(&file)).expect("saved");
    assert_eq!(
        open_study(&root, "loop:tank", "sha256:loop-1").expect("reopened"),
        study
    );
    assert_eq!(read_recording(&root, "loop:tank", &id).expect("read"), file);
    let stored = study_directory(&root, "loop:tank")
        .join("recordings")
        .join(format!("{id}.json"));
    let written = fs::metadata(&stored)
        .and_then(|metadata| metadata.modified())
        .expect("written");
    save_study(&root, &study, std::slice::from_ref(&file)).expect("saved again");
    assert_eq!(
        fs::metadata(&stored)
            .and_then(|metadata| metadata.modified())
            .expect("kept"),
        written,
        "a recording file is never rewritten"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_new_loop_revision_keeps_the_earlier_study_as_history_and_starts_a_new_one() {
    let root = project("history");
    let mut study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let file = recording("loop:tank");
    study.recordings.push(file.id().expect("identified"));
    save_study(&root, &study, &[file]).expect("saved");
    let next = open_study(&root, "loop:tank", "sha256:loop-2").expect("the new revision's study");
    assert_eq!(next, Study::new("loop:tank", "sha256:loop-2"));
    let history = study_directory(&root, "loop:tank")
        .join("history")
        .join(folder_name("sha256:loop-1"));
    assert!(
        history.join("study.json").is_file(),
        "the earlier study stays readable"
    );
    assert!(
        history.join("recordings").is_dir(),
        "its recordings move with it and are never deleted"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_study_of_an_earlier_format_moves_to_history_and_a_new_study_starts() {
    let root = project("earlier-format");
    let directory = study_directory(&root, "loop:tank");
    fs::create_dir_all(&directory).expect("loop folder");
    let earlier = r#"{"schemaVersion":1,"loop":{"id":"loop:tank"},"state":{}}"#;
    fs::write(directory.join("study.json"), earlier).expect("an earlier study");
    assert_eq!(
        open_study(&root, "loop:tank", "sha256:loop-1").expect("opened"),
        Study::new("loop:tank", "sha256:loop-1")
    );
    let kept = fs::read_to_string(
        directory
            .join("history")
            .join("earlier-format")
            .join("study.json"),
    )
    .expect("kept as history");
    assert_eq!(
        kept, earlier,
        "the earlier study is kept unchanged, never deleted"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn reset_starts_a_new_working_study_and_keeps_the_previous_one() {
    let root = project("reset");
    let mut study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    study
        .sections
        .insert("identify".into(), json!({ "model": "first-order" }));
    save_study(&root, &study, &[]).expect("saved");
    let fresh = start_new_study(&root, &study).expect("reset");
    assert!(fresh.sections.is_empty());
    assert_eq!(
        open_study(&root, "loop:tank", "sha256:loop-1").expect("reopened"),
        fresh
    );
    assert!(study_directory(&root, "loop:tank")
        .join("history")
        .join(folder_name("sha256:loop-1"))
        .join("study.json")
        .is_file());
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn nothing_is_written_over_the_storage_limit_or_for_a_recording_the_study_does_not_hold() {
    let root = project("limit");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let unreferenced = save_study(&root, &study, &[recording("loop:tank")]).expect_err("refused");
    assert!(
        unreferenced.message.contains("not part of the study"),
        "{}",
        unreferenced.message
    );
    let directory = study_directory(&root, "loop:tank");
    fs::create_dir_all(&directory).expect("loop folder");
    // A sparse file takes no disk space but counts its full length.
    fs::File::create(directory.join("filler"))
        .and_then(|file| file.set_len(STUDY_STORAGE_LIMIT_BYTES))
        .expect("filler");
    let failure = save_study(&root, &study, &[]).expect_err("over the limit");
    assert!(failure.message.contains("257 MiB"), "{}", failure.message);
    assert!(
        !directory.join("study.json").exists(),
        "nothing was written"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_loop_identifier_becomes_a_safe_folder_name() {
    assert_eq!(folder_name("tank-1.level_a"), "tank-1.level_a");
    let named = folder_name("loop:tank/level");
    assert!(
        named.starts_with("loop_tank_level-") && named.len() == "loop_tank_level-".len() + 8,
        "{named}"
    );
    assert!(
        folder_name("..").starts_with("loop-"),
        "a dot-only name never names a parent folder"
    );
    assert_ne!(
        folder_name("a:b"),
        folder_name("a/b"),
        "the hash keeps different identifiers apart"
    );
}

#[test]
fn an_identify_result_and_the_recording_it_came_from_are_read_back_unchanged() {
    let root = project("identify-round-trip");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let estimation = recording("loop:tank");
    let section = identify_section(&estimation, None);
    assert!(
        section.provisional,
        "a fit from one recording is provisional"
    );
    let text = section.encode().expect("encoded");
    let saved =
        save_identify(&root, &study, &section, std::slice::from_ref(&estimation)).expect("saved");
    assert_eq!(
        saved.recordings,
        vec![estimation.id().expect("identified")],
        "the study lists the recording the result came from"
    );

    // One study document and one recording file; the document refers to the recording by its identifier.
    let directory = study_directory(&root, "loop:tank");
    assert!(directory.join("study.json").is_file());
    assert_eq!(
        fs::read_dir(directory.join("recordings"))
            .expect("recordings")
            .count(),
        1,
        "one recording is one file"
    );

    let reopened = open_study(&root, "loop:tank", "sha256:loop-1").expect("reopened");
    assert_eq!(reopened, saved);
    let stored = read_identify(&root, &reopened)
        .expect("read")
        .expect("a result");
    assert_eq!(
        stored.section.encode().expect("again"),
        text,
        "identical canonical JSON"
    );
    assert_eq!(stored.estimation, estimation);
    assert_eq!(stored.validation, None);
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_validated_result_keeps_both_its_recordings_and_a_stored_recording_need_not_be_given_again() {
    let root = project("identify-validated");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let estimation = recording("loop:tank");
    let validation = recording_of("loop:tank", EvidenceSource::SimulatedPlant, 30.0);
    let section = identify_section(&estimation, Some(&validation));
    assert!(!section.provisional, "a distinct validation recording");
    let saved = save_identify(
        &root,
        &study,
        &section,
        &[estimation.clone(), validation.clone()],
    )
    .expect("saved");
    assert_eq!(saved.recordings.len(), 2);
    let stored = read_identify(&root, &saved)
        .expect("read")
        .expect("a result");
    assert_eq!(stored.estimation, estimation);
    assert_eq!(stored.validation, Some(validation));

    // Saving the result again names recordings that are in the project already.
    let again = save_identify(&root, &saved, &section, &[]).expect("saved again");
    assert_eq!(again, saved);
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn reopening_restores_the_identify_result_from_its_files_and_writes_nothing() {
    let root = project("identify-reopen");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let estimation = recording("loop:tank");
    let section = identify_section(&estimation, None);
    save_identify(&root, &study, &section, std::slice::from_ref(&estimation)).expect("saved");
    let before = snapshot(&root.join("control-studio"));
    let reopened = open_study(&root, "loop:tank", "sha256:loop-1").expect("reopened");
    let stored = read_identify(&root, &reopened)
        .expect("read")
        .expect("a result");
    assert_eq!(stored.section, section);
    assert_eq!(
        snapshot(&root.join("control-studio")),
        before,
        "opening an unchanged study writes no file"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_recording_file_that_exists_is_never_rewritten_by_a_later_identify_save() {
    let root = project("identify-never-rewritten");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let estimation = recording("loop:tank");
    let section = identify_section(&estimation, None);
    let saved =
        save_identify(&root, &study, &section, std::slice::from_ref(&estimation)).expect("saved");
    let file = recording_path(&root, &estimation);
    let mut text = fs::read_to_string(&file).expect("the recording file");
    text.push_str("\n// kept as written\n");
    fs::write(&file, &text).expect("a mark in the file");
    save_identify(&root, &saved, &section, std::slice::from_ref(&estimation)).expect("saved again");
    assert_eq!(
        fs::read_to_string(&file).expect("the recording file"),
        text,
        "a later save of the same recording leaves its file unchanged"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_new_loop_revision_keeps_the_identify_result_as_history_and_starts_without_it() {
    let root = project("identify-history");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let estimation = recording("loop:tank");
    let section = identify_section(&estimation, None);
    save_identify(&root, &study, &section, std::slice::from_ref(&estimation)).expect("saved");
    let next = open_study(&root, "loop:tank", "sha256:loop-2").expect("the new revision's study");
    assert_eq!(next, Study::new("loop:tank", "sha256:loop-2"));
    let history = study_directory(&root, "loop:tank")
        .join("history")
        .join(folder_name("sha256:loop-1"));
    assert!(
        history
            .join("recordings")
            .join(format!("{}.json", estimation.id().expect("identified")))
            .is_file(),
        "its recording moves with it and is never deleted"
    );
    let archived = Study::decode(
        &fs::read_to_string(history.join("study.json")).expect("the earlier study"),
        "loop:tank",
    )
    .expect("the earlier study stays readable");
    assert_eq!(
        archived.section::<IdentifySection>().expect("read"),
        Some(section),
        "the earlier result stays in the history"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn reset_keeps_the_identify_result_in_history_and_starts_an_empty_study() {
    let root = project("identify-reset");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let estimation = recording("loop:tank");
    let section = identify_section(&estimation, None);
    let saved =
        save_identify(&root, &study, &section, std::slice::from_ref(&estimation)).expect("saved");
    let fresh = start_new_study(&root, &saved).expect("reset");
    assert_eq!(fresh, Study::new("loop:tank", "sha256:loop-1"));
    assert_eq!(read_identify(&root, &fresh).expect("nothing to read"), None);
    let history = study_directory(&root, "loop:tank")
        .join("history")
        .join(folder_name("sha256:loop-1"));
    assert!(history.join("study.json").is_file());
    assert!(history
        .join("recordings")
        .join(format!("{}.json", estimation.id().expect("identified")))
        .is_file());
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn nothing_is_written_for_a_new_section_or_recording_over_the_storage_limit() {
    let root = project("identify-limit");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let directory = study_directory(&root, "loop:tank");
    fs::create_dir_all(&directory).expect("loop folder");
    // A sparse file takes no disk space but counts its full length.
    fs::File::create(directory.join("filler"))
        .and_then(|file| file.set_len(STUDY_STORAGE_LIMIT_BYTES))
        .expect("filler");
    let estimation = recording("loop:tank");
    let section = identify_section(&estimation, None);
    let failure = save_identify(&root, &study, &section, std::slice::from_ref(&estimation))
        .expect_err("over the limit");
    assert!(failure.message.contains("257 MiB"), "{}", failure.message);
    assert!(
        !directory.join("study.json").exists(),
        "no study document was written"
    );
    assert!(
        !directory.join("recordings").exists(),
        "no recording file was written"
    );

    let supplied = SuppliedModelSection::new(supplied_model(), None);
    let failure = save_supplied_model(&root, &study, &supplied).expect_err("over the limit");
    assert!(failure.message.contains("257 MiB"), "{}", failure.message);
    let failure = save_recording(&root, &study, &estimation).expect_err("over the limit");
    assert!(failure.message.contains("257 MiB"), "{}", failure.message);
    assert!(
        !directory.join("study.json").exists() && !directory.join("recordings").exists(),
        "nothing was written"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn an_identify_result_whose_recording_is_missing_or_does_not_match_is_neither_written_nor_read() {
    let root = project("identify-refused");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let directory = study_directory(&root, "loop:tank");
    let estimation = recording("loop:tank");
    let section = identify_section(&estimation, None);

    // The recording the section names is neither given nor stored, or another recording is given.
    let missing = save_identify(&root, &study, &section, &[]).expect_err("refused");
    assert!(missing.message.contains("missing"), "{}", missing.message);
    let other = recording_of("loop:tank", EvidenceSource::SimulatedPlant, 99.0);
    let wrong = save_identify(&root, &study, &section, &[other]).expect_err("refused");
    assert!(wrong.message.contains("missing"), "{}", wrong.message);

    // Physical evidence is not simulated evidence (A9).
    let physical = recording_of("loop:tank", EvidenceSource::PhysicalPlant, 20.5);
    let mixed = identify_section(&physical, None);
    let failure = save_identify(&root, &study, &mixed, &[physical]).expect_err("refused");
    assert!(
        failure.message.contains("physical plant"),
        "{}",
        failure.message
    );

    // A recording whose start was not confirmed stays labelled incomplete, and no model came from it (B10.3).
    let incomplete = unconfirmed_recording("loop:tank");
    let from_incomplete = identify_section(&incomplete, None);
    let failure =
        save_identify(&root, &study, &from_incomplete, &[incomplete]).expect_err("refused");
    assert!(
        failure.message.contains("not complete evidence"),
        "{}",
        failure.message
    );
    assert!(
        !directory.join("study.json").exists() && !directory.join("recordings").exists(),
        "nothing was written"
    );

    // A saved result whose recording file is lost, or replaced by another content, is not read.
    let saved =
        save_identify(&root, &study, &section, std::slice::from_ref(&estimation)).expect("saved");
    let file = recording_path(&root, &estimation);
    fs::write(&file, "{}").expect("a replaced file");
    assert!(read_identify(&root, &saved).is_err(), "replaced");
    fs::remove_file(&file).expect("removed");
    let lost = read_identify(&root, &saved).expect_err("refused");
    assert!(lost.message.contains("missing"), "{}", lost.message);
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_recording_without_a_result_keeps_its_verdict_after_a_reload() {
    // A run that ended without a model stays available, labelled (B10.3; E11.8).
    let root = project("recording-verdict");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let incomplete = unconfirmed_recording("loop:tank");
    let id = incomplete.id().expect("identified");
    let saved = save_recording(&root, &study, &incomplete).expect("saved");
    assert_eq!(saved.recordings, vec![id.clone()]);
    assert_eq!(saved.sections, BTreeMap::new(), "no result refers to it");
    let reread = read_recording(&root, "loop:tank", &id).expect("read");
    assert_eq!(reread.evidence.completeness, Completeness::Incomplete);
    assert!(
        reread
            .evidence
            .reason
            .starts_with("Start acknowledgement unconfirmed"),
        "{}",
        reread.evidence.reason
    );
    assert_eq!(reread, incomplete);
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn the_supplied_model_is_written_and_read_back_unchanged() {
    let root = project("supplied-model");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");

    // An unanalysed supplied model.
    let unanalysed = SuppliedModelSection::new(supplied_model(), None);
    let saved = save_supplied_model(&root, &study, &unanalysed).expect("saved");
    let reopened = open_study(&root, "loop:tank", "sha256:loop-1").expect("reopened");
    assert_eq!(reopened, saved);
    let read = read_supplied_model(&reopened)
        .expect("read")
        .expect("a supplied model");
    assert_eq!(
        read.encode().expect("again"),
        unanalysed.encode().expect("encoded"),
        "identical canonical JSON"
    );

    // The same model with its analysis replaces it.
    let model = supplied_model();
    let analysed = SuppliedModelSection::new(model.clone(), Some(supplied_analysis(&model)));
    let saved = save_supplied_model(&root, &reopened, &analysed).expect("saved");
    let reopened = open_study(&root, "loop:tank", "sha256:loop-1").expect("reopened");
    assert_eq!(reopened, saved);
    let read = read_supplied_model(&reopened)
        .expect("read")
        .expect("a supplied model");
    assert_eq!(read, analysed);
    assert_eq!(
        read.encode().expect("again"),
        analysed.encode().expect("encoded")
    );
    assert_eq!(
        saved.recordings,
        Vec::<String>::new(),
        "a supplied model comes from no recording"
    );

    // A forged analysis is not written.
    let mut forged = supplied_analysis(&model);
    forged.result.stable = false;
    let refused = save_supplied_model(
        &root,
        &reopened,
        &SuppliedModelSection::new(model, Some(forged)),
    )
    .expect_err("refused");
    assert!(
        refused.message.contains("stability flag"),
        "{}",
        refused.message
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_recording_identifier_is_never_used_as_a_path() {
    let root = project("identifier");
    for id in ["../study", "../../etc/passwd", "rec-../../x", ""] {
        let failure = read_recording(&root, "loop:tank", id).expect_err("refused");
        assert!(
            failure.message.contains("not a recording identifier"),
            "{}",
            failure.message
        );
    }
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn an_atomic_write_replaces_the_file_whole_and_leaves_no_temporary_file_behind() {
    let root = project("atomic");
    let file = root.join("nested").join("file.json");
    write_atomic(&file, "one").expect("the first write");
    write_atomic(&file, "two").expect("the second write");
    assert_eq!(fs::read_to_string(&file).expect("the file"), "two");
    let names: Vec<String> = fs::read_dir(file.parent().expect("a folder"))
        .expect("the folder")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, vec!["file.json".to_string()]);
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_failed_atomic_write_keeps_the_previous_content_and_leaves_no_temporary_file_behind() {
    let root = project("atomic-failed");
    let file = root.join("target.json");
    fs::create_dir(&file).expect("a folder where the file belongs");
    let failure = write_atomic(&file, "data").expect_err("refused");
    assert!(
        failure.message.contains("could not be stored"),
        "{}",
        failure.message
    );
    let names: Vec<String> = fs::read_dir(&root)
        .expect("the folder")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        names,
        vec!["target.json".to_string()],
        "the failed write leaves only the existing entry"
    );
    assert!(file.is_dir(), "the existing entry is untouched");
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn the_earlier_study_in_history_keeps_the_loop_revision_it_was_written_for() {
    // control-studio-study-store.test.ts (A8.2): a changed binding revision keeps the old study as history, and the
    // history keeps the revision it was written for.
    let root = project("history-revision");
    let mut study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let file = recording("loop:tank");
    study.recordings.push(file.id().expect("identified"));
    save_study(&root, &study, &[file]).expect("saved");
    open_study(&root, "loop:tank", "sha256:loop-2").expect("the new revision's study");
    let kept = fs::read_to_string(
        study_directory(&root, "loop:tank")
            .join("history")
            .join(folder_name("sha256:loop-1"))
            .join("study.json"),
    )
    .expect("the earlier study");
    let archived = Study::decode(&kept, "loop:tank").expect("the earlier study stays readable");
    assert_eq!(
        archived.loop_revision, "sha256:loop-1",
        "the history keeps the revision it was written for"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn a_supplied_model_without_an_identify_result_is_valid_and_reads_no_result() {
    // control-studio-saved-model-authority.test.ts: a supplied model without an identification study stays valid.
    let root = project("supplied-without-identify");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let section = SuppliedModelSection::new(supplied_model(), None);
    let saved = save_supplied_model(&root, &study, &section).expect("saved");
    assert!(
        read_identify(&root, &saved).expect("read").is_none(),
        "the study holds no Identify result"
    );
    assert_eq!(
        read_supplied_model(&saved).expect("read"),
        Some(section),
        "the supplied model reads back whole"
    );
    fs::remove_dir_all(&root).expect("clean up");
}

#[test]
fn the_study_storage_limit_is_256_mib() {
    // control-studio-study-store.test.ts: the limit is the study storage limit of A7 (spec 35, A7: 256 MiB).
    assert_eq!(
        STUDY_STORAGE_LIMIT_BYTES,
        256 * 1024 * 1024,
        "the limit is the study storage limit of A7"
    );
}

#[test]
fn the_current_model_is_the_last_one_kept_and_reset_clears_both() {
    let root = project("current-model");
    let study = open_study(&root, "loop:tank", "sha256:loop-1").expect("a new study");
    let supplied = SuppliedModelSection::new(supplied_model(), None);
    let imported =
        save_supplied_model(&root, &study, &supplied).expect("the supplied model is kept");
    let source = |study: &Study| {
        current_model(study)
            .expect("reads")
            .expect("a current model")
            .source
    };
    assert_eq!(imported.model_source, Some(CurrentModelSource::Supplied));
    assert_eq!(source(&imported), CurrentModelSource::Supplied);

    let estimation = recording("loop:tank");
    let section = identify_section(&estimation, None);
    let fitted = save_identify(
        &root,
        &imported,
        &section,
        std::slice::from_ref(&estimation),
    )
    .expect("the fit is kept");
    assert_eq!(fitted.model_source, Some(CurrentModelSource::Identify));
    assert_eq!(
        source(&fitted),
        CurrentModelSource::Identify,
        "a later Identify makes its fit current (A8.4)"
    );

    let reimported =
        save_supplied_model(&root, &fitted, &supplied).expect("the model is kept again");
    assert_eq!(source(&reimported), CurrentModelSource::Supplied);

    let fresh = start_new_study(&root, &reimported).expect("reset");
    assert_eq!(
        current_model(&fresh).expect("reads"),
        None,
        "Reset clears both models (A8.4)"
    );
    fs::remove_dir_all(&root).expect("clean up");
}
