//! The stable runtime ID and the platform a runtime runs on: the identity that `status` and the
//! mDNS announcement carry (specification 11, "Runtime discovery"; decision O36).
//!
//! The ID is generated once with a random part and kept in the project's
//! `.trust-runtime/runtime_id`; every start reads it back. It stays the same across restarts of
//! one project and differs between two runtimes on one host, which a runtime name or a process ID
//! cannot. A project folder that cannot hold the file, a read-only or foreign one, has the ID kept
//! for the user instead, in truST's folder in the user's state folder, under the project-path hash
//! of specification 11 §6.8.0a. When neither place can hold it, the runtime still starts, with an
//! ID for its process: a tooling file never stops a PLC program (specification 34, ARCH-07).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rand::TryRng;
use smol_str::SmolStr;

use crate::control::endpoint::{canonical_project_root, project_path_hash};
use crate::error::RuntimeError;

/// The folder below the project root that keeps runtime state.
const STATE_DIR: &str = ".trust-runtime";
/// The file that keeps the stable runtime ID.
const ID_FILE: &str = "runtime_id";
/// The folder below truST's state folder that keeps the IDs of projects that cannot hold their own.
const USER_ID_DIR: &str = "runtimes";
/// The longest runtime ID a file may hold, in bytes.
const ID_MAX_LEN: usize = 64;

/// Where the runtime's ID is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeIdPlace {
    /// The project's `.trust-runtime/runtime_id`.
    Project(PathBuf),
    /// The file kept for the user, `<truST state folder>/runtimes/<hash>.id`, because the project
    /// folder cannot hold the ID.
    User(PathBuf),
    /// Nowhere: neither place can hold the ID, so it serves this process only. The text says why.
    NotKept(String),
}

/// The runtime's ID and where it is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeIdentity {
    /// The ID that `status` and the announcement carry.
    pub id: SmolStr,
    /// Where the ID is kept.
    pub place: RuntimeIdPlace,
}

/// The path of the project's runtime ID file, `<project>/.trust-runtime/runtime_id`.
#[must_use]
pub(crate) fn runtime_id_path(project_root: &Path) -> PathBuf {
    project_root.join(STATE_DIR).join(ID_FILE)
}

/// The project's stable runtime ID; see [`runtime_identity`].
///
/// # Errors
///
/// As [`runtime_identity`].
pub fn stable_runtime_id(project_root: &Path) -> Result<SmolStr, RuntimeError> {
    runtime_identity(project_root).map(|identity| identity.id)
}

/// The project's stable runtime ID, generated and kept on first use, and where it is kept
/// (specification 11, runtime discovery). The project's own file comes first. Without one, the ID
/// kept for the user is used when there is one. Otherwise a new ID is written into the project, or,
/// when the project folder cannot hold it, kept for the user. When neither place can hold it, the
/// ID serves this process only and the place says why.
///
/// A file that cannot be read, or that holds a value that is not a valid ID, is an error: it is
/// never silently replaced with a fresh ID.
///
/// # Errors
///
/// A runtime ID file exists but cannot be read or holds no valid ID, or the operating system's
/// secure random source fails.
pub fn runtime_identity(project_root: &Path) -> Result<RuntimeIdentity, RuntimeError> {
    let state = user_state_folder(std::env::consts::OS, |name| std::env::var_os(name));
    runtime_identity_in(project_root, state.as_deref())
}

/// [`runtime_identity`] with truST's state folder of the user given.
fn runtime_identity_in(
    project_root: &Path,
    state_folder: Option<&Path>,
) -> Result<RuntimeIdentity, RuntimeError> {
    let project_file = runtime_id_path(project_root);
    if let Some(id) = read_runtime_id(&project_file)? {
        return Ok(RuntimeIdentity {
            id,
            place: RuntimeIdPlace::Project(project_file),
        });
    }
    let user_file = state_folder.map(|folder| user_id_path(folder, project_root));
    if let Some(path) = &user_file
        && let Some(id) = read_runtime_id(path)?
    {
        return Ok(RuntimeIdentity {
            id,
            place: RuntimeIdPlace::User(path.clone()),
        });
    }
    let id = generate_runtime_id()?;
    let project_failure = match write_runtime_id(&project_file, &id) {
        Ok(()) => {
            return Ok(RuntimeIdentity {
                id,
                place: RuntimeIdPlace::Project(project_file),
            });
        }
        Err(reason) => reason,
    };
    let place = match user_file {
        Some(path) => match write_runtime_id(&path, &id) {
            Ok(()) => RuntimeIdPlace::User(path),
            Err(reason) => RuntimeIdPlace::NotKept(format!("{project_failure}; {reason}")),
        },
        None => RuntimeIdPlace::NotKept(format!(
            "{project_failure}; the user's state folder is not known"
        )),
    };
    Ok(RuntimeIdentity { id, place })
}

/// `<truST state folder>/runtimes/<hash>.id`, where `<hash>` is the hash of the canonical project
/// path that names the fallback socket and the per-user token file (specification 11 §6.8.0a).
fn user_id_path(state_folder: &Path, project_root: &Path) -> PathBuf {
    let canonical =
        canonical_project_root(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    state_folder
        .join(USER_ID_DIR)
        .join(format!("{}.id", project_path_hash(&canonical)))
}

/// truST's folder in the user's state folder, by the platform's convention, as Control Studio's
/// journals use it (specification 35, §0.1b): `$XDG_STATE_HOME/trust` (an absolute value only) or
/// `$HOME/.local/state/trust` on Linux and the other Unix systems, `$HOME/Library/Application
/// Support/trust` on macOS, and `%LOCALAPPDATA%\trust` on Windows. `None` when the variable that
/// the folder starts from is not set.
fn user_state_folder(os: &str, variable: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let folder = |name: &str| {
        variable(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let base = match os {
        "windows" => folder("LOCALAPPDATA"),
        "macos" => folder("HOME").map(|home| home.join("Library").join("Application Support")),
        _ => folder("XDG_STATE_HOME")
            .filter(|path| path.is_absolute())
            .or_else(|| folder("HOME").map(|home| home.join(".local").join("state"))),
    };
    base.map(|base| base.join("trust"))
}

/// The ID in the file at `path`: `None` when there is no file, an error when it cannot be read or
/// does not hold a valid ID.
fn read_runtime_id(path: &Path) -> Result<Option<SmolStr>, RuntimeError> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_runtime_id(text.as_str(), path).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(RuntimeError::InvalidBundle(
            format!("runtime ID file {} cannot be read: {error}", path.display()).into(),
        )),
    }
}

/// Writes `id` into the file at `path`, creating its folder; why it cannot, when it cannot.
fn write_runtime_id(path: &Path, id: &str) -> Result<(), String> {
    let Some(folder) = path.parent() else {
        return Err(format!("runtime ID path {} has no folder", path.display()));
    };
    std::fs::create_dir_all(folder).map_err(|error| {
        format!(
            "runtime ID folder {} cannot be created: {error}",
            folder.display()
        )
    })?;
    std::fs::write(path, id).map_err(|error| {
        format!(
            "runtime ID file {} cannot be written: {error}",
            path.display()
        )
    })
}

/// The platform the runtime runs on: `<os>-<arch>` on a computer, for example `linux-x86_64`
/// (specification 11, `status`). A device reports its board name from its own platform instead.
#[must_use]
pub(crate) fn platform() -> &'static str {
    static PLATFORM: OnceLock<String> = OnceLock::new();
    PLATFORM
        .get_or_init(|| format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH))
        .as_str()
}

fn parse_runtime_id(text: &str, path: &Path) -> Result<SmolStr, RuntimeError> {
    let trimmed = text.trim();
    if is_valid_runtime_id(trimmed) {
        Ok(SmolStr::new(trimmed))
    } else {
        Err(RuntimeError::InvalidBundle(
            format!(
                "runtime ID file {} does not hold a valid runtime ID",
                path.display()
            )
            .into(),
        ))
    }
}

fn is_valid_runtime_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= ID_MAX_LEN
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn generate_runtime_id() -> Result<SmolStr, RuntimeError> {
    let mut bytes = [0_u8; 16];
    let mut rng = rand::rngs::SysRng;
    rng.try_fill_bytes(&mut bytes).map_err(|error| {
        RuntimeError::InvalidBundle(
            format!("runtime ID cannot be generated: the secure random source failed: {error}")
                .into(),
        )
    })?;
    Ok(SmolStr::new(hex::encode(bytes)))
}

#[cfg(test)]
#[path = "runtime_id_tests.rs"]
mod tests;
