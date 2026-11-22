//! A library file's symbol table is collected once, in the file's own project, and each project
//! that sees the file takes it from there (checklist G276c). The file's own project judges it
//! (specification 14 §10.2) and does not see the projects that use it, so its table is the one
//! every user reads; collecting it again in each user cost memory and time with every project that
//! names the library.

use rustc_hash::{FxHashMap, FxHashSet};
use salsa::Setter;

use super::salsa_backend::{ProjectInputs, SalsaState, SourceInput};
use super::{FileId, ProjectId};

/// Gives each project the files of other projects whose symbol table it takes from their own
/// project: a file whose project cannot reach the user through the projects it sees, so that no
/// project's query asks, through its libraries, for its own. A file of a mutual library is
/// collected in each project that sees it.
pub(super) fn share_library_tables(
    state: &mut SalsaState,
    visible: &FxHashMap<ProjectId, Vec<(FileId, SourceInput)>>,
) {
    let sees: FxHashMap<ProjectId, FxHashSet<ProjectId>> = visible
        .iter()
        .map(|(&project, files)| {
            let others = files
                .iter()
                .map(|(file_id, _)| state.layout.project_of(*file_id))
                .filter(|owner| *owner != project)
                .collect();
            (project, others)
        })
        .collect();
    let mut reachable: FxHashMap<ProjectId, FxHashSet<ProjectId>> = FxHashMap::default();
    for (&project, files) in visible {
        let mut shared: Vec<(FileId, ProjectInputs)> = Vec::new();
        for (file_id, _) in files {
            let owner = state.layout.project_of(*file_id);
            if owner == project {
                continue;
            }
            let owner_reaches = reachable
                .entry(owner)
                .or_insert_with(|| reached_from(&sees, owner));
            if owner_reaches.contains(&project) {
                continue;
            }
            if let Some(inputs) = state.projects.get(&owner) {
                shared.push((*file_id, *inputs));
            }
        }
        let Some(inputs) = state.projects.get(&project).copied() else {
            continue;
        };
        if inputs.shared(&state.db) != &shared {
            inputs.set_shared(&mut state.db).to(shared);
        }
    }
}

/// The projects that `from` sees, directly or through the projects it sees.
fn reached_from(
    sees: &FxHashMap<ProjectId, FxHashSet<ProjectId>>,
    from: ProjectId,
) -> FxHashSet<ProjectId> {
    let mut reached = FxHashSet::default();
    let mut stack = vec![from];
    while let Some(project) = stack.pop() {
        for &next in sees.get(&project).into_iter().flatten() {
            if reached.insert(next) {
                stack.push(next);
            }
        }
    }
    reached
}
