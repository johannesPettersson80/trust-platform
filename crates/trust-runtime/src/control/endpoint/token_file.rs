//! The per-user token file of a runtime that generated its control token on Windows (spec 11,
//! section 6.8.0a).
//!
//! A project folder can be readable by other local users, so a Windows runtime keeps the token it
//! generates out of the project: in `%LOCALAPPDATA%\trust\runtimes\<h>.json`, where `<h>` is the
//! project-path hash, a folder that Windows keeps private to the user. The file names the runtime's
//! process and endpoint, so a client takes its token only together with the endpoint file of the
//! same start. The logic does not depend on the platform, so it is tested everywhere.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::RuntimeError;

use super::defaults::{canonical_project_root, project_path_hash};
use super::file::{invalid, write_atomically, ENDPOINT_FILE_SCHEMA_VERSION};

/// The token that a runtime generated, with the start it belongs to.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct TokenFile {
    schema_version: u32,
    /// The endpoint of the start that generated the token.
    pub(super) endpoint: String,
    /// The process id of the runtime that generated the token.
    pub(super) pid: u32,
    auth_token: String,
}

impl fmt::Debug for TokenFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TokenFile")
            .field("schema_version", &self.schema_version)
            .field("endpoint", &self.endpoint)
            .field("pid", &self.pid)
            .field("auth_token", &"<redacted>")
            .finish()
    }
}

impl TokenFile {
    pub(super) fn new(endpoint: String, pid: u32, auth_token: String) -> Self {
        Self {
            schema_version: ENDPOINT_FILE_SCHEMA_VERSION,
            endpoint,
            pid,
            auth_token,
        }
    }

    /// The token.
    pub(super) fn into_token(self) -> String {
        self.auth_token
    }
}

/// `<token dir>/<h>.json` for a project folder, where `<h>` is the hash of its canonical path.
///
/// # Errors
///
/// Returns an error when the project folder cannot be resolved to its canonical path.
pub(super) fn token_file_path(
    token_dir: &Path,
    project_root: &Path,
) -> Result<PathBuf, RuntimeError> {
    let canonical = canonical_project_root(project_root)?;
    Ok(token_dir.join(format!("{}.json", project_path_hash(&canonical))))
}

/// Writes the token file atomically, creating its folder when it is missing.
///
/// # Errors
///
/// Returns an error when the folder or the file cannot be written.
pub(super) fn write_token_file(path: &Path, file: &TokenFile) -> Result<(), RuntimeError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .map_err(|error| invalid(format!("create {}: {error}", dir.display())))?;
    }
    let text = serde_json::to_string(file)
        .map_err(|error| invalid(format!("encode token file: {error}")))?;
    write_atomically(path, text.as_bytes())
}

/// The token file at `path`, or `None` when it is missing, cannot be read, does not parse or has
/// another schema version: a client then has no generated token from it.
pub(super) fn read_token_file(path: &Path) -> Option<TokenFile> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str::<TokenFile>(&text)
        .ok()
        .filter(|file| file.schema_version == ENDPOINT_FILE_SCHEMA_VERSION)
}

/// Removes the token file at `path` when it names process `pid`.
pub(super) fn remove_token_file_of(path: &Path, pid: u32) {
    if read_token_file(path).is_some_and(|file| file.pid == pid) {
        let _ = fs::remove_file(path);
    }
}
