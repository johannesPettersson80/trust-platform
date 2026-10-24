//! The compiled simulator end to end on this computer (specification 44 NX-16): a launch builds the
//! project with the toolchain `TRUST_TOOLCHAIN` names (§4.6), runs it on trust-runtime's scheduler
//! with a control server at the launch's endpoint, and debugs it through the control protocol.

use super::remote::{RemoteEndpoint, RemoteSession};
use super::*;
use serde_json::json;
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
        std::fs::write(root.join("src/main.st"), MAIN).expect("program");
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
    let mut adapter = DebugAdapter::new(crate::DebugSession::new(
        trust_runtime::RuntimeBuilder::new().build().unwrap(),
    ));
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
