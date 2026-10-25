//! Breakpoints that a client sets while an attach is pending (specification 13, session lifecycle):
//! VS Code sends them after `initialized` and before `configurationDone`, and the attach connects
//! only at `configurationDone`. On the C6 bench of 24 October 2026 the adapter dropped them, so an
//! attached session never stopped at a breakpoint the editor already had.

use super::*;
use crate::protocol::{AttachArguments, Breakpoint, BreakpointEventBody};
use serde_json::json;
use std::io::{BufRead, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;

/// A runtime's control endpoint that answers every connection, as a runtime serves the adapter's
/// requests and its stop poller: `debug.state` (running), `debug.stops` (none) and
/// `breakpoints.set`, which resolves each zero-based line up to 20 to itself at column 0 and skips
/// a later one, as a runtime skips a line after the file's last statement. It records every
/// request.
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
                    let id = request["id"].clone();
                    let result = match request["type"].as_str() {
                        Some("debug.state") => json!({"paused": false, "last_stop": null}),
                        Some("debug.stops") => json!({"stops": []}),
                        Some("breakpoints.set") => {
                            let resolved: Vec<Value> = request["params"]["lines"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(Value::as_u64)
                                .filter(|line| *line <= 20)
                                .map(|line| json!({"line": line, "column": 0}))
                                .collect();
                            json!({"status": "ok", "file_id": 0, "resolved": resolved, "generation": 1})
                        }
                        _ => json!({}),
                    };
                    let reply = json!({"id": id, "ok": true, "result": result});
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

/// The path the client names the source by: below the project, ending with its label.
const PATH: &str = "/work/plant/src/main.st";

/// The message of a breakpoint answered while the attach is pending.
const PENDING: &str = "pending until the attach completes";

/// A breakpoint of `PATH` as the adapter reports it.
fn breakpoint(
    id: u32,
    verified: bool,
    line: u32,
    column: Option<u32>,
    message: Option<&str>,
) -> Breakpoint {
    Breakpoint {
        id: Some(id),
        verified,
        message: message.map(str::to_string),
        source: Some(crate::protocol::Source {
            name: None,
            path: Some(PATH.to_string()),
            source_reference: None,
        }),
        line: Some(line),
        column,
        end_line: None,
        end_column: None,
    }
}

fn breakpoint_events(events: &[Value]) -> Vec<Breakpoint> {
    events
        .iter()
        .filter(|event| event["event"] == "breakpoint")
        .map(|event| {
            let body: BreakpointEventBody =
                serde_json::from_value(event["body"].clone()).expect("breakpoint event body");
            assert_eq!(body.reason, "changed");
            body.breakpoint
        })
        .collect()
}

#[test]
fn breakpoints_set_while_an_attach_is_pending_reach_the_runtime_once_it_connects() {
    // Specification 13, session lifecycle, worked by hand with one-based client lines: the client
    // asks for lines 30 and 12 of `/work/plant/src/main.st` while the attach is pending, and is
    // answered unverified, `pending until the attach completes`, with ids 1 and 2 in its order.
    // At `configurationDone` the attach connects and sends `breakpoints.set` with the zero-based
    // lines in ascending order, [11, 29]; the runtime resolves 11 and skips 29, so the events say
    // id 2 is verified at line 12, column 1, and id 1 is unverified with `no statement at or
    // after this line`. Catches breakpoints dropped by the attach, events without the pending
    // answer's ids, a line sent one-based, and a skipped line given another breakpoint's result.
    let (addr, requests) = spawn_runtime();
    let mut adapter = DebugAdapter::new(crate::DebugSession::new());
    let initialized = adapter.dispatch_request(request(1, "initialize", json!({})));
    assert_eq!(initialized.responses.len(), 1);
    let attach = AttachArguments {
        additional: [("endpoint".to_string(), json!(format!("tcp://{addr}")))]
            .into_iter()
            .collect(),
    };
    let pending = adapter.dispatch_request(request(
        2,
        "attach",
        serde_json::to_value(attach).expect("attach arguments"),
    ));
    assert!(
        pending.responses.is_empty(),
        "the attach waits for configurationDone"
    );

    let set = adapter.dispatch_request(request(
        3,
        "setBreakpoints",
        json!({
            "source": {"path": PATH},
            "breakpoints": [{"line": 30}, {"line": 12}],
        }),
    ));
    assert_eq!(set.responses.len(), 1);
    let answered: Vec<Breakpoint> =
        serde_json::from_value(set.responses[0]["body"]["breakpoints"].clone())
            .expect("breakpoints");
    assert_eq!(
        answered,
        [
            breakpoint(1, false, 30, None, Some(PENDING)),
            breakpoint(2, false, 12, None, Some(PENDING)),
        ]
    );
    assert!(
        requests.lock().expect("requests").is_empty(),
        "nothing reaches the runtime before the attach connects"
    );

    let configured = adapter.dispatch_request(request(4, "configurationDone", json!({})));
    let sequences: Vec<u64> = configured
        .responses
        .iter()
        .map(|response| response["request_seq"].as_u64().unwrap_or(0))
        .collect();
    assert_eq!(sequences, [4, 2], "configurationDone, then the attach");
    assert!(configured.responses[1]["success"]
        .as_bool()
        .unwrap_or(false));
    assert_eq!(
        breakpoint_events(&configured.events),
        [
            breakpoint(2, true, 12, Some(1), None),
            breakpoint(
                1,
                false,
                30,
                None,
                Some("no statement at or after this line")
            ),
        ]
    );
    let sets: Vec<Value> = requests
        .lock()
        .expect("requests")
        .iter()
        .filter(|request| request["type"] == "breakpoints.set")
        .map(|request| request["params"].clone())
        .collect();
    assert_eq!(sets, [json!({"source": PATH, "lines": [11, 29]})]);
}
