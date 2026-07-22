use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_PATH_COUNTER: AtomicU64 = AtomicU64::new(1);

fn run_runtime(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
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
        let mut command = Command::new(env!("CARGO_BIN_EXE_trust-runtime"));
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

    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
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
