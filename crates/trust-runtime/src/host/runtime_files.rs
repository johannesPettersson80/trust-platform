//! Where a runtime keeps the files it writes at a path of its configuration (specification 11,
//! section 6.8.0a): an absolute path as it is, and a relative one in the folder of its runtime files,
//! the project folder or, for a runtime started on a program store's `current` link (specification
//! 34 §11.2), the store folder. An install, a deployment or a rollback moves `current` to another
//! bundle, so a file kept in a bundle would stay behind with that bundle.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The folder of a project's runtime files: the store folder for a store's `current` link, and
/// otherwise the project folder itself.
#[must_use]
pub fn runtime_files_folder(project_root: &Path) -> PathBuf {
    crate::control::endpoint::runtime_files_folder(project_root)
}

/// The file that `configured`, a path of the project's configuration for a file the runtime writes,
/// names: as it is when absolute, and in the folder of the runtime's files when relative.
#[must_use]
pub fn runtime_file_path(project_root: &Path, configured: &Path) -> PathBuf {
    if configured.is_absolute() {
        configured.to_path_buf()
    } else {
        runtime_files_folder(project_root).join(configured)
    }
}

/// [`runtime_file_path`] for the start of a runtime. A store runtime's file that an earlier version
/// kept at `configured` in the `current` bundle is moved into the store folder first, unless the
/// store folder already holds it, so that an upgrade keeps it. Each of `companions`, appended to the
/// file's name, names a file that belongs to it, such as a database's journal, and moves with it.
///
/// # Errors
///
/// A folder cannot be created or a file cannot be moved.
pub fn prepare_runtime_file(
    project_root: &Path,
    configured: &Path,
    companions: &[&str],
) -> io::Result<PathBuf> {
    let path = runtime_file_path(project_root, configured);
    let in_bundle = project_root.join(configured);
    if configured.is_relative() && in_bundle != path && !path.exists() && in_bundle.is_file() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        for companion in companions {
            let from = with_suffix(&in_bundle, companion);
            if from.is_file() {
                fs::rename(&from, with_suffix(&path, companion))?;
            }
        }
        fs::rename(&in_bundle, &path)?;
    }
    Ok(path)
}

/// `path` with `suffix` appended to its file name, as `db.sqlite` and `-wal` give `db.sqlite-wal`.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
#[path = "runtime_files_tests.rs"]
mod tests;
