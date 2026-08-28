use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_PATH_COUNTER: AtomicU64 = AtomicU64::new(1);

fn run_runtime(args: &[&str]) -> Output {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args(args)
    .output()
    .expect("run trust-runtime")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr utf-8")
}

fn unique_missing_project(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    let sequence = TEMP_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "trust-runtime-{name}-{}-{stamp}-{sequence}",
        std::process::id()
    ))
}

#[test]
fn simulation_coupling_tutorial_is_a_checkable_standalone_project() {
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/tutorials/09_simulation_coupling");
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args(["check", "--project"])
    .arg(&project)
    .arg("--json")
    .output()
    .expect("check simulation-coupling tutorial");

    assert!(
        output.status.success(),
        "tutorial 09 must compile as a complete project\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("check JSON report");
    assert_eq!(report["ok"], true);
    assert_eq!(report["source_count"], 1);
}

#[test]
fn mistyped_check_command_suggests_the_real_subcommand() {
    let output = run_runtime(&["chek"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("Did you mean: check?"),
        "stderr was:\n{}",
        stderr(&output)
    );
}

#[test]
fn subcommand_suggestion_ignores_global_flags() {
    let output = run_runtime(&["--verbose", "rn"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("Did you mean: run?"),
        "stderr was:\n{}",
        stderr(&output)
    );
}

#[test]
fn distant_unknown_command_is_not_given_a_misleading_suggestion() {
    let output = run_runtime(&["definitely-not-a-command"]);
    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(
        !error.contains("Did you mean:"),
        "distant command must retain the parser error without a suggestion:\n{error}"
    );
}

#[test]
fn deprecated_bundle_alias_warns_and_missing_project_gets_creation_tip() {
    let project = unique_missing_project("deprecated-bundle");
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("run")
    .arg("--bundle")
    .arg(&project)
    .output()
    .expect("run trust-runtime with deprecated bundle alias");

    assert_eq!(output.status.code(), Some(1));
    let error = stderr(&output);
    assert!(
        error.contains("Warning: --bundle is deprecated. Use --project instead."),
        "stderr was:\n{error}"
    );
    assert!(
        error.contains("Tip: run `trust-runtime` in an empty folder"),
        "stderr was:\n{error}"
    );
    assert!(
        !project.exists(),
        "run must not create the missing project {}",
        project.display()
    );
}

#[test]
fn ci_mode_classifies_an_invalid_validate_project() {
    let project = unique_missing_project("ci-validate");
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("validate")
    .arg("--project")
    .arg(&project)
    .arg("--ci")
    .output()
    .expect("run trust-runtime validate --ci");

    assert_eq!(output.status.code(), Some(10));
    assert!(
        stderr(&output).contains("invalid project folder"),
        "stderr was:\n{}",
        stderr(&output)
    );
    assert!(!project.exists());
}

#[test]
fn play_rejects_invalid_options_before_project_creation() {
    let cases: &[(&str, &[&str], &str)] = &[
        (
            "invalid-restart",
            &["--restart", "invalid"],
            "Invalid restart mode",
        ),
        (
            "zero-time-scale",
            &["--time-scale", "0"],
            "--time-scale must be >= 1",
        ),
    ];

    for (name, options, expected_error) in cases {
        let project = unique_missing_project(name);
        let mut command =
            Command::new(std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect(
                "Cargo must provide trust-runtime binary while executing integration tests",
            ));
        command.arg("play").arg("--project").arg(&project);
        command.args(*options).arg("--no-console");
        let output = command.output().expect("run trust-runtime play");

        assert!(!output.status.success(), "{name} must be rejected");
        assert!(
            stderr(&output).contains(expected_error),
            "unexpected {name} error:\n{}",
            stderr(&output)
        );
        assert!(
            !project.exists(),
            "invalid launch options must not create {}",
            project.display()
        );
    }
}

#[test]
fn ide_serve_reports_invalid_primary_runtime_before_server_start() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    let project = std::env::temp_dir().join(format!(
        "trust-runtime-invalid-ide-project-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&project).expect("create invalid IDE project");
    std::fs::write(project.join("runtime.toml"), "not valid TOML = [")
        .expect("write invalid runtime.toml");

    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args(["ide", "serve", "--project"])
    .arg(&project)
    .args(["--listen", "not-an-address"])
    .output()
    .expect("run trust-runtime ide serve");

    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(
        error.contains("runtime.toml") && !error.contains("start standalone IDE web server"),
        "invalid runtime configuration must fail before server startup; stderr was:\n{error}"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn ide_serve_rejects_non_directory_project_before_server_start() {
    let project = unique_missing_project("ide-project-file");
    std::fs::write(&project, "not a project directory").expect("write project-shaped file");

    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args(["ide", "serve", "--project"])
    .arg(&project)
    .args(["--listen", "not-an-address"])
    .output()
    .expect("run trust-runtime ide serve with file project");

    assert!(!output.status.success());
    let error = stderr(&output);
    assert!(
        error.contains("project path")
            && error.contains("not a directory")
            && !error.contains("start standalone IDE web server"),
        "a non-directory project must fail before server startup; stderr was:\n{error}"
    );

    let _ = std::fs::remove_file(project);
}

#[test]
fn deprecated_config_ui_serve_warns_before_shared_project_validation() {
    let project = unique_missing_project("config-ui-project-file");
    std::fs::write(&project, "not a project directory").expect("write project-shaped file");

    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args(["config-ui", "serve", "--project"])
    .arg(&project)
    .args(["--listen", "not-an-address"])
    .output()
    .expect("run deprecated config-ui serve with file project");

    assert!(!output.status.success());
    let error = stderr(&output);
    let warning = error
        .find("`config-ui serve` is deprecated")
        .expect("deprecated config-ui warning");
    let validation = error
        .find("project path")
        .expect("shared project validation error");
    assert!(
        warning < validation
            && error.contains("not a directory")
            && !error.contains("start standalone IDE web server"),
        "the deprecation warning must precede shared validation; stderr was:\n{error}"
    );

    let _ = std::fs::remove_file(project);
}
