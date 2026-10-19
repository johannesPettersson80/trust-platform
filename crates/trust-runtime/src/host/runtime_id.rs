//! The stable runtime ID and the platform a runtime runs on: the identity that `status` and the
//! mDNS announcement carry (specification 11, "Runtime discovery"; decision O36).
//!
//! The ID is generated once with a random part and kept in the project's
//! `.trust-runtime/runtime_id`; every start reads it back. It stays the same across restarts of
//! one project and differs between two runtimes on one host, which a runtime name or a process ID
//! cannot.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rand::TryRng;
use smol_str::SmolStr;

use crate::error::RuntimeError;

/// The folder below the project root that keeps runtime state.
const STATE_DIR: &str = ".trust-runtime";
/// The file that keeps the stable runtime ID.
const ID_FILE: &str = "runtime_id";
/// The longest runtime ID a file may hold, in bytes.
const ID_MAX_LEN: usize = 64;

/// The path of the project's runtime ID file, `<project>/.trust-runtime/runtime_id`.
#[must_use]
pub(crate) fn runtime_id_path(project_root: &Path) -> PathBuf {
    project_root.join(STATE_DIR).join(ID_FILE)
}

/// The project's stable runtime ID, generated and kept on first use.
///
/// A file that cannot be read, or that holds a value that is not a valid ID, is an error: it is
/// never silently replaced with a fresh ID.
///
/// # Errors
///
/// The state folder or file cannot be created or written, the file cannot be read, the operating
/// system's secure random source fails, or the file's content is not a valid ID.
pub fn stable_runtime_id(project_root: &Path) -> Result<SmolStr, RuntimeError> {
    let path = runtime_id_path(project_root);
    match std::fs::read_to_string(&path) {
        Ok(text) => parse_runtime_id(text.as_str(), &path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => create_runtime_id(&path),
        Err(error) => Err(RuntimeError::InvalidBundle(
            format!("runtime ID file {} cannot be read: {error}", path.display()).into(),
        )),
    }
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

fn create_runtime_id(path: &Path) -> Result<SmolStr, RuntimeError> {
    let folder = path.parent().ok_or_else(|| {
        RuntimeError::InvalidBundle(
            format!("runtime ID path {} has no folder", path.display()).into(),
        )
    })?;
    std::fs::create_dir_all(folder).map_err(|error| {
        RuntimeError::InvalidBundle(
            format!(
                "runtime ID folder {} cannot be created: {error}",
                folder.display()
            )
            .into(),
        )
    })?;
    let id = generate_runtime_id()?;
    std::fs::write(path, id.as_str()).map_err(|error| {
        RuntimeError::InvalidBundle(
            format!(
                "runtime ID file {} cannot be written: {error}",
                path.display()
            )
            .into(),
        )
    })?;
    Ok(id)
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
