//! A fake simulator HMI web server for the simulator journey tests: the runtime's page, command and
//! readback stream routes (`/api/hmi/page`, `/api/hmi/command`, `/ws/hmi`) on 127.0.0.1, served by
//! tiny_http and tungstenite as the runtime serves them (`crates/trust-runtime/src/web`), with the
//! answers each test scripts. Like the runtime, a stream first says whether the target runs in
//! simulation, and a command's later outcomes come after a delay: at least a stream period, a
//! timed-out one at the control's confirmation time. It records every request with its cookie and
//! access token.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};
use tungstenite::handshake::derive_accept_key;
use tungstenite::protocol::Role;
use tungstenite::{Message, WebSocket};

/// What a readback stream sends.
#[derive(Debug, Clone)]
pub(super) enum Frame {
    /// A JSON message.
    Text(Value),
    /// The end of the stream.
    Close,
}

/// The answer to a command, and what every open stream sends `delay` after it.
pub(super) struct CommandAnswer {
    pub(super) status: u16,
    pub(super) body: Value,
    pub(super) then: Vec<Frame>,
    pub(super) delay: Duration,
}

/// The runtime's shortest stream period, the least delay of a later outcome.
pub(super) const STREAM_PERIOD: Duration = Duration::from_millis(250);

/// A request as the fake received it.
#[derive(Debug, Clone)]
pub(super) struct Received {
    pub(super) method: String,
    pub(super) url: String,
    pub(super) cookie: Option<String>,
    pub(super) token: Option<String>,
    pub(super) body: Value,
}

/// What the fake serves.
pub(super) struct Script {
    /// The deployed pages' models by name, served with the bundle `bundle-1`.
    pub(super) pages: BTreeMap<String, Value>,
    /// Whether `/ws/hmi` opens; it answers 404 otherwise.
    pub(super) streams: bool,
    /// Whether the target runs in simulation, which a stream says first; nothing when `None`.
    pub(super) simulation: Option<bool>,
    /// What a stream sends when it opens.
    pub(super) on_open: Vec<Frame>,
    /// What happens when a stream opens, before it sends anything.
    pub(super) on_stream: Box<dyn Fn() + Send>,
    /// The answer to each command, given its body.
    pub(super) on_command: Box<dyn Fn(&Value) -> CommandAnswer + Send>,
}

impl Script {
    /// The pages, open streams that send nothing, and commands answered 404.
    pub(super) fn serving(pages: BTreeMap<String, Value>) -> Self {
        Self {
            pages,
            streams: true,
            simulation: Some(true),
            on_open: Vec::new(),
            on_stream: Box::new(|| {}),
            on_command: Box::new(|_| CommandAnswer {
                status: 404,
                body: json!({ "error": "no command service" }),
                then: Vec::new(),
                delay: STREAM_PERIOD,
            }),
        }
    }
}

struct Shared {
    script: Mutex<Script>,
    received: Mutex<Vec<Received>>,
    streams: Mutex<Vec<Sender<Frame>>>,
}

/// A running fake simulator HMI; it stops when dropped.
pub(super) struct FakeSimulator {
    /// Its web address, `http://127.0.0.1:<port>`.
    pub(super) url: String,
    server: Arc<Server>,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl FakeSimulator {
    /// Starts serving `script`.
    pub(super) fn start(script: Script) -> Self {
        let server = Arc::new(Server::http("127.0.0.1:0").expect("a fake simulator HMI"));
        let address = server.server_addr().to_ip().expect("an IP listener");
        let shared = Arc::new(Shared {
            script: Mutex::new(script),
            received: Mutex::default(),
            streams: Mutex::default(),
        });
        let thread = {
            let (server, shared) = (Arc::clone(&server), Arc::clone(&shared));
            thread::spawn(move || {
                for request in server.incoming_requests() {
                    let shared = Arc::clone(&shared);
                    thread::spawn(move || serve(request, &shared));
                }
            })
        };
        Self {
            url: format!("http://{address}"),
            server,
            shared,
            thread: Some(thread),
        }
    }

    /// The requests received so far, in order.
    pub(super) fn received(&self) -> Vec<Received> {
        lock(&self.shared.received).clone()
    }
}

impl Drop for FakeSimulator {
    fn drop(&mut self) {
        self.server.unblock();
        lock(&self.shared.streams).clear();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn serve(mut request: Request, shared: &Shared) {
    let url = request.url().to_owned();
    let header = |name: &'static str| {
        let found = request
            .headers()
            .iter()
            .find(|header| header.field.equiv(name));
        found.map(|header| header.value.as_str().to_owned())
    };
    let (cookie, token) = (header("Cookie"), header("X-Trust-Token"));
    let key = header("Sec-WebSocket-Key").unwrap_or_default();
    let method = request.method().clone();
    // Only a command has a body: tiny_http hands an upgrade request a reader that ends only with the
    // connection.
    let mut text = String::new();
    if method == Method::Post {
        let _ = request.as_reader().read_to_string(&mut text);
    }
    let body = serde_json::from_str(&text).unwrap_or(Value::Null);
    lock(&shared.received).push(Received {
        method: method.to_string(),
        url: url.clone(),
        cookie,
        token,
        body: body.clone(),
    });
    let streams = lock(&shared.script).streams;
    match (&method, url.split('?').next().unwrap_or_default()) {
        (Method::Get, "/api/hmi/page") => {
            let name = url
                .split_once("name=")
                .map_or("", |(_, name)| name)
                .to_owned();
            let model = lock(&shared.script).pages.get(&name).cloned();
            match model {
                Some(model) => respond(
                    request,
                    200,
                    &json!({ "page": { "name": name, "model": model }, "bundle": "bundle-1" }),
                ),
                None => respond(
                    request,
                    404,
                    &json!({ "error": "the project has no such page" }),
                ),
            }
        }
        (Method::Post, "/api/hmi/command") => {
            let answer = (lock(&shared.script).on_command)(&body);
            respond(request, answer.status, &answer.body);
            thread::sleep(answer.delay);
            let mut open = lock(&shared.streams);
            for frame in answer.then {
                open.retain(|stream| stream.send(frame.clone()).is_ok());
            }
        }
        (Method::Get, "/ws/hmi") if streams => stream(request, &key, shared),
        _ => respond(request, 404, &json!({ "error": "not found" })),
    }
}

/// Opens a readback stream, as the runtime's `open_stream` does, and sends its frames until the
/// fake stops or a frame closes it.
fn stream(request: Request, key: &str, shared: &Shared) {
    let accept = derive_accept_key(key.as_bytes());
    let header = Header::from_bytes("Sec-WebSocket-Accept", accept.as_bytes()).expect("a header");
    let response = Response::empty(StatusCode(101)).with_header(header);
    let socket = request.upgrade("websocket", response);
    let mut socket = WebSocket::from_raw_socket(socket, Role::Server, None);
    let (sender, frames) = mpsc::channel();
    let (simulation, on_open) = {
        let script = lock(&shared.script);
        (script.on_stream)();
        (script.simulation, script.on_open.clone())
    };
    if let Some(enabled) = simulation {
        let state = json!({ "simulation": { "enabled": enabled, "time_scale": 1.0 } });
        let _ = sender.send(Frame::Text(state));
    }
    for frame in on_open {
        let _ = sender.send(frame);
    }
    lock(&shared.streams).push(sender);
    loop {
        match frames.recv_timeout(Duration::from_millis(20)) {
            Ok(Frame::Text(message)) => {
                if socket.send(Message::text(message.to_string())).is_err() {
                    return;
                }
            }
            Ok(Frame::Close) => {
                let _ = socket.close(None);
                let _ = socket.flush();
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn respond(request: Request, status: u16, body: &Value) {
    let kind = Header::from_bytes("Content-Type", "application/json").expect("a header");
    let response = Response::from_string(body.to_string())
        .with_status_code(StatusCode(status))
        .with_header(kind);
    let _ = request.respond(response);
}
