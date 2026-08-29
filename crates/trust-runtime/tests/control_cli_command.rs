use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime::bundle_template::render_runtime_toml;

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
    std::fs::create_dir(&path).expect("create control CLI temp directory");
    path
}

fn spawn_control_server(response: Value) -> (String, Receiver<Value>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind control test server");
    let endpoint = format!(
        "tcp://{}",
        listener.local_addr().expect("control test server address")
    );
    let (request_tx, request_rx) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept control client");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set control test read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("set control test write timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone control test stream"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read control request");
        let request = serde_json::from_str(line.trim_end()).expect("parse control request");
        request_tx.send(request).expect("send captured request");
        writeln!(stream, "{}", serde_json::to_string(&response).unwrap())
            .expect("write control response");
    });
    (endpoint, request_rx, server)
}

fn run_ctl(args: &[&str]) -> Output {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("ctl")
    .args(args)
    .env_remove("TRUST_CTL_TOKEN")
    .output()
    .expect("run trust-runtime ctl")
}

fn write_project_with_control_token(root: &Path, token: &str) {
    write_project_with_control_target(root, "tcp://127.0.0.1:1", token);
}

fn write_project_with_control_target(root: &Path, endpoint: &str, token: &str) {
    let runtime = render_runtime_toml(&SmolStr::new("ctl-test"), 10).replacen(
        "endpoint = \"unix:///tmp/trust-runtime.sock\"",
        &format!("endpoint = \"{endpoint}\"\nauth_token = \"{token}\""),
        1,
    );
    std::fs::write(root.join("runtime.toml"), runtime).expect("write runtime config");
}

#[test]
fn ctl_status_sends_authenticated_request_and_prints_summary() {
    let (endpoint, request_rx, server) = spawn_control_server(json!({
        "id": 1,
        "ok": true,
        "result": { "state": "running" }
    }));

    let output = run_ctl(&["--endpoint", &endpoint, "--token", "cli-token", "status"]);
    let request = request_rx.recv().expect("captured control request");
    server.join().expect("control test server");

    assert!(
        output.status.success(),
        "ctl status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "state=running fault=none rt_profile=disabled rt_active=false"
    );
    assert_eq!(
        request,
        json!({ "id": 1, "type": "status", "auth": "cli-token" })
    );
}

#[test]
fn ctl_endpoint_override_preserves_project_token() {
    let project = unique_temp_dir("ctl-project-token");
    write_project_with_control_token(&project, "project-token");
    let (endpoint, request_rx, server) = spawn_control_server(json!({
        "id": 1,
        "ok": true,
        "result": { "ok": true }
    }));

    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("ctl")
    .arg("--project")
    .arg(&project)
    .args(["--endpoint", &endpoint, "health"])
    .env_remove("TRUST_CTL_TOKEN")
    .output()
    .expect("run trust-runtime ctl with project token");
    let request = request_rx.recv().expect("captured control request");
    server.join().expect("control test server");

    assert!(
        output.status.success(),
        "ctl health failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(request["auth"], json!("project-token"));
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn ctl_returns_nonzero_for_rejected_control_response() {
    let (endpoint, request_rx, server) = spawn_control_server(json!({
        "id": 1,
        "ok": false,
        "error": "insufficient role for shutdown",
        "error_code": "insufficient_role"
    }));

    let output = run_ctl(&["--endpoint", &endpoint, "shutdown"]);
    let _request = request_rx.recv().expect("captured control request");
    server.join().expect("control test server");

    assert!(
        !output.status.success(),
        "rejected control command must fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("insufficient role for shutdown"),
        "{stderr}"
    );
    assert!(stderr.contains("insufficient_role"), "{stderr}");
}

#[test]
fn ctl_returns_nonzero_when_control_response_omits_ok() {
    let (endpoint, request_rx, server) = spawn_control_server(json!({
        "id": 1,
        "result": { "state": "running" }
    }));

    let output = run_ctl(&["--endpoint", &endpoint, "status"]);
    let _request = request_rx.recv().expect("captured control request");
    server.join().expect("control test server");

    assert!(
        !output.status.success(),
        "malformed control response must fail"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing boolean 'ok'"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ctl_returns_nonzero_for_malformed_success_results() {
    let cases = [
        (
            "status",
            json!({"id": 1, "ok": true, "result": {}}),
            "status result missing string 'state'",
        ),
        (
            "health",
            json!({"id": 1, "ok": true, "result": {"ok": "yes"}}),
            "health result missing boolean 'ok'",
        ),
        (
            "stats",
            json!({
                "id": 1,
                "ok": true,
                "result": {"tasks": [{"name": "Main"}]}
            }),
            "task result missing numeric 'min_ms'",
        ),
    ];

    for (action, response, expected_error) in cases {
        let (endpoint, request_rx, server) = spawn_control_server(response);
        let output = run_ctl(&["--endpoint", &endpoint, action]);
        let _request = request_rx
            .recv()
            .expect("captured malformed-result request");
        server.join().expect("malformed-result control server");

        assert!(
            !output.status.success(),
            "{action} malformed success result must fail: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn ctl_target_resolution_enforces_endpoint_and_token_precedence() {
    let project = unique_temp_dir("ctl-target-precedence");
    let (project_endpoint, project_request_rx, project_server) =
        spawn_control_server(json!({"id": 1, "ok": true, "result": {"ok": true}}));
    write_project_with_control_target(&project, &project_endpoint, "project-token");

    let project_output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("ctl")
    .arg("--project")
    .arg(&project)
    .arg("health")
    .env("TRUST_CTL_TOKEN", "environment-token")
    .output()
    .expect("run ctl with project endpoint and environment token");
    let project_request = project_request_rx.recv().expect("captured project request");
    project_server.join().expect("project control server");
    assert!(
        project_output.status.success(),
        "{}",
        String::from_utf8_lossy(&project_output.stderr)
    );
    assert_eq!(project_request["auth"], json!("environment-token"));
    assert_eq!(
        String::from_utf8_lossy(&project_output.stdout).trim(),
        "ok=true"
    );

    let (explicit_endpoint, explicit_request_rx, explicit_server) =
        spawn_control_server(json!({"id": 1, "ok": true, "result": {"ok": true}}));
    let explicit_output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("ctl")
    .arg("--project")
    .arg(&project)
    .args([
        "--endpoint",
        &explicit_endpoint,
        "--token",
        "explicit-token",
        "health",
    ])
    .env("TRUST_CTL_TOKEN", "environment-token")
    .output()
    .expect("run ctl with explicit target");
    let explicit_request = explicit_request_rx
        .recv()
        .expect("captured explicit request");
    explicit_server.join().expect("explicit control server");
    assert!(
        explicit_output.status.success(),
        "{}",
        String::from_utf8_lossy(&explicit_output.stderr)
    );
    assert_eq!(explicit_request["auth"], json!("explicit-token"));

    let malformed_project = unique_temp_dir("ctl-malformed-unused-project");
    std::fs::write(malformed_project.join("runtime.toml"), "not = [valid")
        .expect("write malformed unused project");
    let (bypass_endpoint, bypass_request_rx, bypass_server) =
        spawn_control_server(json!({"id": 1, "ok": true, "result": {"ok": true}}));
    let bypass_output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("ctl")
    .arg("--project")
    .arg(&malformed_project)
    .args([
        "--endpoint",
        &bypass_endpoint,
        "--token",
        "resolved-token",
        "health",
    ])
    .env_remove("TRUST_CTL_TOKEN")
    .output()
    .expect("run ctl without loading resolved project");
    let bypass_request = bypass_request_rx.recv().expect("captured bypass request");
    bypass_server.join().expect("bypass control server");
    assert!(
        bypass_output.status.success(),
        "{}",
        String::from_utf8_lossy(&bypass_output.stderr)
    );
    assert_eq!(bypass_request["auth"], json!("resolved-token"));

    let (environment_endpoint, environment_request_rx, environment_server) =
        spawn_control_server(json!({"id": 1, "ok": true, "result": {"ok": true}}));
    let environment_output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("ctl")
    .arg("--project")
    .arg(&malformed_project)
    .args(["--endpoint", &environment_endpoint, "health"])
    .env("TRUST_CTL_TOKEN", "environment-token")
    .output()
    .expect("run ctl without loading project after environment token resolution");
    let environment_request = environment_request_rx
        .recv()
        .expect("captured environment-token request");
    environment_server
        .join()
        .expect("environment-token control server");
    assert!(
        environment_output.status.success(),
        "{}",
        String::from_utf8_lossy(&environment_output.stderr)
    );
    assert_eq!(environment_request["auth"], json!("environment-token"));

    let missing_target = run_ctl(&["status"]);
    assert!(!missing_target.status.success());
    assert!(
        String::from_utf8_lossy(&missing_target.stderr)
            .contains("--endpoint or --project required"),
        "{}",
        String::from_utf8_lossy(&missing_target.stderr)
    );

    let _ = std::fs::remove_dir_all(project);
    let _ = std::fs::remove_dir_all(malformed_project);
}

#[test]
fn ctl_stats_renders_each_task_in_stable_compact_form() {
    let (endpoint, request_rx, server) = spawn_control_server(json!({
        "id": 1,
        "ok": true,
        "result": {
            "tasks": [{
                "name": "Main",
                "min_ms": 1.0,
                "avg_ms": 2.25,
                "max_ms": 3.5,
                "last_ms": 2.0,
                "overruns": 4
            }]
        }
    }));

    let output = run_ctl(&["--endpoint", &endpoint, "stats"]);
    let request = request_rx.recv().expect("captured stats request");
    server.join().expect("stats control server");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        request,
        json!({"id": 1, "type": "tasks.stats", "auth": null})
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "task=Main min_ms=1.000 avg_ms=2.250 max_ms=3.500 last_ms=2.000 overruns=4"
    );
}
