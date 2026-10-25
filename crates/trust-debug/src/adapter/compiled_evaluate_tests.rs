//! `evaluate` and `setBreakpoints` while attached (specification 11 §6.13.2): to a runtime that runs
//! a compiled program the adapter sends expressions compiled with the types of the project the
//! attach names, a breakpoint's condition, hit condition and logpoint message among them, and it
//! reads the control protocol's answers, whose fields are in snake case.

use super::*;
use crate::protocol::AttachArguments;
use base64::Engine as _;
use serde_json::json;
use std::io::{BufRead, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

/// A runtime's control endpoint whose `status` names the native backend, which answers
/// `debug.evaluate` with `TRUE` of type `BOOL`, resolves each zero-based line of a `breakpoints.set`
/// up to 20 to itself at column 0, answers `debug.stops` with one logpoint message, and records
/// every request.
fn spawn_runtime() -> (SocketAddr, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("address");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&requests);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { return };
            let seen = Arc::clone(&seen);
            thread::spawn(move || {
                let mut writer = stream.try_clone().expect("clone stream");
                for line in std::io::BufReader::new(stream).lines() {
                    let Ok(line) = line else { return };
                    let request: Value = serde_json::from_str(&line).expect("request json");
                    seen.lock().expect("requests").push(request.clone());
                    let result = match request["type"].as_str() {
                        Some("status") => json!({"execution_backend": "native"}),
                        Some("debug.state") => json!({"paused": false, "last_stop": null}),
                        Some("debug.evaluate") => {
                            json!({"result": "TRUE", "type": "BOOL", "variables_reference": 0})
                        }
                        Some("breakpoints.set") => {
                            let resolved: Vec<Value> = request["params"]["lines"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(Value::as_u64)
                                .filter(|line| *line <= 20)
                                .map(|line| json!({"line": line, "column": 0}))
                                .collect();
                            json!({"file_id": 0, "resolved": resolved, "generation": 1})
                        }
                        Some("debug.stops") => json!({
                            "stops": [],
                            "logs": [{"text": "speed Int(7)", "file_id": 0, "path": "src/main.st",
                                      "line": 5, "column": 4, "thread": 1}],
                        }),
                        _ => json!({}),
                    };
                    let reply = json!({"id": request["id"], "ok": true, "result": result});
                    if writeln!(writer, "{reply}").is_err() {
                        return;
                    }
                }
            });
        }
    });
    (addr, requests)
}

fn request(seq: u32, command: &str, arguments: Value) -> Request<Value> {
    Request {
        seq,
        message_type: crate::protocol::MessageType::Request,
        command: command.to_string(),
        arguments: Some(arguments),
    }
}

/// A project folder that declares the enumeration `Mode` of `Idle` and `Run` and a program with a
/// variable of it, removed when dropped.
struct Project(PathBuf);

impl Project {
    fn new(name: &str) -> Self {
        let folder = std::env::temp_dir().join(format!(
            "trust-debug-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        std::fs::create_dir_all(folder.join("src")).expect("project folder");
        std::fs::write(
            folder.join("src/types.st"),
            "TYPE Mode : (Idle, Run);\nEND_TYPE\n",
        )
        .expect("types");
        std::fs::write(
            folder.join("src/main.st"),
            "PROGRAM Main\nVAR\n    mode : Mode;\n    speed : INT;\nEND_VAR\nspeed := speed + 1;\nEND_PROGRAM\n",
        )
        .expect("program");
        Self(folder)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An adapter attached to the runtime at `addr`, with `extra` attach arguments.
fn attached(addr: SocketAddr, extra: &[(&str, Value)]) -> DebugAdapter {
    let mut adapter = DebugAdapter::new(crate::DebugSession::new());
    adapter.dispatch_request(request(1, "initialize", json!({})));
    let mut additional: std::collections::BTreeMap<String, Value> =
        [("endpoint".to_string(), json!(format!("tcp://{addr}")))]
            .into_iter()
            .collect();
    for (key, value) in extra {
        additional.insert((*key).to_string(), value.clone());
    }
    let attach = AttachArguments { additional };
    adapter.dispatch_request(request(
        2,
        "attach",
        serde_json::to_value(attach).expect("attach arguments"),
    ));
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

/// The parameters of every `debug.evaluate` the runtime received.
fn evaluations(requests: &Arc<Mutex<Vec<Value>>>) -> Vec<Value> {
    requests
        .lock()
        .expect("requests")
        .iter()
        .filter(|request| request["type"] == "debug.evaluate")
        .map(|request| request["params"].clone())
        .collect()
}

/// Specification 11 §6.13.2, worked by hand: attached to a runtime whose `status` says `native`
/// and with the project folder as `runtimeRoot`, `mode = Mode#Run` goes as `expr_version` 1 and
/// the bytes `=` (4, 12) over the name `mode` (2, length 4) and the enumeration literal (1, 22) of
/// `Mode` and `Run`, and the answer `TRUE` of `BOOL` reaches the client. Catches ST text sent to a
/// runtime that parses none, an enumeration the adapter cannot compile because it never loaded the
/// project's types, and the runtime's snake-case answer read as camel case.
#[test]
fn a_compiled_runtime_receives_the_compiled_expression() {
    let project = Project::new("compiled");
    let (addr, requests) = spawn_runtime();
    let mut adapter = attached(addr, &[("runtimeRoot", json!(project.0.to_string_lossy()))]);
    let answer = adapter.dispatch_request(request(
        4,
        "evaluate",
        json!({"expression": "mode = Mode#Run"}),
    ));
    assert_eq!(answer.responses.len(), 1);
    let response = &answer.responses[0];
    assert!(response["success"].as_bool().unwrap_or(false), "{response}");
    assert_eq!(response["body"]["result"], "TRUE");
    assert_eq!(response["body"]["type"], "BOOL");
    let sent = evaluations(&requests);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0]["expr_version"], 1);
    assert!(
        sent[0].get("expression").is_none(),
        "no ST text: {}",
        sent[0]
    );
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(sent[0]["expr"].as_str().expect("expr"))
        .expect("base64");
    assert_eq!(
        bytes,
        [
            4, 12, 2, 4, 0, b'm', b'o', b'd', b'e', 1, 22, 4, 0, b'M', b'o', b'd', b'e', 3, 0,
            b'R', b'u', b'n'
        ]
    );
}

/// Specification 11 §6.13.2: an expression with a part version 1 cannot say, `ABS(speed)`, is
/// refused by the adapter and no request reaches the runtime. Catches a call sent for the runtime
/// to refuse, or dropped and something else sent.
#[test]
fn an_expression_version_1_cannot_say_never_reaches_the_runtime() {
    let project = Project::new("refused");
    let (addr, requests) = spawn_runtime();
    let mut adapter = attached(addr, &[("runtimeRoot", json!(project.0.to_string_lossy()))]);
    let answer =
        adapter.dispatch_request(request(4, "evaluate", json!({"expression": "ABS(speed)"})));
    let response = &answer.responses[0];
    assert!(!response["success"].as_bool().unwrap_or(true), "{response}");
    assert!(
        response["message"]
            .as_str()
            .unwrap_or_default()
            .contains("a call cannot be compiled into a debug expression of version 1"),
        "{response}"
    );
    assert_eq!(evaluations(&requests), Vec::<Value>::new());
}

/// The parameters of every `breakpoints.set` the runtime received.
fn breakpoint_sets(requests: &Arc<Mutex<Vec<Value>>>) -> Vec<Value> {
    requests
        .lock()
        .expect("requests")
        .iter()
        .filter(|request| request["type"] == "breakpoints.set")
        .map(|request| request["params"].clone())
        .collect()
}

/// The source the client names its breakpoints by.
const PATH: &str = "/work/plant/src/main.st";

/// The base64 of `bytes`, as a compiled expression carries them.
fn base64_of(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Specification 11 §6.13.2, worked by hand: to a compiled runtime, the client's lines 6 to 9
/// go as the zero-based lines 5 to 8 with their options: `speed > INT#10` compiled as `>` (4, 16)
/// over the name `speed` (2, length 5) and the INT literal 10 (1, 3, `0A 00`); the hit condition
/// `>= 3` as `{"op": ">=", "count": 3}`; the message `speed {speed}!` as its text, the compiled name
/// and its text; and `null` for the plain breakpoint. Every breakpoint is answered verified, in the
/// request's order. Catches ST text sent to a runtime that parses none, options dropped or given to
/// another line, and a hit condition sent as the client's text.
#[test]
fn a_compiled_runtime_receives_each_breakpoints_options() {
    let project = Project::new("options");
    let (addr, requests) = spawn_runtime();
    let mut adapter = attached(addr, &[("runtimeRoot", json!(project.0.to_string_lossy()))]);
    let answer = adapter.dispatch_request(request(
        4,
        "setBreakpoints",
        json!({
            "source": {"path": PATH},
            "breakpoints": [
                {"line": 6, "condition": "speed > INT#10"},
                {"line": 7, "hitCondition": ">= 3"},
                {"line": 8, "logMessage": "speed {speed}!"},
                {"line": 9},
            ],
        }),
    ));
    let speed = [2, 5, 0, b's', b'p', b'e', b'e', b'd'];
    let mut condition = vec![4, 16];
    condition.extend_from_slice(&speed);
    condition.extend_from_slice(&[1, 3, 10, 0]);
    assert_eq!(
        breakpoint_sets(&requests),
        [json!({
            "source": PATH,
            "lines": [5, 6, 7, 8],
            "options": [
                {"condition": {"expr_version": 1, "expr": base64_of(&condition)}},
                {"hit_condition": {"op": ">=", "count": 3}},
                {"log_message": ["speed ", {"expr_version": 1, "expr": base64_of(&speed)}, "!"]},
                null,
            ],
        })]
    );
    let answered = &answer.responses[0]["body"]["breakpoints"];
    let lines: Vec<Value> = answered
        .as_array()
        .expect("breakpoints")
        .iter()
        .map(|breakpoint| json!([breakpoint["line"], breakpoint["verified"]]))
        .collect();
    assert_eq!(
        lines,
        [
            json!([6, true]),
            json!([7, true]),
            json!([8, true]),
            json!([9, true])
        ]
    );
}

/// Specification 11 §6.13.2: a condition the adapter cannot compile, a call, answers its breakpoint
/// unverified with the reason, and no breakpoint of the file reaches the runtime. Catches a
/// breakpoint sent without the condition the client asked for.
#[test]
fn a_condition_that_does_not_compile_leaves_its_breakpoint_unverified() {
    let project = Project::new("uncompiled");
    let (addr, requests) = spawn_runtime();
    let mut adapter = attached(addr, &[("runtimeRoot", json!(project.0.to_string_lossy()))]);
    let answer = adapter.dispatch_request(request(
        4,
        "setBreakpoints",
        json!({
            "source": {"path": PATH},
            "breakpoints": [{"line": 6, "condition": "ABS(speed) > INT#1"}],
        }),
    ));
    assert_eq!(breakpoint_sets(&requests), Vec::<Value>::new());
    let answered = &answer.responses[0]["body"]["breakpoints"][0];
    assert_eq!(answered["verified"], false);
    assert!(
        answered["message"]
            .as_str()
            .unwrap_or_default()
            .contains("a call cannot be compiled"),
        "{answered}"
    );
}

/// Specification 11 §6.13.2: `debug.stops` returns the logpoint messages beside the stops, each
/// with its text, path and zero-based position. Catches the messages ignored, and a message read
/// without its source.
#[test]
fn debug_stops_returns_the_logpoint_messages() {
    let (addr, _) = spawn_runtime();
    let mut session =
        super::remote::RemoteSession::connect(super::remote::RemoteEndpoint::Tcp(addr), None)
            .expect("connect");
    let (stops, logs) = session.debug_stops().expect("debug.stops");
    assert_eq!(stops.len(), 0);
    assert_eq!(
        logs,
        [super::remote::RemoteLog {
            text: String::from("speed Int(7)"),
            path: Some(String::from("src/main.st")),
            line: Some(5),
            column: Some(4),
        }]
    );
}
