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

#[test]
fn setup_cancel_mode_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["setup", "--mode", "cancel"])
        .output()
        .expect("run trust-runtime setup cancel");

    assert!(
        output.status.success(),
        "expected setup cancel success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Setup cancelled"));
}

#[test]
fn setup_browser_local_rejects_non_loopback_bind() {
    let project = unique_temp_dir("setup-local-bind");
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("setup")
        .arg("--mode")
        .arg("browser")
        .arg("--access")
        .arg("local")
        .arg("--project")
        .arg(&project)
        .arg("--bind")
        .arg("0.0.0.0")
        .arg("--dry-run")
        .output()
        .expect("run setup browser local");

    assert!(
        !output.status.success(),
        "expected setup browser local non-loopback failure"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("loopback bind"), "stderr was:\n{stderr}");
}

#[test]
fn setup_browser_remote_rejects_loopback_bind() {
    let project = unique_temp_dir("setup-remote-bind");
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("setup")
        .arg("--mode")
        .arg("browser")
        .arg("--access")
        .arg("remote")
        .arg("--project")
        .arg(&project)
        .arg("--bind")
        .arg("127.0.0.1")
        .arg("--token-ttl-minutes")
        .arg("15")
        .arg("--dry-run")
        .output()
        .expect("run setup browser remote");

    assert!(
        !output.status.success(),
        "expected setup browser remote loopback failure"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("must not use a loopback bind"),
        "stderr was:\n{stderr}"
    );
}

#[test]
fn setup_browser_remote_dry_run_shows_token_requirements() {
    let project = unique_temp_dir("setup-remote-dry-run");
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("setup")
        .arg("--mode")
        .arg("browser")
        .arg("--access")
        .arg("remote")
        .arg("--project")
        .arg(&project)
        .arg("--token-ttl-minutes")
        .arg("30")
        .arg("--dry-run")
        .output()
        .expect("run setup browser remote dry-run");

    assert!(
        output.status.success(),
        "expected setup browser remote dry-run success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Token required: yes"));
    assert!(stdout.contains("Token TTL (minutes): 30"));
}

#[test]
fn setup_cli_mode_writes_artifacts_and_next_steps() {
    let project = unique_temp_dir("setup-cli-project");
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("setup")
        .arg("--mode")
        .arg("cli")
        .arg("--project")
        .arg(&project)
        .output()
        .expect("run setup cli mode");

    assert!(
        output.status.success(),
        "expected setup cli success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.join("runtime.toml").is_file());
    assert!(project.join("io.toml").is_file());
    assert!(project.join("program.stbc").is_file());
    assert!(project.join("sources").join("main.st").is_file());
    assert!(project.join("sources").join("config.st").is_file());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Setup complete"));
    assert!(stdout.contains("trust-runtime --project"));

    let _ = std::fs::remove_dir_all(project);
}
