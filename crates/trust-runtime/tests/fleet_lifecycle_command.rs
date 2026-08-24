use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn temp_dir() -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "trust-fleet-lifecycle-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&root).expect("create fleet lifecycle test directory");
    root
}

fn free_loopback_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral loopback port")
        .local_addr()
        .expect("read ephemeral address")
        .port()
}

fn run_fleet(root: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_trust-runtime"));
    command
        .arg("fleet")
        .args(args)
        .args(["--fleet-root"])
        .arg(root)
        .arg("--json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let rendered = format!("{command:?}");
    let mut child = command.spawn().expect("spawn fleet command");
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if child.try_wait().expect("inspect fleet command").is_some() {
            return child
                .wait_with_output()
                .expect("collect fleet command output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .expect("collect timed-out fleet output");
            panic!(
                "fleet command exceeded 15 seconds: {rendered}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn assert_success(output: &Output, operation: &str) {
    assert!(
        output.status.success(),
        "{operation} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

struct RuntimeCleanup {
    root: PathBuf,
}

impl Drop for RuntimeCleanup {
    fn drop(&mut self) {
        let _ = run_fleet(&self.root, &["runtime", "stop", "--name", "cell"]);
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn managed_runtime_lifecycle_runs_through_the_shipped_cli() {
    let root = temp_dir();
    let cleanup = RuntimeCleanup { root: root.clone() };
    let control_port = free_loopback_port();
    let mut web_port = free_loopback_port();
    while web_port == control_port {
        web_port = free_loopback_port();
    }

    let add = run_fleet(
        &root,
        &[
            "runtime",
            "add",
            "--name",
            "cell",
            "--template",
            "simulate",
            "--control-port",
            &control_port.to_string(),
            "--web-port",
            &web_port.to_string(),
        ],
    );
    assert_success(&add, "fleet runtime add");
    let add: serde_json::Value = serde_json::from_slice(&add.stdout).expect("parse add JSON");
    assert_eq!(
        add.get("name").and_then(serde_json::Value::as_str),
        Some("cell")
    );
    let expected_control_endpoint = format!("tcp://127.0.0.1:{control_port}");
    assert_eq!(
        add.get("control_endpoint")
            .and_then(serde_json::Value::as_str),
        Some(expected_control_endpoint.as_str())
    );

    let list = run_fleet(&root, &["list"]);
    assert_success(&list, "fleet list");
    let list: serde_json::Value = serde_json::from_slice(&list.stdout).expect("parse list JSON");
    assert_eq!(
        list.get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    let runtimes = list
        .get("runtimes")
        .and_then(serde_json::Value::as_array)
        .expect("fleet list runtimes");
    assert_eq!(runtimes.len(), 1);
    assert_eq!(
        runtimes[0].get("name").and_then(serde_json::Value::as_str),
        Some("cell")
    );

    let start = run_fleet(&root, &["runtime", "start", "--name", "cell"]);
    assert_success(&start, "fleet runtime start");
    let start: serde_json::Value = serde_json::from_slice(&start.stdout).expect("parse start JSON");
    assert!(matches!(
        start.get("status").and_then(serde_json::Value::as_str),
        Some("running" | "starting")
    ));

    let mut running = None;
    for _ in 0..30 {
        let status = run_fleet(&root, &["runtime", "status", "--name", "cell"]);
        assert_success(&status, "fleet runtime status");
        let status: serde_json::Value =
            serde_json::from_slice(&status.stdout).expect("parse status JSON");
        if status.get("status").and_then(serde_json::Value::as_str) == Some("running") {
            running = Some(status);
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let running = running.expect("managed runtime becomes reachable");
    assert_eq!(
        running
            .get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert!(running
        .get("pid")
        .and_then(serde_json::Value::as_u64)
        .is_some());

    let logs = run_fleet(
        &root,
        &["runtime", "logs", "--name", "cell", "--lines", "5"],
    );
    assert_success(&logs, "fleet runtime logs");
    let logs: serde_json::Value = serde_json::from_slice(&logs.stdout).expect("parse logs JSON");
    assert_eq!(
        logs.get("schema_version")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        logs.get("name").and_then(serde_json::Value::as_str),
        Some("cell")
    );
    assert!(
        logs.get("lines")
            .and_then(serde_json::Value::as_array)
            .is_some(),
        "logs response must contain a JSON line array"
    );

    let stop = run_fleet(&root, &["runtime", "stop", "--name", "cell"]);
    assert_success(&stop, "fleet runtime stop");
    let stop: serde_json::Value = serde_json::from_slice(&stop.stdout).expect("parse stop JSON");
    assert_eq!(
        stop.get("status").and_then(serde_json::Value::as_str),
        Some("stopped")
    );
    assert!(
        !root.join(".trust-runtime/cell.pid").exists(),
        "confirmed stop removes the advisory PID file"
    );

    drop(cleanup);
}
