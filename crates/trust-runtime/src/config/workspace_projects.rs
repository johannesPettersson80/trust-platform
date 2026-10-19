//! The truST runtime projects of a folder (specification 25, section 5.11): the folder itself
//! when it holds a `runtime.toml`, and each direct subfolder that holds one. The config UI's
//! workspace and the Devices & Connections project list both find projects this way.

use std::fs;
use std::path::{Path, PathBuf};

/// The runtime projects of `folder`, sorted: `folder` itself when it holds a `runtime.toml` file,
/// and each direct subfolder that holds one. Deeper folders are not searched, and a folder that
/// cannot be listed has no subfolder projects.
#[must_use]
pub fn find_runtime_projects(folder: &Path) -> Vec<PathBuf> {
    let mut projects = Vec::new();
    if folder.join("runtime.toml").is_file() {
        projects.push(folder.to_path_buf());
    }
    if let Ok(entries) = fs::read_dir(folder) {
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let candidate = entry.path();
            if candidate.join("runtime.toml").is_file() {
                projects.push(candidate);
            }
        }
    }
    projects.sort();
    projects.dedup();
    projects
}
