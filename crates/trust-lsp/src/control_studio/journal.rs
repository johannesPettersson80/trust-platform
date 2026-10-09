//! The operation journal (specification 35, A8.5; runtime protocol annex, E11.2): before Control
//! Studio dispatches a runtime operation it journals the operation's identity, so that after a
//! restart of VS Code it reattaches to that identity instead of replaying a request whose outcome
//! is unknown. The journal lives in the extension's storage folder for the workspace (A8.3), one
//! file per loop, and never holds a credential.
//!
//! Each stage adds the entries of the operations it dispatches: recordings here, experiments with
//! Identify (V1), isolated jobs with Validate (V3) and transactions with Apply (V4).

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::recording::{EvidenceSource, RecordingRequest};

use super::storage::{folder_name, write_atomic};

/// The journal file format.
const JOURNAL_FORMAT: u32 = 1;

/// One journaled runtime operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum JournalEntry {
    /// A recording, journaled before its start (E11.2, E11.3).
    Recording {
        /// The project.
        project_id: String,
        /// The runtime it started on.
        runtime_id: String,
        /// The runtime's session epoch when it started.
        session_epoch: u64,
        /// The loop.
        loop_id: String,
        /// The source fixed when it started (E4.8).
        source: EvidenceSource,
        /// The exact start request: identifier, loop and program revisions, bindings and limits.
        request: RecordingRequest,
    },
}

impl JournalEntry {
    /// The runtime identifier of the operation.
    #[must_use]
    pub fn operation_id(&self) -> &str {
        match self {
            Self::Recording { request, .. } => &request.recording_id,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JournalFile {
    format: u32,
    entries: Vec<JournalEntry>,
}

/// The journal of one loop of one project.
#[derive(Debug)]
pub struct Journal {
    file: PathBuf,
    entries: Vec<JournalEntry>,
}

impl Journal {
    /// Reads the loop's journal from the extension's storage folder; none yet is empty.
    ///
    /// # Errors
    /// The journal exists but cannot be read or is not a journal of this format.
    pub fn open(storage_folder: &Path, project_id: &str, loop_id: &str) -> Result<Self, Failure> {
        let file = storage_folder.join("control-studio").join(format!(
            "{}.journal.json",
            folder_name(&format!("{project_id} {loop_id}"))
        ));
        let entries = match fs::read_to_string(&file) {
            Ok(text) => {
                let journal: JournalFile = serde_json::from_str(&text)
                    .map_err(|error| journal_failure(&file, &error.to_string()))?;
                if journal.format != JOURNAL_FORMAT {
                    return Err(journal_failure(
                        &file,
                        "it was written in a format this version does not read",
                    ));
                }
                journal.entries
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(journal_failure(&file, &error.to_string())),
        };
        Ok(Self { file, entries })
    }

    /// The journaled operations, oldest first.
    #[must_use]
    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    /// Journals an operation before it is dispatched. An identifier already journaled is refused,
    /// because a request is never repeated to resolve an unknown outcome (E0.2, item 7).
    ///
    /// # Errors
    /// The identifier is journaled already, or the journal cannot be written.
    pub fn record(&mut self, entry: JournalEntry) -> Result<(), Failure> {
        if self
            .entries
            .iter()
            .any(|journaled| journaled.operation_id() == entry.operation_id())
        {
            return Err(Failure::new(
                &format!("Operation {} is journaled already; it is read by its identifier, never started again.", entry.operation_id()),
                Cause::Implementation,
                NextAction::None,
            ));
        }
        self.entries.push(entry);
        self.write()
    }

    /// Clears the owner of operation `id` once its terminal outcome is retained (E11.6).
    ///
    /// # Errors
    /// The journal cannot be written.
    pub fn clear(&mut self, id: &str) -> Result<(), Failure> {
        let count = self.entries.len();
        self.entries.retain(|entry| entry.operation_id() != id);
        if self.entries.len() == count {
            return Ok(());
        }
        self.write()
    }

    fn write(&self) -> Result<(), Failure> {
        let text = serde_json::to_string_pretty(&JournalFile {
            format: JOURNAL_FORMAT,
            entries: self.entries.clone(),
        })
        .map_err(|error| journal_failure(&self.file, &error.to_string()))?;
        write_atomic(&self.file, &text)
    }
}

fn journal_failure(file: &Path, reason: &str) -> Failure {
    Failure::new(
        &format!(
            "The operation journal {} could not be used: {reason}.",
            file.display()
        ),
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
mod tests {
    use trust_control::signals::SignalBinding;

    use super::*;

    fn entry(recording_id: &str) -> JournalEntry {
        JournalEntry::Recording {
            project_id: "project:sha256:abc".into(),
            runtime_id: "runtime:local".into(),
            session_epoch: 7,
            loop_id: "loop:tank".into(),
            source: EvidenceSource::SimulatedPlant,
            request: RecordingRequest {
                recording_id: recording_id.into(),
                loop_revision: "sha256:loop".into(),
                program_revision: None,
                bindings: vec![SignalBinding {
                    id: "processVariable".into(),
                    address: Some("%IL0".into()),
                    symbol: None,
                    iec_type: "LREAL".into(),
                }],
                maximum_samples: 1_000,
                maximum_duration_ms: 60_000,
                task: None,
                record_every: 1,
            },
        }
    }

    #[test]
    fn a_journaled_operation_survives_a_restart_until_its_owner_is_cleared() {
        let folder =
            std::env::temp_dir().join(format!("trust-control-journal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let mut journal = Journal::open(&folder, "project:sha256:abc", "loop:tank").expect("empty");
        assert!(journal.entries().is_empty());
        journal.record(entry("recording-1")).expect("journaled");
        let failure = journal
            .record(entry("recording-1"))
            .expect_err("never started twice");
        assert!(
            failure.message.contains("journaled already"),
            "{}",
            failure.message
        );
        let reopened = Journal::open(&folder, "project:sha256:abc", "loop:tank").expect("reopened");
        assert_eq!(reopened.entries(), [entry("recording-1")]);
        let mut reopened = reopened;
        reopened.clear("recording-1").expect("cleared");
        assert!(Journal::open(&folder, "project:sha256:abc", "loop:tank")
            .expect("reopened")
            .entries()
            .is_empty());
        assert!(Journal::open(&folder, "project:sha256:other", "loop:tank")
            .expect("another project")
            .entries()
            .is_empty());
        fs::remove_dir_all(&folder).expect("clean up");
    }
}
