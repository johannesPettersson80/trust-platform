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
    Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("check")
        .arg("--project")
        .arg(project)
        .arg("--json")
        .output()
        .expect("run trust-runtime check")
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

    let output = run_check(&project);
    assert!(
        output.status.success(),
        "expected check success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload = json_stdout(&output);
    assert_eq!(payload["command"], "check");
    assert_eq!(payload["ok"], true);
    assert_eq!(payload["status"], "ok");
    assert_eq!(payload["errors"], 0);
    assert!(payload["source_count"].as_u64().unwrap_or_default() >= 1);
    assert!(payload["bytecode_size"].as_u64().unwrap_or_default() > 0);
    assert!(
        !project.join("program.stbc").exists(),
        "check must not write program.stbc"
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
    assert!(
        !output.status.success(),
        "expected check failure for broken source"
    );
    let payload = json_stdout(&output);
    assert_eq!(payload["ok"], false);
    assert_eq!(payload["status"], "failed");
    assert_eq!(payload["issues"][0]["code"], "compile");
    assert!(payload["issues"][0]["message"]
        .as_str()
        .unwrap_or_default()
        .contains("program compile failed"));
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
