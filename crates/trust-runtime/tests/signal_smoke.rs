#![cfg(unix)]

use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use smol_str::SmolStr;
use trust_runtime::bundle_builder::build_program_stbc;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir(prefix: &str) -> PathBuf {
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
        match fs::create_dir(&dir) {
            Ok(()) => return dir,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => panic!("create temp dir {}: {err}", dir.display()),
        }
    }
    panic!("failed to allocate unique temp dir for '{prefix}'")
}

fn reserve_local_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
}

fn write_signal_project(root: &Path, control_port: u16) {
    fs::create_dir_all(root.join("src")).expect("create src dir");

    let mut runtime_toml = render_runtime_toml(&SmolStr::new("main"), 20);
    runtime_toml = runtime_toml.replace(
        "endpoint = \"unix:///tmp/trust-runtime.sock\"",
        &format!(
            "endpoint = \"tcp://127.0.0.1:{control_port}\"\nauth_token = \"signal-smoke-token\""
        ),
    );
    runtime_toml = runtime_toml.replace("mode = \"production\"", "mode = \"debug\"");
    runtime_toml = runtime_toml.replace("debug_enabled = false", "debug_enabled = true");
    runtime_toml = runtime_toml.replace(
        "[runtime.web]\nenabled = true",
        "[runtime.web]\nenabled = false",
    );
    runtime_toml = runtime_toml.replace(
        "[runtime.discovery]\nenabled = true",
        "[runtime.discovery]\nenabled = false",
    );
    runtime_toml = runtime_toml.replace(
        "[runtime.log]\nlevel = \"info\"",
        "[runtime.log]\nlevel = \"debug\"",
    );
    fs::write(root.join("runtime.toml"), runtime_toml).expect("write runtime.toml");

    let io_template = build_io_config_auto("loopback").expect("build loopback io template");
    fs::write(root.join("io.toml"), render_io_toml(&io_template)).expect("write io.toml");
    fs::write(
        root.join("src").join("main.st"),
        r#"
PROGRAM Main
VAR
    Out AT %QX0.0 : BOOL := TRUE;
END_VAR
Out := TRUE;
END_PROGRAM
"#,
    )
    .expect("write source");
    let report = build_program_stbc(root, None).expect("build program.stbc");
    assert!(report.program_path.is_file(), "program.stbc should exist");
}

fn spawn_runtime(project: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("run")
        .arg("--project")
        .arg(project)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime run")
}

fn wait_for_control_port(mut child: Child, port: u16) -> Child {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("poll child") {
            let output = child.wait_with_output().expect("collect child output");
            panic!(
                "runtime exited before control port opened: {status}; stdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return child;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("runtime control port 127.0.0.1:{port} did not open before timeout");
}

fn send_signal(pid: u32, signal: &str) {
    let status = Command::new("kill")
        .arg(format!("-{signal}"))
        .arg(pid.to_string())
        .status()
        .expect("send signal with kill");
    assert!(
        status.success(),
        "kill -{signal} {pid} failed with {status}"
    );
}

fn wait_for_exit(mut child: Child, signal: &str) -> Output {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if child.try_wait().expect("poll child").is_some() {
            return child.wait_with_output().expect("collect child output");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let output = child
        .wait_with_output()
        .expect("collect killed child output");
    panic!(
        "runtime did not exit within timeout after SIG{signal}; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_graceful_signal_stop(signal: &str) {
    let project = unique_temp_dir(&format!("signal-{}", signal.to_ascii_lowercase()));
    let control_port = reserve_local_port();
    write_signal_project(&project, control_port);

    let child = wait_for_control_port(spawn_runtime(&project), control_port);
    send_signal(child.id(), signal);
    let output = wait_for_exit(child, signal);

    assert!(
        output.status.success(),
        "runtime should exit successfully after SIG{signal}; status={}; stdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let structured = stdout
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).unwrap_or_else(|err| {
                panic!("structured log line must be JSON: {err}; line={line}")
            })
        })
        .collect::<Vec<_>>();
    assert!(
        !structured.is_empty(),
        "runtime must emit structured lifecycle events; stdout:\n{stdout}"
    );
    for event in &structured {
        assert!(
            event["ts"].as_u64().is_some(),
            "structured event must carry a Unix-millisecond timestamp: {event}"
        );
        assert!(
            matches!(
                event["level"].as_str(),
                Some("error" | "warn" | "info" | "debug" | "trace")
            ),
            "structured event must carry a canonical level: {event}"
        );
        assert!(
            event["event"].as_str().is_some(),
            "structured event must carry an event name: {event}"
        );
        assert!(
            event.get("data").is_some(),
            "structured event must carry event data: {event}"
        );
    }
    let exit = structured
        .iter()
        .find(|event| event["event"] == "runtime_exit")
        .unwrap_or_else(|| {
            panic!("runtime must report the ordinary stopped exit path; stdout:\n{stdout}")
        });
    assert_eq!(exit["level"], "debug");
    assert_eq!(exit["data"]["status"], "stopped");
    assert!(
        !stdout.contains("runtime_safe_state_failed"),
        "safe-state application must not fail during SIG{signal} shutdown; stdout:\n{stdout}"
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn sigint_requests_bounded_graceful_stop_in_child_runtime() {
    assert_graceful_signal_stop("INT");
}

#[test]
fn sigterm_requests_bounded_graceful_stop_in_child_runtime() {
    assert_graceful_signal_stop("TERM");
}
