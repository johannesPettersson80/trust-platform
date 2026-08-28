use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let sequence = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}-{sequence}",
        std::process::id()
    ))
}

#[test]
fn setup_cancel_mode_exits_successfully() {
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
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
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
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
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
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
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
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
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
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
    assert!(project.join("src").join("main.st").is_file());
    assert!(project.join("src").join("config.st").is_file());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Setup complete"));
    assert!(stdout.contains("trust-runtime --project"));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn setup_cli_treats_project_path_glob_characters_literally() {
    let project = unique_temp_dir("setup-cli-[literal]");
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("setup")
    .arg("--mode")
    .arg("cli")
    .arg("--project")
    .arg(&project)
    .output()
    .expect("run setup CLI with literal path");

    assert!(
        output.status.success(),
        "literal project path setup failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(project.join("program.stbc").is_file());

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn setup_cli_prefixes_numeric_project_resource_name() {
    let parent = unique_temp_dir("setup-cli-numeric-parent");
    std::fs::create_dir(&parent).expect("create numeric project parent");
    let project = parent.join("123");
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("setup")
    .arg("--mode")
    .arg("cli")
    .arg("--project")
    .arg(&project)
    .output()
    .expect("run setup CLI for numeric project");

    assert!(
        output.status.success(),
        "numeric project setup failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let runtime =
        std::fs::read_to_string(project.join("runtime.toml")).expect("read generated runtime.toml");
    assert!(runtime.contains("name = \"Res123\""), "{runtime}");

    let _ = std::fs::remove_dir_all(parent);
}

#[test]
fn setup_cli_rejects_browser_only_options_without_mutation() {
    let project = unique_temp_dir("setup-cli-browser-options");
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("setup")
    .arg("--mode")
    .arg("cli")
    .arg("--project")
    .arg(&project)
    .arg("--access")
    .arg("remote")
    .arg("--bind")
    .arg("0.0.0.0")
    .arg("--port")
    .arg("9000")
    .arg("--token-ttl-minutes")
    .arg("30")
    .arg("--dry-run")
    .output()
    .expect("run CLI setup with browser-only options");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("browser-only setup options require --mode browser"));
    assert!(
        !project.exists(),
        "rejected mode options must not create the project"
    );
}

#[test]
fn setup_cli_rejects_explicit_default_browser_options_without_mutation() {
    for (label, option, value) in [
        ("explicit-local-access", "--access", "local"),
        ("explicit-default-port", "--port", "8080"),
    ] {
        let project = unique_temp_dir(label);
        let output =
            Command::new(std::env::var_os("CARGO_BIN_EXE_trust-runtime").expect(
                "Cargo must provide trust-runtime binary while executing integration tests",
            ))
            .arg("setup")
            .arg("--mode")
            .arg("cli")
            .arg("--project")
            .arg(&project)
            .arg(option)
            .arg(value)
            .arg("--dry-run")
            .output()
            .expect("run CLI setup with an explicit default browser option");

        assert!(
            !output.status.success(),
            "{option} {value} must not be silently ignored in CLI mode"
        );
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("browser-only setup options require --mode browser"));
        assert!(
            !project.exists(),
            "rejected mode options must not create the project"
        );
    }
}

#[test]
fn setup_cli_dry_run_reports_plan_without_creating_project() {
    let project = unique_temp_dir("setup-cli-dry-run");
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("setup")
    .arg("--mode")
    .arg("cli")
    .arg("--project")
    .arg(&project)
    .arg("--dry-run")
    .output()
    .expect("run CLI setup dry run");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Setup dry run"));
    assert!(!project.exists(), "dry run must not create the project");
}

#[test]
fn setup_without_mode_rejects_noninteractive_input() {
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("setup")
    .output()
    .expect("run setup without a terminal");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("setup requires an interactive terminal, or explicit mode"));
}

#[test]
fn setup_rejects_system_and_guided_flag_mix_without_mutation() {
    let project = unique_temp_dir("setup-mixed-flags");
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("setup")
    .arg("--driver")
    .arg("loopback")
    .arg("--mode")
    .arg("cli")
    .arg("--project")
    .arg(&project)
    .output()
    .expect("run mixed system and guided setup");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("system setup flags"));
    assert!(!project.exists());
}

#[test]
fn wizard_rejects_noninteractive_input_without_mutation() {
    let project = unique_temp_dir("wizard-noninteractive");
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("wizard")
    .arg("--path")
    .arg(&project)
    .output()
    .expect("run wizard without a terminal");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("wizard requires an interactive terminal")
    );
    assert!(!project.exists());
}
