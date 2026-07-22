use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn run_runtime(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(args)
        .output()
        .expect("run trust-runtime")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr utf-8")
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
