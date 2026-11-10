//! What the real-runtime tests of Control Studio's engine share: finding the `trust-runtime` executable, building a
//! project with it, starting `trust-runtime run --simulation` on a copy of an example project, waiting until it answers
//! and has scanned, and the guards that end the runtime process and remove a test's folder even when an assertion fails.
//!
//! `control_studio_runtime.rs` (checklist V0.7) and `control_studio_identify.rs` (V1.6) both include this module with
//! `mod common;`. Every item below is used by both files, so the module needs no `allow(dead_code)`. The code was moved
//! from `control_studio_runtime.rs` as it was; only the items became `pub`.

// Each test crate includes this module and uses a part of it.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use trust_control::loop_definition::LoopDefinition;
use trust_control::runtime::{RuntimeConnection, RuntimeStatus};
use trust_control::signals::{Inspection, InspectionRequest, InspectionSample, SignalBinding};
use trust_control_studio::runtime_client::RuntimeClient;

/// The helpers of the engine's real-runtime tests of stages V4.6 to V4.8 (a lab on a copy of a project, the command line, findings).
pub mod engine;

/// Lesson 30's heater as the real-runtime journeys drive it: its model, its transfers and the wait for its operating point.
pub mod lesson_30;

/// The control socket of a test project: `.control.sock` in the project while that path fits a socket address (100 bytes,
/// under Linux's 108 and macOS's 104); under a long `TMPDIR`, such as the release guard's in its build folder, a short name
/// in `TMPDIR` itself, one for each process and call.
pub fn control_socket(project: &Path) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let socket = project.join(".control.sock");
    if socket.as_os_str().len() <= 100 {
        return socket;
    }
    let short = trust_test_support::unique_temp_path(&format!(
        "tcs-{}-{}.sock",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(
        short.as_os_str().len() <= 100,
        "the socket path {} is too long; use a shorter TMPDIR",
        short.display()
    );
    short
}

/// A known control token for the test runtime. It is not a credential.
pub const TOKEN: &str = "control-studio-runtime-test-token";
/// How long a test waits for the runtime to answer, to scan and to record.
pub const DEADLINE: Duration = Duration::from_secs(60);

/// A `trust-runtime run` child; dropping it kills and reaps the process.
pub struct Runtime(Child);

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Removes the test's folder when the test ends.
pub struct RemoveDir(pub PathBuf);

impl Drop for RemoveDir {
    /// Removes the folder, except while a failed test unwinds: its outputs are then the evidence of the failure, so the folder is
    /// kept and named on the standard error.
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("kept the folder of the failed test: {}", self.0.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Inspects until the runtime has a complete scan (E3); the first answer may be pending.
pub fn inspect_until_available(
    client: &mut RuntimeClient,
    loop_definition: &LoopDefinition,
    bindings: &[SignalBinding],
    program_revision: &str,
) -> InspectionSample {
    let request = InspectionRequest::new(
        &loop_definition.revision,
        Some(program_revision),
        bindings.to_vec(),
    )
    .expect("a valid inspection");
    let deadline = Instant::now() + DEADLINE;
    loop {
        match client.inspect(&request).expect("inspection") {
            Inspection::Available(sample) => return sample,
            Inspection::Pending { reason, .. } => assert!(
                Instant::now() < deadline,
                "no scan within {} s: {reason}",
                DEADLINE.as_secs()
            ),
        }
        sleep(Duration::from_millis(100));
    }
}

/// Waits until the runtime answers `status` (E1.3) and returns the answer.
pub fn wait_for_status(connection: &RuntimeConnection, log: &Path) -> RuntimeStatus {
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Ok(status) =
            RuntimeClient::connect(connection).and_then(|mut client| client.status())
        {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "the runtime did not answer status; its log:\n{}",
            fs::read_to_string(log).unwrap_or_default()
        );
        sleep(Duration::from_millis(200));
    }
}

/// The port of the runtime's own web server, which a project configured with port 0 binds where the operating system
/// chooses (specification 22: a test reads the port a program bound rather than reserve one), as the runtime reports it in
/// its `fleet.topology` answer on its control socket once the listener is bound (specification 11, fleet topology).
#[cfg(unix)]
pub fn bound_web_port(socket: &Path, log: &Path) -> u16 {
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Some(port) = fleet_topology(socket)
            .as_ref()
            .and_then(bound_web_address)
            .and_then(|address| address.rsplit(':').next()?.parse().ok())
        {
            return port;
        }
        assert!(
            Instant::now() < deadline,
            "the runtime reported no bound web address; its log:\n{}",
            fs::read_to_string(log).unwrap_or_default()
        );
        sleep(Duration::from_millis(100));
    }
}

/// The result of one `fleet.topology` request on the control socket at `socket`, which has no token; none while the runtime
/// does not answer.
#[cfg(unix)]
fn fleet_topology(socket: &Path) -> Option<serde_json::Value> {
    use std::io::{BufRead, BufReader, Write};

    let mut stream = std::os::unix::net::UnixStream::connect(socket).ok()?;
    stream
        .write_all(b"{\"id\":1,\"type\":\"fleet.topology\"}\n")
        .ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    serde_json::from_str::<serde_json::Value>(&line)
        .ok()?
        .get("result")
        .cloned()
}

/// The address of the runtime's own web endpoint in a `fleet.topology` answer, once its listener is bound (health
/// `connected`).
fn bound_web_address(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            if map.get("protocol").and_then(serde_json::Value::as_str) == Some("web")
                && map.get("source").and_then(serde_json::Value::as_str) == Some("self")
                && map.get("health").and_then(serde_json::Value::as_str) == Some("connected")
            {
                return map
                    .get("address")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
            }
            map.values().find_map(bound_web_address)
        }
        serde_json::Value::Array(items) => items.iter().find_map(bound_web_address),
        _ => None,
    }
}

/// The `trust-runtime` executable of this target folder, or `TRUST_RUNTIME_BIN`.
pub fn trust_runtime() -> PathBuf {
    if let Some(path) = std::env::var_os("TRUST_RUNTIME_BIN") {
        return PathBuf::from(path);
    }
    let test = std::env::current_exe().expect("the test executable");
    let profile = test
        .parent()
        .and_then(Path::parent)
        .expect("the target profile folder");
    let executable = profile.join("trust-runtime");
    assert!(
        executable.is_file(),
        "{} is missing: run `cargo build -p trust-runtime --bin trust-runtime` first, or set TRUST_RUNTIME_BIN",
        executable.display()
    );
    executable
}

/// The `trust-lsp` executable of this target folder, or `TRUST_LSP_BIN` (the project commands of `trust-lsp command`).
pub fn trust_lsp() -> PathBuf {
    if let Some(path) = std::env::var_os("TRUST_LSP_BIN") {
        return PathBuf::from(path);
    }
    let test = std::env::current_exe().expect("the test executable");
    let profile = test
        .parent()
        .and_then(Path::parent)
        .expect("the target profile folder");
    let executable = profile.join(format!("trust-lsp{}", std::env::consts::EXE_SUFFIX));
    assert!(
        executable.is_file(),
        "{} is missing: run `cargo build -p trust-lsp` first, or set TRUST_LSP_BIN",
        executable.display()
    );
    executable
}

/// Runs `trust-runtime` with `arguments` to its end and requires success, for example `build --project <folder>`.
pub fn run_trust_runtime(arguments: &[&str]) {
    let output = Command::new(trust_runtime())
        .args(arguments)
        .output()
        .expect("run trust-runtime");
    assert!(
        output.status.success(),
        "trust-runtime {arguments:?} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Starts `trust-runtime run --project <project> --simulation`, with its output in `log`.
pub fn start_runtime(project: &Path, log: &Path) -> Runtime {
    let stdout = fs::File::create(log).expect("runtime log");
    let stderr = stdout.try_clone().expect("share the log");
    let child = Command::new(trust_runtime())
        .args([
            "run",
            "--project",
            &project.display().to_string(),
            "--simulation",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .expect("spawn trust-runtime run");
    Runtime(child)
}

/// Points the control endpoint at a socket in the copy, sets the test token and turns the web
/// server off, so parallel runs never share an endpoint or a port. The examples name no endpoint
/// and no token (specification 11 §6.8.0a), so both keys are set at the top of the copy's
/// `[runtime.control]` table, replacing any that the table has; no other table is touched.
pub fn prepare_runtime_toml(project: &Path, socket: &Path) {
    let path = project.join("runtime.toml");
    let text = fs::read_to_string(&path).expect("runtime.toml");
    let mut lines = Vec::new();
    let mut in_control = false;
    let mut has_control = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_control = trimmed == "[runtime.control]";
            lines.push(line.to_string());
            if in_control {
                has_control = true;
                lines.push(format!("endpoint = \"unix://{}\"", socket.display()));
                lines.push(format!("auth_token = \"{TOKEN}\""));
            }
            continue;
        }
        let key = trimmed.split('=').next().map(str::trim);
        if in_control && matches!(key, Some("endpoint" | "auth_token")) {
            continue;
        }
        lines.push(line.to_string());
    }
    assert!(
        has_control,
        "{} has no [runtime.control] table",
        path.display()
    );
    fs::write(
        &path,
        format!("{}\n\n[runtime.web]\nenabled = false\n", lines.join("\n")),
    )
    .expect("write runtime.toml");
}

/// Copies a folder tree: the example project that a test works on.
pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create the copy");
    for entry in fs::read_dir(from).expect("read the example") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("its type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy a file");
        }
    }
}
