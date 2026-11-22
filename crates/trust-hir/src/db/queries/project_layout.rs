//! The projects of the analysis (specification 14 §10.2): each file belongs to one project and is
//! analyzed among the files of its project and of that project's libraries, never among another
//! project's files. The server forms the projects (§7.6); without a layout every file belongs to
//! one project.

use rustc_hash::{FxHashMap, FxHashSet};

use super::FileId;

/// A project of the analysis, numbered by the server. The default project holds every file that
/// the layout does not place.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectId(pub u32);

/// Which project each file belongs to, and which other files each project sees besides its own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectLayout {
    /// The project of each file, which judges it; a file not named here belongs to the default
    /// project.
    pub files: FxHashMap<FileId, ProjectId>,
    /// The files a project sees besides its own: those of its libraries and dependencies,
    /// transitive ones included, which the server resolves. Such a file is judged in its own
    /// project, which does not see the projects that use it.
    pub library_files: FxHashMap<ProjectId, Vec<FileId>>,
}

impl ProjectLayout {
    /// The project of `file_id`.
    #[must_use]
    pub fn project_of(&self, file_id: FileId) -> ProjectId {
        self.files.get(&file_id).copied().unwrap_or_default()
    }

    /// The files that each project with a file of `files` sees: its own and its library files of
    /// `files`, each list ordered by file. A project without a file of its own judges no file and
    /// has no list.
    pub(super) fn visible_files<T: Copy>(
        &self,
        files: &[(FileId, T)],
    ) -> FxHashMap<ProjectId, Vec<(FileId, T)>> {
        let mut own: FxHashMap<ProjectId, Vec<(FileId, T)>> = FxHashMap::default();
        for &(file_id, value) in files {
            own.entry(self.project_of(file_id))
                .or_default()
                .push((file_id, value));
        }
        let present: FxHashMap<FileId, T> = files.iter().copied().collect();
        own.iter()
            .map(|(&project, files)| {
                let mut seen: FxHashSet<FileId> = files.iter().map(|(id, _)| *id).collect();
                let mut visible = files.clone();
                for &file_id in self.library_files.get(&project).into_iter().flatten() {
                    if let Some(&value) = present.get(&file_id)
                        && seen.insert(file_id)
                    {
                        visible.push((file_id, value));
                    }
                }
                visible.sort_by_key(|(file_id, _)| file_id.0);
                (project, visible)
            })
            .collect()
    }
}
