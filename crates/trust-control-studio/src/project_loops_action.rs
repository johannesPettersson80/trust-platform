//! The loop files of the workspace folders, decided by the engine (`controlStudio/projectLoops`; specification 35, A8.3, B1.2 and
//! B1.3). Each folder's files are read and checked for the runtime by [`loops::project_loops`], which holds the duplicate-identifier
//! rule. The answer gives every file's decision and the open request of each accepted loop; the decision on the loop the request
//! names; the loop the workspace opens at once or the choice among its loops; and the loops of an equipment. The extension forwards
//! the request and shows the answer: it decides none of them.

use std::fs;
use std::path::Path;

use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::setup::binding_route::controller_belongs_to_equipment;
use trust_control::setup::messages::{
    ConfiguredLoopRequest, EquipmentLoops, FolderLoops, LoopChoice, LoopIdentity, LoopResolution,
    ProjectLoopsRequest, ProjectLoopsResult, ResolutionState,
};

use crate::commands::to_value;
use crate::loops::{self, ProjectLoop};

/// The sentence the workspace answers when no project folder is open.
const NO_PROJECT: &str = "Open a truST project before opening Control Studio.";

/// Answers `controlStudio/projectLoops`: each folder's loop files for the runtime, the decision on the requested loop, the loop the
/// workspace opens (or its choices), and the loops of the requested equipment.
///
/// # Errors
/// The answer cannot be written.
pub fn answer(request: &ProjectLoopsRequest) -> Result<Value, Failure> {
    let folders: Vec<FolderLoops> = request
        .project_roots
        .iter()
        .map(|root| folder_loops(Path::new(root), &request.runtime_id))
        .collect();
    let declared = request
        .loop_identity
        .as_ref()
        .map(|identity| declaring(&folders, identity));
    let (declaring_root, resolution) =
        declared.map_or((None, None), |found| (found.root, Some(found.resolution)));
    let names_no_loop = request.loop_identity.is_none() && request.equipment.is_none();
    let (open, choices) = if names_no_loop {
        workspace_opening(&folders)
    } else {
        (None, Vec::new())
    };
    let equipment = request
        .equipment
        .as_deref()
        .map(|equipment| equipment_loops(&folders, equipment));
    to_value(&ProjectLoopsResult {
        folders,
        declaring_root,
        resolution,
        open,
        choices,
        equipment,
    })
}

/// The decisions on one folder's loop files. A folder with no loop file has one missing resolution, for its root file.
fn folder_loops(root: &Path, runtime_id: &str) -> FolderLoops {
    let resolutions = match loops::project_loops(root, runtime_id) {
        Ok(entries) if entries.is_empty() => vec![missing_file(
            root.join("control-loop.json").display().to_string(),
            NO_LOOP_FILE,
            NextAction::OpenLoopSetup,
        )],
        Ok(entries) => entries.into_iter().map(resolution).collect(),
        Err(failure) => vec![refused_file(
            root.join("control-loops").display().to_string(),
            &failure.message,
        )],
    };
    FolderLoops {
        project_root: root.display().to_string(),
        resolutions,
    }
}

/// The sentence of a project whose folder has no loop file.
const NO_LOOP_FILE: &str = "This project has no loop definition in control-loop.json or control-loops/. Configure a loop to bind its signals.";

/// One loop file's decision: the open request of an accepted loop, or the reason its file is refused.
fn resolution(entry: ProjectLoop) -> LoopResolution {
    let configuration_path = entry.file.display().to_string();
    match entry.loop_definition {
        Ok(loop_definition) => LoopResolution {
            configuration_path,
            state: ResolutionState::Accepted,
            reason: None,
            failure: None,
            request: Some(ConfiguredLoopRequest::of(loop_definition)),
        },
        Err(reason) => refused_file(configuration_path, &reason),
    }
}

/// A refused loop file: its reason, and the failure that carries it to the page with its next action (A11.2).
fn refused_file(configuration_path: String, reason: &str) -> LoopResolution {
    LoopResolution {
        configuration_path,
        state: ResolutionState::Invalid,
        reason: Some(reason.to_owned()),
        failure: Some(Failure::new(
            reason,
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )),
        request: None,
    }
}

/// A loop file that does not exist, or a loop the project does not declare, with the text that says so and its next action.
fn missing_file(configuration_path: String, text: &str, next_action: NextAction) -> LoopResolution {
    LoopResolution {
        configuration_path,
        state: ResolutionState::Missing,
        reason: Some(text.to_owned()),
        failure: Some(Failure::new(text, Cause::Implementation, next_action)),
        request: None,
    }
}

/// The decision on the loop an identity names: the folder that declares it, and its file's decision (A8.3). The accepted file whose loop
/// declares the revision wins, else the first accepted file that declares the id and project; with none, a refused file that declares
/// them is the decision; with none of those, the loop is missing.
struct Declaring {
    root: Option<String>,
    resolution: LoopResolution,
}

fn declaring(folders: &[FolderLoops], identity: &LoopIdentity) -> Declaring {
    let accepted: Vec<(&str, &LoopResolution)> = folders
        .iter()
        .flat_map(|folder| {
            folder
                .resolutions
                .iter()
                .map(move |resolution| (folder.project_root.as_str(), resolution))
        })
        .filter(|(_, resolution)| {
            resolution.request.as_ref().is_some_and(|request| {
                request.loop_definition.id == identity.id
                    && request.loop_definition.project_id == identity.project_id
            })
        })
        .collect();
    let chosen = accepted
        .iter()
        .find(|(_, resolution)| {
            resolution
                .request
                .as_ref()
                .is_some_and(|request| request.loop_definition.revision == identity.revision)
        })
        .or_else(|| accepted.first());
    if let Some((root, resolution)) = chosen {
        return Declaring {
            root: Some((*root).to_owned()),
            resolution: (*resolution).clone(),
        };
    }
    let refused = folders
        .iter()
        .flat_map(|folder| folder.resolutions.iter())
        .find(|resolution| {
            resolution.state == ResolutionState::Invalid
                && declares(&resolution.configuration_path, identity)
        });
    if let Some(resolution) = refused {
        return Declaring {
            root: None,
            resolution: resolution.clone(),
        };
    }
    let id = &identity.id;
    Declaring {
        root: None,
        resolution: missing_file(
            String::new(),
            &format!("This project declares no usable loop {id}. Configure the loop to bind its signals."),
            NextAction::OpenLoopSetup,
        ),
    }
}

/// Whether the file at `path` declares the loop of `identity` by its raw identifier and project: a refused file does, and is the
/// decision on that loop (A8.3). A file that cannot be read as JSON declares nothing.
fn declares(path: &str, identity: &LoopIdentity) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    value.get("id").and_then(Value::as_str) == Some(identity.id.as_str())
        && value.get("projectId").and_then(Value::as_str) == Some(identity.project_id.as_str())
}

/// The loop the workspace opens at once, or the configured loops the engineer chooses among (B1.2). The configured loops are the files
/// that are not missing. With one, it opens; with none, the first folder's first decision opens (a missing file, which the page names);
/// with several, they are the choices. With no folder, the sentence that asks for a project opens.
fn workspace_opening(folders: &[FolderLoops]) -> (Option<LoopChoice>, Vec<LoopChoice>) {
    let found: Vec<LoopChoice> = folders
        .iter()
        .flat_map(|folder| {
            folder.resolutions.iter().map(move |resolution| LoopChoice {
                project_root: folder.project_root.clone(),
                resolution: resolution.clone(),
            })
        })
        .collect();
    let configured: Vec<LoopChoice> = found
        .iter()
        .filter(|choice| choice.resolution.state != ResolutionState::Missing)
        .cloned()
        .collect();
    match configured.len() {
        0 => (
            Some(found.into_iter().next().unwrap_or_else(no_project)),
            Vec::new(),
        ),
        1 => (configured.into_iter().next(), Vec::new()),
        _ => (None, configured),
    }
}

/// The decision of a workspace with no project folder.
fn no_project() -> LoopChoice {
    LoopChoice {
        project_root: String::new(),
        resolution: missing_file(String::new(), NO_PROJECT, NextAction::None),
    }
}

/// The accepted loops whose controller belongs to the equipment, in folder order. With none, the reason is the first refused file's
/// reason, or else the sentence that asks for a configured loop.
fn equipment_loops(folders: &[FolderLoops], equipment: &str) -> EquipmentLoops {
    let requests: Vec<ConfiguredLoopRequest> = folders
        .iter()
        .flat_map(|folder| folder.resolutions.iter())
        .filter_map(|item| item.request.as_ref())
        .filter(|request| {
            controller_belongs_to_equipment(&request.loop_definition.controller.instance, equipment)
        })
        .cloned()
        .collect();
    let reason = requests
        .is_empty()
        .then(|| no_equipment_loop(folders, equipment));
    EquipmentLoops { requests, reason }
}

fn no_equipment_loop(folders: &[FolderLoops], equipment: &str) -> String {
    folders
        .iter()
        .flat_map(|folder| folder.resolutions.iter())
        .find(|item| item.state == ResolutionState::Invalid)
        .and_then(|item| item.reason.clone())
        .unwrap_or_else(|| {
            format!(
                "No configured Control Studio loop matches {equipment} on the selected runtime. Configure its actual controller signals first."
            )
        })
}

#[cfg(test)]
#[path = "project_loops_action_tests.rs"]
mod tests;
