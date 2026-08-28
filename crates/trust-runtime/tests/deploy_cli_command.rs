use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use smol_str::SmolStr;
use trust_runtime::bundle_builder::build_program_stbc;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};
use trust_runtime::config::RuntimeBundle;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&path).expect("create deploy CLI temp directory");
    path
}

fn portable_runtime_toml() -> String {
    let runtime = render_runtime_toml(&SmolStr::new("deploy-test"), 10);
    #[cfg(windows)]
    {
        return runtime.replacen(
            "endpoint = \"unix:///tmp/trust-runtime.sock\"",
            "endpoint = \"tcp://127.0.0.1:19091\"\nauth_token = \"deploy-test-token\"",
            1,
        );
    }
    #[cfg(not(windows))]
    runtime
}

fn write_bundle(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("create source directory");
    std::fs::write(root.join("runtime.toml"), portable_runtime_toml()).expect("write runtime.toml");
    let io = render_io_toml(&build_io_config_auto("loopback").expect("build loopback config"));
    std::fs::write(root.join("io.toml"), io).expect("write io.toml");
    std::fs::write(
        root.join("src/main.st"),
        "PROGRAM Main\nVAR\n    Counter : DINT;\nEND_VAR\nCounter := Counter + 1;\nEND_PROGRAM\n",
    )
    .expect("write source");
    build_program_stbc(root, None).expect("build deploy fixture bytecode");
}

fn enable_nested_protocol_sidecars(root: &Path) {
    let runtime_path = root.join("runtime.toml");
    let runtime = std::fs::read_to_string(&runtime_path)
        .expect("read runtime.toml")
        .replacen(
            "[runtime.opcua_client]\nenabled = false\nconfig_path = \"opcua_client.toml\"",
            "[runtime.opcua_client]\nenabled = true\nconfig_path = \"config/opcua_client.toml\"",
            1,
        );
    std::fs::write(
        &runtime_path,
        format!("{runtime}\n[runtime.ads]\nenabled = true\nconfig_path = \"config/ads.toml\"\n"),
    )
    .expect("enable protocol sidecars");
    std::fs::create_dir_all(root.join("config")).expect("create sidecar directory");
    std::fs::write(
        root.join("config/ads.toml"),
        r#"
[[connections]]
name = "line1"
target_net_id = "5.23.91.12.1.1"
host = "192.168.10.5"
ams_port = 851
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Temperature"
var = "line1_temp"
type = "REAL"
"#,
    )
    .expect("write ADS sidecar");
    std::fs::write(
        root.join("config/opcua_client.toml"),
        r#"
[[connections]]
name = "line1"
endpoint_url = "opc.tcp://127.0.0.1:4840/trust"
security_policy = "none"
security_mode = "none"
auth = "anonymous"
trust_server_certificate = false

[[connections.points]]
var = "conveyor_speed"
node_id = "ns=2;s=MAIN.conveyor_speed"
type = "REAL"
access = "read"
"#,
    )
    .expect("write OPC UA client sidecar");
}

fn run_deploy(working_dir: &Path, project: &Path, root: &str, label: &str) -> Output {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .current_dir(working_dir)
    .args(["deploy", "--project"])
    .arg(project)
    .args(["--root", root, "--label", label])
    .output()
    .expect("run trust-runtime deploy")
}

fn assert_success(output: &Output, command: &str) {
    assert!(
        output.status.success(),
        "{command} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn resolved_link(path: &Path) -> PathBuf {
    std::fs::canonicalize(path)
        .unwrap_or_else(|error| panic!("resolve deployment link {}: {error}", path.display()))
}

#[test]
fn deploy_with_relative_root_keeps_current_and_immediate_previous_for_rollback() {
    let working_dir = unique_temp_dir("deploy-relative-root");
    let project = working_dir.join("source-project");
    let deploy_root = working_dir.join("deploy-root");
    write_bundle(&project);

    for label in ["version-1", "version-2", "version-3"] {
        let output = run_deploy(&working_dir, &project, "deploy-root", label);
        assert_success(&output, &format!("deploy {label}"));
    }

    assert_eq!(
        resolved_link(&deploy_root.join("current")),
        std::fs::canonicalize(deploy_root.join("bundles/version-3")).unwrap()
    );
    assert_eq!(
        resolved_link(&deploy_root.join("previous")),
        std::fs::canonicalize(deploy_root.join("bundles/version-2")).unwrap()
    );
    assert!(!deploy_root.join("bundles/version-1").exists());

    let rollback = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .current_dir(&working_dir)
    .args(["rollback", "--root", "deploy-root"])
    .output()
    .expect("run trust-runtime rollback");
    assert_success(&rollback, "rollback");
    assert_eq!(
        resolved_link(&deploy_root.join("current")),
        std::fs::canonicalize(deploy_root.join("bundles/version-2")).unwrap()
    );
    assert_eq!(
        resolved_link(&deploy_root.join("previous")),
        std::fs::canonicalize(deploy_root.join("bundles/version-3")).unwrap()
    );

    let reused = run_deploy(&working_dir, &project, "deploy-root", "version-1");
    assert_success(&reused, "reuse pruned deployment label");
    assert_eq!(
        resolved_link(&deploy_root.join("current")),
        std::fs::canonicalize(deploy_root.join("bundles/version-1")).unwrap()
    );
    assert!(
        deploy_root.join("deployments/version-1.txt").is_file(),
        "reused label retains its successful summary"
    );

    let _ = std::fs::remove_dir_all(working_dir);
}

#[test]
fn deploy_rejects_labels_that_escape_the_bundles_directory() {
    let working_dir = unique_temp_dir("deploy-label-containment");
    let project = working_dir.join("source-project");
    write_bundle(&project);

    let output = run_deploy(&working_dir, &project, "deploy-root", "../escaped");

    assert!(!output.status.success(), "escaping deploy label must fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("deployment label"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!working_dir.join("deploy-root/escaped").exists());
    let _ = std::fs::remove_dir_all(working_dir);
}

#[test]
fn deploy_rejects_invalid_bytecode_before_changing_pointers() {
    let working_dir = unique_temp_dir("deploy-invalid-bytecode");
    let project = working_dir.join("source-project");
    write_bundle(&project);
    std::fs::write(project.join("program.stbc"), b"not-bytecode")
        .expect("corrupt deploy fixture bytecode");

    let output = run_deploy(&working_dir, &project, "deploy-root", "invalid-bytecode");

    assert!(
        !output.status.success(),
        "invalid bundle must fail deployment"
    );
    assert!(!working_dir.join("deploy-root/current").exists());
    assert!(!working_dir.join("deploy-root/previous").exists());
    assert!(!working_dir
        .join("deploy-root/bundles/invalid-bytecode")
        .exists());
    let _ = std::fs::remove_dir_all(working_dir);
}

#[cfg(unix)]
#[test]
fn deploy_recovers_from_a_dangling_current_pointer() {
    let working_dir = unique_temp_dir("deploy-dangling-current");
    let project = working_dir.join("source-project");
    let deploy_root = working_dir.join("deploy-root");
    write_bundle(&project);
    std::fs::create_dir_all(&deploy_root).expect("create deploy root");
    std::os::unix::fs::symlink("bundles/missing", deploy_root.join("current"))
        .expect("create dangling current pointer");

    let output = run_deploy(&working_dir, &project, "deploy-root", "version-1");

    assert_success(&output, "deploy over dangling current pointer");
    assert_eq!(
        resolved_link(&deploy_root.join("current")),
        std::fs::canonicalize(deploy_root.join("bundles/version-1")).unwrap()
    );
    assert!(!deploy_root.join("previous").exists());
    let _ = std::fs::remove_dir_all(working_dir);
}

#[cfg(unix)]
#[test]
fn deploy_rejects_a_current_pointer_outside_its_bundle_store() {
    let working_dir = unique_temp_dir("deploy-external-current");
    let project = working_dir.join("source-project");
    let deploy_root = working_dir.join("deploy-root");
    let external = working_dir.join("external-bundle");
    write_bundle(&project);
    std::fs::create_dir_all(&deploy_root).expect("create deploy root");
    std::fs::create_dir_all(&external).expect("create external target");
    std::os::unix::fs::symlink(&external, deploy_root.join("current"))
        .expect("create external current pointer");

    let output = run_deploy(&working_dir, &project, "deploy-root", "version-1");

    assert!(
        !output.status.success(),
        "external current pointer must fail"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("outside deployment bundle store"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(external.is_dir(), "external target must not be removed");
    assert!(!deploy_root.join("bundles/version-1").exists());
    let _ = std::fs::remove_dir_all(working_dir);
}

#[test]
fn deployment_summary_reports_current_bundle_and_any_runtime_file_change() {
    let working_dir = unique_temp_dir("deploy-summary");
    let project = working_dir.join("source-project");
    let deploy_root = working_dir.join("deploy-root");
    write_bundle(&project);

    let first = run_deploy(&working_dir, &project, "deploy-root", "version-1");
    assert_success(&first, "deploy version-1");

    let runtime_path = project.join("runtime.toml");
    let runtime = std::fs::read_to_string(&runtime_path).expect("read runtime.toml");
    assert!(runtime.contains("listen = \"0.0.0.0:8080\""));
    std::fs::write(
        &runtime_path,
        runtime.replacen("listen = \"0.0.0.0:8080\"", "listen = \"0.0.0.0:8081\"", 1),
    )
    .expect("change an untracked runtime setting");

    let second = run_deploy(&working_dir, &project, "deploy-root", "version-2");
    assert_success(&second, "deploy version-2");

    let summary = std::fs::read_to_string(deploy_root.join("deployments/version-2.txt"))
        .expect("read deployment summary");
    let deployed = std::fs::canonicalize(deploy_root.join("bundles/version-2"))
        .expect("canonical deployed bundle");
    assert!(
        summary.contains(&format!("current project version: {}", deployed.display())),
        "summary must identify the deployed bundle:\n{summary}"
    );
    assert!(
        summary.contains("runtime.toml changes:") && !summary.contains("runtime.toml: unchanged"),
        "every runtime.toml content change must be visible:\n{summary}"
    );

    let _ = std::fs::remove_dir_all(working_dir);
}

#[test]
fn deploy_rejects_invalid_previous_slot_without_partial_state_change() {
    let working_dir = unique_temp_dir("deploy-previous-slot");
    let project = working_dir.join("source-project");
    let deploy_root = working_dir.join("deploy-root");
    write_bundle(&project);

    let first = run_deploy(&working_dir, &project, "deploy-root", "version-1");
    assert_success(&first, "deploy version-1");
    let version_1 = resolved_link(&deploy_root.join("current"));
    let first_summary =
        std::fs::read(deploy_root.join("deployments/last.txt")).expect("read first summary");
    std::fs::write(
        deploy_root.join("previous"),
        b"operator-owned pointer slot\n",
    )
    .expect("occupy previous pointer slot");

    let second = run_deploy(&working_dir, &project, "deploy-root", "version-2");

    assert!(
        !second.status.success(),
        "an invalid previous pointer slot must reject deployment"
    );
    assert_eq!(resolved_link(&deploy_root.join("current")), version_1);
    assert_eq!(
        std::fs::read(deploy_root.join("previous")).expect("read pointer-slot file"),
        b"operator-owned pointer slot\n"
    );
    assert_eq!(
        std::fs::read(deploy_root.join("deployments/last.txt")).expect("read last summary"),
        first_summary
    );
    assert!(!deploy_root.join("deployments/version-2.txt").exists());
    assert!(!deploy_root.join("bundles/version-2").exists());
    let _ = std::fs::remove_dir_all(working_dir);
}

#[test]
fn deploy_copies_enabled_protocol_sidecars_into_self_contained_bundle() {
    let working_dir = unique_temp_dir("deploy-protocol-sidecars");
    let project = working_dir.join("source-project");
    let deploy_root = working_dir.join("deploy-root");
    write_bundle(&project);
    enable_nested_protocol_sidecars(&project);
    let expected_ads =
        std::fs::read(project.join("config/ads.toml")).expect("read source ADS sidecar");
    let expected_opcua = std::fs::read(project.join("config/opcua_client.toml"))
        .expect("read source OPC UA sidecar");

    let output = run_deploy(&working_dir, &project, "deploy-root", "version-1");
    assert_success(&output, "deploy bundle with protocol sidecars");

    let deployed = resolved_link(&deploy_root.join("current"));
    assert_eq!(
        std::fs::read(deployed.join("config/ads.toml")).expect("read deployed ADS sidecar"),
        expected_ads
    );
    assert_eq!(
        std::fs::read(deployed.join("config/opcua_client.toml"))
            .expect("read deployed OPC UA sidecar"),
        expected_opcua
    );
    std::fs::remove_dir_all(&project).expect("remove source project");
    RuntimeBundle::load(&deployed).expect("deployed bundle remains self-contained");
    let _ = std::fs::remove_dir_all(working_dir);
}

#[cfg(unix)]
#[test]
fn deploy_rejects_symlinks_in_the_copied_bundle_closure() {
    let working_dir = unique_temp_dir("deploy-source-symlink");
    let project = working_dir.join("source-project");
    let deploy_root = working_dir.join("deploy-root");
    write_bundle(&project);
    std::fs::write(
        project.join("linked-source.st"),
        "PROGRAM Linked END_PROGRAM\n",
    )
    .expect("write symlink target");
    std::os::unix::fs::symlink("../linked-source.st", project.join("src/linked-source.st"))
        .expect("create source symlink");

    let output = run_deploy(&working_dir, &project, "deploy-root", "version-1");

    assert!(!output.status.success(), "bundle symlink must be rejected");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("symbolic link"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!deploy_root.join("current").exists());
    assert!(!deploy_root.join("bundles/version-1").exists());
    let _ = std::fs::remove_dir_all(working_dir);
}

#[cfg(unix)]
#[test]
fn deploy_rejects_symlinked_sidecar_parent_directories() {
    let working_dir = unique_temp_dir("deploy-sidecar-parent-symlink");
    let project = working_dir.join("source-project");
    let deploy_root = working_dir.join("deploy-root");
    let external_config = working_dir.join("external-config");
    write_bundle(&project);
    enable_nested_protocol_sidecars(&project);
    std::fs::rename(project.join("config"), &external_config)
        .expect("move sidecars outside project");
    std::os::unix::fs::symlink(&external_config, project.join("config"))
        .expect("create sidecar parent symlink");

    let output = run_deploy(&working_dir, &project, "deploy-root", "version-1");

    assert!(
        !output.status.success(),
        "symlinked sidecar parent must be rejected"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("symbolic link"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!deploy_root.join("current").exists());
    assert!(!deploy_root.join("bundles/version-1").exists());
    let _ = std::fs::remove_dir_all(working_dir);
}

#[cfg(unix)]
#[test]
fn deploy_without_a_current_bundle_clears_a_stale_previous_pointer() {
    let working_dir = unique_temp_dir("deploy-stale-previous");
    let project = working_dir.join("source-project");
    let deploy_root = working_dir.join("deploy-root");
    let stale = deploy_root.join("bundles/stale");
    write_bundle(&project);
    std::fs::create_dir_all(&stale).expect("create stale bundle directory");
    std::os::unix::fs::symlink(&stale, deploy_root.join("previous"))
        .expect("create stale previous pointer");

    let output = run_deploy(&working_dir, &project, "deploy-root", "version-1");
    assert_success(&output, "deploy without a current bundle");

    assert_eq!(
        resolved_link(&deploy_root.join("current")),
        std::fs::canonicalize(deploy_root.join("bundles/version-1")).unwrap()
    );
    let previous_error = std::fs::symlink_metadata(deploy_root.join("previous"))
        .expect_err("previous pointer must be removed when there was no current bundle");
    assert_eq!(previous_error.kind(), std::io::ErrorKind::NotFound);
    let _ = std::fs::remove_dir_all(working_dir);
}

#[cfg(unix)]
#[test]
fn deploy_rejects_dangling_optional_bundle_symlinks() {
    for entry in ["simulation.toml", "src"] {
        let working_dir = unique_temp_dir("deploy-dangling-optional-symlink");
        let project = working_dir.join("source-project");
        let deploy_root = working_dir.join("deploy-root");
        write_bundle(&project);
        let path = project.join(entry);
        if path.is_dir() {
            std::fs::remove_dir_all(&path).expect("remove optional source directory");
        }
        std::os::unix::fs::symlink(project.join("missing-entry"), &path)
            .expect("create dangling optional symlink");

        let output = run_deploy(&working_dir, &project, "deploy-root", "version-1");

        assert!(
            !output.status.success(),
            "dangling optional symlink {entry} must be rejected"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("symbolic link"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!deploy_root.join("current").exists());
        assert!(!deploy_root.join("bundles/version-1").exists());
        let _ = std::fs::remove_dir_all(working_dir);
    }
}
