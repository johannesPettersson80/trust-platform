//! Scripted runtimes for the tests. Each listens on a local TCP port, accepts any number of
//! connections, and answers each request by its type after the script's delay; a type that the
//! script does not answer is refused. Nothing here reaches the network.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{json, Value};

/// How a scripted runtime answers one request type.
#[derive(Debug, Clone)]
pub(crate) enum Reply {
    /// A successful answer with this result.
    Result(Value),
    /// A refusal with this error code.
    Refuse(&'static str),
    /// This line, which need not be a control response.
    Raw(&'static str),
    /// No answer at all.
    Silent,
}

/// What a scripted runtime answers.
#[derive(Debug, Clone, Default)]
pub(crate) struct Script {
    replies: BTreeMap<String, Reply>,
    delay: Duration,
}

impl Script {
    /// A running runtime that answers every request of the gather.
    pub(crate) fn running() -> Self {
        Self::default()
            .answer("status", Reply::Result(json!({"state": "running"})))
            .answer(
                "fleet.topology",
                Reply::Result(json!({"schema_version": 4, "hosts": [], "links": [], "shared": [], "external": []})),
            )
            .answer(
                "connectors.status",
                Reply::Result(json!({"schema_version": 1, "connectors": []})),
            )
            .answer("comm.capabilities", Reply::Result(json!({"services": []})))
            .answer("comm.schema", Reply::Result(json!({"protocols": []})))
            .answer("events.tail", Reply::Result(json!({"events": []})))
    }

    /// The script, with `reply` for requests of type `kind`.
    #[must_use]
    pub(crate) fn answer(mut self, kind: &str, reply: Reply) -> Self {
        self.replies.insert(kind.to_string(), reply);
        self
    }

    /// The script, waiting `delay` before each answer.
    #[must_use]
    pub(crate) fn delayed(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

/// A running scripted runtime. It stops listening when dropped.
pub(crate) struct FakeRuntime {
    /// Its control endpoint, `tcp://127.0.0.1:<port>`.
    pub(crate) endpoint: String,
    seen: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    server: Option<JoinHandle<()>>,
}

impl FakeRuntime {
    /// Starts a runtime that answers by `script`.
    pub(crate) fn start(script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
        listener
            .set_nonblocking(true)
            .expect("a listener that can stop");
        let endpoint = format!("tcp://{}", listener.local_addr().expect("local address"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let server = {
            let seen = Arc::clone(&seen);
            let stop = Arc::clone(&stop);
            thread::spawn(move || accept(&listener, &script, &seen, &stop))
        };
        Self {
            endpoint,
            seen,
            stop,
            server: Some(server),
        }
    }

    /// The requests that arrived so far.
    pub(crate) fn requests(&self) -> Vec<Value> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// How many requests of type `kind` arrived so far.
    pub(crate) fn count(&self, kind: &str) -> usize {
        self.requests()
            .iter()
            .filter(|request| request["type"] == kind)
            .count()
    }
}

impl Drop for FakeRuntime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
    }
}

fn accept(
    listener: &TcpListener,
    script: &Script,
    seen: &Arc<Mutex<Vec<Value>>>,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                // The accepted stream must block, whatever it inherits from the listener.
                if stream.set_nonblocking(false).is_ok() {
                    let script = script.clone();
                    let seen = Arc::clone(seen);
                    thread::spawn(move || serve(stream, &script, &seen));
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(2));
            }
            Err(_) => return,
        }
    }
}

fn serve(stream: TcpStream, script: &Script, seen: &Mutex<Vec<Value>>) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    while reader.read_line(&mut line).is_ok_and(|read| read > 0) {
        let request: Value = serde_json::from_str(&line).unwrap_or(Value::Null);
        line.clear();
        let kind = request["type"].as_str().unwrap_or_default().to_string();
        let id = request["id"].clone();
        seen.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        thread::sleep(script.delay);
        let answer = match script.replies.get(&kind).cloned() {
            Some(Reply::Result(result)) => {
                json!({"id": id, "ok": true, "result": result}).to_string()
            }
            Some(Reply::Refuse(code)) => {
                json!({"id": id, "ok": false, "error": code.replace('_', " "), "error_code": code})
                    .to_string()
            }
            Some(Reply::Raw(text)) => text.to_string(),
            Some(Reply::Silent) => continue,
            // A `comm.apply` answers as the real runtime does, unless the script overrode it.
            None if kind == "comm.apply" => {
                json!({"id": id, "ok": true, "result": comm_apply_answer(&request)}).to_string()
            }
            None => json!({"id": id, "ok": false, "error": format!("unsupported request {kind}")})
                .to_string(),
        };
        // The client may already have given up on this answer.
        let _ = writeln!(writer, "{answer}");
    }
}

/// The result of a `comm.apply`, shaped as the runtime answers it: the protocol it wrote, the
/// lifecycle effect, its message and the `instance_id` of the connection it wrote
/// (specification 11 §6.8.1a; `crates/trust-runtime/src/control/comm_handlers/apply.rs`, which
/// B7c1 made answer the written connection's ID).
fn comm_apply_answer(request: &Value) -> Value {
    let params = &request["params"];
    let protocol = params["protocol"].as_str().unwrap_or_default();
    let action = params["action"].as_str().unwrap_or("upsert");
    let instance_id = params["instance_id"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| (action == "add").then(|| format!("{protocol}:0")));
    let message = match action {
        "validate" => "Configuration validated. No files were changed.",
        "disable" => {
            "Endpoint disabled. Restart the runtime to apply it; the endpoint stays visible in Devices & Connections."
        }
        _ => "I/O configuration saved. Restart the runtime to apply it.",
    };
    json!({
        "schema_version": 1,
        "protocol": protocol,
        "driver": "",
        "action": action,
        "applied": action != "validate",
        "lifecycle_effect": if action == "validate" { "validate_only" } else { "restart_required" },
        "message": message,
        "instance_id": instance_id,
        "field_errors": [],
    })
}
