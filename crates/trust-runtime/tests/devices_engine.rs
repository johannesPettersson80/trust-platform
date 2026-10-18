//! The Devices & Connections engine against a real `trust-runtime` (design §4.3 B9): the panel
//! shows a started project's runtime with its connections, a saved connection appears in the next
//! view, and the managed lifecycle of a real runtime runs through `act`, its result arriving once
//! in a later view.

use std::fs;
use std::io::Read;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use trust_devices::engine::DevicesEngine;
use trust_devices::request::{
    ActOp, ActRequest, DebugSessionFacts, DebugSessionKind, GatherMode, HostFacts, HostSettings,
    RunMode, Token, ViewOptions, ViewRequest,
};
use trust_devices::view::types::{DevicesView, HostEffect, OperationDone, Runtime, Tone};

/// The deadline of one fleet command.
const FLEET_DEADLINE: Duration = Duration::from_secs(30);
/// How long a test polls the panel for what it waits for.
const POLL_DEADLINE: Duration = Duration::from_secs(30);
/// How often a test polls the panel, as the extension does within its `next_refresh_ms`.
const POLL_PAUSE: Duration = Duration::from_millis(50);
/// The session of the panel the tests drive.
const SESSION: &str = "dc-it";

static FOLDER_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// A folder of its own under the system's temporary folder, removed when it is dropped.
struct TempFolder(PathBuf);

impl TempFolder {
    /// A folder that does not exist yet.
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-devices-engine-{label}-{}-{}",
            std::process::id(),
            FOLDER_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create the test's folder");
        Self(root)
    }

    /// The folder's path.
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Stops a managed runtime and removes its fleet root when a test ends.
struct FleetCleanup {
    root: PathBuf,
}

impl Drop for FleetCleanup {
    fn drop(&mut self) {
        let _ = run_fleet(&self.root, &["runtime", "stop", "--name", CELL]);
    }
}

/// The managed runtime of the lifecycle tests.
const CELL: &str = "cell";

/// The `trust-runtime` executable that Cargo builds for these tests.
fn trust_runtime() -> PathBuf {
    PathBuf::from(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime while executing integration tests"),
    )
}

/// A free loopback port.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind an ephemeral loopback port")
        .local_addr()
        .expect("read the ephemeral address")
        .port()
}

/// `trust-runtime fleet … --fleet-root <root> --json`, within [`FLEET_DEADLINE`].
fn run_fleet(root: &Path, args: &[&str]) -> Output {
    let mut child = Command::new(trust_runtime())
        .arg("fleet")
        .args(args)
        .arg("--fleet-root")
        .arg(root)
        .arg("--json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the fleet command");
    let deadline = Instant::now() + FLEET_DEADLINE;
    loop {
        if let Some(status) = child.try_wait().expect("inspect the fleet command") {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            if let Some(mut out) = child.stdout.take() {
                out.read_to_end(&mut stdout).expect("the fleet stdout");
            }
            if let Some(mut err) = child.stderr.take() {
                err.read_to_end(&mut stderr).expect("the fleet stderr");
            }
            return Output {
                status,
                stdout,
                stderr,
            };
        }
        assert!(
            Instant::now() < deadline,
            "the fleet command {args:?} did not finish"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

/// Fails with the command's output when it did not succeed.
fn assert_success(output: &Output, operation: &str) {
    assert!(
        output.status.success(),
        "{operation} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Scaffolds the managed runtime [`CELL`] of `root` with a free control and web port, and answers
/// its control endpoint.
fn scaffold_runtime(root: &Path) -> String {
    let control = free_port();
    let mut web = free_port();
    while web == control {
        web = free_port();
    }
    let added = run_fleet(
        root,
        &[
            "runtime",
            "add",
            "--name",
            CELL,
            "--template",
            "simulate",
            "--control-port",
            &control.to_string(),
            "--web-port",
            &web.to_string(),
        ],
    );
    assert_success(&added, "fleet runtime add");
    let added: Value = serde_json::from_slice(&added.stdout).expect("parse the add answer");
    added
        .get("control_endpoint")
        .and_then(Value::as_str)
        .expect("the added runtime's control endpoint")
        .to_owned()
}

/// Waits until the managed runtime [`CELL`] answers `running`.
fn wait_until_running(root: &Path) {
    let deadline = Instant::now() + FLEET_DEADLINE;
    loop {
        let status = run_fleet(root, &["runtime", "status", "--name", CELL]);
        assert_success(&status, "fleet runtime status");
        let status: Value = serde_json::from_slice(&status.stdout).expect("parse the status");
        if status.get("status").and_then(Value::as_str) == Some("running") {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "the managed runtime did not reach running: {status}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}

/// One view request of `session` with the number `seq`, as the extension sends it: the workspace
/// folders it has open, and the project the user chose among them.
fn view_request(
    session: &str,
    seq: u64,
    gather: GatherMode,
    folders: &[PathBuf],
    project: Option<&Path>,
    host: &HostFacts,
) -> ViewRequest {
    ViewRequest {
        session: session.to_owned(),
        seq,
        gather,
        project: project.map(Path::to_path_buf),
        workspace_folders: folders.to_vec(),
        view: ViewOptions::default(),
        host: host.clone(),
    }
}

/// Polls the panel as the extension does (design §1.5) until `ready`, and answers that view.
fn poll_until(
    engine: &DevicesEngine,
    session: &str,
    folders: &[PathBuf],
    project: Option<&Path>,
    host: &HostFacts,
    what: &str,
    ready: impl Fn(&DevicesView) -> bool,
) -> DevicesView {
    let deadline = Instant::now() + POLL_DEADLINE;
    let mut seq = 1;
    loop {
        let gather = if seq == 1 {
            GatherMode::Now
        } else {
            GatherMode::Auto
        };
        let reply = engine.view(view_request(session, seq, gather, folders, project, host));
        assert!(
            reply.error.is_none(),
            "the panel answered {:?}",
            reply.error
        );
        let view = reply.view.expect("every answered request holds a view");
        if ready(&view) {
            return view;
        }
        if Instant::now() >= deadline {
            let drawn = serde_json::to_string(&view).unwrap_or_default();
            let shown: String = drawn.chars().take(6000).collect();
            panic!("the panel did not show {what}; its last view: {shown}");
        }
        seq += 1;
        thread::sleep(POLL_PAUSE);
    }
}

/// Every runtime of `view`, in the order it shows them.
fn runtimes(view: &DevicesView) -> impl Iterator<Item = &Runtime> {
    view.computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
}

/// The one runtime card of a view of one project: the simulator that runs the project in simulate
/// mode, which shows as `Simulator` whatever its resource is called (specification 25 §5.6).
fn sole_runtime(view: &DevicesView) -> &Runtime {
    let mut shown = runtimes(view);
    let runtime = shown.next().expect("the project's runtime card");
    assert!(shown.next().is_none(), "one project shows one runtime card");
    runtime
}

/// The result of the operation `operation`, delivered once in a later view's `operations_done`
/// (design §1.4.1).
fn operation_result(
    engine: &DevicesEngine,
    session: &str,
    operation: &str,
) -> (OperationDone, DevicesView) {
    let deadline = Instant::now() + POLL_DEADLINE;
    let mut seq = 1;
    loop {
        let host = HostFacts::default();
        let reply = engine.view(view_request(
            session,
            seq,
            GatherMode::Cached,
            &[],
            None,
            &host,
        ));
        assert!(
            reply.error.is_none(),
            "the panel answered {:?}",
            reply.error
        );
        if let Some(done) = reply
            .operations_done
            .into_iter()
            .find(|done| done.operation == operation)
        {
            return (done, reply.view.expect("a view"));
        }
        assert!(
            Instant::now() < deadline,
            "the operation {operation} reported its result"
        );
        seq += 1;
        thread::sleep(POLL_PAUSE);
    }
}

#[test]
fn view_shows_a_started_project_runtime_with_its_connections() {
    // Design §4.3 B9: "a real `trust-runtime` through `CARGO_BIN_EXE_trust-runtime`, as
    // `tests/fleet_lifecycle_command.rs` starts one", and design §1.5: the first view of a session
    // is the loading shell, and the next polls get the gather. A wrong implementation waits for the
    // gather inside the answer, or never asks the runtime at all.
    let folder = TempFolder::new("started-runtime");
    let root = folder.path();
    let _cleanup = FleetCleanup {
        root: root.to_path_buf(),
    };
    let endpoint = scaffold_runtime(root);
    let started = run_fleet(root, &["runtime", "start", "--name", CELL]);
    assert_success(&started, "fleet runtime start");
    wait_until_running(root);

    let project = root.join(CELL);
    let token = trust_runtime::config::fleet_manifest::read_control_auth_token(&project)
        .expect("read the runtime's auth token")
        .expect("the runtime needs a token on its TCP endpoint");
    let mut host = HostFacts {
        settings: HostSettings {
            mode: RunMode::Online,
            control_endpoint: endpoint.clone(),
            ..HostSettings::default()
        },
        ..HostFacts::default()
    };
    host.tokens.insert(endpoint, Token::new(token));
    let engine = DevicesEngine::new();

    let folders = vec![root.to_path_buf()];
    let first = engine.view(view_request(
        SESSION,
        1,
        GatherMode::Now,
        &folders,
        Some(project.as_path()),
        &host,
    ));
    assert_eq!(
        first.next_refresh_ms,
        trust_devices::engine::WAITING_REFRESH_MS,
        "the first view of a session is the loading shell"
    );

    let view = poll_until(
        &engine,
        SESSION,
        &folders,
        Some(project.as_path()),
        &host,
        "the started runtime with its connections",
        |view| runtimes(view).any(|runtime| !runtime.connections.is_empty()),
    );
    let runtime = runtimes(&view)
        .find(|runtime| !runtime.connections.is_empty())
        .expect("the runtime with its connections");
    assert!(
        runtime.answers,
        "the project's runtime answered its status: {runtime:?}"
    );
    assert_eq!(runtime.state.tone, Tone::Ok);
    assert!(
        runtime
            .connections
            .iter()
            .any(|connection| connection.protocol == "simulated"),
        "the started runtime shows its simulated I/O: {:?}",
        runtime.connections
    );
}

#[test]
fn save_then_view_shows_the_new_connection() {
    // Specification 25 §5.14: "`Save` writes to the runtime chosen in the form"; the open
    // project's own runtime is written in process into the project's files (design §4.3 B7), and
    // the next view reads the changed file (design §1.5). A wrong implementation reuses the cached
    // topology, so the panel never shows the saved connection.
    let folder = TempFolder::new("saved-connection");
    let project = folder.path().join("line");
    fs::create_dir_all(&project).expect("create the project's folder");
    fs::write(project.join("runtime.toml"), runtime_toml("line")).expect("write runtime.toml");
    let host = HostFacts::default();
    let engine = DevicesEngine::new();

    let folders = vec![folder.path().to_path_buf()];
    let before = poll_until(
        &engine,
        SESSION,
        &folders,
        Some(project.as_path()),
        &host,
        "the project's runtime card with its connections",
        // The loading shell draws a placeholder card without connections; the gathered card has
        // the Discovery and Web connections of the project (design §1.5).
        |view| runtimes(view).any(|runtime| !runtime.connections.is_empty()),
    );
    let runtime = sole_runtime(&before);
    assert!(
        !runtime
            .connections
            .iter()
            .any(|connection| connection.protocol == "modbus_tcp"),
        "the project configures no modbus_tcp connection yet: {:?}",
        runtime.connections
    );
    let id = runtime.id.as_str().to_owned();

    let form = format!("modbus_tcp@{id}");
    let mut values = Map::new();
    values.insert("address".to_owned(), json!("10.0.0.9:502"));
    values.insert("unit_id".to_owned(), json!(1));
    let saved = engine.act(&ActRequest {
        session: SESSION.to_owned(),
        op: ActOp {
            action: "connection.save".to_owned(),
            payload: json!({
                "form": form,
                "protocol": "modbus_tcp",
                "runtime": id,
            }),
        },
        values,
        project: Some(project.clone()),
        host: host.clone(),
    });
    assert!(saved.ok, "the save is accepted: {saved:?}");
    let saved_notice = match saved.pending.as_ref() {
        // A save that runs past the panel's wait reports its result in a later view (design §1.5).
        Some(pending) => {
            let (done, _) = operation_result(&engine, SESSION, &pending.operation);
            done.notice
        }
        None => saved.notice.clone(),
    };
    // The notice names the pane that started the save, which is the form's pane (specification
    // 25 §5.13).
    let form_pane = format!("form:{form}");
    assert_eq!(
        saved_notice.as_ref().map(|notice| notice.pane.as_str()),
        Some(form_pane.as_str())
    );
    let io = fs::read_to_string(project.join("io.toml")).expect("the io.toml was written");
    assert!(io.contains("10.0.0.9:502"), "{io}");

    let after = poll_until(
        &engine,
        SESSION,
        &folders,
        Some(project.as_path()),
        &host,
        "the saved modbus_tcp connection of the project's runtime",
        |view| {
            sole_runtime(view)
                .connections
                .iter()
                .any(|connection| connection.protocol == "modbus_tcp")
        },
    );
    let connection = sole_runtime(&after)
        .connections
        .iter()
        .find(|connection| connection.protocol == "modbus_tcp")
        .expect("the saved connection");
    assert_eq!(connection.protocol_name, "Modbus TCP");
}

#[test]
fn managed_start_and_stop_through_act() {
    // Design §4.3 B9: the managed lifecycle through `act`, and design §1.4.1: "A slow action
    // answers ... `pending`. Its final `notice` and `effects` arrive exactly once, in the
    // `operations_done` of a later view." A wrong implementation never runs the lifecycle, or
    // loses its result.
    let folder = TempFolder::new("managed-lifecycle");
    let root = folder.path();
    let _cleanup = FleetCleanup {
        root: root.to_path_buf(),
    };
    let _ = scaffold_runtime(root);
    let executable = trust_runtime();
    let engine = DevicesEngine::new();
    let start = ActRequest {
        session: SESSION.to_owned(),
        op: ActOp {
            action: "runtime.managed_start".to_owned(),
            payload: json!({ "runtime": "trust-fleet-v1:cell", "name": CELL }),
        },
        values: Map::new(),
        project: Some(root.to_path_buf()),
        host: HostFacts {
            runtime_executable: Some(executable.clone()),
            ..HostFacts::default()
        },
    };

    let reply = engine.act(&start);
    let pending = reply.pending.clone().expect("a slow Start is pending");
    let (done, _) = operation_result(&engine, SESSION, &pending.operation);
    let notice = done.notice.clone().expect("the Start reports its result");
    assert_eq!(notice.tone, Tone::Ok, "{notice:?}");
    assert_eq!(notice.pane, pending.pane);
    let attached = done
        .effects
        .iter()
        .find_map(|effect| match effect {
            HostEffect::Attach { endpoint, label } => Some((endpoint.clone(), label.clone())),
            _ => None,
        })
        .expect("a successful Start attaches to the reached runtime");
    assert_eq!(attached.1, "cell (this computer)");
    assert!(
        done.effects.contains(&HostEffect::SetRunTarget {
            id: CELL.to_owned(),
        }),
        "{:?}",
        done.effects
    );

    let stop = ActRequest {
        session: SESSION.to_owned(),
        op: ActOp {
            action: "runtime.managed_stop".to_owned(),
            payload: json!({ "runtime": "trust-fleet-v1:cell", "name": CELL }),
        },
        values: Map::new(),
        project: Some(root.to_path_buf()),
        host: HostFacts {
            runtime_executable: Some(executable),
            debug_session: Some(DebugSessionFacts {
                kind: DebugSessionKind::Attach,
                endpoint: Some(attached.0.clone()),
                label: Some(attached.1.clone()),
                starting: false,
                failure: None,
            }),
            ..HostFacts::default()
        },
    };
    let reply = engine.act(&stop);
    let pending = reply.pending.clone().expect("a slow Stop is pending");
    let (done, _) = operation_result(&engine, SESSION, &pending.operation);
    let notice = done.notice.clone().expect("the Stop reports its result");
    assert_eq!(notice.tone, Tone::Ok, "{notice:?}");
    assert!(
        done.effects.contains(&HostEffect::StopSession),
        "stopping the attached runtime ends its session: {:?}",
        done.effects
    );
}

/// A `runtime.toml` of the resource `line` on a local socket, as the panel's own tests write it.
fn runtime_toml(name: &str) -> String {
    format!(
        r#"[bundle]
version = 1

[resource]
name = "{name}"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-devices-engine-{name}.sock"
mode = "production"
debug_enabled = false

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
"#
    )
}
