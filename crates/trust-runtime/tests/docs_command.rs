use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}",
        std::process::id()
    ))
}

fn trust_dev_command() -> Command {
    Command::new(trust_dev_bin())
}

fn trust_runtime_command_with_dev_alias() -> Command {
    let mut command = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    );
    command.env("TRUST_DEV_BIN", trust_dev_bin());
    command
}

fn trust_dev_bin() -> std::path::PathBuf {
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

#[test]
fn docs_command_generates_markdown_and_html() {
    let project = unique_temp_dir("docs-project");
    let sources = project.join("src");
    let out_dir = project.join("generated-docs");
    std::fs::create_dir_all(&sources).expect("create src");
    std::fs::write(
        sources.join("main.st"),
        r#"
// @brief Adds one to input.
// @param IN Input value.
// @return Incremented value.
FUNCTION Increment : INT
VAR_INPUT
    IN : INT;
END_VAR
Increment := IN + INT#1;
END_FUNCTION
"#,
    )
    .expect("write source");

    let output = trust_dev_command()
        .args([
            "docs",
            "--project",
            project.to_str().expect("project path utf-8"),
            "--out-dir",
            out_dir.to_str().expect("output path utf-8"),
            "--format",
            "both",
        ])
        .output()
        .expect("run trust-dev docs");

    assert!(
        output.status.success(),
        "expected docs command success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let markdown = std::fs::read_to_string(out_dir.join("api.md")).expect("read markdown output");
    let html = std::fs::read_to_string(out_dir.join("api.html")).expect("read html output");
    assert!(markdown.contains("FUNCTION `Increment`"));
    assert!(markdown.contains("**Parameters**"));
    assert!(markdown.contains("`IN`: Input value."));
    assert!(html.contains("<h3>FUNCTION <code>Increment</code></h3>"));
    assert!(html.contains("<strong>Returns:</strong> Incremented value."));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn trust_runtime_docs_alias_forwards_to_trust_dev() {
    let project = unique_temp_dir("docs-alias-project");
    let sources = project.join("src");
    let out_dir = project.join("generated-docs");
    std::fs::create_dir_all(&sources).expect("create src");
    std::fs::write(
        sources.join("main.st"),
        r#"
// @brief Does work.
PROGRAM Main
END_PROGRAM
"#,
    )
    .expect("write source");

    let output = trust_runtime_command_with_dev_alias()
        .args([
            "docs",
            "--project",
            project.to_str().expect("project path utf-8"),
            "--out-dir",
            out_dir.to_str().expect("output path utf-8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("run trust-runtime docs alias");

    assert!(
        output.status.success(),
        "expected docs alias success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("trust-runtime docs"));
    assert!(stderr.contains("trust-dev docs"));
    assert!(stderr.contains("removed no earlier than 2026-10-05"));
    assert!(stderr.contains("separate behavior-change release"));
    assert!(out_dir.join("api.md").exists());

    let _ = std::fs::remove_dir_all(project);
}

#[cfg(unix)]
#[test]
fn docs_alias_ignores_an_unusable_sibling_and_uses_path() {
    let root = unique_temp_dir("docs-unusable-sibling");
    let runner_dir = root.join("runner");
    let project = root.join("project");
    let out_dir = root.join("generated-docs");
    std::fs::create_dir_all(&runner_dir).expect("create runner directory");
    std::fs::create_dir_all(project.join("src")).expect("create project sources");
    std::fs::write(project.join("src/main.st"), "PROGRAM Main\nEND_PROGRAM\n")
        .expect("write project source");

    let copied_runtime = runner_dir.join("trust-runtime");
    std::fs::copy(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
        &copied_runtime,
    )
    .expect("copy runtime binary");
    std::fs::create_dir(runner_dir.join("trust-dev")).expect("create unusable sibling directory");

    let trust_dev = trust_dev_bin();
    let trust_dev_dir = trust_dev.parent().expect("trust-dev binary parent");
    let mut path_entries = vec![trust_dev_dir.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        path_entries.extend(std::env::split_paths(&path));
    }
    let path = std::env::join_paths(path_entries).expect("join PATH entries");

    let output = Command::new(&copied_runtime)
        .env_remove("TRUST_DEV_BIN")
        .env("PATH", path)
        .args(["docs", "--project"])
        .arg(&project)
        .args(["--out-dir"])
        .arg(&out_dir)
        .args(["--format", "markdown"])
        .output()
        .expect("run copied runtime alias");

    assert!(
        output.status.success(),
        "an unusable sibling must not shadow PATH trust-dev:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_dir.join("api.md").is_file());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn docs_alias_maps_a_signaled_child_to_shell_status() {
    use std::os::unix::fs::PermissionsExt;

    let root = unique_temp_dir("docs-signaled-child");
    std::fs::create_dir_all(&root).expect("create signal test directory");
    let fake_trust_dev = root.join("trust-dev-signal");
    std::fs::write(&fake_trust_dev, "#!/bin/sh\nkill -TERM $$\n")
        .expect("write signaled trust-dev shim");
    let mut permissions = std::fs::metadata(&fake_trust_dev)
        .expect("read shim metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&fake_trust_dev, permissions).expect("make shim executable");

    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .env("TRUST_DEV_BIN", &fake_trust_dev)
    .arg("docs")
    .output()
    .expect("run alias with signaled child");

    assert_eq!(
        output.status.code(),
        Some(128 + 15),
        "SIGTERM must retain its conventional shell status; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn docs_alias_propagates_a_normal_child_exit_code() {
    use std::os::unix::fs::PermissionsExt;

    let root = unique_temp_dir("docs-child-exit");
    std::fs::create_dir_all(&root).expect("create child-exit test directory");
    let fake_trust_dev = root.join("trust-dev-exit");
    std::fs::write(&fake_trust_dev, "#!/bin/sh\nexit 37\n").expect("write exiting trust-dev shim");
    let mut permissions = std::fs::metadata(&fake_trust_dev)
        .expect("read shim metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&fake_trust_dev, permissions).expect("make shim executable");

    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .env("TRUST_DEV_BIN", &fake_trust_dev)
    .arg("docs")
    .output()
    .expect("run alias with non-zero child");

    assert_eq!(output.status.code(), Some(37));
    assert!(String::from_utf8_lossy(&output.stderr).contains("trust-dev docs"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn docs_alias_reports_an_unlaunchable_explicit_trust_dev() {
    let root = unique_temp_dir("docs-missing-child");
    let missing_trust_dev = root.join("missing-trust-dev");

    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .env("TRUST_DEV_BIN", &missing_trust_dev)
    .arg("docs")
    .output()
    .expect("run alias with missing explicit child");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("trust-runtime docs"));
    assert!(stderr.contains("Install `trust-dev` beside `trust-runtime` or put it on PATH"));
}
