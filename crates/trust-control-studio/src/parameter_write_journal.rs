//! The journal of the analysis parameter writes (specification 35, A8.5; runtime protocol annex, E11.2 and E11.4). A write's
//! identifier is journaled before the write is dispatched, so that a write whose outcome is unknown is read by its record and is never
//! sent again. One file per loop, beside the operation journals; it holds the identifier and the action, and no credential.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use trust_control::analysis::parameter_writes::ParameterWriteAction;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;

use crate::storage::{folder_name, write_atomic};

/// The write that is journaled and not yet cleared: its identifier and what it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JournaledWrite {
    /// The transaction identifier the write is sent under (E7.3).
    pub transaction_id: String,
    /// What the write does.
    pub action: ParameterWriteAction,
}

/// The journal file of one loop of one project.
#[derive(Debug, Clone)]
pub struct WriteJournal {
    file: PathBuf,
}

impl WriteJournal {
    /// The journal of `loop_definition`, in the folder of the operation journals (A8.3).
    #[must_use]
    pub fn of(storage_folder: &Path, loop_definition: &LoopDefinition) -> Self {
        let name = folder_name(&format!(
            "{} {}",
            loop_definition.project_id, loop_definition.id
        ));
        Self {
            file: storage_folder
                .join("control-studio")
                .join(format!("{name}.parameter-write.json")),
        }
    }

    /// The write that is journaled and not yet cleared, if there is one.
    ///
    /// # Errors
    /// The journal exists but cannot be read or is not a journal of a write.
    pub fn read(&self) -> Result<Option<JournaledWrite>, Failure> {
        match fs::read_to_string(&self.file) {
            Ok(text) => trust_control::json::decode_text(&text)
                .map(Some)
                .map_err(|error| journal_failure(&self.file, &error.to_string())),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(journal_failure(&self.file, &error.to_string())),
        }
    }

    /// Journals a write before it is dispatched (E11.2).
    ///
    /// # Errors
    /// The journal cannot be written; the write must not be dispatched then.
    pub fn record(
        &self,
        transaction_id: &str,
        action: ParameterWriteAction,
    ) -> Result<(), Failure> {
        let entry = JournaledWrite {
            transaction_id: transaction_id.to_owned(),
            action,
        };
        let text = serde_json::to_string_pretty(&entry)
            .map_err(|error| journal_failure(&self.file, &error.to_string()))?;
        write_atomic(&self.file, &format!("{text}\n"))
    }

    /// Clears the journal once the outcome of its write is known (E11.4).
    ///
    /// # Errors
    /// The journal cannot be changed.
    pub fn clear(&self) -> Result<(), Failure> {
        match fs::remove_file(&self.file) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(journal_failure(&self.file, &error.to_string())),
        }
    }
}

fn journal_failure(file: &Path, reason: &str) -> Failure {
    Failure::new(
        &format!(
            "The parameter write journal {} could not be used: {reason}.",
            file.display()
        ),
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "parameter_write_journal_tests.rs"]
mod tests;
