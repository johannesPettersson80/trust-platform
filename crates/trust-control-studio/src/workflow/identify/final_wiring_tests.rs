//! Tests of the keep before the release on the connected run, through the study files of `storage.rs`
//! (`control-studio-final-wiring.test.ts`, test 7 "a Test recording is kept in the study before its runtime buffer is
//! released" and test 8 "a failed keep leaves the runtime buffer for the next Stop").
//!
//! The `TypeScript` tests check the Test page's recording owner, whose Rust counterpart is tested in
//! `runtime_client/recording_coordinator_tests.rs`. The connected run keeps its recording by the same rule (E4.7, E11.6), so
//! these tests apply their assertions to it, against the fake runtime of `identify::test_support`: the run keeps its stopped
//! recording as a recording file of the loop's study, with the verdict that labels it (A8.3, B10.3), before the runtime buffer
//! is released; a keep that the study refuses leaves the buffer and the journal, and the engineer's Cancel keeps the recording
//! later and only then releases it. The other tests of the file wire VS Code, the page or the Test page and are not ported.

use std::fs;
use std::path::PathBuf;

use trust_control::failure::{Cause, Failure};
use trust_control::recording::evidence::RecordingEvidence;
use trust_control::recording::{Completeness, EvidenceSource, RecordedRun, RecordingFile};

use super::test_support::{
    assert_no_model_of_a_held_input, cancel_journaled, empty_journal, folder, passes, reopened,
    run_tank, tank_loop, FakeClock, FakeRuntime, FakeState, StartBehaviour, Study,
};
use super::{IdentifyRequest, RecordingRetention, RunPurpose};
use crate::journal::JournalEntry;
use crate::storage::{open_study, read_recording, save_recording, study_directory};

/// The tank's study in a project, written through `storage.rs`, the only writer of study files (A8.3): each stopped recording
/// becomes a recording file of the working study, with the verdict that labels it (B10.3). Only a recording the storage kept
/// reaches the test support's study, whose log records the keep; a keep the storage refuses fails before the log.
struct ProjectStudy {
    log: Study,
    project_root: PathBuf,
}

impl RecordingRetention for ProjectStudy {
    fn retain(&mut self, run: &RecordedRun, evidence: &RecordingEvidence) -> Result<(), Failure> {
        let study = open_study(&self.project_root, "tank", &run.loop_revision)?;
        let file = RecordingFile::new(
            "tank",
            EvidenceSource::SimulatedPlant,
            run.clone(),
            evidence.clone(),
        );
        save_recording(&self.project_root, &study, &file)?;
        self.log.retain(run, evidence)
    }
}

/// Test 7: an identification stops its recording, keeps it in the loop's study, and only then releases the runtime buffer.
/// The study reopens with the recording file, which holds the run the runtime returned and the verdict that labels it.
#[test]
fn an_identification_keeps_its_recording_in_the_study_with_its_verdict_before_releasing_the_runtime_buffer(
) {
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let journal_folder = folder("wiring-keep-journal");
    let project = folder("wiring-keep-project");
    let journal = empty_journal(&journal_folder);
    let mut study = ProjectStudy {
        log: Study::new(&state),
        project_root: project.clone(),
    };
    let loop_definition = tank_loop("rev-1");
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let Err(failure) = run_tank(&state, &journal, &mut study, &request, RunPurpose::Identify)
    else {
        panic!("the fit of the held plant input finds no model");
    };
    assert_no_model_of_a_held_input(&failure);
    let kept: Vec<String> = state
        .operations()
        .into_iter()
        .filter(|operation| {
            ["recording.stop", "retain", "recording.release"].contains(&operation.as_str())
        })
        .collect();
    assert_eq!(
        kept,
        ["recording.stop", "retain", "recording.release"],
        "the study keeps the samples before the runtime buffer is released, once"
    );
    let started = state.recording_starts();
    assert_eq!(started.len(), 1, "one recording is started");
    let reopened_study = open_study(&project, "tank", "rev-1").expect("the study reopens");
    assert_eq!(
        reopened_study.recordings.len(),
        1,
        "the study lists the kept recording"
    );
    let file = read_recording(&project, "tank", &reopened_study.recordings[0])
        .expect("the recording file reads back");
    assert_eq!(
        file.run.recording_id, started[0].recording_id,
        "the file holds the run of the recording the identification started"
    );
    assert_eq!(file.source, EvidenceSource::SimulatedPlant);
    assert_eq!(
        file.evidence.completeness,
        Completeness::Complete,
        "the verdict is kept with the run"
    );
    assert_eq!(
        file.evidence.reason,
        format!(
            "Complete recording — runtime captured {} scans. Stop: stopped.",
            file.run.samples.len()
        )
    );
    assert_eq!(
        reopened(&journal_folder),
        Vec::<JournalEntry>::new(),
        "nothing is left owned"
    );
    fs::remove_dir_all(&journal_folder).expect("clean up");
    fs::remove_dir_all(&project).expect("clean up");
}

/// Test 8: when the study cannot be written, the identification releases nothing: the runtime keeps the buffer, and the
/// campaign stays journaled with its recording and its experiment. Once the study can be written again, the engineer's Cancel
/// stops the recording, keeps it, and only then releases it and clears the journal.
#[test]
fn a_recording_the_study_cannot_keep_stays_in_the_runtime_and_journaled_until_the_cancel_keeps_it()
{
    let state = FakeState::new(StartBehaviour::Acknowledged, 0);
    let journal_folder = folder("wiring-failed-keep-journal");
    let project = folder("wiring-failed-keep-project");
    let study_folder = study_directory(&project, "tank");
    fs::create_dir_all(&study_folder).expect("the study folder");
    // A file where the study keeps its recording files, so that no recording file can be written.
    let blocker = study_folder.join("recordings");
    fs::write(&blocker, "").expect("a file in the way");
    let journal = empty_journal(&journal_folder);
    let mut study = ProjectStudy {
        log: Study::new(&state),
        project_root: project.clone(),
    };
    let loop_definition = tank_loop("rev-1");
    let request = IdentifyRequest {
        loop_definition: &loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    let Err(failure) = run_tank(&state, &journal, &mut study, &request, RunPurpose::Identify)
    else {
        panic!("the study cannot keep the recording");
    };
    assert!(
        failure
            .message
            .starts_with("The study could not be stored: "),
        "{failure:?}"
    );
    assert_eq!(failure.cause, Cause::Settings);
    assert_eq!(state.count("retain"), 0, "the study kept nothing");
    assert_eq!(
        state.count("recording.release"),
        0,
        "nothing is released before the study keeps the samples"
    );
    let entries = reopened(&journal_folder);
    assert_eq!(
        entries.len(),
        2,
        "the campaign and its experiment stay journaled: {entries:?}"
    );
    let JournalEntry::IdentificationCampaign { campaign } = &entries[0] else {
        panic!("the campaign keeps its recording's place: {entries:?}");
    };
    let started = state.recording_starts();
    assert_eq!(started.len(), 1, "one recording is started");
    assert_eq!(
        campaign.recording.recording_id, started[0].recording_id,
        "the recording's owner stays, so the keep can be retried"
    );

    fs::remove_file(&blocker).expect("the study can be written again");
    cancel_journaled(
        &FakeRuntime::new(&state),
        &FakeClock::new(&state),
        &journal,
        &loop_definition,
        &mut study,
    )
    .expect("the Cancel keeps the recording");
    let operations = state.operations();
    assert_eq!(
        operations[operations.len() - 3..],
        ["recording.stop", "retain", "recording.release"],
        "the next stop keeps the samples, then releases the buffer"
    );
    assert_eq!(
        reopened(&journal_folder),
        Vec::<JournalEntry>::new(),
        "nothing is left owned"
    );
    let kept = open_study(&project, "tank", "rev-1").expect("the study reopens");
    assert_eq!(kept.recordings.len(), 1, "the study lists the recording");
    let file = read_recording(&project, "tank", &kept.recordings[0])
        .expect("the recording file reads back");
    assert_eq!(file.run.recording_id, started[0].recording_id);
    fs::remove_dir_all(&journal_folder).expect("clean up");
    fs::remove_dir_all(&project).expect("clean up");
}
