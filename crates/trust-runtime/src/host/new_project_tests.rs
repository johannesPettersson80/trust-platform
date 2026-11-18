//! The files of a new Structured Text project, the name's checks and `--force` (specification 25 §3.2).

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

/// `runtime.toml` as the extension wrote it at HEAD, without its `endpoint` line. It names the
/// configuration's resource `MainRes`, the web server on this computer and RETAIN values in
/// `retain.bin`, and the mesh without a token, which the runtime would refuse when empty.
const EXTENSION_RUNTIME_TOML: &str = r#"[bundle]
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

/// `.gitignore`, which the extension did not write: the endpoint's folder and the builds'.
const EXPECTED_GITIGNORE: &str = "# The runtime's endpoint file: it can hold a control token.\n\
        /.trust-runtime/\n# The folder the builds write.\n/build/\n";

/// A unique folder under the system temporary folder, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let path = trust_test_support::unique_temp_path(&format!("trust-new-project-{label}"));
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
    let runtime = RuntimeConfig::load(project.join("runtime.toml")).expect("runtime.toml parses");
    assert!(runtime.control_endpoint.is_none());
    IoConfig::load(project.join("io.toml")).expect("io.toml parses");
}

#[test]
fn the_project_st_compiles() {
    let scratch = Scratch::new("compile");
    let project = create_project(scratch.path(), "my-plc", false).expect("create project");
    let report =
        crate::bundle_builder::check_program(&project, &[]).expect("the new project's ST compiles");
    // The build runs the resource that runtime.toml names, which the configuration declares,
    // compared without regard to case (specification 11 §6.8.1d). Wrong behaviour caught: a
    // new project whose build refuses its own runtime.toml.
    assert!(
        matches!(report.resources.as_slice(), [name] if name.eq_ignore_ascii_case("MainRes")),
        "{:?}",
        report.resources
    );
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
fn force_keeps_an_existing_gitignore_and_adds_the_endpoint_and_build_folders() {
    // Specification 25 §3.2: an existing `.gitignore` is kept and gains each line of the start
    // that it lacks, once. Catches a project creator that replaces the user's `.gitignore`, one
    // that leaves the build's folder to version control, and one that repeats a line.
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
    assert_eq!(
        text.lines().filter(|line| *line == "/build/").count(),
        1,
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
