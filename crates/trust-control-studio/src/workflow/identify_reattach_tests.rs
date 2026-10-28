use serde_json::{json, Value};

use super::*;

/// A recording owner of the tank loop on the simulator (A8.5) at program revision `revision`, from `source`.
fn owner(recording_id: &str, source: &str, revision: Option<&str>) -> Value {
    let mut owner = json!({
        "schemaVersion": 1, "recordingId": recording_id, "projectId": "project:tank", "runtimeId": "simulator",
        "loopId": "tank", "loopRevision": "sha256:loop", "source": source,
        "bindings": [{ "id": "processVariable", "address": "%IL0", "iec_type": "LREAL" }]
    });
    if let Some(revision) = revision {
        owner["programRevision"] = json!(revision);
    }
    owner
}

/// The journal record of an identification campaign still in its baseline, as the engine writes it (A8.5).
fn campaign(source: &str, revision: &str) -> JournalEntry {
    serde_json::from_value(json!({
        "kind": "identification-campaign",
        "campaign": {
            "schemaVersion": 1, "loopRevision": "sha256:loop", "phase": "baseline",
            "recording": owner("recording-7", source, Some(revision)), "channelsRemaining": 1
        }
    }))
    .expect("the campaign's journal record")
}

/// The mode checkpoint of the identification, taken on program revision `revision` (A6.2).
fn mode_checkpoint(revision: &str) -> JournalEntry {
    serde_json::from_value(json!({
        "kind": "identification-mode",
        "operationId": "identification-mode-1",
        "checkpoint": {
            "loopRevision": "sha256:loop",
            "original": {
                "program_revision": revision, "binding_revision": "binding:tank", "values": {},
                "status": "idle", "reason": null, "transaction": null
            }
        }
    }))
    .expect("the mode checkpoint's journal record")
}

#[test]
fn a_new_program_revision_ends_the_campaign_and_its_mode_change() {
    // The simulator restarted while VS Code was closed (A2.2): the campaign's recording and the mode change belong to the old
    // revision and can never complete. Caught: a journal that blocks the loop for ever after a restart.
    let entries = [
        campaign("Simulation", "sha256:before"),
        mode_checkpoint("sha256:before"),
    ];
    assert_eq!(
        ended_by_new_revision(&entries, "sha256:now"),
        Some(EndedEntries {
            operation_ids: vec![
                "recording-7".to_string(),
                "identification-mode-1".to_string()
            ],
            journaled_revision: "sha256:before".to_string(),
        })
    );
}

#[test]
fn the_revision_the_identification_ran_on_ends_nothing() {
    // The same simulator still runs: its experiment may still hold the step, so the reattach follows it (A8.6). Caught: a journal
    // cleared under a running experiment.
    let entries = [
        campaign("Simulation", "sha256:before"),
        mode_checkpoint("sha256:before"),
    ];
    assert_eq!(ended_by_new_revision(&entries, "sha256:before"), None);
}

#[test]
fn a_physical_recording_is_never_ended_by_its_revision() {
    // A physical target's operation stays unresolved until a fresh review (E11.5). Caught: an unresolved experiment on a real plant
    // dropped from the journal.
    let entries = [campaign("Physical", "sha256:before")];
    assert_eq!(ended_by_new_revision(&entries, "sha256:now"), None);
}

#[test]
fn a_recording_that_names_no_revision_is_left_to_its_recovery() {
    // Without a revision the journal says nothing about the identity, so the recovery reads the runtime as before. Caught: a lone
    // owner cleared on a guess.
    let lone: JournalEntry = serde_json::from_value(
        json!({ "kind": "recording-owner", "owner": owner("recording-9", "Simulation", None) }),
    )
    .expect("the owner's journal record");
    assert_eq!(ended_by_new_revision(&[lone], "sha256:now"), None);
}
