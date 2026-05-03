use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const EXAMPLE_TEST_TIMEOUT: Duration = Duration::from_secs(120);
const EXAMPLE_TEST_PROGRESS_INTERVAL: Duration = Duration::from_secs(10);
const OSCAT_AGGREGATE_TRIGGER_EXAMPLE: &str = "airport_baggage_command_observer";
const OSCAT_AGGREGATE_TRIGGER_NAMESPACE: &str = "OSCAT_airport_baggage_command_observer_oop";

fn trust_dev_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_trust-dev") {
        return path.into();
    }
    if let Ok(path) = std::env::var("TRUST_DEV_BIN") {
        return path.into();
    }
    let exe = std::env::current_exe().expect("current test exe path");
    let debug_dir = exe
        .parent()
        .and_then(|deps| deps.parent())
        .expect("target debug dir");
    debug_dir.join(format!("trust-dev{}", std::env::consts::EXE_SUFFIX))
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExampleGateEvent {
    Started {
        index: usize,
        total: usize,
        project: PathBuf,
    },
    Passed {
        index: usize,
        total: usize,
        project: PathBuf,
    },
    Failed {
        index: usize,
        total: usize,
        project: PathBuf,
        message: String,
    },
}

impl ExampleGateEvent {
    fn log_line(&self) -> String {
        match self {
            ExampleGateEvent::Started {
                index,
                total,
                project,
            } => format!(
                "[oscat examples] starting {index}/{total}: {}",
                project.display()
            ),
            ExampleGateEvent::Passed {
                index,
                total,
                project,
            } => format!(
                "[oscat examples] passed {index}/{total}: {}",
                project.display()
            ),
            ExampleGateEvent::Failed {
                index,
                total,
                project,
                ..
            } => format!(
                "[oscat examples] failed {index}/{total}: {}",
                project.display()
            ),
        }
    }
}

include!("oscat_oop_examples/structural_expectations.rs");

fn examples_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
}

fn oscat_examples_root() -> PathBuf {
    examples_root().join("OSCAT")
}

fn oscat_example_dirs() -> Vec<PathBuf> {
    let mut dirs = std::fs::read_dir(oscat_examples_root())
        .expect("read examples/OSCAT")
        .map(|entry| entry.expect("read OSCAT example entry"))
        .filter(|entry| entry.file_type().expect("OSCAT entry file type").is_dir())
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    dirs.sort();
    dirs
}

fn oscat_example_projects() -> Vec<PathBuf> {
    let mut projects = Vec::new();
    for example_dir in oscat_example_dirs() {
        projects.push(example_dir.join("non-oop"));
        projects.push(example_dir.join("oop"));
    }
    projects
}

fn example_oop_path(slug: &str) -> PathBuf {
    oscat_examples_root().join(slug).join("oop")
}

struct TempProject {
    path: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-runtime-{name}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("src"))
            .unwrap_or_else(|err| panic!("create temp project {}: {err}", path.display()));
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn source_line_starts_with_keyword(line: &str, keyword: &str) -> bool {
    line.split_whitespace()
        .next()
        .is_some_and(|word| word.eq_ignore_ascii_case(keyword))
}

fn source_without_configuration_blocks(source: &str) -> String {
    let mut output = String::new();
    let mut skipping_configuration = false;
    for line in source.lines() {
        if source_line_starts_with_keyword(line, "CONFIGURATION") {
            skipping_configuration = true;
            continue;
        }
        if skipping_configuration {
            if source_line_starts_with_keyword(line, "END_CONFIGURATION") {
                skipping_configuration = false;
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn dependency_manifest_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn aggregate_dependency_manifest(workspace_root: &Path) -> String {
    let oscat_path = workspace_root.join("libraries").join("oscat");
    let oscat_oop_path = oscat_path.join("oop");
    format!(
        r#"[project]
include_paths = ["src"]
stdlib = "iec"

[dependencies]
OSCAT = {{ path = "{}", version = "0.1.0" }}
OscatOop = {{ path = "{}", version = "0.1.0" }}
"#,
        dependency_manifest_path(&oscat_path),
        dependency_manifest_path(&oscat_oop_path)
    )
}

fn write_oscat_namespace_aggregate_project(slug: &str, namespace: &str) -> TempProject {
    let temp = TempProject::new(slug);
    let workspace_root = examples_root()
        .parent()
        .expect("examples dir has workspace parent")
        .to_path_buf();
    let manifest = aggregate_dependency_manifest(&workspace_root);
    std::fs::write(temp.path().join("trust-lsp.toml"), manifest)
        .unwrap_or_else(|err| panic!("write aggregate manifest: {err}"));

    let source_dir = example_oop_path(slug).join("src");
    let mut source_files = std::fs::read_dir(&source_dir)
        .unwrap_or_else(|err| panic!("read {}: {err}", source_dir.display()))
        .map(|entry| entry.expect("read OSCAT aggregate source entry").path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("st"))
        })
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_none_or(|name| !name.eq_ignore_ascii_case("Configuration.st"))
        })
        .collect::<Vec<_>>();
    source_files.sort();

    let mut aggregate = format!("NAMESPACE {namespace}\nUSING {namespace};\n");
    for source_file in source_files {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|err| panic!("read {}: {err}", source_file.display()));
        aggregate.push_str(&source_without_configuration_blocks(&source));
        aggregate.push('\n');
    }
    aggregate.push_str("END_NAMESPACE\n");
    std::fs::write(temp.path().join("src").join("Aggregate.st"), aggregate)
        .unwrap_or_else(|err| panic!("write aggregate source: {err}"));

    temp
}

fn example_child_started_line(child_id: u32, project: &Path) -> String {
    format!(
        "[oscat examples] child pid={child_id} command=trust-dev test --project {} timeout={}s",
        project.display(),
        EXAMPLE_TEST_TIMEOUT.as_secs()
    )
}

fn example_child_progress_line(child_id: u32, project: &Path, elapsed: Duration) -> String {
    format!(
        "[oscat examples] child pid={child_id} still running elapsed={}s project={}",
        elapsed.as_secs(),
        project.display()
    )
}

fn example_child_timeout_line(child_id: u32, project: &Path, elapsed: Duration) -> String {
    format!(
        "[oscat examples] child pid={child_id} timed out reason=timeout elapsed={}s timeout={}s project={}",
        elapsed.as_secs(),
        EXAMPLE_TEST_TIMEOUT.as_secs(),
        project.display()
    )
}

fn run_example_st_tests_at(project: &Path) -> Result<(), String> {
    let mut child = Command::new(trust_dev_bin())
        .args(["test", "--project"])
        .arg(project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run trust-dev test");

    let started = Instant::now();
    let child_id = child.id();
    let mut next_progress = EXAMPLE_TEST_PROGRESS_INTERVAL;
    eprintln!("{}", example_child_started_line(child_id, project));
    loop {
        if child
            .try_wait()
            .expect("poll trust-dev example test")
            .is_some()
        {
            let output = child
                .wait_with_output()
                .expect("collect trust-dev example test output");
            let elapsed = started.elapsed();
            if output.status.success() {
                eprintln!(
                    "[oscat examples] child pid={child_id} completed status={} elapsed={}ms project={}",
                    output.status,
                    elapsed.as_millis(),
                    project.display()
                );
                return Ok(());
            }

            return Err(format!(
                "expected ST example tests to pass at {}\nchild pid: {}\nelapsed: {}ms\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
                project.display(),
                child_id,
                elapsed.as_millis(),
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        if started.elapsed() >= EXAMPLE_TEST_TIMEOUT {
            let elapsed = started.elapsed();
            let timeout_line = example_child_timeout_line(child_id, project, elapsed);
            eprintln!("{timeout_line}");
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .expect("collect timed-out trust-runtime example test output");
            return Err(format!(
                "{timeout_line}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let elapsed = started.elapsed();
        if elapsed >= next_progress {
            eprintln!(
                "{}",
                example_child_progress_line(child_id, project, elapsed)
            );
            next_progress += EXAMPLE_TEST_PROGRESS_INTERVAL;
        }

        thread::sleep(Duration::from_millis(100));
    }
}

fn assert_example_project_tests_pass_with<RunProject, Report>(
    projects: &[PathBuf],
    mut run_project: RunProject,
    mut report: Report,
) where
    RunProject: FnMut(&Path) -> Result<(), String>,
    Report: FnMut(ExampleGateEvent),
{
    assert!(
        !projects.is_empty(),
        "expected at least one OSCAT example project"
    );
    let mut failures = Vec::new();
    let total = projects.len();

    for (offset, project) in projects.iter().enumerate() {
        let index = offset + 1;
        report(ExampleGateEvent::Started {
            index,
            total,
            project: project.clone(),
        });
        match run_project(project) {
            Ok(()) => report(ExampleGateEvent::Passed {
                index,
                total,
                project: project.clone(),
            }),
            Err(message) => {
                report(ExampleGateEvent::Failed {
                    index,
                    total,
                    project: project.clone(),
                    message: message.clone(),
                });
                failures.push(message);
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} OSCAT OOP example project(s) failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

fn assert_example_project_tests_pass(projects: &[PathBuf]) {
    assert_example_project_tests_pass_with(projects, run_example_st_tests_at, |event| {
        eprintln!("{}", event.log_line());
    });
}

fn assert_pattern_structure(name: &str, needles: &[&str]) {
    let main_st = example_oop_path(name).join("src").join("Main.st");
    let source = std::fs::read_to_string(&main_st)
        .unwrap_or_else(|err| panic!("read {}: {err}", main_st.display()));
    for needle in needles {
        assert!(
            source.contains(needle),
            "expected {name} to contain pattern marker {needle:?} in {}",
            main_st.display()
        );
    }
}

#[test]
fn oscat_examples_use_grouped_oop_non_oop_layout() {
    let root = oscat_examples_root();
    assert!(root.is_dir(), "expected {} to exist", root.display());

    let legacy_dirs = std::fs::read_dir(examples_root())
        .expect("read examples root")
        .map(|entry| entry.expect("read examples entry"))
        .filter(|entry| {
            entry
                .file_type()
                .expect("examples entry file type")
                .is_dir()
        })
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("oscat_components_")
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    assert!(
        legacy_dirs.is_empty(),
        "OSCAT comparison projects must live under examples/OSCAT, found legacy dirs: {legacy_dirs:?}"
    );

    let example_dirs = oscat_example_dirs();
    assert_eq!(
        example_dirs.len(),
        49,
        "expected 49 paired OSCAT examples under {}",
        root.display()
    );

    for example_dir in example_dirs {
        let readme = example_dir.join("README.md");
        let readme_text = std::fs::read_to_string(&readme)
            .unwrap_or_else(|err| panic!("read {}: {err}", readme.display()));
        for marker in [
            "## Folder Layout",
            "## What This Example Teaches",
            "OOP pattern:",
            "## How The Pair Teaches OOP",
            "`non-oop/`",
            "`oop/`",
        ] {
            assert!(
                readme_text.contains(marker),
                "expected {} to contain {marker:?}",
                readme.display()
            );
        }

        for variant in ["non-oop", "oop"] {
            let project = example_dir.join(variant);
            assert!(
                project.join("trust-lsp.toml").is_file(),
                "expected {} to contain trust-lsp.toml",
                project.display()
            );
            assert!(
                project.join("src").join("Main.st").is_file(),
                "expected {} to contain src/Main.st",
                project.display()
            );
            assert!(
                project.join("src").join("Tests.st").is_file(),
                "expected {} to contain src/Tests.st",
                project.display()
            );
        }
    }
}

#[test]
fn oscat_example_gate_reports_active_project_before_running_child() {
    use std::cell::RefCell;

    let projects = vec![
        PathBuf::from("/tmp/oscat-example-a"),
        PathBuf::from("/tmp/oscat-example-b"),
    ];
    let events = RefCell::new(Vec::new());

    assert_example_project_tests_pass_with(
        &projects,
        |project| {
            let expected_index = if project == projects[0] { 1 } else { 2 };
            assert_eq!(
                events.borrow().last(),
                Some(&ExampleGateEvent::Started {
                    index: expected_index,
                    total: projects.len(),
                    project: project.to_path_buf(),
                }),
                "OSCAT gate must report the active project before running the child command"
            );
            Ok(())
        },
        |event| events.borrow_mut().push(event),
    );

    assert_eq!(
        events.into_inner(),
        vec![
            ExampleGateEvent::Started {
                index: 1,
                total: 2,
                project: projects[0].clone(),
            },
            ExampleGateEvent::Passed {
                index: 1,
                total: 2,
                project: projects[0].clone(),
            },
            ExampleGateEvent::Started {
                index: 2,
                total: 2,
                project: projects[1].clone(),
            },
            ExampleGateEvent::Passed {
                index: 2,
                total: 2,
                project: projects[1].clone(),
            },
        ],
    );
}

#[test]
fn oscat_example_child_lines_include_pid_project_and_elapsed_context() {
    let project = PathBuf::from("/tmp/oscat-example-a");

    assert_eq!(
        example_child_started_line(42, &project),
        format!(
            "[oscat examples] child pid=42 command=trust-dev test --project {} timeout={}s",
            project.display(),
            EXAMPLE_TEST_TIMEOUT.as_secs()
        )
    );
    assert_eq!(
        example_child_progress_line(42, &project, Duration::from_secs(31)),
        format!(
            "[oscat examples] child pid=42 still running elapsed=31s project={}",
            project.display()
        )
    );
    assert_eq!(
        example_child_timeout_line(42, &project, Duration::from_secs(121)),
        format!(
            "[oscat examples] child pid=42 timed out reason=timeout elapsed=121s timeout={}s project={}",
            EXAMPLE_TEST_TIMEOUT.as_secs(),
            project.display()
        )
    );
}

#[test]
fn oscat_airport_baggage_namespace_aggregate_trigger_passes() {
    let aggregate = write_oscat_namespace_aggregate_project(
        OSCAT_AGGREGATE_TRIGGER_EXAMPLE,
        OSCAT_AGGREGATE_TRIGGER_NAMESPACE,
    );
    run_example_st_tests_at(aggregate.path()).unwrap_or_else(|message| panic!("{message}"));
}

#[test]
fn oscat_aggregate_manifest_uses_toml_safe_dependency_paths() {
    let workspace_root = PathBuf::from(r"C:\Users\runneradmin\work\trust-platform");
    let manifest = aggregate_dependency_manifest(&workspace_root);
    let parsed: toml::Value =
        toml::from_str(&manifest).expect("aggregate dependency manifest must parse as TOML");

    assert_eq!(
        parsed["dependencies"]["OSCAT"]["path"].as_str(),
        Some("C:/Users/runneradmin/work/trust-platform/libraries/oscat")
    );
    assert_eq!(
        parsed["dependencies"]["OscatOop"]["path"].as_str(),
        Some("C:/Users/runneradmin/work/trust-platform/libraries/oscat/oop")
    );
}

#[test]
#[ignore = "expensive OSCAT gate runs all 98 paired example projects through trust-runtime CLI"]
fn oscat_oop_example_st_unit_tests_pass() {
    let projects = oscat_example_projects();
    assert_example_project_tests_pass(&projects);
}

#[test]
fn oscat_oop_examples_contain_claimed_pattern_structures() {
    for (name, needles) in STRUCTURAL_EXPECTATIONS {
        assert_pattern_structure(name, needles);
    }
}
