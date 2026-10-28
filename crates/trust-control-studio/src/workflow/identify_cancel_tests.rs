//! The Cancel of a lone recording owner (`cancel_recording_owner`, E11.6), against the fake runtime of `identify::test_support`:
//! its recording is stopped, kept in the study before the runtime buffer is released, and only its owner is cleared, so the
//! other operations of the journal stay journaled. The fake answers at once, so nothing here waits for the clock.

use std::fs;

use trust_control::experiment::{
    ControlEvidenceSource, RecordingRecoveryBinding, RecordingRecoveryOwner,
};

use super::cancel_recording_owner;
use crate::journal::JournalEntry;
use crate::workflow::identify::test_support::{
    empty_journal, folder, reopened, tank_loop, FakeRuntime, FakeState, StartBehaviour, Study,
};

/// The owner of the tank's recording `recording_id`, as a journaled recording start keeps it (E11.2).
fn owner(recording_id: &str) -> RecordingRecoveryOwner {
    RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: recording_id.to_owned(),
        project_id: "project".to_owned(),
        runtime_id: "runtime".to_owned(),
        loop_id: "tank".to_owned(),
        loop_revision: "rev-1".to_owned(),
        source: ControlEvidenceSource::Simulation,
        program_revision: Some("program".to_owned()),
        start_acknowledgement_unconfirmed: None,
        bindings: vec![RecordingRecoveryBinding {
            id: "processVariable".to_owned(),
            address: None,
            symbol: Some("Main.TT101.Value".to_owned()),
            iec_type: "LREAL".to_owned(),
        }],
    }
}

/// E11.6 and E4.7: a lone recording owner is cancelled by stopping its recording, keeping the run in the study, releasing the
/// runtime buffer, and clearing its owner. The other journaled operation stays journaled.
#[test]
fn cancelling_a_lone_recording_owner_keeps_its_recording_and_clears_only_its_owner() {
    let folder = folder("cancel-owner");
    let journal = empty_journal(&folder);
    for recording_id in ["rec-a", "rec-b"] {
        journal
            .lock()
            .record(JournalEntry::RecordingOwner {
                owner: owner(recording_id),
            })
            .expect("the owner is journaled");
    }
    let state = FakeState::new(StartBehaviour::Acknowledged, 10_000);
    let runtime = FakeRuntime::new(&state);
    let mut study = Study::new(&state);
    cancel_recording_owner(
        &runtime,
        &journal,
        &tank_loop("rev-1"),
        &owner("rec-b"),
        &mut study,
    )
    .expect("the lone owner is cancelled");
    assert_eq!(
        state.calls(),
        [
            "recording.stop rec-b",
            "retain rec-b",
            "recording.release rec-b"
        ],
        "the recording is stopped, kept, and only then released"
    );
    let left: Vec<String> = reopened(&folder)
        .iter()
        .map(|entry| entry.operation_id().to_owned())
        .collect();
    assert_eq!(left, ["rec-a"], "the other operation stays journaled");
    let _ = fs::remove_dir_all(&folder);
}
