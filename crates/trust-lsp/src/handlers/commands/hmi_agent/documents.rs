//! Workspace document snapshots (specification 43, §4 and §8 repair 2): what a page holds when a
//! draft is prepared, applied or reviewed, and the revision that names that observation.
//!
//! Port of `editors/vscode/src/hmiBuilder/agent/documents.ts` (`WorkspaceDocuments`, `sameContent`).
//! The extension passes the open editor documents by project-relative path, with their version and
//! dirty state; the disk is read here with `std::fs`. This module is the only place where the
//! snapshot rule lives.
//!
//! - A page path is normalized and workspace-relative: no empty, `.` or `..` part, and no backslash,
//!   colon (drive letters and URIs) or NUL.
//! - Every existing ancestor of the path must resolve inside the workspace, so a symbolic link cannot
//!   lead out of it, including for a page that does not exist yet.
//! - A dirty open buffer is authoritative: its text is used and the disk is not consulted, until the
//!   customer saves or discards the buffer (spec 43, §4).
//! - A clean open document is an editor copy of its file: it keeps the editor's text and version, but
//!   it never makes a deleted file exist.
//! - The revision is the editor version (`disk` when the page is not open), a colon, and the SHA-256
//!   of a presence marker and the text. Opening, closing or saving a page changes the revision but not
//!   its text, so it is no conflict (see [`same_content`]).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::{AgentError, AgentResult, PROJECT_UNAVAILABLE};

/// The largest page the engine reads, holds or writes: 1 MiB (specification 43, §4).
pub(super) const MAX_FILE_BYTES: u64 = 1_048_576;

/// The observed state of one page: its text, whether it exists, and the revision that names it
/// (specification 43, §4). A draft keeps its pages' snapshots as its base, and the inspection tools
/// return them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DocumentSnapshot {
    /// The project-relative path of the page.
    pub(super) file: String,
    /// Whether the page exists; a dirty buffer always does.
    pub(super) exists: bool,
    /// The text of the page, without a UTF-8 byte order mark.
    pub(super) text: String,
    /// `<editor version or disk>:<SHA-256 of the presence marker and the text>`.
    pub(super) revision: String,
    /// The editor version of an open document; `None` for a page read from disk.
    pub(super) version: Option<i64>,
    /// Whether the open editor buffer holds unsaved changes.
    pub(super) dirty: bool,
}

/// An open editor document of the project as the extension reports it: its text, its editor version
/// and whether it holds unsaved changes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OpenDocument {
    /// The text the editor holds.
    pub(super) text: String,
    /// The editor's document version.
    pub(super) version: i64,
    /// Whether the buffer differs from the file on disk.
    pub(super) dirty: bool,
}

/// The absolute path of a page inside the workspace `root`.
///
/// # Errors
///
/// `INVALID_ARGUMENT` when `file` is not a normalized workspace-relative path. `PROJECT_UNAVAILABLE`
/// when the path, or an existing ancestor of it, resolves outside the workspace, or cannot be
/// inspected. Only a missing path is treated as a page that does not exist yet.
pub(super) fn resolve(root: &Path, file: &str) -> AgentResult<PathBuf> {
    if !is_relative_page_path(file) {
        return Err(AgentError::invalid(
            "file must be a normalized workspace-relative path.",
        ));
    }
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        AgentError::new(
            PROJECT_UNAVAILABLE,
            format!("the workspace is unavailable: {error}"),
        )
    })?;
    let mut target = root.to_path_buf();
    for part in file.split('/') {
        target.push(part);
    }
    // Resolve every existing ancestor, including symbolic links below a page that does not exist.
    let mut current = target.clone();
    loop {
        match fs::canonicalize(&current) {
            Ok(canonical) if canonical.starts_with(&canonical_root) => return Ok(target),
            Ok(_) => return Err(escape()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // A dangling symbolic link has no target that could be proven inside the workspace.
                if fs::symlink_metadata(&current).is_ok() {
                    return Err(escape());
                }
                if !current.pop() {
                    return Err(escape());
                }
            }
            Err(error) => {
                return Err(AgentError::new(
                    PROJECT_UNAVAILABLE,
                    format!("cannot inspect {file}: {error}"),
                ));
            }
        }
    }
}

/// The observed state of one page: the open editor copy when there is one, otherwise the disk.
///
/// # Errors
///
/// The refusals of [`resolve`]; `INVALID_ARGUMENT` when the page is not a file; `LIMIT_EXCEEDED`
/// when its text is larger than 1 MiB; `PROJECT_UNAVAILABLE` when the disk cannot be read.
pub(super) fn snapshot(
    root: &Path,
    file: &str,
    open: Option<&OpenDocument>,
) -> AgentResult<DocumentSnapshot> {
    let path = resolve(root, file)?;
    let (exists, text, version, dirty) = match open {
        // The unsaved buffer is the page as the customer sees it; the disk does not matter.
        Some(document) if document.dirty => {
            (true, document.text.clone(), Some(document.version), true)
        }
        // A clean editor copy keeps the editor's text and version, but a deleted file stays deleted.
        Some(document) => {
            if checked_file(&path, file)? {
                (true, document.text.clone(), Some(document.version), false)
            } else {
                absent()
            }
        }
        None => match read_disk(&path, file)? {
            Some(text) => (true, text, None, false),
            None => absent(),
        },
    };
    if u64::try_from(text.len()).unwrap_or(u64::MAX) > MAX_FILE_BYTES {
        return Err(AgentError::limit("file exceeds 1 MiB."));
    }
    let label = version.map_or_else(|| "disk".to_string(), |version| version.to_string());
    let revision = format!("{label}:{}", presence_hash(exists, &text));
    Ok(DocumentSnapshot {
        file: file.to_string(),
        exists,
        text,
        revision,
        version,
        dirty,
    })
}

/// Whether two snapshots hold the same page: both absent, or both present with the same text. Opening,
/// closing or saving a page changes its version and dirty state, not its content, so it is no
/// conflict (`sameContent` in `documents.ts`; specification 43, §8 repair 2).
#[must_use]
pub(super) fn same_content(first: &DocumentSnapshot, second: &DocumentSnapshot) -> bool {
    first.exists == second.exists && first.text == second.text
}

/// SHA-256 of `bytes` as lowercase hexadecimal text.
#[must_use]
pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The observed state of a page that does not exist.
fn absent() -> (bool, String, Option<i64>, bool) {
    (false, String::new(), None, false)
}

/// Whether `file` is a normalized relative path: no empty, `.` or `..` part, and no backslash, colon
/// or NUL (`WorkspaceDocuments.uri` in `documents.ts`).
#[must_use]
fn is_relative_page_path(file: &str) -> bool {
    !file.is_empty()
        && !file
            .chars()
            .any(|character| matches!(character, '\\' | ':' | '\0'))
        && file
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// The refusal of a path that resolves outside the workspace.
fn escape() -> AgentError {
    AgentError::new(PROJECT_UNAVAILABLE, "path escapes the workspace.")
}

/// Whether the page exists as a file of at most 1 MiB; `false` when nothing exists at the path. Any
/// other error, such as an invalid path below a file, is an error and not a missing page.
fn checked_file(path: &Path, file: &str) -> AgentResult<bool> {
    match fs::metadata(path) {
        Ok(metadata) if !metadata.is_file() => Err(AgentError::invalid("expected a file.")),
        Ok(metadata) if metadata.len() > MAX_FILE_BYTES => {
            Err(AgentError::limit("file exceeds 1 MiB."))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AgentError::new(
            PROJECT_UNAVAILABLE,
            format!("cannot read {file}: {error}"),
        )),
    }
}

/// The text of a page on disk, without a UTF-8 byte order mark, or `None` when it does not exist.
fn read_disk(path: &Path, file: &str) -> AgentResult<Option<String>> {
    if !checked_file(path, file)? {
        return Ok(None);
    }
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(AgentError::new(
                PROJECT_UNAVAILABLE,
                format!("cannot read {file}: {error}"),
            ));
        }
    };
    // An editor model holds its text without a byte order mark. The disk text is read the same way,
    // so opening an unchanged page cannot look like a change (specification 43, §8 repair 2).
    let decoded = String::from_utf8_lossy(&bytes);
    Ok(Some(
        decoded
            .strip_prefix('\u{feff}')
            .unwrap_or(&decoded)
            .to_owned(),
    ))
}

/// SHA-256 of the presence marker followed by the text, as the revision names them.
#[must_use]
fn presence_hash(exists: bool, text: &str) -> String {
    let marker: &[u8] = if exists { b"present\0" } else { b"absent\0" };
    let mut hasher = Sha256::new();
    hasher.update(marker);
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
#[path = "documents_tests.rs"]
mod tests;
