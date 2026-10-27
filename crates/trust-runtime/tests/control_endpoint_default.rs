//! The default control endpoint, end to end (spec 11, section 6.8.0a).
//!
//! A runtime whose `[runtime.control]` table omits `endpoint` listens on the platform default and
//! publishes where it is in `<project>/.trust-runtime/control.json`. `ctl` then finds the runtime
//! through that file, with no endpoint and no token on its command line. The Unix test checks the
//! socket's directory and file modes, the endpoint that `ctl endpoint --json` reports, and that a
//! clean shutdown removes the file. A second Unix test starts a runtime in a read-only project
//! folder: it runs on the fallback socket without its endpoint file and says so. The Windows tests,
//! behind `cfg(windows)`, check the bound loopback port, the generated token in the per-user token
//! file, and `not_running` before the runtime starts. `ctl endpoint --json` reports every failure
//! as an error object with a stable code on every platform.
//!
//! `ctl shutdown --exit` ends a runtime's process, which removes its endpoint file; a plain
//! `shutdown` stops the program and keeps the process, and so the file, up (specification 34,
//! STATE-08). The tests build the program with the toolchain of specification 44 §4.6 at
//! `TRUST_TOOLCHAIN`.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use trust_runtime::bundle_builder::build_program_image;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml};
use trust_runtime::native::{Target, Toolchain};

/// How long a runtime may take to bind, publish, answer, or exit after shutdown.
const WAIT: Duration = Duration::from_secs(30);
/// The interval between polls.
const POLL: Duration = Duration::from_millis(50);

/// A project with no endpoint and no token, so every platform uses its default. Web, discovery,
/// mesh and OPC UA are off, so the control endpoint is the only socket the runtime opens.
const RUNTIME_TOML: &str = r#"[bundle]
version = 1

[resource]
name = "default-endpoint"
cycle_interval_ms = 50

[runtime]
execution_backend = "vm"

[runtime.control]
mode = "production"
debug_enabled = false

[runtime.web]
enabled = false
listen = "127.0.0.1:0"
auth = "local"
tls = false

[runtime.discovery]
enabled = false

[runtime.mesh]
enabled = false
listen = "127.0.0.1:0"
tls = false
publish = []
subscribe = {}

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

/// A scratch project folder, removed when the test ends.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(prefix: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let sequence = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "trust-runtime-{prefix}-{}-{nanos}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create endpoint test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Writes the project: runtime.toml, a loopback io.toml, one program, and its program image.
fn write_project(root: &Path) {
    std::fs::create_dir_all(root.join("src")).expect("create source directory");
    std::fs::write(root.join("runtime.toml"), RUNTIME_TOML).expect("write runtime.toml");
    let io = render_io_toml(&build_io_config_auto("loopback").expect("build loopback config"));
    std::fs::write(root.join("io.toml"), io).expect("write io.toml");
    std::fs::write(
        root.join("src/main.st"),
        // The loopback io.toml maps %IX0.0 and %QX0.0, so the program declares both: a running
        // runtime refuses a mapping outside the program's process image.
        "PROGRAM Main\nVAR\n    Input AT %IX0.0 : BOOL;\n    Output AT %QX0.0 : BOOL;\n    Counter : DINT;\nEND_VAR\nCounter := Counter + 1;\nOutput := Input;\nEND_PROGRAM\n",
    )
    .expect("write source");
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("a computer target");
    build_program_image(root, None, target, &toolchain, false, true)
        .expect("build the endpoint test program image");
}

fn trust_runtime_command() -> Command {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
}

/// A `trust-runtime ctl --project <root>` call with no endpoint or token on the command line.
fn ctl(root: &Path, args: &[&str]) -> Output {
    trust_runtime_command()
        .arg("ctl")
        .arg("--project")
        .arg(root)
        .args(args)
        .env_remove("TRUST_CTL_TOKEN")
        .output()
        .expect("run trust-runtime ctl")
}

fn describe(output: &Output) -> String {
    format!(
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn runtime_log(root: &Path) -> String {
    std::fs::read_to_string(root.join("play.log")).unwrap_or_default()
}

/// `ctl endpoint --json` output that is an error object: its code, after checking that the
/// command exited non-zero.
fn endpoint_error_code(output: &Output) -> String {
    assert!(
        !output.status.success(),
        "a failure exits non-zero\n{}",
        describe(output)
    );
    let report: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("error object JSON: {error}\n{}", describe(output)));
    assert!(
        report["error"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty()),
        "{report}"
    );
    report["error"]["code"]
        .as_str()
        .expect("error code")
        .to_string()
}

/// A `trust-runtime play` process, killed when the test ends unless it has already exited.
struct Runtime {
    child: Child,
}

impl Runtime {
    fn start(root: &Path) -> Self {
        Self::start_logged(root, &root.join("play.log"), &[])
    }

    /// Starts the runtime with its log at `log` and the environment variables `env` set.
    fn start_logged(root: &Path, log: &Path, env: &[(&str, &Path)]) -> Self {
        let log = File::create(log).expect("create runtime log");
        let stderr = log.try_clone().expect("clone runtime log");
        let mut command = trust_runtime_command();
        command
            .arg("play")
            .arg("--project")
            .arg(root)
            .arg("--no-console")
            .env_remove("TRUST_CTL_TOKEN")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(stderr));
        for (name, value) in env {
            command.env(name, value);
        }
        let child = command.spawn().expect("start trust-runtime play");
        Self { child }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// The endpoint file that the runtime publishes, once it exists and parses.
fn wait_for_endpoint_file(root: &Path, runtime: &mut Runtime) -> Value {
    let path = root.join(".trust-runtime").join("control.json");
    let deadline = Instant::now() + WAIT;
    loop {
        if let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(value) = serde_json::from_str::<Value>(&text)
        {
            return value;
        }
        if let Ok(Some(status)) = runtime.child.try_wait() {
            panic!(
                "the runtime exited ({status}) before it published {}\n{}",
                path.display(),
                runtime_log(root)
            );
        }
        assert!(
            Instant::now() < deadline,
            "the runtime did not publish {}\n{}",
            path.display(),
            runtime_log(root)
        );
        std::thread::sleep(POLL);
    }
}

/// `ctl status` once it succeeds, which means the endpoint answers.
fn wait_for_status(root: &Path, runtime: &mut Runtime) -> Output {
    let deadline = Instant::now() + WAIT;
    loop {
        let output = ctl(root, &["status"]);
        if output.status.success() {
            return output;
        }
        assert!(
            Instant::now() < deadline,
            "ctl status never succeeded\n{}\n{}",
            describe(&output),
            runtime_log(root)
        );
        assert!(
            runtime.child.try_wait().expect("poll runtime").is_none(),
            "the runtime exited\n{}",
            runtime_log(root)
        );
        std::thread::sleep(POLL);
    }
}

fn wait_for_exit(runtime: &mut Runtime) -> ExitStatus {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(status) = runtime.child.try_wait().expect("poll runtime") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "the runtime did not exit after shutdown"
        );
        std::thread::sleep(POLL);
    }
}

#[cfg(unix)]
#[test]
fn a_runtime_without_an_endpoint_is_found_through_its_endpoint_file() {
    use std::os::unix::fs::PermissionsExt;

    let root = ScratchDir::new("unix-default");
    write_project(root.path());
    let mut runtime = Runtime::start(root.path());

    let file = wait_for_endpoint_file(root.path(), &mut runtime);
    let endpoint = file["endpoint"]
        .as_str()
        .expect("endpoint text")
        .to_string();
    assert!(endpoint.starts_with("unix://"), "{endpoint}");
    assert_eq!(file["schema_version"], 1);
    assert_eq!(file["pid"].as_u64(), Some(u64::from(runtime.child.id())));
    assert!(
        file.get("auth_token").is_none(),
        "a Unix default generates no token: {file}"
    );

    let directory_mode = std::fs::metadata(root.path().join(".trust-runtime"))
        .expect("endpoint directory")
        .permissions()
        .mode();
    let file_mode = std::fs::metadata(root.path().join(".trust-runtime").join("control.json"))
        .expect("endpoint file")
        .permissions()
        .mode();
    assert_eq!(directory_mode & 0o777, 0o700);
    assert_eq!(file_mode & 0o777, 0o600);

    let status = wait_for_status(root.path(), &mut runtime);
    assert!(
        String::from_utf8_lossy(&status.stdout).starts_with("state="),
        "{}",
        describe(&status)
    );

    let report = ctl(root.path(), &["endpoint", "--json"]);
    assert!(report.status.success(), "{}", describe(&report));
    let report: Value = serde_json::from_slice(&report.stdout).expect("endpoint report JSON");
    assert_eq!(report["source"], "endpoint_file");
    assert_eq!(report["endpoint"], endpoint);
    assert!(report["auth_token"].is_null(), "{report}");

    let shutdown = ctl(root.path(), &["shutdown", "--exit"]);
    assert!(shutdown.status.success(), "{}", describe(&shutdown));
    let exit = wait_for_exit(&mut runtime);
    assert!(exit.success(), "{exit}\n{}", runtime_log(root.path()));
    assert!(
        !root
            .path()
            .join(".trust-runtime")
            .join("control.json")
            .exists(),
        "a clean stop removes the endpoint file"
    );
}

#[cfg(windows)]
#[test]
fn a_windows_runtime_publishes_its_bound_port_and_keeps_its_token_out_of_the_project() {
    let root = ScratchDir::new("windows-default");
    write_project(root.path());

    // Before the runtime starts, the Windows default has no port.
    let before = ctl(root.path(), &["endpoint", "--json"]);
    assert_eq!(endpoint_error_code(&before), "not_running");

    let mut runtime = Runtime::start(root.path());

    let file = wait_for_endpoint_file(root.path(), &mut runtime);
    let endpoint = file["endpoint"]
        .as_str()
        .expect("endpoint text")
        .to_string();
    let port = endpoint
        .strip_prefix("tcp://127.0.0.1:")
        .expect("loopback endpoint");
    assert_ne!(port, "0", "the bound port replaces the requested port zero");
    assert!(
        file.get("auth_token").is_none(),
        "the generated token is not written into the project folder: {file}"
    );

    let status = wait_for_status(root.path(), &mut runtime);
    assert!(
        String::from_utf8_lossy(&status.stdout).starts_with("state="),
        "{}",
        describe(&status)
    );

    let report = ctl(root.path(), &["endpoint", "--json"]);
    assert!(report.status.success(), "{}", describe(&report));
    let report: Value = serde_json::from_slice(&report.stdout).expect("endpoint report JSON");
    assert_eq!(report["source"], "endpoint_file");
    let token = report["auth_token"]
        .as_str()
        .expect("the token from the per-user token file");
    assert_eq!(token.len(), 43, "{token}");
    let token_files = std::fs::read_dir(
        std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA"))
            .join("trust")
            .join("runtimes"),
    )
    .expect("the per-user token folder")
    .filter_map(Result::ok)
    .filter(|entry| std::fs::read_to_string(entry.path()).is_ok_and(|text| text.contains(token)))
    .count();
    assert_eq!(token_files, 1, "one per-user token file holds the token");

    let shutdown = ctl(root.path(), &["shutdown", "--exit"]);
    assert!(shutdown.status.success(), "{}", describe(&shutdown));
    let exit = wait_for_exit(&mut runtime);
    assert!(exit.success(), "{exit}\n{}", runtime_log(root.path()));
    assert!(
        !root
            .path()
            .join(".trust-runtime")
            .join("control.json")
            .exists(),
        "a clean stop removes the endpoint file"
    );
}

/// Every failure of `ctl endpoint --json` is an error object with a stable code and a non-zero exit
/// (spec 11, section 6.8.0a).
#[test]
fn ctl_endpoint_reports_each_failure_as_an_error_object() {
    let root = ScratchDir::new("endpoint-errors");
    write_project(root.path());

    let no_project = trust_runtime_command()
        .args(["ctl", "endpoint", "--json"])
        .env_remove("TRUST_CTL_TOKEN")
        .output()
        .expect("run trust-runtime ctl endpoint");
    assert_eq!(endpoint_error_code(&no_project), "no_project");

    let missing = ctl(&root.path().join("absent"), &["endpoint", "--json"]);
    assert_eq!(endpoint_error_code(&missing), "no_project");

    let with_endpoint = trust_runtime_command()
        .arg("ctl")
        .arg("--project")
        .arg(root.path())
        .args(["--endpoint", "tcp://127.0.0.1:9", "endpoint", "--json"])
        .env_remove("TRUST_CTL_TOKEN")
        .output()
        .expect("run trust-runtime ctl endpoint");
    assert_eq!(endpoint_error_code(&with_endpoint), "invalid_argument");

    let endpoint_dir = root.path().join(".trust-runtime");
    std::fs::create_dir_all(&endpoint_dir).expect("create endpoint folder");
    std::fs::write(endpoint_dir.join("control.json"), "not json").expect("write endpoint file");
    let invalid_file = ctl(root.path(), &["endpoint", "--json"]);
    assert_eq!(endpoint_error_code(&invalid_file), "endpoint_file_invalid");
    std::fs::remove_file(endpoint_dir.join("control.json")).expect("remove endpoint file");

    std::fs::write(root.path().join("runtime.toml"), "not = [valid").expect("break runtime.toml");
    let invalid_config = ctl(root.path(), &["endpoint", "--json"]);
    assert_eq!(endpoint_error_code(&invalid_config), "invalid_config");
}

/// Gives the owner write access to a folder again when the test ends, so it can be removed.
#[cfg(unix)]
struct WritableAgain(PathBuf);

#[cfg(unix)]
impl Drop for WritableAgain {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// One newline-delimited control request on a Unix socket, without a token: the owner of an
/// owner-only socket has every right (spec 11, section 6.9.5).
#[cfg(unix)]
fn unix_request(endpoint: &str, request: &str) -> Value {
    use std::io::{BufRead, BufReader, Write};

    let path = endpoint.strip_prefix("unix://").expect("unix endpoint");
    let mut stream = std::os::unix::net::UnixStream::connect(path).expect("connect socket");
    stream
        .set_read_timeout(Some(WAIT))
        .expect("set read timeout");
    writeln!(stream, "{request}").expect("send request");
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .expect("read response");
    serde_json::from_str(&line).expect("response JSON")
}

/// A runtime in a project folder it cannot write runs on: it binds the fallback socket in the
/// per-user runtime directory, reports a warning, and its status says that the endpoint file is
/// unavailable. A tooling file never stops a PLC program (spec 11, section 6.8.0a; spec 34,
/// ARCH-07).
#[cfg(unix)]
#[test]
fn a_runtime_in_a_read_only_project_runs_without_its_endpoint_file() {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    let scratch = ScratchDir::new("read-only");
    let project = scratch.path().join("project");
    write_project(&project);
    // A short private runtime directory, so the fallback socket path fits on every platform.
    let runtime_dir = PathBuf::from(format!(
        "/tmp/trust-rd-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&runtime_dir)
        .expect("create runtime directory");
    let _runtime_dir_cleanup = RemoveDirLater(runtime_dir.clone());
    std::fs::set_permissions(&project, std::fs::Permissions::from_mode(0o555))
        .expect("make the project folder read-only");
    let _writable = WritableAgain(project.clone());
    if std::fs::write(project.join("probe"), "x").is_ok() {
        eprintln!("skipped: this process writes into read-only folders");
        return;
    }

    let log = scratch.path().join("play.log");
    // The project cannot hold the runtime ID either, so the runtime keeps it in the user's state
    // folder (spec 11, runtime discovery); the test gives it a state folder of its own.
    let state_dir = scratch.path().join("state");
    std::fs::create_dir_all(&state_dir).expect("create state folder");
    let env = [
        ("XDG_RUNTIME_DIR", runtime_dir.as_path()),
        ("XDG_STATE_HOME", state_dir.as_path()),
    ];
    let mut runtime = Runtime::start_logged(&project, &log, &env);
    let ctl_env = |args: &[&str]| {
        trust_runtime_command()
            .arg("ctl")
            .arg("--project")
            .arg(&project)
            .args(args)
            .env_remove("TRUST_CTL_TOKEN")
            .env("XDG_RUNTIME_DIR", &runtime_dir)
            .output()
            .expect("run trust-runtime ctl")
    };

    let deadline = Instant::now() + WAIT;
    let status = loop {
        let output = ctl_env(&["status"]);
        if output.status.success() {
            break output;
        }
        assert!(
            Instant::now() < deadline,
            "ctl status never succeeded\n{}\n{}",
            describe(&output),
            std::fs::read_to_string(&log).unwrap_or_default()
        );
        assert!(
            runtime.child.try_wait().expect("poll runtime").is_none(),
            "the runtime stopped although only its endpoint file could not be written\n{}",
            std::fs::read_to_string(&log).unwrap_or_default()
        );
        std::thread::sleep(POLL);
    };
    assert!(
        String::from_utf8_lossy(&status.stdout).starts_with("state="),
        "{}",
        describe(&status)
    );
    assert!(!project.join(".trust-runtime").exists());

    let report = ctl_env(&["endpoint", "--json"]);
    assert!(report.status.success(), "{}", describe(&report));
    let report: Value = serde_json::from_slice(&report.stdout).expect("endpoint report JSON");
    assert_eq!(report["source"], "default");
    let endpoint = report["endpoint"]
        .as_str()
        .expect("endpoint text")
        .to_string();
    assert!(
        endpoint.starts_with(&format!("unix://{}/trust-", runtime_dir.display())),
        "the runtime listens on the fallback socket: {endpoint}"
    );

    let status = unix_request(&endpoint, r#"{"id":1,"type":"status"}"#);
    assert_eq!(
        status["result"]["endpoint_file"]["state"], "unavailable",
        "{status}"
    );
    assert!(
        status["result"]["endpoint_file"]["error"]
            .as_str()
            .is_some_and(|error| error.contains(".trust-runtime")),
        "{status}"
    );
    let events = unix_request(
        &endpoint,
        r#"{"id":2,"type":"events.tail","params":{"limit":50}}"#,
    );
    assert!(
        events
            .to_string()
            .contains("control_endpoint_file_unavailable"),
        "{events}"
    );
    assert!(
        std::fs::read_to_string(&log)
            .unwrap_or_default()
            .contains("control_endpoint_file_unavailable"),
        "the warning is logged"
    );
    assert!(
        std::fs::read_to_string(&log)
            .unwrap_or_default()
            .contains("runtime_id_kept_for_user"),
        "the runtime ID that the project cannot hold is kept for the user"
    );

    let shutdown = ctl_env(&["shutdown", "--exit"]);
    assert!(shutdown.status.success(), "{}", describe(&shutdown));
    let exit = wait_for_exit(&mut runtime);
    assert!(exit.success(), "{exit}");
}

/// Removes a folder when the test ends.
#[cfg(unix)]
struct RemoveDirLater(PathBuf);

#[cfg(unix)]
impl Drop for RemoveDirLater {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
