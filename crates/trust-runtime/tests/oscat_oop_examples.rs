use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

include!("../../../tests/support/repository_source_oracle.rs");

const EXAMPLE_TEST_TIMEOUT: Duration = Duration::from_secs(300);
const EXAMPLE_TEST_PROGRESS_INTERVAL: Duration = Duration::from_secs(10);
const OSCAT_AGGREGATE_TRIGGER_EXAMPLE: &str = "airport_baggage_command_observer";
const OSCAT_AGGREGATE_TRIGGER_NAMESPACE: &str = "OSCAT_airport_baggage_command_observer_oop";

fn trust_dev_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_trust-dev") {
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
        let path = trust_test_support::unique_temp_path(&format!(
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
        let source = repository_source_tree_read_to_string!(
            (&source_file, &workspace_root),
            roots = ["examples/OSCAT"],
            extension = "st",
        )
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
    let workspace_root = examples_root()
        .parent()
        .expect("examples dir has workspace parent")
        .to_path_buf();
    let main_st = example_oop_path(name).join("src").join("Main.st");
    let source = repository_source_tree_read_to_string!(
        (&main_st, &workspace_root),
        roots = ["examples/OSCAT"],
        extension = "st",
    )
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
    let workspace_root = examples_root()
        .parent()
        .expect("examples dir has workspace parent")
        .to_path_buf();
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
        let readme_text = repository_source_tree_read_to_string!(
            (&readme, &workspace_root),
            roots = ["examples/OSCAT"],
            extension = "md",
        )
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

/// The located I/O of the configuration `source`: each `<name> AT %I…` or `AT %Q…` declaration as
/// `(name, address, direction)`, `IN` for `%I` and `OUT` for `%Q`, in the source's order.
fn located_io(source: &str) -> Vec<(String, String, String)> {
    source
        .lines()
        .filter_map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            let (name, at, address) = (tokens.first()?, tokens.get(1)?, tokens.get(2)?);
            let direction = match address.get(..2)? {
                "%I" => "IN",
                "%Q" => "OUT",
                _ => return None,
            };
            (*at == "AT").then(|| (name.to_string(), address.to_string(), direction.to_string()))
        })
        .collect()
}

/// The rows of the "Integration map" table of `readme` whose address is a located address, as
/// `(tag, address, direction)`; rows of local variables (address `local`) are left out.
fn integration_map(readme: &str) -> Option<Vec<(String, String, String)>> {
    let map = readme.split("## Integration map").nth(1)?;
    Some(
        map.lines()
            .take_while(|line| !line.starts_with("## "))
            .filter(|line| line.starts_with("| `"))
            .filter_map(|line| {
                let cells: Vec<&str> = line.split('|').map(str::trim).collect();
                let address = cells.get(2)?.strip_prefix('`')?.strip_suffix('`')?;
                address.starts_with('%').then(|| {
                    (
                        cells[1].trim_matches('`').to_string(),
                        address.to_string(),
                        cells.get(3).copied().unwrap_or_default().to_string(),
                    )
                })
            })
            .collect(),
    )
}

#[test]
fn every_oscat_integration_map_lists_the_io_its_configuration_binds() {
    // Each OSCAT README's "Integration map" is the located I/O of its example's configuration:
    // the name, the address and the direction (IN for %I, OUT for %Q), in the configuration's
    // order, for each variant that binds any. Catches a map that drifted from the code, as
    // boiler_room_heating_plant's had: other instance names, other bits, and words on the command
    // bits' byte (checklist G235).
    let mut failures = Vec::new();
    for dir in oscat_example_dirs() {
        let readme = dir.join("README.md");
        let Some(rows) = std::fs::read_to_string(&readme)
            .ok()
            .and_then(|text| integration_map(&text))
        else {
            continue;
        };
        for variant in ["oop", "non-oop"] {
            let configuration = dir.join(variant).join("src").join("Configuration.st");
            let Ok(source) = std::fs::read_to_string(&configuration) else {
                continue;
            };
            let bound = located_io(&source);
            if !bound.is_empty() && rows != bound {
                failures.push(format!(
                    "{}: the integration map {rows:?} is not the I/O of {}: {bound:?}",
                    readme.display(),
                    configuration.display()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The test programs of the OSCAT manuals' worked example, the cold storage alarm, by project
/// (`non-oop/src/Tests.st` and `oop/src/Tests.st`).
const WORKED_EXAMPLE_TESTS: [(&str, &[&str]); 2] = [
    ("non-oop", &["ColdStorageAlarmClassicDetectsWarmChamber"]),
    (
        "oop",
        &[
            "ColdStorageAlarmOopDetectWarmChamber",
            "ColdStorageAlarmTracksChamberTemperatureChanges",
            "ColdStorageAlarmHoldsThroughHysteresisBand",
            "ColdStorageAlarmBoundaryAtExactLimits",
            "ColdStorageTwoChambersAreIsolated",
            "ColdStorageInvertedLimitsRaiseError",
        ],
    ),
];

#[test]
fn the_oscat_manuals_worked_example_passes_its_st_tests() {
    // Specification 22: a library manual's worked example is a program whose tests CI runs. The
    // OSCAT and OSCAT OOP manuals show the cold storage alarm and name its test programs: one in the
    // classic project and six in the OOP one, each of which must run and pass, while the sweep of
    // all 98 projects stays an explicit gate. Catches a worked example whose tests fail, tests that
    // are no longer found (an empty run would pass on its exit status), and a test that the manual
    // describes but the example no longer has.
    for (variant, names) in WORKED_EXAMPLE_TESTS {
        let project = oscat_examples_root()
            .join("cold_storage_alarm")
            .join(variant);
        let output = Command::new(trust_dev_bin())
            .args(["test", "--output", "json", "--project"])
            .arg(&project)
            .output()
            .expect("run trust-dev test");
        let shown = || {
            format!(
                "{}: status {}\nstdout:\n{}\nstderr:\n{}",
                project.display(),
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        };
        assert!(output.status.success(), "{}", shown());
        let report: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|_| panic!("{}", shown()));
        let passed: std::collections::BTreeSet<&str> = report["tests"]
            .as_array()
            .unwrap_or_else(|| panic!("{}", shown()))
            .iter()
            .filter(|test| test["status"] == "passed")
            .filter_map(|test| test["name"].as_str())
            .collect();
        let expected: std::collections::BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(passed, expected, "{}", shown());
        assert_eq!(report["summary"]["total"], names.len(), "{}", shown());
    }
}
