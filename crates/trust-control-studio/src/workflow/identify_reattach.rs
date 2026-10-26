//! The reattach of an interrupted identification that reopening makes (specification 35, A8.5, A8.6; runtime protocol annex,
//! E11.5): before it follows the journaled campaign, it asks whether the simulator the campaign ran on is still the one that
//! runs. A restart or a reload gives a new program revision, which ends every operation bound to the old one (A2.2, E9.3), so a
//! journal of that revision can never complete: its recording and experiment are gone with the old session. On a simulated
//! target the identification then ends with the simulator: its journal entries are cleared, nothing is kept or restored, and the
//! answer says so (B3.9). A physical target's operation is never ended this way; it stays unresolved until a fresh review
//! (E11.5).

use std::path::Path;

use trust_control::commissioning::checkpoint::IdentificationModeCheckpoint;
use trust_control::experiment::{ControlEvidenceSource, RecordingRecoveryOwner};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::texts::ended_with_restarted_simulator;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;

use crate::journal::{Journal, JournalEntry};
use crate::runtime_client::RuntimeClient;

/// The identification entries of a journal that a new program revision ended: their operation identifiers, and the revision they
/// were journaled at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndedEntries {
    /// The identifiers to clear, in journal order.
    pub operation_ids: Vec<String>,
    /// The program revision the first of them ran on.
    pub journaled_revision: String,
}

/// The identification entries of `entries` that ran on another program revision than `current`, the revision of the simulated
/// runtime that runs now (A2.2): the campaign, the lone recording owners and the experiments whose recording was simulated, and
/// the mode checkpoint of the identification. An entry that names no revision, an operation of a physical target, a validation
/// job and an Apply's checkpoint are left as they are. `None` when no entry ended.
#[must_use]
pub fn ended_by_new_revision(entries: &[JournalEntry], current: &str) -> Option<EndedEntries> {
    let mut operation_ids = Vec::new();
    let mut journaled_revision = None;
    for entry in entries {
        let Some(revision) = identification_revision(entry) else {
            continue;
        };
        if revision != current {
            journaled_revision.get_or_insert_with(|| revision.to_string());
            operation_ids.push(entry.operation_id().to_string());
        }
    }
    journaled_revision.map(|journaled_revision| EndedEntries {
        operation_ids,
        journaled_revision,
    })
}

/// The program revision an identification entry ran on, when it is one of a simulated target and names it.
fn identification_revision(entry: &JournalEntry) -> Option<&str> {
    match entry {
        JournalEntry::IdentificationCampaign { campaign } => {
            simulated_revision(&campaign.recording)
        }
        JournalEntry::RecordingOwner { owner } => simulated_revision(owner),
        JournalEntry::Experiment { recovery } => recovery
            .recording_owner
            .as_ref()
            .and_then(simulated_revision),
        JournalEntry::IdentificationMode { checkpoint, .. } => Some(mode_revision(checkpoint)),
        JournalEntry::IsolatedJob { .. } | JournalEntry::Commissioning { .. } => None,
    }
}

/// The program revision of a simulated recording; none for a physical one, which only a fresh review resolves (E11.5).
fn simulated_revision(owner: &RecordingRecoveryOwner) -> Option<&str> {
    match owner.source {
        ControlEvidenceSource::Simulation => owner.program_revision.as_deref(),
        ControlEvidenceSource::Physical => None,
    }
}

/// The program revision the identification changed the loop's mode on.
fn mode_revision(checkpoint: &IdentificationModeCheckpoint) -> &str {
    &checkpoint.original.program_revision
}

/// Ends the interrupted identification of `loop_definition` whose simulator restarted or reloaded its program since it was
/// journaled in `storage_folder` (A2.2, A8.6, E11.5): reads the runtime's status, and when it is a simulated target whose program
/// revision is not the one the identification ran on, clears the identification's entries and answers the failure that says so,
/// with Identify as the next action (B3.9). `None` when the runtime is physical or nothing of the journal ended; the reattach then
/// follows the journal as before.
///
/// # Errors
/// The runtime cannot be reached or read, or the journal cannot be read or written.
pub fn end_with_restarted_simulator(
    connection: &RuntimeConnection,
    storage_folder: &Path,
    loop_definition: &LoopDefinition,
) -> Result<Option<Failure>, Failure> {
    let status = RuntimeClient::connect(connection)?.status()?;
    if status.source() != EvidenceSource::SimulatedPlant {
        return Ok(None);
    }
    let mut journal = Journal::open(
        storage_folder,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    let Some(ended) = ended_by_new_revision(journal.entries(), &status.program_revision) else {
        return Ok(None);
    };
    for id in &ended.operation_ids {
        journal.clear(id)?;
    }
    Ok(Some(Failure::new(
        &ended_with_restarted_simulator(&ended.journaled_revision, &status.program_revision),
        Cause::Runtime,
        NextAction::Identify,
    )))
}

#[cfg(test)]
#[path = "identify_reattach_tests.rs"]
mod tests;
