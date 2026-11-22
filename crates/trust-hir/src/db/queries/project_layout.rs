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

/// Which project each file belongs to, and which projects' files each project sees besides its
/// own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectLayout {
    /// The project of each file; a file not named here belongs to the default project.
    pub files: FxHashMap<FileId, ProjectId>,
    /// The projects whose files a project sees besides its own: its libraries and dependencies,
    /// transitive ones included, which the server resolves. A library is judged in its own project.
    pub libraries: FxHashMap<ProjectId, Vec<ProjectId>>,
}

impl ProjectLayout {
    /// The project of `file_id`.
    #[must_use]
    pub fn project_of(&self, file_id: FileId) -> ProjectId {
        self.files.get(&file_id).copied().unwrap_or_default()
    }

    /// The files that each project with a file of `files` sees: its own and its libraries', each
    /// list ordered by file. A project without a file of its own judges no file and has no list.
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
        own.iter()
            .map(|(&project, files)| {
                let mut seen: FxHashSet<FileId> = files.iter().map(|(id, _)| *id).collect();
                let mut visible = files.clone();
                for library in self.libraries.get(&project).into_iter().flatten() {
                    for &(file_id, value) in own.get(library).into_iter().flatten() {
                        if seen.insert(file_id) {
                            visible.push((file_id, value));
                        }
                    }
                }
                visible.sort_by_key(|(file_id, _)| file_id.0);
                (project, visible)
            })
            .collect()
    }
}
