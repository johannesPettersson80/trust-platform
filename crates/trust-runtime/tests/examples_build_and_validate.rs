//! Example projects and resource selection, checked through the real `trust-runtime` binary.
//!
//! Each runnable `examples/**/runtime.toml` project with ST sources is copied to a private temp
//! root. Its trust-lsp.toml dependencies are copied to the same positions relative to the
//! repository root, as `ExampleRuntime::library` does in `support/runtime_process.rs`, so the
//! relative dependency paths still resolve. The copy must build and validate. The repository is
//! never written.
//!
//! The OSCAT examples are ST test projects: their sources declare `TEST_PROGRAM`s that no
//! CONFIGURATION binds, and ordinary runtime construction rejects those (spec 22, "Test case
//! execution"). `oscat_oop_examples.rs` runs them through the test runner. Here every example,
//! OSCAT included, is checked for a configured resource its parsed sources declare.
#![cfg(feature = "full")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use trust_runtime::bundle_builder::inspect_project_layout;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};
use trust_syntax::SyntaxKind;

/// Left out of every copy: earlier builds and tool caches. The copy is built from its sources.
const SKIPPED_ENTRIES: [&str; 4] = ["program.stbc", "target", ".trust", "node_modules"];
/// Examples whose trust-lsp.toml depends on a checkout outside this repository, so no copy can be
/// built here. `openot_telemetry.rs` builds this one's `Main.st` with the vendored OpenOT library
/// (`tests/fixtures/openot`).
const EXTERNAL_DEPENDENCY_EXAMPLES: [&str; 1] = ["openot_multi_program"];
/// Each build uses one core; four workers keep a CI runner responsive.
const MAX_BUILD_WORKERS: usize = 4;
/// The launch error for a configured resource the bytecode does not declare (spec 11 §6.7.1).
/// `artifact::select_resource` produces it, so `play` and `validate` print the same text.
const UNDECLARED_RESOURCE_ERROR: &str = "resource 'Missing' not found in supplied bytecode";
/// `play` fails in `load_runtime` before it starts anything. The bound only stops a regression that
/// starts the runtime instead of failing; the test then kills the process and fails.
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(60);
/// Declares one resource, `Line1`, so a configured name must select it.
const DECLARED_RESOURCE_SOURCE: &str = r#"CONFIGURATION Plant
RESOURCE Line1 ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION

PROGRAM Main
VAR
    Counter : INT := 0;
END_VAR
Counter := Counter + 1;
END_PROGRAM
"#;
/// Declares no resource, so the compiler gives the module the legacy placeholder `RESOURCE`.
const PLACEHOLDER_SOURCE: &str = r#"PROGRAM Main
VAR
    Counter : INT := 0;
END_VAR
Counter := Counter + 1;
END_PROGRAM
"#;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

#[test]
fn every_runnable_example_builds_and_validates() {
    let repo = repository_root();
    let examples = repo.join("examples");
    let external = EXTERNAL_DEPENDENCY_EXAMPLES.map(|name| examples.join(name));
    for project in &external {
        assert!(
            project.join("runtime.toml").is_file(),
            "{} is listed as an external-dependency example but has no runtime.toml",
            project.display()
        );
    }
    let projects = example_projects_with_st_sources(&examples)
        .into_iter()
        .filter(|project| !is_oscat_example(&examples, project) && !external.contains(project))
        .collect::<Vec<_>>();
    assert!(
        !projects.is_empty(),
        "no runnable example project with ST sources under {}",
        examples.display()
    );

    let failures = failures_in_parallel(&repo, &projects, |project| {
        build_and_validate_copy(&repo, project)
    });
    assert!(
        failures.is_empty(),
        "{} of {} runnable example projects failed to build or validate:\n{}",
        failures.len(),
        projects.len(),
        failures.join("\n")
    );
}

/// Launch selects the configured resource by name, compared case-insensitively, or the legacy
/// placeholder the compiler gives sources that declare no resource (spec 11 §6.7.1).
#[test]
fn every_example_configures_a_resource_its_sources_declare() {
    let repo = repository_root();
    let projects = example_projects_with_st_sources(&repo.join("examples"));
    let mut failures = Vec::new();
    for project in &projects {
        let configured = configured_resource_name(project);
        let declared = parsed_sources(project).resources;
        if !declared.is_empty()
            && !declared
                .iter()
                .any(|resource| resource.eq_ignore_ascii_case(&configured))
        {
            failures.push(format!(
                "{}: runtime.toml names resource '{configured}', the sources declare {declared:?}",
                relative_to(&repo, project)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} example projects configure an undeclared resource:\n{}",
        failures.len(),
        projects.len(),
        failures.join("\n")
    );
}

/// Keeps the runnable set honest: an example outside `examples/OSCAT` that gained a
/// `TEST_PROGRAM` could no longer be built, and an OSCAT example without one would go unbuilt.
#[test]
fn only_the_oscat_examples_declare_test_programs() {
    let repo = repository_root();
    let examples = repo.join("examples");
    let mismatched = example_projects_with_st_sources(&examples)
        .into_iter()
        .filter(|project| {
            parsed_sources(project).test_programs != is_oscat_example(&examples, project)
        })
        .map(|project| relative_to(&repo, &project))
        .collect::<Vec<_>>();
    assert!(
        mismatched.is_empty(),
        "these examples declare TEST_PROGRAMs outside examples/OSCAT, or none inside it: {mismatched:?}"
    );
}

#[test]
fn validate_rejects_a_configured_resource_the_bytecode_does_not_declare() {
    let root = TempRoot::new("undeclared-resource");
    let project = root.path().join("project");
    write_project(&project, DECLARED_RESOURCE_SOURCE, "Missing");
    let build = cli(&["build"], &project);
    assert!(build.status.success(), "{}", failure("build", &build));

    let validate = cli(&["validate"], &project);
    assert!(
        !validate.status.success(),
        "validate must reject a resource the bytecode does not declare"
    );
    let validate_error = stderr_text(&validate);
    assert!(
        validate_error.contains(UNDECLARED_RESOURCE_ERROR),
        "validate must report the launch error; stderr:\n{validate_error}"
    );

    let (launch_status, launch_error) = launch_failure(root.path(), &project);
    assert!(
        !launch_status.success(),
        "play must not start an undeclared resource"
    );
    assert!(
        launch_error.contains(UNDECLARED_RESOURCE_ERROR),
        "play must report the same error as validate; stderr:\n{launch_error}"
    );
}

#[test]
fn validate_accepts_the_configured_resource_by_case_and_by_legacy_placeholder() {
    let root = TempRoot::new("resource-selection");
    let by_case = root.path().join("by-case");
    write_project(&by_case, DECLARED_RESOURCE_SOURCE, "line1");
    let placeholder = root.path().join("placeholder");
    write_project(&placeholder, PLACEHOLDER_SOURCE, "Anything");

    for project in [&by_case, &placeholder] {
        let build = cli(&["build"], project);
        assert!(build.status.success(), "{}", failure("build", &build));
    }
    let validate = cli(&["validate", "--ci"], &by_case);
    assert!(
        validate.status.success(),
        "{}",
        failure("validate", &validate)
    );
    let payload: serde_json::Value =
        serde_json::from_slice(&validate.stdout).expect("validate --ci prints JSON");
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["resource"], "line1");

    let validate = cli(&["validate", "--ci"], &placeholder);
    assert!(
        validate.status.success(),
        "{}",
        failure("validate", &validate)
    );
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("resolve the repository root")
}

fn relative_to(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn trust_runtime() -> Command {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
}

/// Runs `trust-runtime <args> --project <project>`.
fn cli(args: &[&str], project: &Path) -> Output {
    trust_runtime()
        .args(args)
        .arg("--project")
        .arg(project)
        .output()
        .unwrap_or_else(|error| panic!("run trust-runtime {}: {error}", args.join(" ")))
}

fn failure(step: &str, output: &Output) -> String {
    format!(
        "{step} exited with {}:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim_end()
    )
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Starts `play` on the project and waits for it to exit, logging stderr under `logs`.
fn launch_failure(logs: &Path, project: &Path) -> (ExitStatus, String) {
    let stderr_path = logs.join("play.stderr");
    let mut child = trust_runtime()
        .args(["play", "--project"])
        .arg(project)
        .stdin(Stdio::null())
        .stdout(fs::File::create(logs.join("play.stdout")).expect("create play stdout log"))
        .stderr(fs::File::create(&stderr_path).expect("create play stderr log"))
        .spawn()
        .expect("spawn trust-runtime play");
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll trust-runtime play") {
            break status;
        }
        if started.elapsed() >= LAUNCH_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            panic!("play still running after {LAUNCH_TIMEOUT:?}; it should fail before starting");
        }
        thread::sleep(Duration::from_millis(20));
    };
    let stderr = fs::read_to_string(&stderr_path).expect("read play stderr log");
    (status, stderr)
}

/// Every folder under `examples` that has a runtime.toml and ST sources under its `src/`. A
/// runtime.toml in a folder without such sources (for example a nested `src/runtime.toml`) is not
/// a project of its own.
fn example_projects_with_st_sources(examples: &Path) -> Vec<PathBuf> {
    let mut projects = Vec::new();
    collect_projects(examples, &mut projects);
    projects.sort();
    projects
}

fn collect_projects(dir: &Path, projects: &mut Vec<PathBuf>) {
    if dir.join("runtime.toml").is_file() && contains_st_source(&dir.join("src")) {
        projects.push(dir.to_path_buf());
    }
    let entries =
        fs::read_dir(dir).unwrap_or_else(|error| panic!("read {}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("read example entry").path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if path.is_dir() && !name.starts_with('.') && name != "target" && name != "node_modules" {
            collect_projects(&path, projects);
        }
    }
}

/// Whether the tree under `dir` holds a Structured Text source.
fn contains_st_source(dir: &Path) -> bool {
    !st_sources(dir).is_empty()
}

/// The `.st` and `.pou` files under `dir`, the extensions the bundle builder collects
/// (`collect_source_paths`, `host/bundle_builder/build.rs`).
fn st_sources(dir: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    collect_st_sources(dir, &mut sources);
    sources
}

fn collect_st_sources(dir: &Path, sources: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_st_sources(&path, sources);
        } else if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("st") || extension.eq_ignore_ascii_case("pou")
            })
        {
            sources.push(path);
        }
    }
}

fn is_oscat_example(examples: &Path, project: &Path) -> bool {
    project.starts_with(examples.join("OSCAT"))
}

/// What a project's own sources declare, read with the truST parser.
struct ParsedSources {
    /// The names of the declared RESOURCEs.
    resources: Vec<String>,
    /// Whether any source declares a `TEST_PROGRAM`.
    test_programs: bool,
}

fn parsed_sources(project: &Path) -> ParsedSources {
    let mut parsed = ParsedSources {
        resources: Vec::new(),
        test_programs: false,
    };
    for path in st_sources(&project.join("src")) {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let root = trust_syntax::parser::parse(&text).syntax();
        parsed.test_programs |= root
            .descendants_with_tokens()
            .any(|element| element.kind() == SyntaxKind::KwTestProgram);
        parsed.resources.extend(
            root.descendants()
                .filter(|node| node.kind() == SyntaxKind::Resource)
                .filter_map(|resource| {
                    resource
                        .children()
                        .find(|child| child.kind() == SyntaxKind::Name)
                })
                .filter_map(|name| {
                    name.descendants_with_tokens()
                        .filter_map(|element| element.into_token())
                        .find(|token| token.kind() == SyntaxKind::Ident)
                        .map(|token| token.text().to_string())
                }),
        );
    }
    parsed
}

/// The `[resource] name` of a project's runtime.toml.
fn configured_resource_name(project: &Path) -> String {
    let path = project.join("runtime.toml");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let runtime: toml::Value =
        toml::from_str(&text).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
    runtime
        .get("resource")
        .and_then(|resource| resource.get("name"))
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("{} has no [resource] name", path.display()))
        .to_string()
}

/// Runs `check` on every project on up to [`MAX_BUILD_WORKERS`] threads and returns the
/// failures in project order.
fn failures_in_parallel(
    repo: &Path,
    projects: &[PathBuf],
    check: impl Fn(&Path) -> Result<(), String> + Sync,
) -> Vec<String> {
    let next = AtomicUsize::new(0);
    let failures = Mutex::new(Vec::new());
    let workers = thread::available_parallelism()
        .map_or(1, usize::from)
        .min(MAX_BUILD_WORKERS);
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while let Some(project) = projects.get(next.fetch_add(1, Ordering::Relaxed)) {
                    if let Err(error) = check(project) {
                        failures
                            .lock()
                            .expect("failure list lock")
                            .push(format!("{}\n    {error}", relative_to(repo, project)));
                    }
                }
            });
        }
    });
    let mut failures = failures.into_inner().expect("failure list lock");
    failures.sort();
    failures
}

/// Copies one example and its dependencies, then builds and validates the copy. A failed build
/// is reported alone, because validate would only repeat it.
fn build_and_validate_copy(repo: &Path, project: &Path) -> Result<(), String> {
    let root = TempRoot::new("example");
    let copy = root.path().join(strip_repo(repo, project)?);
    copy_tree(project, &copy)?;
    let layout = inspect_project_layout(project, None)
        .map_err(|error| format!("resolve trust-lsp.toml dependencies: {error:#}"))?;
    for dependency in &layout.dependency_roots {
        copy_tree(dependency, &root.path().join(strip_repo(repo, dependency)?))?;
    }
    let build = cli(&["build"], &copy);
    if !build.status.success() {
        return Err(failure("build", &build));
    }
    #[cfg(windows)]
    use_tcp_control_endpoint(&copy.join("runtime.toml"))?;
    let validate = cli(&["validate"], &copy);
    if !validate.status.success() {
        return Err(failure("validate", &validate));
    }
    Ok(())
}

/// The shipped examples name Unix-socket control endpoints, which Windows lacks, so `validate`
/// rejects them there. On Windows the copy gets a loopback TCP endpoint with a token, as
/// `portable_runtime_toml` gives the generated projects; everything else is validated unchanged.
/// It is compiled everywhere, so every platform type-checks it, and called only on Windows.
#[cfg_attr(not(windows), allow(dead_code))]
fn use_tcp_control_endpoint(runtime_toml: &Path) -> Result<(), String> {
    let text = fs::read_to_string(runtime_toml)
        .map_err(|error| format!("read {}: {error}", runtime_toml.display()))?;
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| format!("parse {}: {error}", runtime_toml.display()))?;
    let control = &mut document["runtime"]["control"];
    let unix_endpoint = control
        .get("endpoint")
        .and_then(|endpoint| endpoint.as_str())
        .is_some_and(|endpoint| endpoint.starts_with("unix://"));
    if !unix_endpoint {
        return Ok(());
    }
    control["endpoint"] = toml_edit::value("tcp://127.0.0.1:0");
    if control.get("auth_token").is_none() {
        control["auth_token"] = toml_edit::value("trust-examples-token");
    }
    fs::write(runtime_toml, document.to_string())
        .map_err(|error| format!("write {}: {error}", runtime_toml.display()))
}

fn strip_repo<'a>(repo: &Path, path: &'a Path) -> Result<&'a Path, String> {
    path.strip_prefix(repo)
        .map_err(|_| format!("{} is outside the repository", path.display()))
}

/// Copies a directory tree, leaving out [`SKIPPED_ENTRIES`]. Symbolic links are followed.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|error| format!("create {}: {error}", to.display()))?;
    let entries =
        fs::read_dir(from).map_err(|error| format!("read {}: {error}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read {}: {error}", from.display()))?;
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|name| SKIPPED_ENTRIES.contains(&name))
        {
            continue;
        }
        let source = entry.path();
        let target = to.join(&name);
        let metadata = fs::metadata(&source)
            .map_err(|error| format!("inspect {}: {error}", source.display()))?;
        if metadata.is_dir() {
            copy_tree(&source, &target)?;
        } else {
            fs::copy(&source, &target)
                .map_err(|error| format!("copy {}: {error}", source.display()))?;
        }
    }
    Ok(())
}

/// Writes a project with `source`, a runtime.toml that names `resource`, and a loopback io.toml.
fn write_project(project: &Path, source: &str, resource: &str) {
    write_file(&project.join("src").join("main.st"), source);
    let runtime = render_runtime_toml(&resource.into(), 10);
    write_file(
        &project.join("runtime.toml"),
        &portable_runtime_toml(runtime),
    );
    let io = build_io_config_auto("loopback").expect("build loopback io template");
    write_file(&project.join("io.toml"), &render_io_toml(&io));
}

/// The default control endpoint is a unix socket, which Windows lacks. Use TCP with a token there,
/// as `config_schema_command.rs` does. `validate` only parses the endpoint.
fn portable_runtime_toml(runtime: String) -> String {
    #[cfg(windows)]
    {
        return runtime.replacen(
            "endpoint = \"unix:///tmp/trust-runtime.sock\"",
            "endpoint = \"tcp://127.0.0.1:0\"\nauth_token = \"trust-examples-token\"",
            1,
        );
    }
    #[cfg(not(windows))]
    runtime
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("create {}: {error}", parent.display()));
    }
    fs::write(path, contents).unwrap_or_else(|error| panic!("write {}: {error}", path.display()));
}

/// A private directory under the system temp directory, removed when dropped.
struct TempRoot(PathBuf);

impl TempRoot {
    fn new(prefix: &str) -> Self {
        for _ in 0..64 {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos();
            let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "trust-runtime-{prefix}-{}-{nanos}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create temp directory {}: {error}", path.display()),
            }
        }
        panic!("failed to allocate a unique temp directory for {prefix}")
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
