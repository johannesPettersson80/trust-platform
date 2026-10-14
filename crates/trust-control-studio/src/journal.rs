//! The operation journal (specification 35, A8.5; runtime protocol annex, E11.2): before Control
//! Studio dispatches a runtime operation it journals the operation's identity, so that after a
//! restart of VS Code it reattaches to that identity instead of replaying a request whose outcome
//! is unknown. The journal lives in the extension's storage folder for the workspace (A8.3), one
//! file per loop, and never holds a credential.
//!
//! Each stage adds the entries of the operations it dispatches: the owners of runtime recordings here, Test experiments and
//! the identification campaign with Identify (V1, with their recording owners), isolated jobs with Validate (V3) and
//! transactions with Apply (V4). An entry is the operation's runtime identity: one entry per identifier.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use trust_control::commissioning::checkpoint::{
    CheckpointStage, CommissioningCheckpoint, IdentificationModeCheckpoint,
};
use trust_control::experiment::{ControlExperimentRecovery, RecordingRecoveryOwner};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::campaign::journal::IdentificationCampaignJournal;

use super::storage::{folder_name, write_atomic};
use crate::workflow::validate::IsolatedJobOwner;

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
    /// A runtime recording's owner, journaled before its start is dispatched (E11.2). It holds the identity that the stop,
    /// the release and the recovery need, and whether the start's acknowledgement was lost (E11.4). It holds no credential.
    RecordingOwner {
        /// The owner (`RecordingRecoveryOwner`).
        owner: RecordingRecoveryOwner,
    },
    /// A Test experiment, journaled before its dispatch (E11.2, A8.5): its identity first, then, once its recording is
    /// journaled, its reviewed plan and the recording it runs with. Cleared once the experiment is terminal.
    Experiment {
        /// The stored recovery (`ControlExperimentRecovery`).
        recovery: Box<ControlExperimentRecovery>,
    },
    /// An identification campaign, journaled before each dispatch (A8.5, B3.4): its recording once that has started, then its
    /// step's plan, baseline and start before the step is sent, so that a reopened panel reattaches to a running step. Its
    /// operation is its recording, whose owner it carries, so it takes the place of that owner's entry. Cleared once the
    /// campaign has settled: no experiment of it is left running and its recording is kept.
    IdentificationCampaign {
        /// The campaign's record (`IdentificationCampaignJournal`).
        campaign: Box<IdentificationCampaignJournal>,
    },
    /// An isolated test run of Validate, journaled before its start is dispatched (E11.2, E8.4): its identity, so that the
    /// run is read or cancelled by its identifier after a restart. Cleared once its result is read or its end confirmed.
    IsolatedJob {
        /// The job's owner (`IsolatedJobOwner`).
        job: Box<IsolatedJobOwner>,
    },
    /// The commissioning checkpoint of an Apply (A8.3, A8.5, B6.4), journaled before its first write and rewritten as each
    /// transaction is dispatched. Kept while the settings are applied, so that Restore works after a restart of the engine;
    /// cleared once a Restore confirms, or a later Apply replaces it. Its operation is the checkpoint's own identifier.
    Commissioning {
        /// The checkpoint (`CommissioningCheckpoint`), whose identifier names the operation.
        checkpoint: Box<CommissioningCheckpoint>,
    },
    /// The mode checkpoint of an Identify campaign that moved the loop to the other mode (A6.2, A8.6), journaled before the mode
    /// transfer is dispatched. Cleared once the loop is back in the mode it was found in.
    IdentificationMode {
        /// The identifier of the mode change, which the journal reads and clears it by.
        operation_id: String,
        /// The mode checkpoint (`IdentificationModeCheckpoint`).
        checkpoint: Box<IdentificationModeCheckpoint>,
    },
}

impl JournalEntry {
    /// The runtime identifier of the operation: an identification campaign's is its recording's, and a checkpoint's is its own.
    #[must_use]
    pub fn operation_id(&self) -> &str {
        match self {
            Self::RecordingOwner { owner } => &owner.recording_id,
            Self::Experiment { recovery } => &recovery.experiment_id,
            Self::IdentificationCampaign { campaign } => &campaign.recording.recording_id,
            Self::IsolatedJob { job } => &job.job_id,
            Self::Commissioning { checkpoint } => &checkpoint.id,
            Self::IdentificationMode { operation_id, .. } => operation_id,
        }
    }

    /// Whether the entry holds an operation that runs or whose outcome is unconfirmed (A8.5), which another operation of the
    /// loop waits for. A standing Apply's checkpoint, completed and kept for Restore, is not one (B7), nor is a recovered or
    /// retired checkpoint.
    #[must_use]
    pub fn holds_unfinished_operation(&self) -> bool {
        match self {
            Self::Commissioning { checkpoint } => !matches!(
                checkpoint.stage,
                CheckpointStage::Completed | CheckpointStage::Recovered | CheckpointStage::Retired
            ),
            _ => true,
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
                let journal: JournalFile = trust_control::json::decode_text(&text)
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

    /// Journals an operation, or replaces the entry of the same identifier: an experiment's record is written with its
    /// identity first and again with its plan and recording owner (`storeExperimentRecovery`). It dispatches nothing.
    ///
    /// # Errors
    /// The journal cannot be written.
    pub fn persist(&mut self, entry: JournalEntry) -> Result<(), Failure> {
        match self
            .entries
            .iter()
            .position(|journaled| journaled.operation_id() == entry.operation_id())
        {
            Some(index) => self.entries[index] = entry,
            None => self.entries.push(entry),
        }
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
    use serde_json::json;
    use trust_control::experiment::{
        ControlEvidenceSource, ControlExperimentPlan, RecordingRecoveryBinding,
    };
    use trust_control::identification::campaign::baseline::BaselineSummary;

    use super::*;

    fn owner(recording_id: &str) -> RecordingRecoveryOwner {
        RecordingRecoveryOwner {
            schema_version: 1,
            recording_id: recording_id.into(),
            project_id: "project:sha256:abc".into(),
            runtime_id: "runtime:local".into(),
            loop_id: "loop:tank".into(),
            loop_revision: "sha256:loop".into(),
            source: ControlEvidenceSource::Simulation,
            program_revision: None,
            start_acknowledgement_unconfirmed: None,
            bindings: vec![RecordingRecoveryBinding {
                id: "processVariable".into(),
                address: Some("%IL0".into()),
                symbol: None,
                iec_type: "LREAL".into(),
            }],
        }
    }

    fn entry(recording_id: &str) -> JournalEntry {
        JournalEntry::RecordingOwner {
            owner: owner(recording_id),
        }
    }

    #[test]
    fn an_identification_campaign_takes_the_place_of_its_recording_owner() {
        let folder = std::env::temp_dir().join(format!(
            "trust-control-journal-campaign-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        let project = "project:sha256:abc";
        let mut journal = Journal::open(&folder, project, "loop:tank").expect("empty");
        journal
            .persist(entry("recording-9"))
            .expect("the owner is journaled before the start");
        let baseline_phase = JournalEntry::IdentificationCampaign {
            campaign: Box::new(IdentificationCampaignJournal::baseline_phase(
                "sha256:loop",
                owner("recording-9"),
                1,
            )),
        };
        assert_eq!(
            baseline_phase.operation_id(),
            "recording-9",
            "the campaign's operation is its recording"
        );
        journal
            .persist(baseline_phase.clone())
            .expect("journaled before the baseline");
        assert_eq!(
            journal.entries(),
            [baseline_phase],
            "the campaign carries the owner and takes its place"
        );
        let plan: ControlExperimentPlan = serde_json::from_value(json!({
            "id": "experiment-9", "source": "simulation", "kind": "setpoint_step", "affectedSignal": "LevelSP",
            "valueType": "LREAL", "from": 45, "to": 47, "unit": "%", "minimum": 0, "maximum": 100, "durationMs": 570_000,
            "expectedScan": 7, "stopConditions": ["Abort"], "restorationValue": 45, "requiresPhysicalApproval": false,
            "trajectory": [{ "atMs": 0, "value": 47 }]
        }))
        .expect("the plan decodes");
        let summary = BaselineSummary {
            mean: 45.0,
            noise: 0.03,
            half_range: 0.05,
            spacing_seconds: 1.0,
            window_seconds: 30.0,
            input_mean: None,
            input_noise: None,
        };
        let step_phase = JournalEntry::IdentificationCampaign {
            campaign: Box::new(IdentificationCampaignJournal::step_phase(
                "sha256:loop",
                owner("recording-9"),
                plan,
                summary,
                30.0,
                0,
            )),
        };
        journal
            .persist(step_phase.clone())
            .expect("journaled before the step is sent");
        let reopened = Journal::open(&folder, project, "loop:tank").expect("reopened");
        assert_eq!(
            reopened.entries(),
            [step_phase],
            "one entry per campaign, its step replacing its baseline"
        );
        let mut journal = reopened;
        journal
            .clear("recording-9")
            .expect("cleared once the campaign has settled");
        assert_eq!(
            Journal::open(&folder, project, "loop:tank")
                .expect("reopened")
                .entries(),
            Vec::<JournalEntry>::new()
        );
        fs::remove_dir_all(&folder).expect("clean up");
    }

    #[test]
    fn a_journaled_operation_survives_a_restart_until_its_owner_is_cleared() {
        let folder =
            std::env::temp_dir().join(format!("trust-control-journal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let mut journal = Journal::open(&folder, "project:sha256:abc", "loop:tank").expect("empty");
        assert_eq!(journal.entries(), [] as [JournalEntry; 0]);
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
        assert_eq!(
            Journal::open(&folder, "project:sha256:abc", "loop:tank")
                .expect("reopened")
                .entries(),
            [] as [JournalEntry; 0]
        );
        assert_eq!(
            Journal::open(&folder, "project:sha256:other", "loop:tank")
                .expect("another project")
                .entries(),
            Vec::<JournalEntry>::new()
        );
        fs::remove_dir_all(&folder).expect("clean up");
    }

    #[test]
    fn a_recording_owner_is_replaced_not_repeated_and_an_experiment_is_journaled_in_two_steps() {
        let folder = std::env::temp_dir().join(format!(
            "trust-control-journal-owners-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        let project = "project:sha256:abc";
        let owner = RecordingRecoveryOwner {
            schema_version: 1,
            recording_id: "recording-7".into(),
            project_id: project.into(),
            runtime_id: "runtime:local".into(),
            loop_id: "loop:tank".into(),
            loop_revision: "sha256:loop".into(),
            source: ControlEvidenceSource::Simulation,
            program_revision: None,
            start_acknowledgement_unconfirmed: None,
            bindings: vec![RecordingRecoveryBinding {
                id: "processVariable".into(),
                address: Some("%IL0".into()),
                symbol: None,
                iec_type: "LREAL".into(),
            }],
        };
        let mut journal = Journal::open(&folder, project, "loop:tank").expect("empty");
        journal
            .persist(JournalEntry::RecordingOwner {
                owner: owner.clone(),
            })
            .expect("persisted before the start");
        let mut unconfirmed = owner.clone();
        unconfirmed.start_acknowledgement_unconfirmed = Some(true);
        journal
            .persist(JournalEntry::RecordingOwner {
                owner: unconfirmed.clone(),
            })
            .expect("replaced with its acknowledgement state");
        let reopened = Journal::open(&folder, project, "loop:tank").expect("reopened");
        assert_eq!(
            reopened.entries(),
            [JournalEntry::RecordingOwner { owner: unconfirmed }],
            "one entry per recording"
        );

        let mut journal = reopened;
        let identity = ControlExperimentRecovery {
            schema_version: 1,
            experiment_id: "experiment-1".into(),
            project_id: project.into(),
            loop_id: "loop:tank".into(),
            loop_revision: "sha256:loop".into(),
            plan: None,
            recording_owner: None,
        };
        journal
            .persist(JournalEntry::Experiment {
                recovery: Box::new(identity.clone()),
            })
            .expect("the identity first");
        let with_owner = ControlExperimentRecovery {
            schema_version: 2,
            recording_owner: Some(owner),
            ..identity
        };
        journal
            .persist(JournalEntry::Experiment {
                recovery: Box::new(with_owner),
            })
            .expect("then its recording owner");
        assert_eq!(journal.entries().len(), 2);
        journal
            .clear("experiment-1")
            .expect("cleared once terminal");
        journal.clear("recording-7").expect("cleared");
        assert_eq!(
            Journal::open(&folder, project, "loop:tank")
                .expect("reopened")
                .entries(),
            [] as [JournalEntry; 0]
        );
        fs::remove_dir_all(&folder).expect("clean up");
    }

    #[test]
    fn clearing_one_owner_keeps_the_other_journaled_operations() {
        // control-studio-study-store.test.ts: the Test journal clears only its owner (A8.5); another journaled operation
        // of the loop keeps its identity.
        let folder = std::env::temp_dir().join(format!(
            "trust-control-journal-clear-one-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&folder);
        let project = "project:sha256:abc";
        let mut journal = Journal::open(&folder, project, "loop:tank").expect("empty");
        journal
            .persist(entry("recording-3"))
            .expect("the owner is journaled");
        let identity = ControlExperimentRecovery {
            schema_version: 1,
            experiment_id: "experiment-3".into(),
            project_id: project.into(),
            loop_id: "loop:tank".into(),
            loop_revision: "sha256:loop".into(),
            plan: None,
            recording_owner: None,
        };
        journal
            .persist(JournalEntry::Experiment {
                recovery: Box::new(identity.clone()),
            })
            .expect("the experiment is journaled");
        journal
            .clear("recording-3")
            .expect("only the owner is cleared");
        let reopened = Journal::open(&folder, project, "loop:tank").expect("reopened");
        assert_eq!(
            reopened.entries(),
            [JournalEntry::Experiment {
                recovery: Box::new(identity),
            }],
            "the experiment keeps its identity"
        );
        fs::remove_dir_all(&folder).expect("clean up");
    }
}
