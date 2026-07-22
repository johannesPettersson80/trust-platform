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
    Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("ctl")
        .args(args)
        .env_remove("TRUST_CTL_TOKEN")
        .output()
        .expect("run trust-runtime ctl")
}

fn write_project_with_control_token(root: &Path, token: &str) {
    let runtime = render_runtime_toml(&SmolStr::new("ctl-test"), 10).replacen(
        "endpoint = \"unix:///tmp/trust-runtime.sock\"",
        &format!("endpoint = \"tcp://127.0.0.1:1\"\nauth_token = \"{token}\""),
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

    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
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
