//! Where a project's runtime keeps the files that its clients read (specification 11, section
//! 6.8.0a): the folder of its runtime files and, in it, the endpoint file
//! `.trust-runtime/control.json`, in which a running runtime publishes the endpoint it bound.
//!
//! The runtime writes these files, and every client reads them through this module, so that
//! `trust-runtime ctl` and a service (specification 48 RU-62) find a project's runtime alike.

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The link of a program store to the bundle that runs (specification 34 §11.2). Replacing it is a
/// deployment's commit point.
pub const CURRENT_LINK: &str = "current";
/// The directory that holds the endpoint file and the default socket, in the folder of the
/// project's runtime files: the project folder, or the program store for a store's `current` link
/// (spec 11, section 6.8.0a).
pub const ENDPOINT_DIR: &str = ".trust-runtime";
/// The endpoint file's name inside [`ENDPOINT_DIR`].
pub const ENDPOINT_FILE: &str = "control.json";
/// The only schema version of the endpoint file that this build writes and reads.
pub const ENDPOINT_FILE_SCHEMA_VERSION: u32 = 1;

/// The store a project folder belongs to: the folder that holds `project`, when `project` is a
/// store's `current` link. Any other folder has no store.
#[must_use]
pub fn store_root(project: &Path) -> Option<PathBuf> {
    let is_link =
        fs::symlink_metadata(project).is_ok_and(|metadata| metadata.file_type().is_symlink());
    if is_link && project.file_name() == Some(OsStr::new(CURRENT_LINK)) {
        project.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

/// The folder that holds a project's runtime files, its `.trust-runtime` directory, and whose path
/// gives `<h>` (spec 11, section 6.8.0a): the program store that holds `project_root` when it is a
/// store's `current` link (spec 34 §11.2), so that the files stay with the runtime when an install
/// or a rollback moves `current` to another bundle, and otherwise the project folder itself.
#[must_use]
pub fn runtime_files_folder(project_root: &Path) -> PathBuf {
    store_root(project_root).unwrap_or_else(|| project_root.to_path_buf())
}

/// The path of the endpoint file for a project folder, in the folder of its runtime files.
#[must_use]
pub fn endpoint_file_path(project_root: &Path) -> PathBuf {
    runtime_files_folder(project_root)
        .join(ENDPOINT_DIR)
        .join(ENDPOINT_FILE)
}

/// What the running runtime publishes about its control endpoint.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointFile {
    /// The schema version, always 1.
    pub schema_version: u32,
    /// The bound endpoint, with the real port for `tcp://127.0.0.1:0`.
    pub endpoint: String,
    /// The process id of the runtime that published the file.
    pub pid: u32,
    /// The token the runtime generated for this start. It is present only when the runtime
    /// generated one and keeps it in this file (Unix-like systems).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
}

impl fmt::Debug for EndpointFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EndpointFile")
            .field("schema_version", &self.schema_version)
            .field("endpoint", &self.endpoint)
            .field("pid", &self.pid)
            .field(
                "auth_token",
                &self.auth_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// Why an endpoint file that exists was not used (spec 11, section 6.8.0a).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EndpointFileError {
    /// The file exists but cannot be read, for example because it belongs to another user. A
    /// client that has another way to the runtime skips it and resolves on.
    #[error("{0}")]
    Unreadable(String),
    /// The file was read but does not parse or has another schema version.
    #[error("{0}")]
    Invalid(String),
}

/// Reads the project's endpoint file, or `None` when no runtime has published one.
///
/// # Errors
///
/// Returns [`EndpointFileError::Unreadable`] when the file exists but cannot be read, and
/// [`EndpointFileError::Invalid`] when it does not parse or has an unknown schema version.
pub fn read_endpoint_file(project_root: &Path) -> Result<Option<EndpointFile>, EndpointFileError> {
    let path = endpoint_file_path(project_root);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(EndpointFileError::Unreadable(format!(
                "read {}: {error}",
                path.display()
            )))
        }
    };
    let file: EndpointFile = serde_json::from_str(&text).map_err(|error| {
        EndpointFileError::Invalid(format!("parse {}: {error}", path.display()))
    })?;
    if file.schema_version != ENDPOINT_FILE_SCHEMA_VERSION {
        return Err(EndpointFileError::Invalid(format!(
            "{} has unsupported schema_version {}",
            path.display(),
            file.schema_version
        )));
    }
    Ok(Some(file))
}
