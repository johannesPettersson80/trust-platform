//! The loops a project declares (specification 35, §17.1): `control-loop.json` and the JSON files
//! of `control-loops/`. Signal names are never guessed from Structured Text.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::{LoopDefinition, LoopScope};
use trust_control::messages::LoopEntry;

/// One loop file and what it holds.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectLoop {
    /// The file.
    pub file: PathBuf,
    /// The checked loop, or why the file cannot be opened.
    pub loop_definition: Result<LoopDefinition, String>,
}

/// The project's loop files: `control-loop.json` when present, then `control-loops/*.json` in name
/// order.
///
/// # Errors
/// `control-loops/` exists but cannot be read.
pub fn loop_files(project_root: &Path) -> Result<Vec<PathBuf>, Failure> {
    let mut files = Vec::new();
    let root_file = project_root.join("control-loop.json");
    if root_file.is_file() {
        files.push(root_file);
    }
    let directory = project_root.join("control-loops");
    match fs::read_dir(&directory) {
        Ok(entries) => {
            let mut more: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.is_file()
                        && path
                            .extension()
                            .is_some_and(|extension| extension == "json")
                })
                .collect();
            more.sort();
            files.extend(more);
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(Failure::new(
                &format!("The control-loops directory could not be read: {error}."),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ))
        }
    }
    Ok(files)
}

/// Every loop file of the project, read and checked for the runtime `runtime_id`. A loop
/// identifier declared twice makes both files unusable.
///
/// # Errors
/// As [`loop_files`].
pub fn project_loops(project_root: &Path, runtime_id: &str) -> Result<Vec<ProjectLoop>, Failure> {
    let mut loops: Vec<ProjectLoop> = loop_files(project_root)?
        .into_iter()
        .map(|file| {
            let loop_definition = read_loop(&file, runtime_id);
            ProjectLoop {
                file,
                loop_definition,
            }
        })
        .collect();
    let ids: Vec<Option<String>> = loops
        .iter()
        .map(|entry| {
            entry
                .loop_definition
                .as_ref()
                .ok()
                .map(|loop_definition| loop_definition.id.clone())
        })
        .collect();
    for (entry, id) in loops.iter_mut().zip(&ids) {
        if let Some(id) = id {
            if ids.iter().flatten().filter(|other| *other == id).count() > 1 {
                entry.loop_definition = Err(format!(
                    "Duplicate control loop ID {id} in this project. Give each loop a unique ID."
                ));
            }
        }
    }
    Ok(loops)
}

/// The project's loops as the panel lists them.
///
/// # Errors
/// As [`loop_files`].
pub fn list(project_root: &Path, runtime_id: &str) -> Result<Vec<LoopEntry>, Failure> {
    Ok(project_loops(project_root, runtime_id)?
        .into_iter()
        .map(|entry| {
            let file = entry.file.display().to_string();
            match entry.loop_definition {
                Ok(loop_definition) => LoopEntry {
                    file,
                    loop_id: Some(loop_definition.id),
                    name: Some(loop_definition.name),
                    problem: None,
                },
                Err(problem) => LoopEntry {
                    file,
                    loop_id: None,
                    name: None,
                    problem: Some(problem),
                },
            }
        })
        .collect())
}

/// The checked loop `loop_id` of the project.
///
/// # Errors
/// The project declares no usable loop of that identifier.
pub fn open(
    project_root: &Path,
    runtime_id: &str,
    loop_id: &str,
) -> Result<LoopDefinition, Failure> {
    let loops = project_loops(project_root, runtime_id)?;
    let mut problems = Vec::new();
    for entry in loops {
        match entry.loop_definition {
            Ok(loop_definition) if loop_definition.id == loop_id => return Ok(loop_definition),
            Ok(_) => {}
            Err(problem) => problems.push(problem),
        }
    }
    let detail = if problems.is_empty() {
        String::new()
    } else {
        format!(" {}", problems.join(" "))
    };
    Err(Failure::new(
        &format!("This project declares no usable loop {loop_id}. Configure the loop to bind its signals.{detail}"),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    ))
}

fn read_loop(file: &Path, runtime_id: &str) -> Result<LoopDefinition, String> {
    let name = file.file_name().map_or_else(
        || file.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let text =
        fs::read_to_string(file).map_err(|error| format!("{name} could not be read: {error}."))?;
    let loop_definition =
        LoopDefinition::decode(&text).map_err(|rejection| format!("{name}: {rejection}."))?;
    if loop_definition.runtime_id != runtime_id {
        return Err(format!(
            "{name}: the loop uses {}, but {runtime_id} is the selected target.",
            loop_definition.runtime_id
        ));
    }
    let scope = LoopScope {
        project_id: loop_definition.project_id.clone(),
        runtime_id: runtime_id.to_string(),
    };
    loop_definition
        .validate(&scope)
        .map_err(|rejection| format!("{name}: {rejection}."))?;
    Ok(loop_definition)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str =
        include_str!("../../../../examples/control-pid-commissioning/control-loop.json");

    fn project(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("trust-control-loops-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("control-loops")).expect("project folder");
        root
    }

    fn example_runtime() -> String {
        LoopDefinition::decode(EXAMPLE).expect("example").runtime_id
    }

    #[test]
    fn lists_the_root_loop_then_the_folder_in_name_order_and_opens_one_by_id() {
        let root = project("list");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("root loop");
        fs::write(root.join("control-loops/b-broken.json"), "{ not json").expect("broken loop");
        let loops = list(&root, &example_runtime()).expect("listed");
        assert_eq!(loops.len(), 2);
        assert!(loops[0].problem.is_none(), "{:?}", loops[0].problem);
        assert!(
            loops[1]
                .problem
                .as_deref()
                .is_some_and(|problem| problem.starts_with("b-broken.json:")),
            "{:?}",
            loops[1].problem
        );
        let id = loops[0].loop_id.clone().expect("the example's id");
        assert_eq!(open(&root, &example_runtime(), &id).expect("opened").id, id);
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn a_loop_of_another_runtime_or_a_duplicate_id_cannot_be_opened() {
        let root = project("refused");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("root loop");
        let other = list(&root, "runtime:other").expect("listed");
        assert!(
            other[0]
                .problem
                .as_deref()
                .is_some_and(|problem| problem.contains("is the selected target")),
            "{:?}",
            other[0].problem
        );
        fs::write(root.join("control-loops/copy.json"), EXAMPLE).expect("duplicate");
        let loops = list(&root, &example_runtime()).expect("listed");
        assert!(loops.iter().all(|entry| entry
            .problem
            .as_deref()
            .is_some_and(|problem| problem.starts_with("Duplicate control loop ID"))));
        let id = LoopDefinition::decode(EXAMPLE).expect("example").id;
        let failure = open(&root, &example_runtime(), &id).expect_err("refused");
        assert_eq!(failure.next_action, NextAction::OpenLoopSetup);
        fs::remove_dir_all(&root).expect("clean up");
    }
}
