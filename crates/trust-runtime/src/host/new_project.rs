//! The files that `trust-runtime new` writes for a new project (specification 25, Create Project,
//! and specification 11 section 6.8.0a), for each of its starts (specification 48 RU-51): a
//! Structured Text project here, and the Rust starts in `new_project_rust.rs`.
//!
//! The texts are the ones the VS Code extension wrote before the CLI owned the project template.
//! Two lines differ: `runtime.toml` names no control endpoint, so the platform default applies, and
//! `.gitignore` keeps the endpoint file out of version control, because that file can hold a token.
//! An existing `.gitignore` is kept and only gains that line.

use std::fs;
use std::path::{Path, PathBuf};

#[path = "new_project_rust.rs"]
mod rust_start;

/// `src/Main.st`: the empty program that a new project opens on.
const MAIN_ST_SOURCE: &str = r#"PROGRAM Main
END_PROGRAM
"#;

/// `src/config.st`: instantiates the program, so the project runs and no "unused program" warning
/// appears on first open. The instance name differs from the POU name. INTERVAL matches runtime.toml.
const CONFIG_ST_SOURCE: &str = r#"CONFIGURATION Config
RESOURCE MainRes ON PLC
    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#;

/// `trust-lsp.toml`: the source root.
const PROJECT_TOML_SOURCE: &str = r#"include_paths = ["src"]
"#;

/// `runtime.toml`: the full section set that the runtime config parser requires, since retain,
/// watchdog and fault are not optional. The resource is the one `src/config.st` declares, which a
/// build requires (specification 11 §6.8.1d). The web server listens on this computer only, so a
/// deployed HMI opens in a browser at once, and RETAIN values are kept in `retain.bin`. The
/// control section names no endpoint, so every platform uses its default (specification 11,
/// section 6.8.0a).
const RUNTIME_TOML_SOURCE: &str = r#"[bundle]
version = 1

[resource]
name = "MainRes"
cycle_interval_ms = 10

[runtime.control]
mode = "production"
debug_enabled = false

[runtime.web]
enabled = true
listen = "127.0.0.1:8080"
auth = "local"
tls = false

[runtime.tls]
mode = "disabled"
require_remote = false

[runtime.discovery]
enabled = false
service_name = "truST"
advertise = false
interfaces = []

[runtime.mesh]
enabled = false
listen = "0.0.0.0:5200"
tls = false
publish = []

[runtime.observability]
enabled = false
sample_interval_ms = 1000
mode = "all"
include = []
history_path = "history/historian.jsonl"
max_entries = 20000
prometheus_enabled = true
prometheus_path = "/metrics"

[runtime.log]
level = "info"

[runtime.retain]
mode = "file"
path = "retain.bin"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = "halt"

[runtime.fault]
policy = "halt"
"#;

/// `io.toml`: simulated I/O, so the project runs with no hardware or brokers.
const IO_TOML_SOURCE: &str = r#"# Simulated I/O so the project runs with no hardware or brokers, on any machine. Devices & Connections
# ("Set up runtime…" → add a device) writes real drivers here when you wire one up.
[io]
driver = "simulated"
params = {}
"#;

/// `.vscode/launch.json`: the simulator run target that the truST sidebar offers.
const LAUNCH_JSON_SOURCE: &str = r#"{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "structured-text",
      "request": "launch",
      "name": "truST Simulator",
      "program": "${workspaceFolder}/src/config.st",
      "stopOnEntry": false
    }
  ]
}
"#;

/// `.vscode/settings.json`: the truST sidebar, not the status bar, is the run-target surface.
const VSCODE_SETTINGS_SOURCE: &str = r#"{
  "debug.showInStatusBar": "never"
}
"#;

/// `.gitignore`: the endpoint file can hold a control token, so it stays out of version control.
/// An existing `.gitignore` is never replaced; it only gains the `/.trust-runtime/` line.
const GITIGNORE_SOURCE: &str = crate::control::endpoint::ENDPOINT_DIR_GITIGNORE;

/// Every file of a new project except `.gitignore`, relative to the project folder, with its text.
/// With `force`, these files are overwritten and no other file is touched.
const SCAFFOLD_FILES: [(&str, &str); 7] = [
    ("src/Main.st", MAIN_ST_SOURCE),
    ("src/config.st", CONFIG_ST_SOURCE),
    ("trust-lsp.toml", PROJECT_TOML_SOURCE),
    ("runtime.toml", RUNTIME_TOML_SOURCE),
    ("io.toml", IO_TOML_SOURCE),
    (".vscode/launch.json", LAUNCH_JSON_SOURCE),
    (".vscode/settings.json", VSCODE_SETTINGS_SOURCE),
];

/// Names that Windows reserves for devices, with or without an extension (`CON`, `nul.txt`).
const WINDOWS_RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Characters that a Windows file name cannot hold, besides the path separators.
const WINDOWS_FORBIDDEN_CHARACTERS: [char; 7] = [':', '*', '?', '"', '<', '>', '|'];

/// The code that names why `trust-runtime new` refused or failed. Its text is the JSON `code`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewProjectErrorCode {
    /// The name is not one plain folder name on every platform: it is empty, `.` or `..`,
    /// contains a path separator, a drive prefix or another character that Windows refuses, ends
    /// with a dot, or is a name that Windows reserves for a device.
    Name,
    /// The project folder exists, is not empty, and `--force` is not set.
    Exists,
    /// The project folder, or the parent folder, exists and is not a folder.
    NotDirectory,
    /// A folder or file could not be read or written.
    Io,
}

impl NewProjectErrorCode {
    /// The code as the JSON answer spells it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Exists => "exists",
            Self::NotDirectory => "not-directory",
            Self::Io => "io",
        }
    }
}

/// Why a project was not created: the code, and a message for a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProjectError {
    /// The code that names the failed check.
    pub code: NewProjectErrorCode,
    /// What went wrong, in words that say what to do.
    pub message: String,
}

impl std::fmt::Display for NewProjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for NewProjectError {}

/// What a new project starts as (specification 25 §3.2, specification 48 RU-51).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProjectStart {
    /// A Structured Text project.
    #[default]
    StructuredText,
    /// A Structured Text project with a Rust crate in `rust/`, whose function block `src/Main.st`
    /// calls (`trust-runtime new --rust`).
    StructuredTextWithRust,
    /// A Rust-first project: the crate is the project, and `trust.toml` its configuration
    /// (specification 48 §14, `trust-runtime new --rust-first`).
    RustFirst,
}

impl ProjectStart {
    /// The file a new project of this start opens on, relative to the project folder with `/`:
    /// its program, or its crate's source in a Rust start.
    #[must_use]
    pub fn focus(self) -> &'static str {
        match self {
            Self::StructuredText => "src/Main.st",
            Self::StructuredTextWithRust => "rust/src/lib.rs",
            Self::RustFirst => "src/lib.rs",
        }
    }
}

/// Creates the Structured Text project `name` in the folder `parent` and returns the absolute
/// project folder, as [`create_project_as`] does for [`ProjectStart::StructuredText`].
///
/// # Errors
///
/// Those of [`create_project_as`].
pub fn create_project(parent: &Path, name: &str, force: bool) -> Result<PathBuf, NewProjectError> {
    create_project_as(parent, name, ProjectStart::StructuredText, force)
}

/// Creates the project `name` of the start `start` in the folder `parent` and returns the absolute
/// project folder.
///
/// The name is trimmed first. An existing project folder that is not empty is refused unless
/// `force` is set. With `force`, the scaffold files are overwritten and every other file is kept.
/// An existing `.gitignore` is kept too and gains each of the start's lines that it lacks.
///
/// # Errors
///
/// Returns the error of the first check that refuses the request, or of the first I/O step that
/// fails. Nothing is written when a check refuses the request.
pub fn create_project_as(
    parent: &Path,
    name: &str,
    start: ProjectStart,
    force: bool,
) -> Result<PathBuf, NewProjectError> {
    let name = checked_name(name)?;
    let parent = std::path::absolute(parent)
        .map_err(|error| io_error(format!("resolve {}: {error}", parent.display())))?;
    if parent.exists() && !parent.is_dir() {
        return Err(not_a_folder(&parent));
    }
    let target = parent.join(name);
    if target.exists() && !target.is_dir() {
        return Err(not_a_folder(&target));
    }
    if target.is_dir() && !force && !is_empty_folder(&target)? {
        return Err(NewProjectError {
            code: NewProjectErrorCode::Exists,
            message: format!(
                "{} already exists and is not empty; use --force to overwrite the project files",
                target.display()
            ),
        });
    }
    match start {
        ProjectStart::StructuredText => write_files(&target, &SCAFFOLD_FILES)?,
        ProjectStart::StructuredTextWithRust | ProjectStart::RustFirst => {
            rust_start::write(&target, name, start)?;
        }
    }
    let gitignore = target.join(".gitignore");
    crate::control::endpoint::ensure_endpoint_dir_ignored(&target, GITIGNORE_SOURCE)
        .map_err(|error| io_error(format!("write {}: {error}", gitignore.display())))?;
    ignore_build_folders(&target, start)?;
    Ok(target)
}

/// Makes the project's `.gitignore` list the folder its builds write, `/build/`, and in a Rust start
/// the folder where the user's own cargo builds the crate, appending the lines it lacks
/// (specification 25 §3.2).
///
/// # Errors
///
/// A `.gitignore` that cannot be read or written.
fn ignore_build_folders(target: &Path, start: ProjectStart) -> Result<(), NewProjectError> {
    let (lines, comment): (&[&str], &str) = match start {
        ProjectStart::StructuredText => (&["/build/"], "# The folder the builds write.\n"),
        ProjectStart::StructuredTextWithRust => (
            &["/build/", "/rust/target/"],
            "# The folder the builds write, and the one where your own cargo builds the crate.\n",
        ),
        ProjectStart::RustFirst => (
            &["/build/", "/target/"],
            "# The folder the builds write, and the one where your own cargo builds the crate.\n",
        ),
    };
    let path = target.join(".gitignore");
    let failed = |error: std::io::Error| io_error(format!("write {}: {error}", path.display()));
    let mut text = fs::read_to_string(&path).map_err(failed)?;
    let missing: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| !text.lines().any(|held| held.trim() == *line))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(comment);
    for line in missing {
        text.push_str(line);
        text.push('\n');
    }
    fs::write(&path, text).map_err(failed)
}

/// Writes each of `files`, relative to `target` with `/`, with its text, making its folders.
fn write_files(target: &Path, files: &[(&str, &str)]) -> Result<(), NewProjectError> {
    for (relative, text) in files {
        let path = target.join(relative);
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)
                .map_err(|error| io_error(format!("create {}: {error}", folder.display())))?;
        }
        fs::write(&path, text)
            .map_err(|error| io_error(format!("write {}: {error}", path.display())))?;
    }
    Ok(())
}

/// The trimmed name, when it is one plain folder name on every platform, so that the project
/// folder is always inside the parent folder (specification 25, Create Project).
fn checked_name(name: &str) -> Result<&str, NewProjectError> {
    match folder_name_problem(name) {
        None => Ok(name.trim()),
        Some(problem) => Err(NewProjectError {
            code: NewProjectErrorCode::Name,
            message: problem.to_string(),
        }),
    }
}

/// Why `name`, trimmed, is not one plain folder name on every platform: empty, `.` or `..`, a path
/// separator, a drive prefix or another character that Windows refuses, a final dot, or a name that
/// Windows reserves for a device. `None` for a good name. A new project and a copied example both
/// take their folder name by this rule (specification 25 §3.1, §3.2).
#[must_use]
pub fn folder_name_problem(name: &str) -> Option<&'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        Some("the project name is required")
    } else if trimmed.contains('/') || trimmed.contains('\\') {
        Some("the project name must not contain a path separator")
    } else if trimmed == "." || trimmed == ".." {
        Some("the project name is invalid")
    } else {
        windows_name_problem(trimmed)
    }
}

/// Why Windows would not take `name` as one folder name inside the parent folder, checked on every
/// platform so that a project name works wherever the project is opened. `C:x` is a path relative
/// to drive C, not a folder name.
fn windows_name_problem(name: &str) -> Option<&'static str> {
    if name
        .chars()
        .any(|ch| WINDOWS_FORBIDDEN_CHARACTERS.contains(&ch) || ch.is_control())
    {
        return Some("the project name must not contain a drive prefix or any of : * ? \" < > |");
    }
    // The name is trimmed, so only a dot can end it; Windows drops a final dot from a folder name.
    if name.ends_with('.') {
        return Some("the project name must not end with a dot");
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    if WINDOWS_RESERVED_NAMES
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(stem))
    {
        return Some("the project name is a name that Windows reserves for a device");
    }
    None
}

fn is_empty_folder(folder: &Path) -> Result<bool, NewProjectError> {
    let mut entries = fs::read_dir(folder)
        .map_err(|error| io_error(format!("read {}: {error}", folder.display())))?;
    Ok(entries.next().is_none())
}

fn not_a_folder(path: &Path) -> NewProjectError {
    NewProjectError {
        code: NewProjectErrorCode::NotDirectory,
        message: format!("{} exists and is not a folder", path.display()),
    }
}

fn io_error(message: String) -> NewProjectError {
    NewProjectError {
        code: NewProjectErrorCode::Io,
        message,
    }
}

#[cfg(test)]
#[path = "new_project_tests.rs"]
mod tests;
