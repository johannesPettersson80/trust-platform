//! The Discover pane's remembered choices per project (specification 25 §2, decision O14): the ticked
//! sources of `Only search` and the device IDs that the project ignores, in
//! `<project>/.trust-runtime/devices.toml` under `[discover]`. This is the one reader and writer of that
//! file: the panel and `trust-runtime discover --project` both use it, so they agree on the choices.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The folder of a project's state, which holds the file of the panel's choices.
const STATE_FOLDER: &str = ".trust-runtime";
/// The file of the panel's choices in that folder.
const FILE_NAME: &str = "devices.toml";

/// What a project remembers of the Discover pane (specification 25 §2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Memory {
    /// The ticked source IDs of `Only search`; `None` until a tick is saved, which is the first run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocols: Option<Vec<String>>,
    /// The device IDs that the project ignores, until they are undone.
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub ignored: BTreeSet<String>,
}

/// The layout of the file: one `[discover]` table.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct File {
    discover: Memory,
}

/// The file that remembers the choices of `project`.
#[must_use]
pub fn path(project: &Path) -> PathBuf {
    project.join(STATE_FOLDER).join(FILE_NAME)
}

/// What `project` remembers; nothing while its file does not exist yet.
///
/// # Errors
/// The file exists but cannot be read, or does not hold a valid `[discover]` table; the message
/// names the file.
pub fn load(project: &Path) -> Result<Memory, String> {
    let file = path(project);
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Memory::default());
        }
        Err(error) => return Err(format!("{}: {error}", file.display())),
    };
    toml::from_str::<File>(&text)
        .map(|file| file.discover)
        .map_err(|error| format!("{}: {error}", file.display()))
}

/// Saves `memory` as the file of `project`. The text is written to a temporary file and renamed over
/// the old file, so a reader never sees half of it.
///
/// # Errors
/// The folder or the file cannot be written; the message names the file.
pub fn save(project: &Path, memory: &Memory) -> Result<(), String> {
    let file = path(project);
    let text = toml::to_string(&File {
        discover: memory.clone(),
    })
    .map_err(|error| format!("{}: {error}", file.display()))?;
    let folder = project.join(STATE_FOLDER);
    fs::create_dir_all(&folder).map_err(|error| format!("{}: {error}", folder.display()))?;
    let temporary = folder.join(format!("{FILE_NAME}.tmp"));
    fs::write(&temporary, text).map_err(|error| format!("{}: {error}", temporary.display()))?;
    fs::rename(&temporary, &file).map_err(|error| format!("{}: {error}", file.display()))
}
