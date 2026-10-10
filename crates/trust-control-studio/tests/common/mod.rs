//! What the real-runtime tests of Control Studio's engine share: finding the `trust-runtime` executable, building a
//! project with it, starting `trust-runtime run --simulation` on a copy of an example project, waiting until it answers
//! and has scanned, and the guards that end the runtime process and remove a test's folder even when an assertion fails.
//!
//! `control_studio_runtime.rs` (checklist V0.7) and `control_studio_identify.rs` (V1.6) both include this module with
//! `mod common;`. Every item below is used by both files, so the module needs no `allow(dead_code)`. The code was moved
//! from `control_studio_runtime.rs` as it was; only the items became `pub`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use trust_control::loop_definition::LoopDefinition;
use trust_control::runtime::{RuntimeConnection, RuntimeStatus};
use trust_control::signals::{Inspection, InspectionRequest, InspectionSample, SignalBinding};
use trust_control_studio::runtime_client::RuntimeClient;

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
    fn drop(&mut self) {
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
/// server off, so parallel runs never share an endpoint or a port.
pub fn prepare_runtime_toml(project: &Path, socket: &Path) {
    let path = project.join("runtime.toml");
    let text = fs::read_to_string(&path).expect("runtime.toml");
    let lines: Vec<String> = text
        .lines()
        .map(|line| match line.split('=').next().map(str::trim) {
            Some("endpoint") => format!("endpoint = \"unix://{}\"", socket.display()),
            Some("auth_token") => format!("auth_token = \"{TOKEN}\""),
            _ => line.to_string(),
        })
        .collect();
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
