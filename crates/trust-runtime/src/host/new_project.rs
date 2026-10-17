//! The files that `trust-runtime new` writes for a new project (specification 25, Create Project,
//! and specification 11 section 6.8.0a).
//!
//! The texts are the ones the VS Code extension wrote before the CLI owned the project template.
//! Two lines differ: `runtime.toml` names no control endpoint, so the platform default applies, and
//! `.gitignore` keeps the endpoint file out of version control, because that file can hold a token.
//! An existing `.gitignore` is kept and only gains that line.

use std::fs;
use std::path::{Path, PathBuf};

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
/// watchdog and fault are not optional. The control section names no endpoint, so every platform
/// uses its default (specification 11, section 6.8.0a).
const RUNTIME_TOML_SOURCE: &str = r#"[bundle]
version = 1

[resource]
name = "Simulator"
cycle_interval_ms = 10

[runtime.control]
mode = "production"
debug_enabled = false

[runtime.web]
enabled = false
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
mode = "none"
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

/// Creates the project `name` in the folder `parent` and returns the absolute project folder.
///
/// The name is trimmed first. An existing project folder that is not empty is refused unless
/// `force` is set. With `force`, the scaffold files are overwritten and every other file is kept.
/// An existing `.gitignore` is kept too and gains the `/.trust-runtime/` line when it lacks it.
///
/// # Errors
///
/// Returns the error of the first check that refuses the request, or of the first I/O step that
/// fails. Nothing is written when a check refuses the request.
pub fn create_project(parent: &Path, name: &str, force: bool) -> Result<PathBuf, NewProjectError> {
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
    for (relative, text) in SCAFFOLD_FILES {
        let path = target.join(relative);
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)
                .map_err(|error| io_error(format!("create {}: {error}", folder.display())))?;
        }
        fs::write(&path, text)
            .map_err(|error| io_error(format!("write {}: {error}", path.display())))?;
    }
    let gitignore = target.join(".gitignore");
    crate::control::endpoint::ensure_endpoint_dir_ignored(&target, GITIGNORE_SOURCE)
        .map_err(|error| io_error(format!("write {}: {error}", gitignore.display())))?;
    Ok(target)
}

/// The trimmed name, when it is one plain folder name on every platform, so that the project
/// folder is always inside the parent folder (specification 25, Create Project).
fn checked_name(name: &str) -> Result<&str, NewProjectError> {
    let trimmed = name.trim();
    let problem = if trimmed.is_empty() {
        Some("the project name is required")
    } else if trimmed.contains('/') || trimmed.contains('\\') {
        Some("the project name must not contain a path separator")
    } else if trimmed == "." || trimmed == ".." {
        Some("the project name is invalid")
    } else {
        windows_name_problem(trimmed)
    };
    match problem {
        None => Ok(trimmed),
        Some(problem) => Err(NewProjectError {
            code: NewProjectErrorCode::Name,
            message: problem.to_string(),
        }),
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
mod tests {
    use super::*;
    use crate::config::{IoConfig, RuntimeConfig};

    /// `src/Main.st` as the extension wrote it at HEAD (`editors/vscode/src/newProject.ts`).
    const EXTENSION_MAIN_ST: &str = "PROGRAM Main\nEND_PROGRAM\n";

    /// `src/config.st` as the extension wrote it at HEAD.
    const EXTENSION_CONFIG_ST: &str = r#"CONFIGURATION Config
RESOURCE MainRes ON PLC
    TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM MainInstance WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#;

    /// `trust-lsp.toml` as the extension wrote it at HEAD.
    const EXTENSION_PROJECT_TOML: &str = "include_paths = [\"src\"]\n";

    /// `runtime.toml` as the extension wrote it at HEAD, without its `endpoint` line and without
    /// the empty `[runtime.mesh] auth_token = ""`, which the runtime refuses ("must not be empty"):
    /// the extension's project never validated.
    const EXTENSION_RUNTIME_TOML: &str = r#"[bundle]
version = 1

[resource]
name = "Simulator"
cycle_interval_ms = 10

[runtime.control]
mode = "production"
debug_enabled = false

[runtime.web]
enabled = false
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
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = "halt"

[runtime.fault]
policy = "halt"
"#;

    /// `io.toml` as the extension wrote it at HEAD.
    const EXTENSION_IO_TOML: &str = r#"# Simulated I/O so the project runs with no hardware or brokers, on any machine. Devices & Connections
# ("Set up runtime…" → add a device) writes real drivers here when you wire one up.
[io]
driver = "simulated"
params = {}
"#;

    /// `.vscode/launch.json` as the extension wrote it at HEAD.
    const EXTENSION_LAUNCH_JSON: &str = r#"{
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

    /// `.vscode/settings.json` as the extension wrote it at HEAD.
    const EXTENSION_VSCODE_SETTINGS: &str = r#"{
  "debug.showInStatusBar": "never"
}
"#;

    /// `.gitignore`, which the extension did not write.
    const EXPECTED_GITIGNORE: &str =
        "# The runtime's endpoint file: it can hold a control token.\n/.trust-runtime/\n";

    /// A unique folder under the system temporary folder, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default();
            let path = std::env::temp_dir().join(format!(
                "trust-new-project-{label}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create scratch folder");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn each_scaffold_file_has_the_text_the_extension_wrote() {
        let scratch = Scratch::new("texts");
        let project = create_project(scratch.path(), "my-plc", false).expect("create project");
        let expected = [
            ("src/Main.st", EXTENSION_MAIN_ST),
            ("src/config.st", EXTENSION_CONFIG_ST),
            ("trust-lsp.toml", EXTENSION_PROJECT_TOML),
            ("runtime.toml", EXTENSION_RUNTIME_TOML),
            ("io.toml", EXTENSION_IO_TOML),
            (".vscode/launch.json", EXTENSION_LAUNCH_JSON),
            (".vscode/settings.json", EXTENSION_VSCODE_SETTINGS),
            (".gitignore", EXPECTED_GITIGNORE),
        ];
        for (relative, text) in expected {
            assert_eq!(
                fs::read_to_string(project.join(relative)).expect("read scaffold file"),
                text,
                "{relative}"
            );
        }
    }

    #[test]
    fn runtime_toml_names_no_control_endpoint() {
        let scratch = Scratch::new("endpoint");
        let project = create_project(scratch.path(), "my-plc", false).expect("create project");
        let text = fs::read_to_string(project.join("runtime.toml")).expect("read runtime.toml");
        assert!(text.contains("[runtime.control]"));
        assert!(!text.contains("endpoint"));
    }

    #[test]
    fn the_project_files_parse_with_the_runtime_config_parser() {
        let scratch = Scratch::new("parse");
        let project = create_project(scratch.path(), "my-plc", false).expect("create project");
        let runtime =
            RuntimeConfig::load(project.join("runtime.toml")).expect("runtime.toml parses");
        assert!(runtime.control_endpoint.is_none());
        IoConfig::load(project.join("io.toml")).expect("io.toml parses");
    }

    #[test]
    fn the_project_st_compiles() {
        let scratch = Scratch::new("compile");
        let project = create_project(scratch.path(), "my-plc", false).expect("create project");
        crate::bundle_builder::check_program_stbc(&project, None)
            .expect("the new project's ST compiles");
    }

    #[test]
    fn each_refused_name_has_the_name_code() {
        let scratch = Scratch::new("names");
        for name in ["", "   ", "a/b", "a\\b", ".", ".."] {
            let error = create_project(scratch.path(), name, false).expect_err("name refused");
            assert_eq!(error.code, NewProjectErrorCode::Name, "name {name:?}");
        }
    }

    #[test]
    fn a_name_that_windows_would_not_take_as_one_folder_is_refused_on_every_platform() {
        let scratch = Scratch::new("windows-names");
        for name in [
            "C:x",
            "a:b",
            "CON",
            "nul",
            "Com1.txt",
            "lpt9",
            "AUX.log",
            "name.",
            "a*b",
            "a?b",
            "a<b",
            "a>b",
            "a|b",
            "a\"b",
            "tab\tname",
        ] {
            let error = create_project(scratch.path(), name, false).expect_err("name refused");
            assert_eq!(error.code, NewProjectErrorCode::Name, "name {name:?}");
        }
        assert!(
            fs::read_dir(scratch.path())
                .expect("read scratch folder")
                .next()
                .is_none(),
            "a refused name writes nothing"
        );
    }

    #[test]
    fn names_that_only_resemble_reserved_device_names_are_accepted() {
        let scratch = Scratch::new("near-reserved");
        for name in ["CONSOLE", "com10", "nul-device", "my.plc"] {
            let project = create_project(scratch.path(), name, false).expect("name accepted");
            assert_eq!(project, scratch.path().join(name));
        }
    }

    #[test]
    fn force_keeps_an_existing_gitignore_and_adds_the_endpoint_folder() {
        let scratch = Scratch::new("force-gitignore");
        let folder = scratch.path().join("my-plc");
        fs::create_dir_all(&folder).expect("create folder");
        fs::write(folder.join(".gitignore"), "target/\n*.log\n").expect("write .gitignore");

        create_project(scratch.path(), "my-plc", true).expect("force create");

        let text = fs::read_to_string(folder.join(".gitignore")).expect("read .gitignore");
        assert!(text.starts_with("target/\n*.log\n"), "{text}");
        assert!(
            text.lines()
                .any(|line| line == crate::control::endpoint::ENDPOINT_DIR_GITIGNORE_LINE),
            "{text}"
        );

        create_project(scratch.path(), "my-plc", true).expect("force create again");

        assert_eq!(
            fs::read_to_string(folder.join(".gitignore")).expect("read .gitignore"),
            text,
            "a .gitignore that lists the folder is left as it is"
        );
    }

    #[test]
    fn the_name_is_trimmed_before_it_names_the_folder() {
        let scratch = Scratch::new("trim");
        let project = create_project(scratch.path(), "  my-plc \n", false).expect("create project");
        assert!(project.is_absolute());
        assert_eq!(project, scratch.path().join("my-plc"));
    }

    #[test]
    fn a_non_empty_folder_is_refused_without_force_and_left_as_it_was() {
        let scratch = Scratch::new("exists");
        let folder = scratch.path().join("my-plc");
        fs::create_dir_all(&folder).expect("create folder");
        fs::write(folder.join("notes.txt"), "keep\n").expect("write note");

        let error = create_project(scratch.path(), "my-plc", false).expect_err("refused");

        assert_eq!(error.code, NewProjectErrorCode::Exists);
        assert!(!folder.join("runtime.toml").exists());
        assert_eq!(
            fs::read_to_string(folder.join("notes.txt")).expect("read note"),
            "keep\n"
        );
    }

    #[test]
    fn an_empty_existing_folder_takes_the_project() {
        let scratch = Scratch::new("empty");
        fs::create_dir_all(scratch.path().join("my-plc")).expect("create folder");
        let project = create_project(scratch.path(), "my-plc", false).expect("create project");
        assert!(project.join("runtime.toml").is_file());
    }

    #[test]
    fn force_overwrites_the_scaffold_and_keeps_every_other_file() {
        let scratch = Scratch::new("force");
        let project = create_project(scratch.path(), "my-plc", false).expect("create project");
        fs::write(project.join("src/Main.st"), "PROGRAM Edited\nEND_PROGRAM\n").expect("edit");
        fs::write(project.join("notes.txt"), "keep\n").expect("write note");

        create_project(scratch.path(), "my-plc", true).expect("force create");

        assert_eq!(
            fs::read_to_string(project.join("src/Main.st")).expect("read Main.st"),
            EXTENSION_MAIN_ST
        );
        assert_eq!(
            fs::read_to_string(project.join("notes.txt")).expect("read note"),
            "keep\n"
        );
    }

    #[test]
    fn a_file_where_the_project_folder_goes_is_not_a_folder() {
        let scratch = Scratch::new("file-target");
        fs::write(scratch.path().join("my-plc"), "x").expect("write file");
        let error = create_project(scratch.path(), "my-plc", true).expect_err("refused");
        assert_eq!(error.code, NewProjectErrorCode::NotDirectory);
    }

    #[test]
    fn a_scaffold_folder_that_is_a_file_is_an_io_error() {
        let scratch = Scratch::new("io");
        let folder = scratch.path().join("my-plc");
        fs::create_dir_all(&folder).expect("create folder");
        fs::write(folder.join("src"), "not a folder\n").expect("write file");
        let error = create_project(scratch.path(), "my-plc", true).expect_err("refused");
        assert_eq!(error.code, NewProjectErrorCode::Io);
    }

    #[test]
    fn a_file_as_the_parent_is_not_a_folder() {
        let scratch = Scratch::new("file-parent");
        let parent = scratch.path().join("parent.txt");
        fs::write(&parent, "x").expect("write file");
        let error = create_project(&parent, "my-plc", false).expect_err("refused");
        assert_eq!(error.code, NewProjectErrorCode::NotDirectory);
    }
}
