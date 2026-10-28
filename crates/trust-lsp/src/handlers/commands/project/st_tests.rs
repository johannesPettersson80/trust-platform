//! The Testing view's tests (specification 25 §11.1; specification 22, "Test POU discovery").
//!
//! The tests of a file are its `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK` declarations as
//! `trust_runtime::test_discovery` finds them for every tool: namespace-qualified names and
//! one-based declaration lines. A file's text is its editor's when the editor holds unsaved changes
//! or when the request names the editor (its code lenses), else the file's on disk. Without named
//! files, every `*.st`, `*.ST`, `*.pou` and `*.POU` below the workspace folders counts, outside
//! `node_modules`, `target` and `.git` folders.
//!
//! A test belongs to the project `trust-dev test` runs it in: the nearest folder at or above its
//! file that holds `src` or `sources`, else its workspace folder; a file in neither has no tests to
//! run. A run's result belongs to the test with the same file, line, kind and name (the name in any
//! letter case); this module gives both sides the keys the extension compares.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::Url;
use trust_runtime::test_discovery::{discover_tests, DiscoveredTest};

use super::decode;
use super::project_root::containing_folder;
use crate::state::ServerState;

/// The folders whose files are no project sources.
const SKIPPED_FOLDERS: [&str; 3] = ["node_modules", "target", ".git"];
/// The kinds a test declaration has.
const TEST_KINDS: [&str; 2] = ["TEST_PROGRAM", "TEST_FUNCTION_BLOCK"];

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
enum Args {
    /// The tests of `files`, or of every Structured Text file below `folders`.
    Discover {
        #[serde(default)]
        files: Option<Vec<FileArg>>,
        #[serde(default)]
        folders: Vec<PathBuf>,
        /// The files whose editors hold unsaved changes.
        #[serde(default)]
        unsaved: Vec<PathBuf>,
    },
    /// The project a test run uses.
    Project {
        #[serde(default, rename = "projectPath")]
        project_path: Option<PathBuf>,
        #[serde(default)]
        path: Option<PathBuf>,
        #[serde(default, rename = "activePath")]
        active_path: Option<PathBuf>,
        #[serde(default)]
        folders: Vec<PathBuf>,
    },
    /// The test a run command names by its file, line, kind and name.
    Identify {
        path: PathBuf,
        line: usize,
        kind: String,
        name: String,
        #[serde(rename = "projectRoot")]
        project_root: PathBuf,
    },
    /// The keys of a `trust-dev test --output json` run's results.
    Results {
        #[serde(rename = "projectRoot")]
        project_root: PathBuf,
        #[serde(default)]
        tests: Vec<RuntimeCase>,
    },
}

/// A file the extension names: its path and, when its editor's text counts (the editor's code
/// lenses), its URI.
#[derive(Deserialize)]
struct FileArg {
    path: PathBuf,
    #[serde(default)]
    uri: Option<String>,
}

/// One result of `trust-dev test --output json`.
#[derive(Deserialize)]
struct RuntimeCase {
    #[serde(default)]
    name: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    file: String,
    #[serde(default)]
    line: i64,
    #[serde(default)]
    message: Option<String>,
}

/// `discover`: `{ ok, tests }`. `project`: `{ ok, root, projectKey }`. `identify`: `{ ok, test }`.
/// `results`: `{ ok, projectKey, cases }`.
pub(super) fn answer(state: &ServerState, argument: Value) -> Result<Value, String> {
    match decode::<Args>(argument)? {
        Args::Discover {
            files,
            folders,
            unsaved,
        } => {
            let files = match files {
                Some(files) => files,
                None => structured_text_files(&folders)
                    .into_iter()
                    .map(|path| FileArg { path, uri: None })
                    .collect(),
            };
            let tests: Vec<Value> = files
                .iter()
                .flat_map(|file| file_tests(state, file, &folders, &unsaved))
                .collect();
            Ok(json!({ "ok": true, "tests": tests }))
        }
        Args::Project {
            project_path,
            path,
            active_path,
            folders,
        } => {
            let root = run_project(
                project_path.as_deref(),
                path.as_deref(),
                active_path.as_deref(),
                &folders,
            );
            Ok(json!({
                "ok": true,
                "root": root.as_ref().map(|root| root.display().to_string()),
                "projectKey": root.as_deref().map(path_key),
            }))
        }
        Args::Identify {
            path,
            line,
            kind,
            name,
            project_root,
        } => {
            let kind = kind.to_uppercase();
            let test = TEST_KINDS
                .contains(&kind.as_str())
                .then(|| test_json(&path, line, &kind, &name, &project_root));
            Ok(json!({ "ok": true, "test": test }))
        }
        Args::Results {
            project_root,
            tests,
        } => {
            let cases: Vec<Value> = tests
                .iter()
                .map(|case| case_json(&project_root, case))
                .collect();
            Ok(json!({ "ok": true, "projectKey": path_key(&project_root), "cases": cases }))
        }
    }
}

/// The tests of one file with their project, none when the file cannot be read or belongs to no
/// project.
fn file_tests(
    state: &ServerState,
    file: &FileArg,
    folders: &[PathBuf],
    unsaved: &[PathBuf],
) -> Vec<Value> {
    let Some(text) = source_text(state, file, unsaved) else {
        return Vec::new();
    };
    if !mentions_a_test(&text) {
        return Vec::new();
    }
    let Some(project_root) = file_project(&file.path, folders) else {
        return Vec::new();
    };
    discover_tests([(file.path.as_path(), text.as_str())])
        .iter()
        .map(|test: &DiscoveredTest| {
            test_json(
                &file.path,
                test.line,
                test.kind.label(),
                &test.name,
                &project_root,
            )
        })
        .collect()
}

/// A file's text: its editor's when the request names the editor or the editor holds unsaved
/// changes, else the file's, else an open editor's when the file cannot be read. A saved editor
/// that has not yet taken a change on disk never hides it.
fn source_text(state: &ServerState, file: &FileArg, unsaved: &[PathBuf]) -> Option<String> {
    let editor = |uri: Option<Url>| {
        uri.and_then(|uri| state.get_document(&uri))
            .filter(|document| document.is_open)
            .map(|document| document.content)
    };
    let named = file.uri.as_deref().and_then(|uri| Url::parse(uri).ok());
    if named.is_some() || unsaved.contains(&file.path) {
        let uri = named.or_else(|| Url::from_file_path(&file.path).ok());
        if let Some(text) = editor(uri) {
            return Some(text);
        }
    }
    std::fs::read_to_string(&file.path)
        .ok()
        .or_else(|| editor(Url::from_file_path(&file.path).ok()))
}

/// Whether `text` names a test keyword at all, in any letter case; a file that does not declares
/// no test, so it is not parsed.
fn mentions_a_test(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("test_program") || lower.contains("test_function_block")
}

/// Every `*.st`, `*.ST`, `*.pou` and `*.POU` below `folders`, outside the skipped folders, by path.
fn structured_text_files(folders: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = BTreeSet::new();
    for folder in folders {
        collect_files(folder, &mut files);
    }
    files.into_iter().collect()
}

fn collect_files(folder: &Path, files: &mut BTreeSet<PathBuf>) {
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
                collect_files(&path, files);
            }
        } else if (file_type.is_file() || (file_type.is_symlink() && path.is_file()))
            && is_structured_text(&path)
        {
            files.insert(path);
        }
    }
}

fn is_structured_text(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("st" | "ST" | "pou" | "POU")
    )
}

/// Whether `folder` holds `src` or `sources`.
fn has_source_folder(folder: &Path) -> bool {
    folder.join("src").exists() || folder.join("sources").exists()
}

/// The nearest folder at or above `path` (the path itself when it is a folder) that holds `src` or
/// `sources`.
fn nearest_source_root(path: &Path) -> Option<PathBuf> {
    let start = if path.is_dir() { path } else { path.parent()? };
    start
        .ancestors()
        .find(|folder| has_source_folder(folder))
        .map(Path::to_path_buf)
}

/// The project of a file: its nearest source root, else its workspace folder.
fn file_project(path: &Path, folders: &[PathBuf]) -> Option<PathBuf> {
    nearest_source_root(path).or_else(|| containing_folder(path, folders).map(Path::to_path_buf))
}

/// The project a run command uses: a named project that holds `src` or `sources`; else the project
/// of a named file or folder; else the project of the active editor's file; else the first
/// workspace folder.
fn run_project(
    project_path: Option<&Path>,
    path: Option<&Path>,
    active_path: Option<&Path>,
    folders: &[PathBuf],
) -> Option<PathBuf> {
    if let Some(project) = project_path.filter(|project| has_source_folder(project)) {
        return Some(project.to_path_buf());
    }
    if let Some(project) = path.and_then(|path| file_project(path, folders)) {
        return Some(project);
    }
    if let Some(project) = active_path.and_then(|path| file_project(path, folders)) {
        return Some(project);
    }
    folders.first().cloned()
}

/// One test as the Testing view keeps it.
fn test_json(path: &Path, line: usize, kind: &str, name: &str, project_root: &Path) -> Value {
    let uri = Url::from_file_path(path)
        .map(|uri| uri.to_string())
        .unwrap_or_else(|()| path.display().to_string());
    let file_key = path_key(path);
    json!({
        "id": format!("{uri}::{line}::{kind}::{name}"),
        "key": test_key(&file_key, line, kind, name),
        "nameKey": name_key(kind, name),
        "path": path.display().to_string(),
        "fileKey": file_key,
        "line": line,
        "kind": kind,
        "name": name,
        "projectRoot": project_root.display().to_string(),
        "projectKey": path_key(project_root),
    })
}

/// One run result with the keys of the test it belongs to; `key` is `null` for a result of no
/// test kind or without a line. A relative file is relative to the project.
fn case_json(project_root: &Path, case: &RuntimeCase) -> Value {
    let kind = case.kind.to_uppercase();
    let path = if Path::new(&case.file).is_absolute() {
        PathBuf::from(&case.file)
    } else {
        project_root.join(&case.file)
    };
    let file_key = path_key(&path);
    let key = (TEST_KINDS.contains(&kind.as_str()) && case.line > 0).then(|| {
        test_key(
            &file_key,
            usize::try_from(case.line).unwrap_or_default(),
            &kind,
            &case.name,
        )
    });
    let outcome = ["passed", "failed", "error"]
        .contains(&case.status.as_str())
        .then_some(case.status.as_str());
    json!({
        "key": key,
        "nameKey": name_key(&kind, &case.name),
        "path": path.display().to_string(),
        "fileKey": file_key,
        "line": case.line,
        "kind": kind,
        "name": case.name,
        "status": case.status,
        "message": case.message,
        "outcome": outcome,
    })
}

fn test_key(file_key: &str, line: usize, kind: &str, name: &str) -> String {
    format!("{file_key}::{line}::{kind}::{}", name.to_lowercase())
}

fn name_key(kind: &str, name: &str) -> String {
    format!("{kind}::{}", name.to_lowercase())
}

/// The key of a file or folder: its canonical path when it exists (so a path through a link and
/// the runner's resolved path agree), else its absolute path without `.` and `..`; in lower case on
/// Windows, whose file names ignore case.
fn path_key(path: &Path) -> String {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| lexical_absolute(path));
    let key = resolved.display().to_string();
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    }
}

fn lexical_absolute(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
#[path = "st_tests_tests.rs"]
mod tests;
