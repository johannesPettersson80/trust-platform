//! The compiled simulator end to end on this computer (specification 44 NX-16): a launch builds the
//! project with the toolchain `TRUST_TOOLCHAIN` names (§4.6), runs it on trust-runtime's scheduler
//! with a control server at the launch's endpoint, and debugs it through the control protocol.

use super::remote::{RemoteEndpoint, RemoteSession};
use super::*;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::{Duration as WallDuration, Instant};

/// The project's types.
const TYPES: &str = "TYPE Mode : (Idle, Run);\nEND_TYPE\n";

/// The program, `count` counting the scans; line 7 is the `IF` that sets `mode` from the fourth.
const MAIN: &str = "PROGRAM Main\nVAR\n    count : INT;\n    mode : Mode;\nEND_VAR\ncount := count + 1;\nIF count > 3 THEN\n    mode := Mode#Run;\nEND_IF\nEND_PROGRAM\n";

/// A project folder with `MAIN` and `TYPES` in `src`, and a socket path for its control server,
/// both removed when dropped.
struct Project {
    root: PathBuf,
    socket: PathBuf,
}

impl Project {
    fn new() -> Self {
        Self::with_main(MAIN)
    }

    /// The project with `main` in place of [`MAIN`].
    fn with_main(main: &str) -> Self {
        let stamp = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        );
        let root = std::env::temp_dir().join(format!("trust-simulator-{stamp}"));
        std::fs::create_dir_all(root.join("src")).expect("project folder");
        std::fs::write(root.join("src/types.st"), TYPES).expect("types");
        std::fs::write(root.join("src/main.st"), main).expect("program");
        let socket = std::env::temp_dir().join(format!("trust-simulator-{stamp}.sock"));
        Self { root, socket }
    }

    fn main(&self) -> PathBuf {
        self.root.join("src/main.st")
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
        let _ = std::fs::remove_file(&self.socket);
    }
}

fn request(seq: u32, command: &str, arguments: Value) -> Request<Value> {
    Request {
        seq,
        message_type: crate::protocol::MessageType::Request,
        command: command.to_string(),
        arguments: Some(arguments),
    }
}

/// The one response of `outcome`, which succeeded.
fn succeeded(outcome: &DispatchOutcome) -> &Value {
    let response = outcome.responses.last().expect("a response");
    assert!(response["success"].as_bool().unwrap_or(false), "{response}");
    response
}

/// Specification 44 NX-16 and specification 11 §6.13.2, worked by hand: the breakpoint on line 7
/// with the condition `count = 5`, set while the launch waits for the configuration, reaches the
/// compiled program and holds its fifth scan; there, in the held frame, `count * INT#2` is 10 and
/// `mode = Mode#Run` is TRUE (the fourth scan set it); `continue` releases the scan, `stReload`
/// changes the program online with the program built again, and `disconnect` ends the session.
/// Catches a launch that runs the bytecode engine though the toolchain builds the program,
/// breakpoints set before the launch that never reach the runtime, a condition dropped (the first
/// scan would stop with `count` 1), an evaluation that misses the held frame or the project's
/// enumeration, and a reload refused for a compiled launch.
#[test]
fn a_launch_runs_the_program_compiled_and_debugs_it_through_the_control_protocol() {
    trust_runtime::native::Toolchain::find()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let project = Project::new();
    let mut adapter = DebugAdapter::new(crate::DebugSession::new());
    adapter.dispatch_request(request(1, "initialize", json!({})));
    let launch = adapter.dispatch_request(request(
        2,
        "launch",
        json!({
            "program": project.main().to_string_lossy(),
            "runtimeRoot": project.root.to_string_lossy(),
            "controlEndpoint": format!("unix://{}", project.socket.display()),
        }),
    ));
    assert!(launch.responses.is_empty(), "the launch waits");
    let pending = adapter.dispatch_request(request(
        3,
        "setBreakpoints",
        json!({
            "source": {"path": project.main().to_string_lossy()},
            "breakpoints": [{"line": 7, "condition": "count = INT#5"}],
        }),
    ));
    assert_eq!(
        succeeded(&pending)["body"]["breakpoints"][0]["message"],
        "pending until the launch completes"
    );

    let configured = adapter.dispatch_request(request(4, "configurationDone", json!({})));
    assert!(
        configured
            .responses
            .iter()
            .all(|response| response["success"].as_bool().unwrap_or(false)),
        "{:?}",
        configured.responses
    );
    assert!(adapter.simulator.is_some(), "the program runs compiled");
    let changed: Vec<&Value> = configured
        .events
        .iter()
        .filter(|event| event["event"] == "breakpoint")
        .collect();
    assert_eq!(changed.len(), 1, "{:?}", configured.events);
    assert_eq!(
        changed[0]["body"]["breakpoint"]["verified"], true,
        "{}",
        changed[0]
    );
    assert_eq!(changed[0]["body"]["breakpoint"]["line"], 7);

    let mut client = RemoteSession::connect(RemoteEndpoint::Unix(project.socket.clone()), None)
        .expect("connect");
    let deadline = Instant::now() + WallDuration::from_secs(30);
    while !client.debug_state().expect("debug.state").paused {
        assert!(Instant::now() < deadline, "no hold within 30 s");
        std::thread::sleep(WallDuration::from_millis(20));
    }

    let stack = adapter.dispatch_request(request(5, "stackTrace", json!({"threadId": 1})));
    let frame = succeeded(&stack)["body"]["stackFrames"][0]["id"].clone();
    for (expression, result) in [("count * INT#2", "10"), ("mode = Mode#Run", "TRUE")] {
        let answer = adapter.dispatch_request(request(
            6,
            "evaluate",
            json!({"expression": expression, "frameId": frame}),
        ));
        assert_eq!(succeeded(&answer)["body"]["result"], result, "{expression}");
    }

    succeeded(&adapter.dispatch_request(request(7, "continue", json!({"threadId": 1}))));
    std::fs::write(
        project.main(),
        MAIN.replace("count := count + 1;", "count := count + 2;"),
    )
    .expect("the changed program");
    succeeded(&adapter.dispatch_request(request(8, "stReload", json!({}))));
    succeeded(&adapter.dispatch_request(request(9, "disconnect", json!({}))));
    assert!(adapter.simulator.is_none());
}

/// A loopback address that no listener holds when it is returned.
fn reserve_loopback_addr() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("address");
    drop(listener);
    addr
}

/// The adapter after the launch of `project` with its control server at `endpoint`, authenticated
/// with `auth`, and `configurationDone`.
fn launched(project: &Project, endpoint: &str, auth: &str) -> DebugAdapter {
    let mut adapter = DebugAdapter::new(crate::DebugSession::new());
    adapter.dispatch_request(request(1, "initialize", json!({})));
    let launch = adapter.dispatch_request(request(
        2,
        "launch",
        json!({
            "program": project.main().to_string_lossy(),
            "runtimeRoot": project.root.to_string_lossy(),
            "controlEndpoint": endpoint,
            "controlAuthToken": auth,
        }),
    ));
    assert!(launch.responses.is_empty(), "the launch waits");
    let configured = adapter.dispatch_request(request(3, "configurationDone", json!({})));
    assert!(
        configured
            .responses
            .iter()
            .all(|response| response["success"].as_bool().unwrap_or(false)),
        "{:?}",
        configured.responses
    );
    adapter
}

/// The control server's answer at `addr` to `request`, which carries `auth`.
fn send_control_request(addr: SocketAddr, auth: &str, mut request: Value) -> Value {
    let deadline = Instant::now() + WallDuration::from_secs(2);
    loop {
        match TcpStream::connect(addr) {
            Ok(mut stream) => {
                stream
                    .set_read_timeout(Some(WallDuration::from_secs(2)))
                    .expect("read timeout");
                request
                    .as_object_mut()
                    .expect("a control request is an object")
                    .insert("auth".to_string(), json!(auth));
                writeln!(stream, "{request}").expect("send the request");
                let mut line = String::new();
                BufReader::new(stream)
                    .read_line(&mut line)
                    .expect("read the answer");
                return serde_json::from_str(&line).expect("the answer is JSON");
            }
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "the control endpoint {addr} accepts no connection: {error}"
                );
                std::thread::sleep(WallDuration::from_millis(20));
            }
        }
    }
}

/// The result of a control answer that succeeded.
fn control_result<'a>(response: &'a Value, label: &str) -> &'a Value {
    assert_eq!(response["ok"], json!(true), "{label} failed: {response}");
    &response["result"]
}

/// Specification 44 NX-16: a launch whose control endpoint is already in use fails, and no program
/// runs in the adapter. Catches a launch that answers success with a runtime that no client can
/// reach.
#[test]
fn a_launch_on_a_busy_control_endpoint_fails() {
    trust_runtime::native::Toolchain::find()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let project = Project::new();
    let occupied = TcpListener::bind("127.0.0.1:0").expect("bind");
    let mut adapter = DebugAdapter::new(crate::DebugSession::new());
    adapter.dispatch_request(request(1, "initialize", json!({})));
    let launch = adapter.dispatch_request(request(
        2,
        "launch",
        json!({
            "program": project.main().to_string_lossy(),
            "runtimeRoot": project.root.to_string_lossy(),
            "controlEndpoint": format!("tcp://{}", occupied.local_addr().expect("address")),
            "controlAuthToken": "debug-control-token",
        }),
    ));
    assert!(launch.responses.is_empty(), "the launch waits");
    let configured = adapter.dispatch_request(request(3, "configurationDone", json!({})));
    assert!(
        configured.responses.iter().any(|response| {
            response["command"] == "launch" && !response["success"].as_bool().unwrap_or(true)
        }),
        "{:?}",
        configured.responses
    );
    assert!(adapter.simulator.is_none(), "no program runs");
    assert!(adapter.remote_session.is_none());
}

/// Specification 44 NX-16 with the control protocol's `comm.apply`: the simulator's control server
/// edits the `io.toml` of the launch's project folder. The first Modbus driver creates the file, a
/// second one is added beside the first, and each answer says that a restart is required. Catches a
/// control server that writes another folder, and one that replaces the drivers already there.
#[test]
fn the_simulator_applies_communication_settings_to_the_launch_project() {
    trust_runtime::native::Toolchain::find()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let project = Project::new();
    let addr = reserve_loopback_addr();
    let auth = "debug-control-token";
    let mut adapter = launched(&project, &format!("tcp://{addr}"), auth);

    let schema = send_control_request(
        addr,
        auth,
        json!({"id": 1, "type": "comm.schema", "params": {"protocol": "modbus_tcp"}}),
    );
    control_result(&schema, "comm.schema");
    let topology = send_control_request(addr, auth, json!({"id": 2, "type": "fleet.topology"}));
    control_result(&topology, "fleet.topology");

    let modbus = |id: u32, address: &str, unit_id: u32, input_start: u32, output_start: u32| {
        json!({
            "id": id,
            "type": "comm.apply",
            "params": {
                "protocol": "modbus_tcp",
                "action": "add",
                "params": {
                    "address": address,
                    "unit_id": unit_id,
                    "input_start": input_start,
                    "output_start": output_start,
                    "timeout_ms": 750,
                    "on_error": "warn"
                }
            }
        })
    };
    let create = send_control_request(addr, auth, modbus(3, "127.0.0.1:1502", 2, 5, 7));
    let created = control_result(&create, "comm.apply create");
    assert_eq!(created["lifecycle_effect"], "restart_required", "{create}");
    assert_eq!(created["applied"], true, "{create}");
    let io_toml = project.root.join("io.toml");
    let created_text = std::fs::read_to_string(&io_toml).expect("comm.apply creates io.toml");
    assert!(created_text.contains("127.0.0.1:1502"), "{created_text}");

    let update = send_control_request(addr, auth, modbus(4, "127.0.0.1:1503", 3, 9, 11));
    let updated = control_result(&update, "comm.apply update");
    assert_eq!(updated["lifecycle_effect"], "restart_required", "{update}");
    let updated_text = std::fs::read_to_string(&io_toml).expect("comm.apply updates io.toml");
    assert!(updated_text.contains("127.0.0.1:1502"), "{updated_text}");
    assert!(updated_text.contains("127.0.0.1:1503"), "{updated_text}");
    assert!(
        updated_text.matches("modbus-tcp").count() >= 2,
        "{updated_text}"
    );
    succeeded(&adapter.dispatch_request(request(9, "disconnect", json!({}))));
}

/// Specification 44 NX-16 with the HMI's control requests: the simulator's control server serves
/// the HMI schema of the launch's project and the values the compiled program publishes, here
/// `Main.speed` with its starting value 1.5 and good quality once the program has run. Catches a
/// control server without the project's HMI, and values that do not come from the running
/// program.
#[test]
fn the_simulator_serves_the_hmi_schema_and_values() {
    trust_runtime::native::Toolchain::find()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let project = Project::with_main(
        "PROGRAM Main\nVAR\n    speed : REAL := 1.5;\n    running : BOOL := TRUE;\nEND_VAR\nEND_PROGRAM\n",
    );
    let addr = reserve_loopback_addr();
    let auth = "debug-control-hmi-token";
    let mut adapter = launched(&project, &format!("tcp://{addr}"), auth);

    let schema = send_control_request(addr, auth, json!({"id": 5, "type": "hmi.schema.get"}));
    let speed_id = control_result(&schema, "hmi.schema.get")["widgets"]
        .as_array()
        .expect("the schema's widgets")
        .iter()
        .find(|widget| widget["path"] == "Main.speed")
        .and_then(|widget| widget["id"].as_str())
        .expect("Main.speed is in the HMI schema")
        .to_string();

    let deadline = Instant::now() + WallDuration::from_secs(30);
    loop {
        let values = send_control_request(
            addr,
            auth,
            json!({"id": 6, "type": "hmi.values.get", "params": {"ids": [speed_id]}}),
        );
        let result = control_result(&values, "hmi.values.get");
        let record = &result["values"][&speed_id];
        if result["connected"] == true && record["q"] == "good" {
            assert_eq!(record["v"].as_f64(), Some(1.5), "{values}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no good value within 30 s: {values}"
        );
        std::thread::sleep(WallDuration::from_millis(20));
    }
    succeeded(&adapter.dispatch_request(request(9, "disconnect", json!({}))));
}
