//! The projects of the workspace folders (specification 14 §7.6): the root of each workspace
//! folder is a project, and so is every folder below it that holds a project config. A file
//! belongs to the deepest project folder that holds it, and a file outside every project folder to
//! the project whose library folder holds it; a project also sees the files of its library
//! folders, except those of a project nested below such a folder. The analysis judges each file in
//! its own project and sees nothing of another project's files (§10.2).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustc_hash::{FxHashMap, FxHashSet};
use tower_lsp::lsp_types::Url;
use trust_hir::db::{FileId, ProjectId, ProjectLayout};
use trust_hir::SourceKey;

use crate::config::ProjectConfig;

use super::library_surface::ProjectFolders;
use super::path::{canonicalize_through_existing_ancestor, uri_to_path};
use super::ServerState;

impl ServerState {
    /// Sets the projects nested below the workspace folder `folder`, each `(root, config)` of
    /// `projects` governing the files below its root; a project the folder held before and holds no
    /// longer is forgotten. A nested project never chooses the language rules
    /// ([`ServerState::primary_workspace_config`]).
    pub(crate) fn set_nested_projects(&self, folder: &Url, projects: Vec<(Url, ProjectConfig)>) {
        // Resolved before the configurations become visible, outside every lock.
        let resolved: Vec<(Url, Arc<ProjectFolders>, ProjectConfig)> = projects
            .into_iter()
            .map(|(root, config)| (root, Arc::new(ProjectFolders::of(&config)), config))
            .collect();
        let roots: Vec<Url> = resolved.iter().map(|(root, _, _)| root.clone()).collect();
        let previous = self
            .nested_projects
            .write()
            .insert(folder.clone(), roots.clone())
            .unwrap_or_default();
        let gone: Vec<&Url> = previous
            .iter()
            .filter(|root| !roots.contains(root))
            .collect();
        {
            let mut folders = self.project_folders.write();
            for root in &gone {
                folders.remove(*root);
            }
            for (root, project_folders, _) in &resolved {
                folders.insert(root.clone(), Arc::clone(project_folders));
            }
        }
        {
            let mut docs = self.library_docs.write();
            for root in gone.iter().copied().chain(&roots) {
                docs.remove(root);
            }
        }
        let mut configs = self.workspace_configs.write();
        for root in gone {
            configs.remove(root);
        }
        for (root, _, config) in resolved {
            configs.insert(root, config);
        }
    }

    /// Places every indexed file in its project and gives the analysis the layout (specification
    /// 14 §7.6, §10.2).
    pub(crate) fn refresh_project_layout(&self) {
        let layout = self.project_layout();
        self.project
            .write()
            .database_mut()
            .set_project_layout(layout);
    }

    /// The layout of the indexed files over the configured projects, numbered in the order of
    /// their resolved folders from 1. A file that neither a project folder nor a library folder
    /// holds, a file opened from another folder, belongs while it is open to the root project of
    /// the first workspace folder (§7.2.1), as it belonged to the one project before folders held
    /// several.
    fn project_layout(&self) -> ProjectLayout {
        let mut roots: Vec<(PathBuf, Url)> = self
            .workspace_configs
            .read()
            .keys()
            .filter_map(|root| {
                uri_to_path(root)
                    .map(|path| (canonicalize_through_existing_ancestor(&path), root.clone()))
            })
            .collect();
        roots.sort();
        roots.dedup_by(|later, earlier| later.0 == earlier.0);
        let ids: FxHashMap<PathBuf, ProjectId> = roots
            .iter()
            .zip(1..)
            .map(|((path, _), number)| (path.clone(), ProjectId(number)))
            .collect();
        let folders = self.project_folders.read().clone();
        let libraries: Vec<(ProjectId, Vec<PathBuf>)> = roots
            .iter()
            .filter_map(|(path, root)| {
                let project_folders = folders.get(root)?;
                Some((ids[path], project_folders.libraries().to_vec()))
            })
            .filter(|(_, library_folders)| !library_folders.is_empty())
            .collect();
        let sources: Vec<(FileId, PathBuf)> = self
            .project
            .read()
            .sources()
            .iter()
            .filter_map(|(key, file_id)| match key {
                SourceKey::Path(path) => Some((file_id, path.clone())),
                SourceKey::Virtual(_) => None,
            })
            .collect();

        // The root project of the first workspace folder, which takes a file opened from elsewhere.
        let first_root = self.workspace_folders().first().and_then(|folder| {
            let path = canonicalize_through_existing_ancestor(&uri_to_path(folder)?);
            ids.get(&path).copied()
        });
        let mut layout = ProjectLayout::default();
        // The depth of the project folder that holds each file, so that a library folder leaves
        // out the files of a project nested below it.
        let mut held_at: FxHashMap<FileId, usize> = FxHashMap::default();
        for (file_id, path) in &sources {
            let holder = path
                .ancestors()
                .find_map(|folder| ids.get(folder).map(|id| (*id, folder.components().count())));
            if let Some((id, depth)) = holder {
                layout.files.insert(*file_id, id);
                held_at.insert(*file_id, depth);
            } else if let Some(id) = library_owner(&libraries, path).or(first_root) {
                layout.files.insert(*file_id, id);
            }
        }
        for (project, library_folders) in &libraries {
            let mut seen: FxHashSet<FileId> = FxHashSet::default();
            for folder in library_folders {
                let depth = folder.components().count();
                for (file_id, path) in &sources {
                    if path.starts_with(folder)
                        && held_at.get(file_id).is_none_or(|held| *held <= depth)
                        && layout.files.get(file_id) != Some(project)
                    {
                        seen.insert(*file_id);
                    }
                }
            }
            if !seen.is_empty() {
                let mut files: Vec<FileId> = seen.into_iter().collect();
                files.sort_by_key(|file_id| file_id.0);
                layout.library_files.insert(*project, files);
            }
        }
        layout
    }
}

/// The highest-priority configuration among the workspace folders' roots: a nested project never
/// chooses the language rules (specification 14 §7.6).
pub(super) fn primary_workspace_config(state: &ServerState) -> Option<ProjectConfig> {
    let nested: FxHashSet<Url> = state
        .nested_projects
        .read()
        .values()
        .flatten()
        .cloned()
        .collect();
    state
        .workspace_configs
        .read()
        .iter()
        .filter(|(root, _)| !nested.contains(*root))
        .map(|(_, config)| config.clone())
        .max_by_key(|config| config.workspace.priority)
}

/// The project whose library folder holds `path`, a file that no project folder holds: the
/// deepest such folder decides, then the project numbered first.
fn library_owner(libraries: &[(ProjectId, Vec<PathBuf>)], path: &Path) -> Option<ProjectId> {
    libraries
        .iter()
        .filter_map(|(project, folders)| {
            folders
                .iter()
                .filter(|folder| path.starts_with(folder))
                .map(|folder| folder.components().count())
                .max()
                .map(|depth| (depth, *project))
        })
        .max_by(|(depth_a, project_a), (depth_b, project_b)| {
            depth_a.cmp(depth_b).then(project_b.cmp(project_a))
        })
        .map(|(_, project)| project)
}
