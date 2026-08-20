use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value as JsonValue;
use smol_str::SmolStr;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    for _ in 0..64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let seq = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "trust-runtime-{prefix}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        match std::fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => panic!("create temp dir {}: {err}", dir.display()),
        }
    }
    panic!("failed to allocate unique temp dir for '{prefix}'")
}

fn make_runtime_toml_portable(runtime_toml: String) -> String {
    #[cfg(windows)]
    {
        return runtime_toml.replacen(
            "endpoint = \"unix:///tmp/trust-runtime.sock\"",
            "endpoint = \"tcp://127.0.0.1:0\"\nauth_token = \"trust-ci-token\"",
            1,
        );
    }
    #[cfg(not(windows))]
    runtime_toml
}

fn write_project_fixture(root: &std::path::Path, source: &str) {
    std::fs::create_dir_all(root.join("src")).expect("create source directory");
    let runtime_toml = make_runtime_toml_portable(render_runtime_toml(&SmolStr::new("main"), 100));
    let io_template = build_io_config_auto("loopback").expect("build loopback io template");
    let io_toml = render_io_toml(&io_template);
    std::fs::write(root.join("runtime.toml"), runtime_toml).expect("write runtime.toml");
    std::fs::write(root.join("io.toml"), io_toml).expect("write io.toml");
    std::fs::write(root.join("src").join("main.st"), source).expect("write main source");
}

fn run_check(project: &std::path::Path) -> Output {
    run_check_with_args(project, &["--json"])
}

fn run_check_with_args(project: &std::path::Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_trust-runtime"));
    command
        .arg("check")
        .arg("--project")
        .arg(project)
        .args(args);
    command.output().expect("run trust-runtime check")
}

fn json_stdout(output: &Output) -> JsonValue {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout should be JSON ({error}); stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn check_accepts_project_without_writing_program_stbc() {
    let project = unique_temp_dir("check-ok");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
VAR
    Counter : INT := 0;
END_VAR
Counter := Counter + 1;
END_PROGRAM
"#,
    );

    let bytecode_path = project.join("program.stbc");
    std::fs::write(&bytecode_path, b"existing-bytecode").expect("write existing bytecode");

    let output = run_check(&project);
    assert!(
        output.status.success(),
        "expected check success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload = json_stdout(&output);
    assert_eq!(payload["version"], 1);
    assert_eq!(payload["command"], "check");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["errors"], 0);
    assert_eq!(payload["warnings"], 0);
    assert_eq!(
        payload["source_count"].as_u64(),
        payload["sources"]
            .as_array()
            .map(|sources| sources.len() as u64)
    );
    assert!(payload["bytecode_size"].as_u64().unwrap_or_default() > 0);
    assert_eq!(
        std::fs::read(&bytecode_path).expect("read existing bytecode"),
        b"existing-bytecode",
        "check must not replace program.stbc"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_reports_compile_error_as_json_issue() {
    let project = unique_temp_dir("check-compile-error");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
VAR
    Counter : INT := 0;
END_VAR
Counter :=
END_PROGRAM
"#,
    );

    let output = run_check(&project);
    assert_eq!(output.status.code(), Some(11));
    let payload = json_stdout(&output);
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["status"], "failed");
    assert_eq!(payload["issues"][0]["code"], "compile");
    assert!(payload["issues"][0]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("program compile failed"));
    assert!(payload["source_count"].as_u64().unwrap_or_default() >= 1);
    assert_eq!(payload["bytecode_size"], JsonValue::Null);
    assert!(
        !project.join("program.stbc").exists(),
        "failed check must not write program.stbc"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_reports_config_error_as_json_issue() {
    let project = unique_temp_dir("check-config-error");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    let runtime_path = project.join("runtime.toml");
    let mut runtime_text = std::fs::read_to_string(&runtime_path).expect("read runtime.toml");
    runtime_text.push_str("\n[runtime.extra]\nflag = true\n");
    std::fs::write(&runtime_path, runtime_text).expect("write invalid runtime.toml");

    let output = run_check(&project);
    assert!(
        !output.status.success(),
        "expected check failure for invalid runtime.toml"
    );
    let payload = json_stdout(&output);
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["status"], "failed");
    assert!(payload["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .any(|issue| issue["code"] == "config.runtime"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_aggregates_required_and_optional_config_errors_with_stable_exit_code() {
    let project = unique_temp_dir("check-config-aggregate");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    std::fs::remove_file(project.join("io.toml")).expect("remove required io.toml");
    std::fs::write(project.join("ads.toml"), "connections = [").expect("write invalid ads.toml");
    std::fs::write(project.join("opcua_client.toml"), "connections = [")
        .expect("write invalid opcua_client.toml");

    let output = run_check(&project);
    assert_eq!(output.status.code(), Some(10));
    let payload = json_stdout(&output);
    let issues = payload["issues"].as_array().expect("issues array");
    for expected in ["config.io", "config.ads", "config.opcua_client"] {
        assert!(
            issues.iter().any(|issue| issue["code"] == expected),
            "missing {expected} in {issues:?}"
        );
    }
    assert_eq!(payload["errors"].as_u64(), Some(3));
    assert!(payload["bytecode_size"].as_u64().unwrap_or_default() > 0);
    assert!(!project.join("program.stbc").exists());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_reports_source_layout_error_with_invalid_config_exit_code() {
    let project = unique_temp_dir("check-source-layout");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    std::fs::remove_dir_all(project.join("src")).expect("remove source directory");

    let output = run_check(&project);
    assert_eq!(output.status.code(), Some(10));
    let payload = json_stdout(&output);
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["status"], "failed");
    assert_eq!(payload["source_count"], 0);
    assert_eq!(payload["sources"], serde_json::json!([]));
    assert_eq!(payload["bytecode_size"], JsonValue::Null);
    assert!(payload["issues"]
        .as_array()
        .expect("issues")
        .iter()
        .any(|issue| issue["code"] == "sources"));
    assert!(!project.join("program.stbc").exists());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_ci_reports_sources_override_and_local_dependencies() {
    let project = unique_temp_dir("check-ci-dependencies");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    std::fs::rename(project.join("src"), project.join("custom_sources"))
        .expect("rename source directory");
    let dependency_root = project.join("deps").join("lib-a");
    std::fs::create_dir_all(dependency_root.join("src"))
        .expect("create dependency source directory");
    std::fs::write(
        dependency_root.join("src").join("lib.st"),
        r#"
FUNCTION LibValue : INT
LibValue := 7;
END_FUNCTION
"#,
    )
    .expect("write dependency source");
    std::fs::write(
        project.join("trust-lsp.toml"),
        "[dependencies]\nLibA = \"deps/lib-a\"\n",
    )
    .expect("write dependency manifest");

    let output = run_check_with_args(&project, &["--ci", "--sources", "custom_sources"]);
    assert!(
        output.status.success(),
        "expected CI check success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload = json_stdout(&output);
    assert_eq!(payload["version"], 1);
    assert_eq!(payload["command"], "check");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["source_count"], 2);
    assert_eq!(
        payload["resolved_dependencies"],
        serde_json::json!(["LibA"])
    );
    assert_eq!(
        payload["dependency_roots"]
            .as_array()
            .expect("dependency roots")
            .len(),
        1
    );
    let sources = payload["sources"].as_array().expect("sources");
    assert!(sources.iter().any(|path| path
        .as_str()
        .is_some_and(|path| path.ends_with("custom_sources/main.st"))));
    assert!(sources.iter().any(|path| path
        .as_str()
        .is_some_and(|path| path.ends_with("deps/lib-a/src/lib.st"))));
    assert!(!project.join("program.stbc").exists());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_mixed_config_and_compile_errors_uses_config_exit_and_reports_both() {
    let project = unique_temp_dir("check-mixed-errors");
    write_project_fixture(
        &project,
        r#"
PROGRAM Main
Main :=
END_PROGRAM
"#,
    );
    std::fs::remove_file(project.join("io.toml")).expect("remove required io.toml");

    let output = run_check(&project);
    assert_eq!(output.status.code(), Some(10));
    let payload = json_stdout(&output);
    let issues = payload["issues"].as_array().expect("issues");
    assert!(issues.iter().any(|issue| issue["code"] == "config.io"));
    assert!(issues.iter().any(|issue| issue["code"] == "compile"));
    assert_eq!(payload["errors"], 2);
    assert!(payload["source_count"].as_u64().unwrap_or_default() >= 1);
    assert_eq!(payload["bytecode_size"], JsonValue::Null);
    assert!(!project.join("program.stbc").exists());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn check_human_output_reports_success_and_failure() {
    let valid_project = unique_temp_dir("check-human-ok");
    write_project_fixture(
        &valid_project,
        r#"
PROGRAM Main
END_PROGRAM
"#,
    );
    let success = run_check_with_args(&valid_project, &[]);
    assert!(success.status.success());
    let success_stdout = String::from_utf8_lossy(&success.stdout);
    assert!(success_stdout.contains("Project check passed"));
    assert!(success_stdout.contains("Sources: 1 file(s)"));

    let invalid_project = unique_temp_dir("check-human-failed");
    write_project_fixture(
        &invalid_project,
        r#"
PROGRAM Main
Main :=
END_PROGRAM
"#,
    );
    let failure = run_check_with_args(&invalid_project, &[]);
    assert_eq!(failure.status.code(), Some(11));
    let failure_stderr = String::from_utf8_lossy(&failure.stderr);
    assert!(failure_stderr.contains("Project check failed"));
    assert!(failure_stderr.contains("program compile failed"));

    let _ = std::fs::remove_dir_all(valid_project);
    let _ = std::fs::remove_dir_all(invalid_project);
}
