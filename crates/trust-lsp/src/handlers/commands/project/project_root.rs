//! The project that Compile checks (specification 25 §6.2): the nearest folder above the active
//! file that holds `trust-lsp.toml`; without one, the workspace folder of the active file; without
//! an active file, the first workspace folder.
//!
//! The runtime project of a workspace folder (specification 11 §6.8.0a): the folder when it holds
//! `runtime.toml`, else its `bundle` folder when that holds one, else none. Attach, the runtime
//! connection and the simulation speed find a project's runtime there.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};

use super::decode;

#[derive(Deserialize)]
struct Args {
    /// The active editor's file, when it is a file on disk.
    #[serde(default)]
    path: Option<PathBuf>,
    /// The workspace folders, in VS Code's order.
    #[serde(default)]
    folders: Vec<PathBuf>,
    /// A workspace folder whose runtime project is asked for.
    #[serde(default, rename = "runtimeFolder")]
    runtime_folder: Option<PathBuf>,
}

/// `{ ok: true, root, runtimeProject }`: `root` is `null` when there is no folder to check, and
/// `runtimeProject` the runtime project of `runtimeFolder`, `null` without one.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    let args: Args = decode(argument)?;
    let root = compile_root(args.path.as_deref(), &args.folders);
    let runtime_project = args
        .runtime_folder
        .as_deref()
        .and_then(runtime_project_folder);
    Ok(json!({
        "ok": true,
        "root": root.map(|root| root.display().to_string()),
        "runtimeProject": runtime_project.map(|folder| folder.display().to_string()),
    }))
}

fn compile_root(path: Option<&Path>, folders: &[PathBuf]) -> Option<PathBuf> {
    if let Some(path) = path {
        if let Some(root) = nearest_manifest_folder(path) {
            return Some(root);
        }
        if let Some(folder) = containing_folder(path, folders) {
            return Some(folder.to_path_buf());
        }
    }
    folders.first().cloned()
}

/// The nearest folder at or above `path` that holds `trust-lsp.toml`.
pub(super) fn nearest_manifest_folder(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() { path } else { path.parent()? };
    start
        .ancestors()
        .find(|folder| folder.join("trust-lsp.toml").is_file())
        .map(Path::to_path_buf)
}

/// The folder of a workspace folder's runtime project: the folder itself when it holds
/// `runtime.toml`, else its `bundle` folder when that holds one.
pub(super) fn runtime_project_folder(folder: &Path) -> Option<PathBuf> {
    [folder.to_path_buf(), folder.join("bundle")]
        .into_iter()
        .find(|candidate| candidate.join("runtime.toml").is_file())
}

/// The innermost of `folders` that contains `path`, as VS Code names a file's workspace folder.
pub(super) fn containing_folder<'a>(path: &Path, folders: &'a [PathBuf]) -> Option<&'a Path> {
    folders
        .iter()
        .filter(|folder| path.starts_with(folder))
        .max_by_key(|folder| folder.components().count())
        .map(PathBuf::as_path)
}

#[cfg(test)]
#[path = "project_root_tests.rs"]
mod tests;
