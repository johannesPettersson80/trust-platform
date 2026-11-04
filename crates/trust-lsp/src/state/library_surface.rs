//! Library files are API surface for the unused-symbol warnings W001, W002 and W009
//! (specification 14 §7.6, specification 28 §1).
//!
//! A project's own source folders are its `include_paths`, or its root when none are set; its
//! library folders are its `[[libraries]]` folders, its `[project] library_paths` and its resolved
//! `[dependencies]` packages. For each file the deepest folder that holds it decides, and an own
//! source folder wins over an equally deep library folder.
//!
//! Folders are compared where they resolve on disk, so that a link, a junction, a short Windows
//! name and a canonicalized `[dependencies]` path all name the folder the client opened. A
//! project's folders are resolved once, when its configuration is set; a document is resolved
//! only when its project has library folders, and for a file outside every workspace root, which
//! follows the project whose library folder holds it (specification 14 §7.6.4).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tower_lsp::lsp_types::Url;

use crate::config::ProjectConfig;

use super::path::{
    canonicalize_through_existing_ancestor, uri_to_path, workspace_config_match_for_uri,
};
use super::ServerState;

/// The configuration that governs a source file's diagnostics, and whether the file is library
/// surface under it.
#[derive(Debug, Clone)]
pub struct DiagnosticScope {
    /// The project configuration whose `[diagnostics]` settings apply to the file.
    pub config: ProjectConfig,
    /// Whether the file is library surface, so that W001, W002 and W009 are dropped for it.
    pub library_surface: bool,
}

/// A project's own source folders and library folders, resolved on disk.
#[derive(Debug)]
pub(super) struct ProjectFolders {
    own: Vec<PathBuf>,
    libraries: Vec<PathBuf>,
}

impl ProjectFolders {
    /// Resolves the own source folders and the library folders of `config`.
    pub(super) fn of(config: &ProjectConfig) -> Self {
        Self {
            own: config
                .source_folders()
                .iter()
                .map(|folder| canonicalize_through_existing_ancestor(folder))
                .collect(),
            libraries: config
                .libraries
                .iter()
                .map(|library| canonicalize_through_existing_ancestor(&library.path))
                .collect(),
        }
    }

    /// Whether the resolved `file` is library surface.
    fn is_library_surface(&self, file: &Path) -> bool {
        library_holds(&self.own, &self.libraries, file)
    }
}

/// Whether a library folder holds `file` deeper than every own folder that holds it: the deepest
/// folder that holds a file decides, and an own folder wins over an equally deep library folder.
pub(crate) fn library_holds(own: &[PathBuf], libraries: &[PathBuf], file: &Path) -> bool {
    deepest_holding(libraries, file)
        .is_some_and(|library| deepest_holding(own, file).is_none_or(|own| library > own))
}

/// The depth, in path components, of the deepest of `folders` that holds `file`. Component-wise,
/// so that `libs/vendor` does not hold `libs/vendor-other/x.st`.
fn deepest_holding(folders: &[PathBuf], file: &Path) -> Option<usize> {
    folders
        .iter()
        .filter(|folder| file.starts_with(folder))
        .map(|folder| folder.components().count())
        .max()
}

/// The configuration that governs `uri`'s diagnostics, and whether the file is library surface in
/// that project: the workspace root that holds the file, or for a file outside every root the
/// project whose library folder holds it.
pub(super) fn diagnostic_scope_for_uri(state: &ServerState, uri: &Url) -> Option<DiagnosticScope> {
    if let Some((root, config)) = workspace_config_match_for_uri(state, uri) {
        let folders = state.project_folders.read().get(&root).cloned();
        let library_surface = folders.is_some_and(|folders| {
            !folders.libraries.is_empty()
                && resolved_document(uri).is_some_and(|file| folders.is_library_surface(&file))
        });
        return Some(DiagnosticScope {
            config,
            library_surface,
        });
    }
    let file = resolved_document(uri)?;
    let (config, folders) = library_owner(state, &file)?;
    Some(DiagnosticScope {
        library_surface: folders.is_library_surface(&file),
        config,
    })
}

/// The project whose library folder holds the resolved `file`, a file outside every workspace
/// root: the deepest such folder decides across projects, then the higher `[workspace]
/// priority`, then the root that sorts first.
fn library_owner(state: &ServerState, file: &Path) -> Option<(ProjectConfig, Arc<ProjectFolders>)> {
    let candidates: Vec<(usize, Url, Arc<ProjectFolders>)> = state
        .project_folders
        .read()
        .iter()
        .filter_map(|(root, folders)| {
            deepest_holding(&folders.libraries, file)
                .map(|depth| (depth, root.clone(), Arc::clone(folders)))
        })
        .collect();
    let deepest = candidates.iter().map(|(depth, _, _)| *depth).max()?;
    let configs = state.workspace_configs.read();
    candidates
        .into_iter()
        .filter(|(depth, _, _)| *depth == deepest)
        .filter_map(|(_, root, folders)| configs.get(&root).map(|config| (root, config, folders)))
        .max_by(|(root_a, a, _), (root_b, b, _)| {
            a.workspace
                .priority
                .cmp(&b.workspace.priority)
                .then_with(|| root_b.as_str().cmp(root_a.as_str()))
        })
        .map(|(_, config, folders)| (config.clone(), folders))
}

/// The document's path resolved on disk, through its deepest existing folder when it is missing.
fn resolved_document(uri: &Url) -> Option<PathBuf> {
    uri_to_path(uri).map(|path| canonicalize_through_existing_ancestor(&path))
}

#[cfg(test)]
#[path = "library_surface_tests.rs"]
mod tests;
