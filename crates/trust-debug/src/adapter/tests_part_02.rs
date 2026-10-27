use super::*;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[test]
fn dispatch_initialize_emits_initialized_event() {
    let mut adapter = DebugAdapter::new(DebugSession::new());
    let request = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "initialize".to_string(),
        arguments: Some(serde_json::to_value(InitializeArguments::default()).unwrap()),
    };

    let outcome = adapter.dispatch_request(request);
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<InitializeResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    let capabilities = response.body.unwrap().capabilities;
    assert_eq!(capabilities.supports_conditional_breakpoints, Some(true));
    assert_eq!(
        capabilities.supports_hit_conditional_breakpoints,
        Some(true)
    );
    assert_eq!(capabilities.supports_log_points, Some(true));
    let saw_initialized = outcome.events.iter().any(|value| {
        let event: Event<serde_json::Value> = serde_json::from_value(value.clone()).unwrap();
        event.event == "initialized"
    });
    assert!(saw_initialized);
    let internal_messages = outcome
        .events
        .iter()
        .filter_map(|value| {
            let event: Event<serde_json::Value> = serde_json::from_value(value.clone()).unwrap();
            if event.event != "trustDebugInternal" {
                return None;
            }
            event.body.and_then(|body| {
                body.get("message")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
        })
        .collect::<Vec<_>>();
    assert!(!internal_messages.is_empty());
    assert!(internal_messages
        .iter()
        .all(|message| message.contains("[trust-debug]")));
    let visible_internal_output = outcome.events.iter().any(|value| {
        let event: Event<serde_json::Value> = serde_json::from_value(value.clone()).unwrap();
        event.event == "output"
            && event.body.as_ref().is_some_and(|body| {
                body.get("output")
                    .and_then(|value| value.as_str())
                    .is_some_and(|output| output.contains("[trust-debug]"))
            })
    });
    assert!(
        !visible_internal_output,
        "internal debug traces must not be visible Debug Console output: {internal_messages:?}"
    );
}

#[test]
fn dispatch_launch_does_not_emit_initialized_event_without_initialize() {
    let mut adapter = DebugAdapter::new(DebugSession::new());

    let mut additional = BTreeMap::new();
    additional.insert(
        "program".to_string(),
        serde_json::Value::String("main.st".to_string()),
    );
    let launch_request = Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "launch".to_string(),
        arguments: Some(serde_json::to_value(LaunchArguments { additional }).unwrap()),
    };

    let outcome = adapter.dispatch_request(launch_request);
    let saw_initialized = outcome.events.iter().any(|value| {
        let event: Event<serde_json::Value> = serde_json::from_value(value.clone()).unwrap();
        event.event == "initialized"
    });
    assert!(!saw_initialized);
}

#[test]
fn attach_set_expression_forwards_remote_io_force_and_release() {
    let (addr, requests, server) = spawn_remote_io_force_server();
    let mut adapter = DebugAdapter::new(DebugSession::new());
    adapter.remote_session = Some(
        super::super::remote::RemoteSession::connect(
            super::super::remote::RemoteEndpoint::Tcp(addr),
            Some("token".to_string()),
        )
        .expect("remote session should connect"),
    );

    let force = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setExpression".to_string(),
        arguments: Some(
            serde_json::to_value(SetExpressionArguments {
                expression: "%QX0.0".to_string(),
                value: "force: TRUE".to_string(),
                frame_id: None,
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(force);
    assert_eq!(outcome.responses.len(), 1);
    assert_eq!(outcome.events.len(), 1);
    let response: Response<SetExpressionResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "remote force failed: {:?}",
        response.message
    );
    let event: Event<IoStateEventBody> = serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert!(event
        .body
        .unwrap()
        .outputs
        .iter()
        .any(|entry| entry.address == "%QX0.0" && entry.forced));

    let release = Request {
        seq: 2,
        message_type: MessageType::Request,
        command: "setExpression".to_string(),
        arguments: Some(
            serde_json::to_value(SetExpressionArguments {
                expression: "%QX0.0".to_string(),
                value: "release".to_string(),
                frame_id: None,
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(release);
    assert_eq!(outcome.responses.len(), 1);
    assert_eq!(outcome.events.len(), 1);
    let response: Response<SetExpressionResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "remote release failed: {:?}",
        response.message
    );
    let event: Event<IoStateEventBody> = serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert!(event
        .body
        .unwrap()
        .outputs
        .iter()
        .any(|entry| entry.address == "%QX0.0" && !entry.forced));

    drop(adapter);
    server.join().expect("server should stop cleanly");
    let seen = requests.lock().expect("requests").clone();
    let types = seen
        .iter()
        .filter_map(|request| request.get("type").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    assert!(
        types.contains(&"io.force"),
        "setExpression force should call io.force: {types:?}"
    );
    assert!(
        types.contains(&"io.unforce"),
        "setExpression release should call io.unforce: {types:?}"
    );
}

#[test]
fn attach_st_io_force_and_release_forward_remote_io_force_and_release() {
    let (addr, requests, server) = spawn_remote_io_force_server();
    let mut adapter = DebugAdapter::new(DebugSession::new());
    adapter.remote_session = Some(
        super::super::remote::RemoteSession::connect(
            super::super::remote::RemoteEndpoint::Tcp(addr),
            Some("token".to_string()),
        )
        .expect("remote session should connect"),
    );

    let force = Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "stIoForce".to_string(),
        arguments: Some(
            serde_json::to_value(IoWriteArguments {
                address: "%QX0.0".to_string(),
                value: "TRUE".to_string(),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(force);
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<serde_json::Value> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "stIoForce failed in attach mode: {:?}",
        response.message
    );
    assert_eq!(
        outcome.events.len(),
        0,
        "stIoForce must not emit an optimistic stale remote snapshot"
    );
    let state = Request::<serde_json::Value> {
        seq: 2,
        message_type: MessageType::Request,
        command: "stIoState".to_string(),
        arguments: None,
    };
    let outcome = adapter.dispatch_request(state);
    assert_eq!(outcome.events.len(), 1);
    let event: Event<IoStateEventBody> = serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert!(event
        .body
        .unwrap()
        .outputs
        .iter()
        .any(|entry| entry.address == "%QX0.0" && entry.forced));

    let release = Request {
        seq: 3,
        message_type: MessageType::Request,
        command: "stIoRelease".to_string(),
        arguments: Some(
            serde_json::to_value(IoReleaseArguments {
                address: "%QX0.0".to_string(),
            })
            .unwrap(),
        ),
    };
    let outcome = adapter.dispatch_request(release);
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<serde_json::Value> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(
        response.success,
        "stIoRelease failed in attach mode: {:?}",
        response.message
    );
    assert_eq!(
        outcome.events.len(),
        0,
        "stIoRelease must not emit an optimistic stale remote snapshot"
    );
    let state = Request::<serde_json::Value> {
        seq: 4,
        message_type: MessageType::Request,
        command: "stIoState".to_string(),
        arguments: None,
    };
    let outcome = adapter.dispatch_request(state);
    assert_eq!(outcome.events.len(), 1);
    let event: Event<IoStateEventBody> = serde_json::from_value(outcome.events[0].clone()).unwrap();
    assert!(event
        .body
        .unwrap()
        .outputs
        .iter()
        .any(|entry| entry.address == "%QX0.0" && !entry.forced));

    drop(adapter);
    server.join().expect("server should stop cleanly");
    let seen = requests.lock().expect("requests").clone();
    let types = seen
        .iter()
        .filter_map(|request| request.get("type").and_then(serde_json::Value::as_str))
        .collect::<Vec<_>>();
    assert!(
        types.contains(&"io.force"),
        "stIoForce should call io.force: {types:?}"
    );
    assert!(
        types.contains(&"io.unforce"),
        "stIoRelease should call io.unforce: {types:?}"
    );
}

fn spawn_remote_io_force_server() -> (
    SocketAddr,
    Arc<Mutex<Vec<serde_json::Value>>>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let server_requests = Arc::clone(&requests);
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client connection");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .expect("write timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut forced = false;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            let request: serde_json::Value =
                serde_json::from_str(&line).expect("control request json");
            server_requests
                .lock()
                .expect("requests lock")
                .push(request.clone());
            let id = request
                .get("id")
                .cloned()
                .unwrap_or_else(|| serde_json::json!(0));
            let response = match request.get("type").and_then(serde_json::Value::as_str) {
                Some("io.force") => {
                    forced = true;
                    serde_json::json!({"id": id, "ok": true, "result": {"status": "forced"}})
                }
                Some("io.unforce") => {
                    forced = false;
                    serde_json::json!({"id": id, "ok": true, "result": {"status": "released"}})
                }
                Some("io.read") => serde_json::json!({
                    "id": id,
                    "ok": true,
                    "result": {
                        "snapshot": {
                            "inputs": [],
                            "outputs": [{
                                "name": "OUT0",
                                "address": "%QX0.0",
                                "value": "Bool(false)",
                                "forced": forced
                            }],
                            "memory": []
                        }
                    }
                }),
                other => serde_json::json!({
                    "id": id,
                    "ok": false,
                    "error": format!("unexpected request {other:?}")
                }),
            };
            writeln!(stream, "{response}").expect("write control response");
        }
    });
    (addr, requests, handle)
}

/// Specification 35, A3.5 and A6.5: a live experiment that the simulator's control server starts runs
/// on the scans of the simulator's program (specification 44 NX-16), which keeps its PID loop
/// running against its simulated plant, and the Debug Adapter keeps its I/O view while the control
/// server serves Control Studio. Catches an experiment that stops the loop, a plant that does not
/// advance, and a control server that takes the I/O stream from the adapter.
#[test]
fn debug_control_server_experiment_keeps_the_typed_pid_loop_running() {
    let project_root = unique_project_root("debug-control-pid-experiment");
    let main_path = project_root.join("main.st");
    fs::write(
        &main_path,
        include_str!(
            "../../../../editors/vscode/src/test/fixtures/control-engineering/live-simulation-project/src/main.st"
        ),
    )
    .unwrap();
    fs::write(
        project_root.join("control.st"),
        include_str!("../../../../libraries/control/src/control.st"),
    )
    .unwrap();
    fs::write(
        project_root.join("io.toml"),
        include_str!(
            "../../../../editors/vscode/src/test/fixtures/control-engineering/live-simulation-project/io.toml"
        ),
    )
    .unwrap();
    fs::write(
        project_root.join("simulation.toml"),
        include_str!(
            "../../../../editors/vscode/src/test/fixtures/control-engineering/live-simulation-project/simulation.toml"
        ),
    )
    .unwrap();

    trust_runtime::native::Toolchain::find()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let mut adapter = DebugAdapter::new(DebugSession::new());
    let addr = reserve_loopback_addr();
    let auth = "debug-control-pid-token";
    launch_debug_control_session(&mut adapter, &project_root, &format!("tcp://{addr}"), auth);
    thread::sleep(Duration::from_millis(250));
    let scan_before = io_state_scan(&mut adapter)
        .expect("control server must not replace the Debug Adapter I/O stream");

    // The experiment advances on the scans that the compiled runtime publishes to the debug control,
    // with the scan's task runs and its simulation facts.
    let started = send_control_request(
        addr,
        auth,
        serde_json::json!({
            "id": 31,
            "type": "control.experiment.start",
            "params": {
                "experiment_id": "typed-lreal-step",
                "source": "simulation",
                "kind": "setpoint_step",
                "address": "%ML0",
                // The setpoint marker holds its initialized 20.0 (Specification 11 §6.5), which the
                // experiment restores.
                "value": 25,
                "restoration_value": 20,
                "value_type": "LREAL",
                "minimum": 0,
                "maximum": 100,
                "duration_ms": 10_000,
                "expected_scan": 1,
                // The experiment runs at the boundaries of the loop's task and checks its mode itself.
                "task": "MainTask",
                "mode_binding": { "symbol": "P1.AutoMode", "manual_value": false }
            }
        }),
    );
    assert_control_ok(&started, "control.experiment.start");

    thread::sleep(Duration::from_millis(350));
    let scan_after = io_state_scan(&mut adapter).expect("the Debug Adapter keeps its I/O view");
    let feedback = lreal_input(&adapter, "%IL0");
    adapter.stop_simulator();
    assert!(
        scan_after > scan_before,
        "debug control experiment stopped the PID loop at scan {scan_before}"
    );
    assert!(
        feedback.is_some_and(|value| value > 0.0),
        "debug runner must advance the configured simulated plant, got {feedback:?}"
    );

    fs::remove_dir_all(project_root).ok();
}

/// Specification 35, A3.5: a valid typed LREAL write to the marker the PID's setpoint is bound to
/// (`%ML0`) is applied at the next scan of the simulator's program, which goes on running. Catches a
/// debug runtime that faults the scan on a typed LREAL marker write.
#[test]
fn debug_runtime_accepts_a_typed_lreal_control_experiment_write() {
    let project_root = unique_project_root("control-experiment-lreal");
    let src = project_root.join("src");
    fs::create_dir_all(&src).unwrap();
    let main_path = src.join("main.st");
    fs::write(
        &main_path,
        include_str!(
            "../../../../editors/vscode/src/test/fixtures/control-engineering/live-simulation-project/src/main.st"
        ),
    )
    .unwrap();
    fs::write(
        src.join("control.st"),
        include_str!("../../../../libraries/control/src/control.st"),
    )
    .unwrap();
    fs::write(
        project_root.join("io.toml"),
        include_str!(
            "../../../../editors/vscode/src/test/fixtures/control-engineering/live-simulation-project/io.toml"
        ),
    )
    .unwrap();

    let (_session, simulator) = super::super::control_bridge::start_simulator(
        &project_root,
        &main_path,
        trust_runtime::control::ControlEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        None,
    );
    let state = simulator.control_server().state();
    let scan = first_snapshot_scan(&state);
    let queued = state.debug.try_enqueue_io_write(
        trust_runtime::io::IoAddress::parse("%ML0").unwrap(),
        trust_runtime::value::Value::LWord(5.0_f64.to_bits()),
    );
    // The write is applied in the scan after the one running now at the latest; the scan after
    // that one shows whether the program went on.
    let deadline = Instant::now() + Duration::from_secs(5);
    let fault = loop {
        if let Some(fault) = state.resource.last_error() {
            break Some(fault);
        }
        let latest = state
            .io_snapshot
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|snapshot| snapshot.scan);
        if latest.is_some_and(|latest| latest > scan + 1) {
            break None;
        }
        assert!(
            Instant::now() < deadline,
            "the simulator scanned no more after the write"
        );
        thread::sleep(Duration::from_millis(5));
    };
    simulator.stop();
    assert!(queued.is_ok(), "the write is refused: {queued:?}");
    assert!(
        fault.is_none(),
        "the debug runtime faulted after a valid typed LREAL write: {fault:?}"
    );

    fs::remove_dir_all(project_root).ok();
}

/// The scan of the I/O state that `stIoState` reports for the adapter's program.
fn io_state_scan(adapter: &mut DebugAdapter) -> Option<u64> {
    let outcome = adapter.dispatch_request(Request {
        seq: 40,
        message_type: MessageType::Request,
        command: "stIoState".to_string(),
        arguments: None,
    });
    outcome.events.iter().find_map(|value| {
        let event: Event<IoStateEventBody> = serde_json::from_value(value.clone()).ok()?;
        event.body?.scan
    })
}

/// The value of the LREAL input `address` in the I/O snapshot that the control server of the
/// adapter's simulator holds.
fn lreal_input(adapter: &DebugAdapter, address: &str) -> Option<f64> {
    let address = trust_runtime::io::IoAddress::parse(address).ok()?;
    let snapshot = adapter
        .simulator
        .as_ref()?
        .control_server()
        .state()
        .io_snapshot
        .lock()
        .ok()?
        .clone()?;
    let entry = snapshot
        .inputs
        .into_iter()
        .find(|entry| entry.address == address)?;
    match entry.value {
        trust_runtime::io::IoSnapshotValue::Value(trust_runtime::value::Value::LReal(value)) => {
            Some(value)
        }
        trust_runtime::io::IoSnapshotValue::Value(trust_runtime::value::Value::LWord(bits)) => {
            Some(f64::from_bits(bits))
        }
        _ => None,
    }
}

/// The scan of the first I/O snapshot that the control server `state` holds.
fn first_snapshot_scan(state: &trust_runtime::control::ControlState) -> u64 {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let scan = state
            .io_snapshot
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|snapshot| snapshot.scan);
        if let Some(scan) = scan {
            return scan;
        }
        assert!(
            Instant::now() < deadline,
            "the simulator published no I/O snapshot"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

pub(super) fn unique_project_root(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-debug-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

pub(super) fn reserve_loopback_addr() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    addr
}

fn initialize_adapter_for_launch(adapter: &mut DebugAdapter, seq: u32) {
    let outcome = adapter.dispatch_request(Request {
        seq,
        message_type: MessageType::Request,
        command: "initialize".to_string(),
        arguments: Some(serde_json::to_value(InitializeArguments::default()).unwrap()),
    });
    assert_eq!(outcome.responses.len(), 1);
    let response: Response<InitializeResponseBody> =
        serde_json::from_value(outcome.responses[0].clone()).unwrap();
    assert!(response.success, "initialize must succeed before launch");
}

/// Launches the program `main.st` of the project at `project_root` in `adapter` with its control
/// server at `endpoint`, authenticated with `auth`, and ends the configuration, as a client does.
pub(super) fn launch_debug_control_session(
    adapter: &mut DebugAdapter,
    project_root: &Path,
    endpoint: &str,
    auth: &str,
) {
    initialize_adapter_for_launch(adapter, 9);
    let mut additional = BTreeMap::new();
    additional.insert(
        "program".to_string(),
        serde_json::Value::String(project_root.join("main.st").display().to_string()),
    );
    additional.insert(
        "runtimeRoot".to_string(),
        serde_json::Value::String(project_root.display().to_string()),
    );
    additional.insert(
        "controlEndpoint".to_string(),
        serde_json::Value::String(endpoint.to_string()),
    );
    additional.insert(
        "controlAuthToken".to_string(),
        serde_json::Value::String(auth.to_string()),
    );

    let launch = Request {
        seq: 10,
        message_type: MessageType::Request,
        command: "launch".to_string(),
        arguments: Some(serde_json::to_value(LaunchArguments { additional }).unwrap()),
    };
    let launch_outcome = adapter.dispatch_request(launch);
    assert!(
        launch_outcome.responses.is_empty(),
        "launch should defer until configurationDone"
    );

    let configuration_done = Request {
        seq: 11,
        message_type: MessageType::Request,
        command: "configurationDone".to_string(),
        arguments: None,
    };
    let configured = adapter.dispatch_request(configuration_done);
    assert!(
        configured.responses.iter().all(|value| {
            let response: Response<serde_json::Value> =
                serde_json::from_value(value.clone()).unwrap();
            response.success
        }),
        "configurationDone/launch responses should succeed: {:?}",
        configured.responses
    );
}

pub(super) fn send_control_request(
    addr: SocketAddr,
    auth: &str,
    mut request: serde_json::Value,
) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut last_error = None;
    loop {
        match TcpStream::connect(addr) {
            Ok(mut stream) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                request
                    .as_object_mut()
                    .expect("control request should be an object")
                    .insert("auth".to_string(), serde_json::json!(auth));
                writeln!(stream, "{request}").unwrap();
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                return serde_json::from_str(&line).expect("control response should be json");
            }
            Err(error) if Instant::now() < deadline => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!(
                "control endpoint {addr} did not accept connections: last={last_error:?}, final={error}"
            ),
        }
    }
}

pub(super) fn assert_control_ok(response: &serde_json::Value, label: &str) {
    assert_eq!(
        response.get("ok").and_then(serde_json::Value::as_bool),
        Some(true),
        "{label} failed: {response}"
    );
}

pub(super) fn control_result(response: &serde_json::Value) -> &serde_json::Value {
    response.get("result").expect("control response result")
}
