//! The chosen project's inputs that need no runtime (specification 25 §5.11): the project list,
//! the configured topology, the runtimes that its `fleet.toml` manages, and the catalog. The
//! project's files are read again only after one of them changed.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use trust_runtime::config::fleet_manifest::{
    load_manifest, FleetManifestRuntime, FLEET_MANIFEST_FILE,
};
use trust_runtime::config::workspace_projects::find_runtime_projects;
use trust_runtime::config::RuntimeConfig;
use trust_runtime::control::{offline_comm_schema_json, offline_fleet_topology_json};

use crate::request::GatherMode;

/// The project inputs of one gather.
#[derive(Debug, Clone)]
pub struct ProjectInputs {
    /// The truST projects of the workspace folders: each folder that holds a `runtime.toml`, and
    /// each of its direct subfolders that holds one.
    pub projects: Vec<PathBuf>,
    /// The chosen project, which is also the fleet root: the requested one when it is listed,
    /// otherwise the first.
    pub chosen: Option<PathBuf>,
    /// The chosen project's configured topology, as `trust-runtime comm topology` reads it.
    pub topology: Option<Result<Value, String>>,
    /// The runtimes that this computer manages: the entries of the chosen project's
    /// `fleet.toml`. A project without that file is a fleet of its own, with none.
    pub managed: Result<Vec<FleetManifestRuntime>, String>,
    /// The communication catalog that needs no runtime, as `trust-runtime comm schema` answers.
    pub catalog: Result<Value, String>,
}

/// What a session keeps of the project's files between gathers.
#[derive(Debug, Default)]
pub(super) struct ProjectCache {
    read: Option<ProjectRead>,
    catalog: Option<Result<Value, String>>,
}

#[derive(Debug)]
struct ProjectRead {
    project: PathBuf,
    stamps: Vec<Stamp>,
    topology: Result<Value, String>,
    managed: Result<Vec<FleetManifestRuntime>, String>,
}

/// A file's modification time and length when it was read; `None` when it was missing.
#[derive(Debug, PartialEq, Eq)]
struct Stamp {
    path: PathBuf,
    seen: Option<(SystemTime, u64)>,
}

impl Stamp {
    fn of(path: PathBuf) -> Self {
        let seen = fs::metadata(&path)
            .ok()
            .and_then(|metadata| Some((metadata.modified().ok()?, metadata.len())));
        Self { path, seen }
    }

    fn unchanged(&self) -> bool {
        Self::of(self.path.clone()) == *self
    }
}

impl ProjectCache {
    /// The project inputs for the workspace `folders` and the `requested` project. A `now`
    /// gather reads the project's files again; any other reads them again only after one of
    /// them changed.
    pub(super) fn read(
        &mut self,
        folders: &[PathBuf],
        requested: Option<&Path>,
        mode: GatherMode,
    ) -> ProjectInputs {
        let projects = project_list(folders);
        let chosen = requested
            .and_then(|requested| projects.iter().find(|project| *project == requested))
            .or_else(|| projects.first())
            .cloned();
        let catalog = self
            .catalog
            .get_or_insert_with(|| offline_comm_schema_json(None))
            .clone();
        let (topology, managed) = match &chosen {
            Some(project) => {
                let read = self.current(project, mode);
                (Some(read.topology.clone()), read.managed.clone())
            }
            None => (None, Ok(Vec::new())),
        };
        ProjectInputs {
            projects,
            chosen,
            topology,
            managed,
            catalog,
        }
    }

    fn current(&mut self, project: &Path, mode: GatherMode) -> &ProjectRead {
        let keep = mode != GatherMode::Now
            && self.read.as_ref().is_some_and(|read| {
                read.project == project && read.stamps.iter().all(Stamp::unchanged)
            });
        if !keep {
            self.read = None;
        }
        self.read.get_or_insert_with(|| read_project(project))
    }
}

/// The projects of `folders`, in their order, each listed once.
fn project_list(folders: &[PathBuf]) -> Vec<PathBuf> {
    let mut projects = Vec::new();
    for folder in folders {
        for project in find_runtime_projects(folder) {
            if !projects.contains(&project) {
                projects.push(project);
            }
        }
    }
    projects
}

fn read_project(project: &Path) -> ProjectRead {
    // The stamps are taken before the reads, so a change during a read is seen next time.
    let stamps = project_files(project).into_iter().map(Stamp::of).collect();
    ProjectRead {
        project: project.to_path_buf(),
        stamps,
        topology: offline_fleet_topology_json(project),
        managed: load_manifest(&project.join(FLEET_MANIFEST_FILE))
            .map(|manifest| manifest.runtime)
            .map_err(|error| format!("{error:#}")),
    }
}

/// The files that the project inputs come from: `runtime.toml`, `io.toml`, `fleet.toml`, and the
/// ADS and OPC UA client files that `runtime.toml` names.
fn project_files(project: &Path) -> Vec<PathBuf> {
    let runtime_toml = project.join("runtime.toml");
    let mut files = vec![project.join("io.toml"), project.join(FLEET_MANIFEST_FILE)];
    if let Ok(runtime) = RuntimeConfig::load(&runtime_toml) {
        files.push(project.join(&runtime.ads.config_path));
        files.push(project.join(&runtime.opcua_client.config_path));
    }
    files.push(runtime_toml);
    files
}

#[cfg(test)]
mod tests;
