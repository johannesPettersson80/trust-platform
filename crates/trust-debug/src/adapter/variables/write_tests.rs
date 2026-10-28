//! `setVariable` and `setExpression` on a runtime that runs a compiled program (specification 13,
//! "setVariable and setExpression"): the runtime requests the adapter makes while the debugger
//! holds the scan and between scans, for a write, a force (`force: X`) and a release (`release`);
//! an I/O address and an I/O row, which the I/O requests write; the refusals, which reach the
//! user; and a busy runtime, to which the adapter sends a write again. The runtime is a fake control
//! endpoint that answers as specification 11 §6.13.4 says `debug.set` answers, and records each
//! request with its parameters.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::protocol::{
    Event, IoWriteArguments, MessageType, Request, Response, SetExpressionArguments,
    SetExpressionResponseBody, SetVariableArguments,
};
use crate::DebugSession;

use super::super::remote::{RemoteEndpoint, RemoteSession};
use super::super::DebugAdapter;

/// The requests the fake runtime received, in order: each one's type and parameters.
type Received = Arc<Mutex<Vec<(String, Value)>>>;

/// How the fake runtime answers a request of a type with its parameters: the result, or the
/// refusal's text.
type Answer = Box<dyn FnMut(&str, &Value) -> Result<Value, String> + Send>;

/// The refusal of a runtime whose command queue is full, which nothing was queued by; the fake
/// sends it with the stable code `resource_busy`, as the runtime does (specification 11 §6.9).
const BUSY: &str = "resource command queue is full; the request was not queued";

/// A runtime's control endpoint on a loopback port that answers each request with `answer`, and
/// what it received.
fn fake_runtime(mut answer: Answer) -> (SocketAddr, Received, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let addr = listener.local_addr().expect("the port's address");
    let received: Received = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&received);
    let server = thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("a read timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("a second handle"));
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let request: Value = serde_json::from_str(&line).expect("a request is one JSON line");
            let kind = request["type"].as_str().unwrap_or_default().to_string();
            let params = request["params"].clone();
            seen.lock()
                .expect("the requests")
                .push((kind.clone(), params.clone()));
            let id = request["id"].clone();
            let reply = match answer(&kind, &params) {
                Ok(result) => json!({"id": id, "ok": true, "result": result}),
                Err(error) if error == BUSY => {
                    json!({"id": id, "ok": false, "error": error, "error_code": "resource_busy"})
                }
                Err(error) => json!({"id": id, "ok": false, "error": error}),
            };
            if writeln!(stream, "{reply}").is_err() {
                break;
            }
        }
    });
    (addr, received, server)
}

/// An adapter attached to the runtime at `addr`.
fn attached(addr: SocketAddr) -> DebugAdapter {
    let mut adapter = DebugAdapter::new(DebugSession::new());
    adapter.remote_session = Some(
        RemoteSession::connect(RemoteEndpoint::Tcp(addr), None).expect("the session connects"),
    );
    adapter
}

/// Ends the session and the fake runtime, and returns what the runtime received.
fn finish(
    adapter: DebugAdapter,
    received: &Received,
    server: thread::JoinHandle<()>,
) -> Vec<(String, Value)> {
    drop(adapter);
    server.join().expect("the fake runtime ends");
    received.lock().expect("the requests").clone()
}

fn set_variable(reference: u32, name: &str, value: &str) -> Request<Value> {
    Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setVariable".to_string(),
        arguments: Some(
            serde_json::to_value(SetVariableArguments {
                variables_reference: reference,
                name: name.to_string(),
                value: value.to_string(),
            })
            .expect("the arguments serialize"),
        ),
    }
}

fn io_write(address: &str, value: &str) -> Request<Value> {
    Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "stIoWrite".to_string(),
        arguments: Some(
            serde_json::to_value(IoWriteArguments {
                address: address.to_string(),
                value: value.to_string(),
            })
            .expect("the arguments serialize"),
        ),
    }
}

fn set_expression(expression: &str, value: &str, frame_id: Option<u32>) -> Request<Value> {
    Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setExpression".to_string(),
        arguments: Some(
            serde_json::to_value(SetExpressionArguments {
                expression: expression.to_string(),
                value: value.to_string(),
                frame_id,
            })
            .expect("the arguments serialize"),
        ),
    }
}

/// What one request answered: the response, and the names of the events that came with it.
struct Answered {
    response: Response<SetExpressionResponseBody>,
    events: Vec<Event<Value>>,
}

impl Answered {
    fn event_names(&self) -> Vec<&str> {
        self.events
            .iter()
            .map(|event| event.event.as_str())
            .collect()
    }

    /// The value the response answers with.
    fn value(&self) -> (&str, Option<&str>) {
        let body = self.response.body.as_ref().expect("a response with a body");
        (body.value.as_str(), body.r#type.as_deref())
    }
}

fn dispatch(adapter: &mut DebugAdapter, request: Request<Value>) -> Answered {
    let outcome = adapter.dispatch_request(request);
    assert_eq!(outcome.responses.len(), 1, "one response");
    Answered {
        response: serde_json::from_value(outcome.responses[0].clone()).expect("a response"),
        events: outcome
            .events
            .into_iter()
            .map(|event| serde_json::from_value(event).expect("an event"))
            .collect(),
    }
}

/// The I/O state of the fake runtime: one output, `lamp` at `%QX0.0`, forced or not.
fn io_snapshot(forced: bool) -> Value {
    json!({"snapshot": {
        "inputs": [],
        "outputs": [{"name": "lamp", "address": "%QX0.0", "value": "Bool(false)", "forced": forced}],
        "memory": []
    }})
}

fn requests(list: &[(&str, Value)]) -> Vec<(String, Value)> {
    list.iter()
        .map(|(kind, params)| ((*kind).to_string(), params.clone()))
        .collect()
}

#[test]
fn set_variable_at_a_hold_writes_once_into_the_held_scan_and_answers_the_runtime_value() {
    // Specification 13 with specification 44 §13.2: while the debugger holds the scan, `debug.set`
    // writes the value into the held scan, so nothing is queued after it; the answer is the value
    // the runtime holds, 41 for `16#29`, with its type, and `invalidated` makes the client read the
    // variables again. Catches a write at a hold queued again with `set` (it would be written a
    // second time at the next cycle start), an answer of the typed text, and no refresh.
    let (addr, received, server) = fake_runtime(Box::new(|kind, _| match kind {
        "debug.set" => {
            Ok(json!({"path": "Main.Count", "type": "INT", "value": "41", "written": true}))
        }
        other => Err(format!("unexpected request {other}")),
    }));
    let mut adapter = attached(addr);
    let answered = dispatch(&mut adapter, set_variable(7, "Count", "16#29"));
    let seen = finish(adapter, &received, server);

    assert!(answered.response.success, "{:?}", answered.response.message);
    assert_eq!(answered.value(), ("41", Some("INT")));
    assert_eq!(answered.event_names(), ["invalidated"]);
    assert_eq!(
        answered.events[0].body,
        Some(json!({"areas": ["variables"]}))
    );
    assert_eq!(
        seen,
        requests(&[(
            "debug.set",
            json!({"variables_reference": 7, "name": "Count", "value": "16#29"})
        )])
    );
}

#[test]
fn set_variable_between_scans_queues_the_write_by_the_path_the_runtime_names() {
    // Specification 13 with specification 11 §6.13.4 and "Live Values": between scans `debug.set`
    // writes nothing and names the variable's path, and the adapter queues the write with `set` by
    // that path for the next cycle start. Catches a write between scans that is never queued, and
    // one queued by the listing's name, which names no variable of the program.
    let (addr, received, server) = fake_runtime(Box::new(|kind, _| match kind {
        "debug.set" => Ok(json!({
            "path": "Main.Pumps[2].Speed", "type": "INT", "value": "5", "written": false
        })),
        "set" => Ok(json!({"status": "queued"})),
        other => Err(format!("unexpected request {other}")),
    }));
    let mut adapter = attached(addr);
    let answered = dispatch(&mut adapter, set_variable(9, "Speed", "5"));
    let seen = finish(adapter, &received, server);

    assert!(answered.response.success, "{:?}", answered.response.message);
    assert_eq!(answered.value(), ("5", Some("INT")));
    assert_eq!(answered.event_names(), ["invalidated"]);
    assert_eq!(
        seen,
        requests(&[
            (
                "debug.set",
                json!({"variables_reference": 9, "name": "Speed", "value": "5"})
            ),
            (
                "set",
                json!({"target": "global:Main.Pumps[2].Speed", "value": "5"})
            ),
        ])
    );
}

#[test]
fn set_expression_forces_and_releases_a_path_in_the_frame_it_names() {
    // Specification 13: `force: 7` on `Main.Pumps[2].Speed`, held in frame 3, is written into the
    // held scan by `debug.set` with `force`, looked up from that frame, and held with `var.force`
    // by the path the runtime names; `release` reads the variable for its path and ends the force
    // with `var.unforce`. A path that names no I/O row is a variable: the adapter asks the I/O
    // state first. Catches a force that is only written once, a release that leaves the force, the
    // frame lost from the lookup, and a force or release by the expression's text.
    let (addr, received, server) = fake_runtime(Box::new(|kind, params| match kind {
        "io.read" => Ok(io_snapshot(false)),
        "debug.set" => Ok(json!({
            "path": "Main.Pumps[2].Speed",
            "type": "INT",
            "value": "7",
            "written": params.get("value").is_some(),
        })),
        "var.force" => Ok(json!({"status": "forced"})),
        "var.unforce" => Ok(json!({"status": "released"})),
        other => Err(format!("unexpected request {other}")),
    }));
    let mut adapter = attached(addr);
    let forced = dispatch(
        &mut adapter,
        set_expression("Main.Pumps[2].Speed", "force: 7", Some(3)),
    );
    let released = dispatch(
        &mut adapter,
        set_expression("Main.Pumps[2].Speed", "release", Some(3)),
    );
    let seen = finish(adapter, &received, server);

    assert!(forced.response.success, "{:?}", forced.response.message);
    assert!(released.response.success, "{:?}", released.response.message);
    assert_eq!(forced.value(), ("7", Some("INT")));
    assert_eq!(forced.event_names(), ["invalidated"]);
    assert_eq!(released.event_names(), ["invalidated"]);
    assert_eq!(
        seen,
        requests(&[
            ("io.read", Value::Null),
            (
                "debug.set",
                json!({"path": "Main.Pumps[2].Speed", "frame_id": 3, "value": "7", "force": true})
            ),
            (
                "var.force",
                json!({"target": "global:Main.Pumps[2].Speed", "value": "7"})
            ),
            ("io.read", Value::Null),
            (
                "debug.set",
                json!({"path": "Main.Pumps[2].Speed", "frame_id": 3})
            ),
            (
                "var.unforce",
                json!({"target": "global:Main.Pumps[2].Speed"})
            ),
        ])
    );
}

#[test]
fn an_io_address_and_an_io_row_are_written_with_the_io_requests() {
    // Specification 13: an I/O address in `setExpression`, a row name of the I/O state, and a row
    // of the I/O scope in `setVariable` (which `debug.set` answers with its address) are written,
    // forced and released at the address with `io.write`, `io.force` and `io.unforce`, and the
    // I/O state is sent again, the force marked. Catches an address taken for a variable path,
    // a row written by `set`, and a force the I/O view does not show.
    let mut forced = false;
    let (addr, received, server) = fake_runtime(Box::new(move |kind, _| match kind {
        "io.read" => Ok(io_snapshot(forced)),
        "io.write" => Ok(json!({"status": "queued"})),
        "io.force" => {
            forced = true;
            Ok(json!({"status": "forced"}))
        }
        "debug.set" => Ok(json!({"address": "%QX0.0", "path": null, "written": false})),
        other => Err(format!("unexpected request {other}")),
    }));
    let mut adapter = attached(addr);
    let by_address = dispatch(&mut adapter, set_expression("%QX0.0", "TRUE", None));
    let by_row_name = dispatch(&mut adapter, set_expression("lamp", "force: TRUE", None));
    let by_listing = dispatch(&mut adapter, set_variable(12, "lamp", "FALSE"));
    let seen = finish(adapter, &received, server);

    for answered in [&by_address, &by_row_name, &by_listing] {
        assert!(answered.response.success, "{:?}", answered.response.message);
    }
    assert_eq!(by_address.value().0, "TRUE");
    assert_eq!(by_address.event_names(), ["stIoState"]);
    assert_eq!(by_row_name.value().0, "TRUE");
    assert_eq!(by_row_name.event_names(), ["stIoState"]);
    let outputs = &by_row_name.events[0].body.as_ref().expect("the I/O state")["outputs"];
    assert_eq!(outputs[0]["forced"], json!(true), "{outputs}");
    assert_eq!(by_listing.value().0, "FALSE");
    assert_eq!(by_listing.event_names(), ["invalidated", "stIoState"]);
    assert_eq!(
        seen,
        requests(&[
            ("io.write", json!({"address": "%QX0.0", "value": "TRUE"})),
            ("io.read", Value::Null),
            ("io.read", Value::Null),
            ("io.force", json!({"address": "%QX0.0", "value": "TRUE"})),
            ("io.read", Value::Null),
            (
                "debug.set",
                json!({"variables_reference": 12, "name": "lamp", "value": "FALSE"})
            ),
            ("io.write", json!({"address": "%QX0.0", "value": "FALSE"})),
            ("io.read", Value::Null),
        ])
    );
}

#[test]
fn a_refusal_reaches_the_user_and_nothing_follows_it() {
    // Specification 13 with specification 11 §6.13.4: the runtime's refusal of a value its type
    // does not take is the response's message, word for word, and nothing is queued; a release of
    // a variable without a path, a local of a held frame, is refused, as a local is never forced.
    // Catches a refusal replaced by a generic text, a queued write after a refusal, and a release
    // sent with an empty path.
    let (addr, received, server) = fake_runtime(Box::new(|kind, params| match kind {
        "debug.set" if params.get("value").is_some() => {
            Err("'TRUE' is not a value of type INT".to_string())
        }
        "debug.set" => Ok(json!({"path": null, "type": "INT", "value": "12", "written": false})),
        other => Err(format!("unexpected request {other}")),
    }));
    let mut adapter = attached(addr);
    let refused = dispatch(&mut adapter, set_variable(2, "Limit", "TRUE"));
    let local = dispatch(&mut adapter, set_variable(1, "N", "release"));
    let seen = finish(adapter, &received, server);

    assert!(!refused.response.success);
    assert_eq!(
        refused.response.message.as_deref(),
        Some("'TRUE' is not a value of type INT")
    );
    assert!(refused.events.is_empty());
    assert!(!local.response.success);
    let reason = local.response.message.unwrap_or_default();
    assert!(reason.contains("never forced"), "{reason}");
    assert_eq!(
        seen,
        requests(&[
            (
                "debug.set",
                json!({"variables_reference": 2, "name": "Limit", "value": "TRUE"})
            ),
            ("debug.set", json!({"variables_reference": 1, "name": "N"})),
        ])
    );
}

#[test]
fn a_live_values_write_refused_as_busy_is_sent_again_after_its_waits_until_it_is_taken() {
    // Specification 13, "setVariable and setExpression" (a busy runtime), with specification 11
    // §6.9: the runtime refuses `io.write` twice with `resource_busy`, its command queue full and
    // nothing queued, and takes the third. The adapter sends it again after 75 ms and after 150 ms
    // more, so `stIoWrite` is answered once, successfully, at least 225 ms after it was made.
    // Catches a busy refusal shown to the user at once, which made the extension retry it itself,
    // and a send again without its wait.
    let mut busy = 2;
    let (addr, received, server) = fake_runtime(Box::new(move |kind, _| match kind {
        "io.write" if busy > 0 => {
            busy -= 1;
            Err(BUSY.to_string())
        }
        "io.write" => Ok(json!({"status": "queued"})),
        other => Err(format!("unexpected request {other}")),
    }));
    let mut adapter = attached(addr);
    let started = Instant::now();
    let answered = dispatch(&mut adapter, io_write("%QX0.0", "TRUE"));
    let waited = started.elapsed();
    let seen = finish(adapter, &received, server);

    assert!(answered.response.success, "{:?}", answered.response.message);
    assert!(waited >= Duration::from_millis(225), "{waited:?}");
    let write = ("io.write", json!({"address": "%QX0.0", "value": "TRUE"}));
    assert_eq!(seen, requests(&[write.clone(), write.clone(), write]));
}

#[test]
fn a_write_still_busy_after_five_more_sends_shows_the_runtimes_refusal() {
    // Specification 13 (a busy runtime): the runtime refuses every `set` as busy. After the first
    // send and five more, with 75 + 150 + 250 + 400 + 600 = 1,475 ms of waits between them, the
    // adapter answers with the runtime's refusal; `debug.set`, which named the path, was taken at
    // once and is sent once. Catches a retry without end, another number of sends, and the refusal
    // replaced by another text.
    let (addr, received, server) = fake_runtime(Box::new(|kind, _| match kind {
        "debug.set" => Ok(json!({
            "path": "Main.Speed", "type": "INT", "value": "5", "written": false
        })),
        "set" => Err(BUSY.to_string()),
        other => Err(format!("unexpected request {other}")),
    }));
    let mut adapter = attached(addr);
    let started = Instant::now();
    let answered = dispatch(&mut adapter, set_variable(9, "Speed", "5"));
    let waited = started.elapsed();
    let seen = finish(adapter, &received, server);

    assert!(!answered.response.success);
    assert_eq!(answered.response.message.as_deref(), Some(BUSY));
    assert!(waited >= Duration::from_millis(1_475), "{waited:?}");
    let queued = ("set", json!({"target": "global:Main.Speed", "value": "5"}));
    let mut expected = vec![(
        "debug.set",
        json!({"variables_reference": 9, "name": "Speed", "value": "5"}),
    )];
    expected.extend(std::iter::repeat_n(queued, 6));
    assert_eq!(seen, requests(&expected));
}
