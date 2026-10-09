//! Specification 35, A7 and A8.2 to A8.4: the study files of a project.

use serde_json::json;
use trust_control::limits::STUDY_STORAGE_LIMIT_BYTES;
use trust_control::recording::{EvidenceSource, RecordedRun};

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

fn recording(loop_id: &str) -> RecordingFile {
    let run: RecordedRun = serde_json::from_value(json!({
        "recording_id": "recording-1", "loop_revision": "sha256:loop-1",
        "bindings": [{ "id": "processVariable", "address": "%IL0", "iec_type": "LREAL" }],
        "stop_reason": "sample_limit",
        "samples": [{ "scan": 1, "runtime_time_ns": 100_000_000, "captured_at_ms": 1, "values": [20.5], "forced": [false], "quality": ["good"] }],
        "acquisition": { "method": "runtime_scan_buffer", "completeness": "complete", "observedSampleCount": 1,
            "scanRange": { "first": 1, "last": 1 }, "missingScanRanges": [], "droppedObservedSamples": 0,
            "actualCycleTimingAvailable": true, "freshnessAvailable": false, "reason": "" }
    }))
    .expect("a recorded run");
    RecordingFile::new(loop_id, EvidenceSource::SimulatedPlant, run)
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
