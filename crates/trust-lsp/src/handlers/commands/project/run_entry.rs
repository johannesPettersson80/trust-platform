//! The simulator's entry (specification 25 §11.6; specification 13 `stReload`): the CONFIGURATION
//! files a debug session can start from and the one it starts from, the default CONFIGURATION when
//! the workspace has none, and whether a CONFIGURATION file can start a session.
//!
//! A CONFIGURATION file is a Structured Text file below the workspace folders, outside
//! `node_modules`, `target` and `.git`, that declares a `CONFIGURATION`. With several, an
//! interactive start lets the user pick; an automatic one takes the one it remembers, else the only
//! one in the workspace folder of the active Structured Text file, else the first by path.
//!
//! The default CONFIGURATION runs one PROGRAM in task `MainTask` every resource cycle: its
//! `RESOURCE` and its task interval are the `[resource]` name and `cycle_interval_ms` of the
//! project's `runtime.toml`, which a build requires the program to match (specification 11
//! §6.8.1d), else `Res` and 10 ms, the simulator's cycle without one (specification 13). It is
//! written to `configuration.st`, else `configuration_2.st` … `configuration_100.st`, the first free
//! name. Its PROGRAM is one the active file declares, else one the simulator's sources declare.
//!
//! Every `PROGRAM <instance> [WITH <task>] : <type>` of the CONFIGURATION must name a PROGRAM that
//! the simulator's sources declare. Those sources are chosen exactly as the debug adapter chooses
//! them (`trust_runtime::runtime_sources::launch_sources`): the runtime source options, the entry
//! and the project's local package dependencies. A type is declared when a PROGRAM's
//! namespace-qualified name is the type, in any letter case, or, for a type without a namespace,
//! when a PROGRAM's own name is. An open editor's text counts instead of its file.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;
use trust_runtime::runtime_sources::{launch_sources, RuntimeSourceOptions};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::decode;
use crate::state::ServerState;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
enum Args {
    /// Whether `program` can start a debug session.
    Check {
        program: PathBuf,
        #[serde(default)]
        options: SourceOptionsArgs,
    },
    /// The CONFIGURATION files of the workspace and the one a session starts from.
    Configurations {
        #[serde(default)]
        folders: Vec<PathBuf>,
        /// The file the extension remembers from the last start.
        #[serde(default)]
        remembered: Option<PathBuf>,
        /// The active (or last) Structured Text editor's file.
        #[serde(default)]
        active: Option<PathBuf>,
        /// The user picks among several; else one is chosen.
        #[serde(default)]
        interactive: bool,
    },
    /// The PROGRAM types a default CONFIGURATION can run.
    Programs {
        #[serde(default)]
        active: Option<PathBuf>,
        #[serde(default)]
        options: SourceOptionsArgs,
        #[serde(default)]
        interactive: bool,
    },
    /// The workspace folder the default CONFIGURATION goes to.
    Folder {
        /// The file that declares the chosen PROGRAM.
        #[serde(default, rename = "programPath")]
        program_path: Option<PathBuf>,
        #[serde(default)]
        active: Option<PathBuf>,
        #[serde(default)]
        folders: Vec<PathBuf>,
        #[serde(default)]
        interactive: bool,
    },
    /// Writes the default CONFIGURATION of `program` into `folder`.
    Create { folder: PathBuf, program: String },
    /// Whether `path` declares a CONFIGURATION.
    Declares { path: PathBuf },
}

/// The folders whose files are no project sources.
const SKIPPED_FOLDERS: [&str; 3] = ["node_modules", "target", ".git"];
/// How many default CONFIGURATION file names are tried.
const CONFIGURATION_NAMES: usize = 100;

/// The runtime source options as the extension sends them to the debug adapter.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceOptionsArgs {
    #[serde(default)]
    runtime_root: Option<PathBuf>,
    #[serde(default)]
    runtime_include_globs: Vec<String>,
    #[serde(default)]
    runtime_exclude_globs: Vec<String>,
    #[serde(default)]
    runtime_ignore_pragmas: Option<Vec<String>>,
}

/// `check`: `{ ok, problem }`, `problem` being `null` or why the entry cannot start.
/// `configurations`: `{ ok, configurations, chosen, note }`, `chosen` being `null` when the user
/// picks or none exists. `programs`: `{ ok, programs, fromActive, chosen, note }`, each program
/// `{ name, path }`. `folder`: `{ ok, folder, pick }`. `create`: `{ ok, path }`. `declares`:
/// `{ ok, configuration }`.
pub(super) fn answer(state: &ServerState, argument: Value) -> Result<Value, String> {
    let read = |path: &Path| read_source(state, path);
    match decode::<Args>(argument)? {
        Args::Check { program, options } => {
            Ok(json!({ "ok": true, "problem": entry_problem(&program, &options, &read) }))
        }
        Args::Configurations {
            folders,
            remembered,
            active,
            interactive,
        } => {
            let configurations = configuration_files(&folders, &read);
            let (chosen, note) = choose_configuration(
                &configurations,
                &folders,
                remembered.as_deref(),
                active.as_deref(),
                interactive,
            );
            Ok(json!({
                "ok": true,
                "configurations": configurations.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
                "chosen": chosen.map(|path| path.display().to_string()),
                "note": note,
            }))
        }
        Args::Programs {
            active,
            options,
            interactive,
        } => program_candidates(active.as_deref(), &options, interactive, &read),
        Args::Folder {
            program_path,
            active,
            folders,
            interactive,
        } => {
            let (folder, pick) = configuration_folder(
                program_path.as_deref(),
                active.as_deref(),
                &folders,
                interactive,
            );
            Ok(json!({
                "ok": true,
                "folder": folder.map(|folder| folder.display().to_string()),
                "pick": pick,
            }))
        }
        Args::Create { folder, program } => {
            let path = create_configuration(&folder, &program)?;
            Ok(json!({ "ok": true, "path": path.display().to_string() }))
        }
        Args::Declares { path } => Ok(json!({
            "ok": true,
            "configuration": read(&path).is_ok_and(|text| declares_configuration(&text)),
        })),
    }
}

/// The workspace folder of the default CONFIGURATION: the folder of the chosen PROGRAM's file; else
/// the only folder; else, when the user picks, none (`pick`); else the active file's folder, else
/// the first. Without folders there is none.
fn configuration_folder(
    program_path: Option<&Path>,
    active: Option<&Path>,
    folders: &[PathBuf],
    interactive: bool,
) -> (Option<PathBuf>, bool) {
    if let Some(folder) =
        program_path.and_then(|path| super::project_root::containing_folder(path, folders))
    {
        return (Some(folder.to_path_buf()), false);
    }
    match folders {
        [] => (None, false),
        [only] => (Some(only.clone()), false),
        _ if interactive => (None, true),
        _ => {
            let folder = active
                .and_then(|active| super::project_root::containing_folder(active, folders))
                .unwrap_or(&folders[0]);
            (Some(folder.to_path_buf()), false)
        }
    }
}

/// The files below `folders` that declare a CONFIGURATION, by path.
fn configuration_files(
    folders: &[PathBuf],
    read: &dyn Fn(&Path) -> std::io::Result<String>,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for folder in folders {
        collect_structured_text(folder, &mut files);
    }
    files.sort();
    files.dedup();
    files
        .into_iter()
        .filter(|file| read(file).is_ok_and(|text| declares_configuration(&text)))
        .collect()
}

fn collect_structured_text(folder: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            let skipped = entry
                .file_name()
                .to_str()
                .is_some_and(|name| SKIPPED_FOLDERS.contains(&name));
            if !skipped {
                collect_structured_text(&path, files);
            }
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("st" | "ST" | "pou" | "POU")
        ) {
            files.push(path);
        }
    }
}

fn declares_configuration(text: &str) -> bool {
    parse(text)
        .syntax()
        .descendants()
        .any(|node| node.kind() == SyntaxKind::Configuration)
}

/// The CONFIGURATION a session starts from, and the note an automatic choice among several leaves.
fn choose_configuration(
    configurations: &[PathBuf],
    folders: &[PathBuf],
    remembered: Option<&Path>,
    active: Option<&Path>,
    interactive: bool,
) -> (Option<PathBuf>, Option<String>) {
    match configurations {
        [] => (None, None),
        [only] => (Some(only.clone()), None),
        _ if interactive => (None, None),
        _ => {
            if let Some(remembered) =
                remembered.filter(|remembered| configurations.iter().any(|path| path == remembered))
            {
                return (Some(remembered.to_path_buf()), None);
            }
            let active_folder =
                active.and_then(|active| super::project_root::containing_folder(active, folders));
            let in_active_folder: Vec<&PathBuf> = active_folder
                .map(|folder| {
                    configurations
                        .iter()
                        .filter(|path| {
                            super::project_root::containing_folder(path, folders) == Some(folder)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let chosen = match in_active_folder.as_slice() {
                [only] => (*only).clone(),
                _ => configurations[0].clone(),
            };
            let note = format!(
                "Multiple CONFIGURATION files found; using {}.",
                chosen.display()
            );
            (Some(chosen), Some(note))
        }
    }
}

/// The PROGRAM types a default CONFIGURATION can run: the active file's, else the simulator's
/// sources' (each name once).
fn program_candidates(
    active: Option<&Path>,
    options: &SourceOptionsArgs,
    interactive: bool,
    read: &dyn Fn(&Path) -> std::io::Result<String>,
) -> Result<Value, String> {
    let program_json = |program: &DeclaredProgram, path: &Path| json!({ "name": program.qualified, "path": path.display().to_string() });
    if let Some(active) = active
        && let Ok(text) = read(active)
    {
        let programs = declared_programs(&text);
        if !programs.is_empty() {
            let chosen =
                (!interactive || programs.len() == 1).then(|| program_json(&programs[0], active));
            return Ok(json!({
                "ok": true,
                "programs": programs.iter().map(|program| program_json(program, active)).collect::<Vec<_>>(),
                "fromActive": true,
                "chosen": chosen,
                "note": Value::Null,
            }));
        }
    }
    let options = RuntimeSourceOptions {
        root: options.runtime_root.clone(),
        include_globs: options.runtime_include_globs.clone(),
        exclude_globs: options.runtime_exclude_globs.clone(),
        ignore_pragmas: options.runtime_ignore_pragmas.clone(),
    };
    let sources = trust_runtime::runtime_sources::project_sources(&options, read)
        .map_err(|error| format!("The simulator's sources cannot be read: {error:#}"))?;
    let mut programs: Vec<(DeclaredProgram, PathBuf)> = Vec::new();
    for (path, text) in &sources {
        for program in declared_programs(text) {
            if !programs
                .iter()
                .any(|(known, _)| known.qualified == program.qualified)
            {
                programs.push((program, PathBuf::from(path)));
            }
        }
    }
    if programs.is_empty() {
        return Err("No PROGRAM declarations found to create a configuration.".to_owned());
    }
    let listed: Vec<Value> = programs
        .iter()
        .map(|(program, path)| program_json(program, path))
        .collect();
    let (chosen, note) = if interactive {
        (Value::Null, Value::Null)
    } else {
        let (program, path) = programs
            .iter()
            .min_by(|(left, _), (right, _)| left.qualified.cmp(&right.qualified))
            .expect("a program");
        let note = (programs.len() > 1)
            .then(|| format!("Multiple PROGRAM types found; using {}.", program.qualified));
        (program_json(program, path), json!(note))
    };
    Ok(json!({
        "ok": true,
        "programs": listed,
        "fromActive": false,
        "chosen": chosen,
        "note": note,
    }))
}

/// Writes the default CONFIGURATION of `program` to the first free name in `folder`.
fn create_configuration(folder: &Path, program: &str) -> Result<PathBuf, String> {
    let program = program.trim();
    let valid = !program.is_empty()
        && program.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
                && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        });
    if !valid {
        return Err(format!("'{program}' is not a PROGRAM name."));
    }
    let (resource, cycle_ms) = project_resource(folder);
    let text = format!(
        "CONFIGURATION Conf\n  RESOURCE {resource} ON PLC\n    TASK MainTask (INTERVAL := T#{cycle_ms}ms, PRIORITY := 1);\n    PROGRAM P1 WITH MainTask : {program};\n  END_RESOURCE\nEND_CONFIGURATION\n"
    );
    for index in 0..CONFIGURATION_NAMES {
        let name = if index == 0 {
            "configuration.st".to_owned()
        } else {
            format!("configuration_{}.st", index + 1)
        };
        let path = folder.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                use std::io::Write as _;
                file.write_all(text.as_bytes())
                    .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("Could not create {}: {error}", path.display())),
        }
    }
    Err(format!(
        "No free configuration file name in {}.",
        folder.display()
    ))
}

/// The `[resource]` name and cycle in milliseconds of the project's runtime.toml, else `Res` and
/// 10 ms.
fn project_resource(folder: &Path) -> (String, i64) {
    super::project_root::runtime_project_folder(folder)
        .and_then(|project| {
            trust_runtime::config::RuntimeConfig::load(project.join("runtime.toml")).ok()
        })
        .map_or_else(
            || ("Res".to_owned(), 10),
            |config| {
                let cycle = config.cycle_interval.as_millis();
                (
                    config.resource_name.to_string(),
                    if cycle > 0 { cycle } else { 10 },
                )
            },
        )
}

/// The text of `path`: an open editor's, else the file's.
fn read_source(state: &ServerState, path: &Path) -> std::io::Result<String> {
    if let Some(document) = Url::from_file_path(path)
        .ok()
        .and_then(|uri| state.get_document(&uri))
        .filter(|document| document.is_open)
    {
        return Ok(document.content);
    }
    std::fs::read_to_string(path)
}

fn entry_problem(
    program: &Path,
    options: &SourceOptionsArgs,
    read: &dyn Fn(&Path) -> std::io::Result<String>,
) -> Option<String> {
    let Ok(text) = read(program) else {
        return Some("Failed to read CONFIGURATION file.".to_owned());
    };
    let configured = configured_program_types(&text);
    if configured.is_empty() {
        return Some("CONFIGURATION has no PROGRAM entries. Add a PROGRAM binding.".to_owned());
    }
    let options = RuntimeSourceOptions {
        root: options.runtime_root.clone(),
        include_globs: options.runtime_include_globs.clone(),
        exclude_globs: options.runtime_exclude_globs.clone(),
        ignore_pragmas: options.runtime_ignore_pragmas.clone(),
    };
    let sources = match launch_sources(program, &options, read) {
        Ok(sources) => sources,
        Err(error) => return Some(format!("The simulator's sources cannot be read: {error:#}")),
    };
    let declared: Vec<DeclaredProgram> = sources
        .iter()
        .flat_map(|(_, text)| declared_programs(text))
        .collect();
    let missing: Vec<&str> = configured
        .iter()
        .map(String::as_str)
        .filter(|configured| !declared.iter().any(|program| program.matches(configured)))
        .collect();
    (!missing.is_empty()).then(|| {
        format!(
            "Unknown PROGRAM type(s): {}. Check that the file defining them is in the workspace and included in runtime sources.",
            missing.join(", ")
        )
    })
}

/// A PROGRAM that a source declares.
struct DeclaredProgram {
    /// Its name with the namespaces around it, `Ns.Inner.Main`.
    qualified: String,
    /// Its own name, `Main`.
    name: String,
}

impl DeclaredProgram {
    fn matches(&self, configured: &str) -> bool {
        self.qualified.eq_ignore_ascii_case(configured)
            || (!configured.contains('.') && self.name.eq_ignore_ascii_case(configured))
    }
}

/// The types of the `PROGRAM … : <type>` entries of the CONFIGURATIONs in `text`, in order.
fn configured_program_types(text: &str) -> Vec<String> {
    parse(text)
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::ProgramConfig)
        .filter_map(|node| program_config_type(&node))
        .collect()
}

/// The type of one `PROGRAM <instance> [WITH <task>] : <type>` entry: the node after its colon.
fn program_config_type(node: &SyntaxNode) -> Option<String> {
    let mut after_colon = false;
    for element in node.children_with_tokens() {
        if element.kind() == SyntaxKind::Colon {
            after_colon = true;
        } else if let (true, Some(child)) = (after_colon, element.as_node()) {
            let text = significant_text(child);
            return (!text.is_empty()).then_some(text);
        }
    }
    None
}

/// The PROGRAM declarations of `text` (not `TEST_PROGRAM`).
fn declared_programs(text: &str) -> Vec<DeclaredProgram> {
    parse(text)
        .syntax()
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::Program && starts_with_program_keyword(node))
        .filter_map(|node| {
            let name = node
                .children()
                .find(|child| child.kind() == SyntaxKind::Name)
                .map(|name| significant_text(&name))
                .filter(|name| !name.is_empty())?;
            let mut parts: Vec<String> = node
                .ancestors()
                .filter(|ancestor| ancestor.kind() == SyntaxKind::Namespace)
                .filter_map(|namespace| {
                    namespace
                        .children()
                        .find(|child| {
                            matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName)
                        })
                        .map(|name| significant_text(&name))
                })
                .collect();
            parts.reverse();
            parts.push(name.clone());
            Some(DeclaredProgram {
                qualified: parts.join("."),
                name,
            })
        })
        .collect()
}

fn starts_with_program_keyword(node: &SyntaxNode) -> bool {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .is_some_and(|token| token.kind() == SyntaxKind::KwProgram)
}

/// The text of `node` without comments and blanks: `Ns . Main` is `Ns.Main`.
fn significant_text(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.text().to_string())
        .collect()
}

#[cfg(test)]
#[path = "run_entry_tests.rs"]
mod tests;
