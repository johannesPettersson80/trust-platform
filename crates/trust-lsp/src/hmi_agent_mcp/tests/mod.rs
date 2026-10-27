//! Tests of `trust-lsp hmi-agent` (specification 43, §3.1) against a fake loopback on 127.0.0.1.
//!
//! The expected answers are the rules of §3.1, which keep the behaviour of the TypeScript stdio
//! client and bridge they replace (`agent/mcpClient.ts`, `agent/bridge.ts` before 26 October 2026);
//! where §3.1 changed an answer, the test says so.

mod calls;
mod connection;
mod protocol;
mod transport;

use std::collections::HashSet;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::connection::Connection;
use super::server::{Server, StartError};
use super::Limits;

/// The capability of the fake loopback.
const CAPABILITY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
/// The workspace of the test connection.
const WORKSPACE: &str = "project-a";
/// How long a test waits for something that must happen.
const PATIENCE: Duration = Duration::from_secs(10);

/// The tools the fake loopback lists.
fn tools() -> Value {
    json!([
        {"name": "hmi_get_context", "description": "Read the project", "inputSchema": {"type": "object"}},
        {
            "name": "hmi_render_draft",
            "description": "Render a draft",
            "inputSchema": {"type": "object"},
            "annotations": {"readOnlyHint": true, "openWorldHint": false}
        }
    ])
}

/// The tool result the fake loopback answers a cancelled call with.
fn cancelled_result() -> Value {
    json!({"content": [{"type": "text", "text": "cancelled"}], "isError": true})
}

/// The failure answer of a request the loopback could not answer (§3.1).
fn unavailable(id: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32000,
            "message": "HMI authoring connection unavailable or request limit exceeded. Reconnect in the selected VS Code window."
        }
    })
}

/// A refusal with an error code and message.
fn refusal(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// A tool call request.
fn call(id: Value, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": arguments}})
}

/// A cancellation notification.
fn cancel(request: Value) -> Value {
    json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": request}})
}

/// A ping request.
fn ping(id: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "ping"})
}

/// The product's limits with other times.
fn timed(call_time: Duration, bridge_time: Duration) -> Limits {
    Limits {
        call_time,
        bridge_time,
        ..Limits::PRODUCT
    }
}

/// How the fake loopback answers a tool call.
#[derive(Clone)]
enum Calls {
    /// At once, with this tool result.
    Answer(Value),
    /// After this delay, with this tool result.
    Late(Duration, Value),
    /// Once the loopback receives the call's cancellation, with `cancelled_result()`.
    UntilCancelled,
    /// Never: the connection stays open without an answer.
    Never,
    /// With this HTTP status and no body.
    Status(u16),
    /// With this body, written in two parts split at the byte offset, 100 ms apart.
    Split(Vec<u8>, usize),
}

/// One request the fake loopback received.
#[derive(Debug, Clone)]
struct Received {
    route: String,
    authorization: String,
    body: Value,
}

/// What the fake loopback has received.
#[derive(Default)]
struct Log {
    received: Vec<Received>,
    cancelled: HashSet<String>,
}

struct Shared {
    calls: Calls,
    tools: Value,
    log: Mutex<Log>,
    changed: Condvar,
}

/// A fake window loopback: it checks the capability, records each request and answers the three
/// routes as `agent/bridge.ts` does.
struct FakeBridge {
    address: SocketAddr,
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
}

impl FakeBridge {
    fn start(calls: Calls) -> Self {
        Self::with_tools(calls, json!({"tools": tools()}))
    }

    /// A loopback whose tool route answers `tools`.
    fn with_tools(calls: Calls, tools: Value) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind the fake loopback");
        let address = listener.local_addr().expect("the fake loopback's address");
        let shared = Arc::new(Shared {
            calls,
            tools,
            log: Mutex::new(Log::default()),
            changed: Condvar::new(),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let (accepting, stopping) = (Arc::clone(&shared), Arc::clone(&stop));
        thread::spawn(move || {
            for stream in listener.incoming() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(stream) = stream {
                    let shared = Arc::clone(&accepting);
                    thread::spawn(move || answer(stream, &shared));
                }
            }
        });
        Self {
            address,
            shared,
            stop,
        }
    }

    /// The connection to this loopback.
    fn connection(&self) -> Connection {
        Connection {
            address: self.address,
            capability: CAPABILITY.to_owned(),
            workspace_id: WORKSPACE.to_owned(),
        }
    }

    fn log(&self) -> MutexGuard<'_, Log> {
        self.shared.log.lock().expect("the fake loopback's log")
    }

    /// The requests received so far.
    fn received(&self) -> Vec<Received> {
        self.log().received.clone()
    }

    /// The requests received so far on a route.
    fn received_on(&self, route: &str) -> Vec<Received> {
        self.received()
            .into_iter()
            .filter(|request| request.route == route)
            .collect()
    }

    /// Waits until `count` requests have arrived on a route, and returns them.
    fn wait_for(&self, route: &str, count: usize) -> Vec<Received> {
        let deadline = Instant::now() + PATIENCE;
        let mut log = self.log();
        loop {
            let on_route: Vec<Received> = log
                .received
                .iter()
                .filter(|request| request.route == route)
                .cloned()
                .collect();
            if on_route.len() >= count {
                return on_route;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(
                !left.is_zero(),
                "{count} requests on {route} did not arrive"
            );
            log = self
                .shared
                .changed
                .wait_timeout(log, left)
                .expect("the log")
                .0;
        }
    }
}

impl Drop for FakeBridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
    }
}

/// Answers one request of a connection to the fake loopback.
fn answer(stream: TcpStream, shared: &Shared) {
    let Ok(mut reader) = stream.try_clone().map(BufReader::new) else {
        return;
    };
    let mut writer = stream;
    let _ = writer.set_nodelay(true);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let route = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
        .to_owned();
    let (mut length, mut authorization) = (0, String::new());
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.trim_end().split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse().unwrap_or(0);
            } else if name.eq_ignore_ascii_case("authorization") {
                authorization = value.trim().to_owned();
            }
        }
    }
    let mut body = vec![0; length];
    if reader.read_exact(&mut body).is_err() {
        return;
    }
    let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let id = body["id"].as_str().unwrap_or_default().to_owned();
    {
        let mut log = shared.log.lock().expect("the log");
        if route == "/mcp/cancel" {
            log.cancelled.insert(id.clone());
        }
        log.received.push(Received {
            route: route.clone(),
            authorization: authorization.clone(),
            body,
        });
        shared.changed.notify_all();
    }
    if authorization != format!("Bearer {CAPABILITY}") {
        return respond(&mut writer, 401, b"", None);
    }
    match route.as_str() {
        "/mcp/tools" => respond(&mut writer, 200, shared.tools.to_string().as_bytes(), None),
        "/mcp/cancel" => respond(&mut writer, 200, b"{}", None),
        "/mcp/call" => match shared.calls.clone() {
            Calls::Answer(result) => respond(&mut writer, 200, result.to_string().as_bytes(), None),
            Calls::Late(delay, result) => {
                thread::sleep(delay);
                respond(&mut writer, 200, result.to_string().as_bytes(), None);
            }
            Calls::UntilCancelled => {
                let deadline = Instant::now() + PATIENCE;
                let mut log = shared.log.lock().expect("the log");
                while !log.cancelled.contains(&id) && Instant::now() < deadline {
                    let left = deadline.saturating_duration_since(Instant::now());
                    log = shared.changed.wait_timeout(log, left).expect("the log").0;
                }
                let result = if log.cancelled.contains(&id) {
                    cancelled_result()
                } else {
                    json!({"content": [{"type": "text", "text": "never cancelled"}]})
                };
                drop(log);
                respond(&mut writer, 200, result.to_string().as_bytes(), None);
            }
            Calls::Never => {
                // Holds the connection until the server closes it.
                let _ = reader.read(&mut [0; 1]);
            }
            Calls::Status(status) => respond(&mut writer, status, b"", None),
            Calls::Split(body, at) => respond(&mut writer, 200, &body, Some(at)),
        },
        _ => respond(&mut writer, 404, b"", None),
    }
}

/// Writes an answer with a `Content-Length` body, in two parts when `split` names a byte offset.
fn respond(writer: &mut TcpStream, status: u16, body: &[u8], split: Option<usize>) {
    let head = format!(
        "HTTP/1.1 {status} Fake\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let (first, second) = body.split_at(split.unwrap_or(body.len()));
    let mut start = head.into_bytes();
    start.extend_from_slice(first);
    let _ = writer.write_all(&start).and_then(|()| writer.flush());
    if !second.is_empty() {
        thread::sleep(Duration::from_millis(100));
        let _ = writer.write_all(second);
    }
}

/// A running server whose standard input and output the test holds.
struct Session {
    input: Option<Sender<Vec<u8>>>,
    output: Receiver<String>,
    served: Option<JoinHandle<()>>,
}

impl Session {
    fn start(bridge: &FakeBridge, limits: Limits) -> Self {
        Self::try_start(bridge.connection(), limits).expect("the server starts")
    }

    fn try_start(connection: Connection, limits: Limits) -> Result<Self, StartError> {
        let (lines, output) = mpsc::channel();
        let sink = LineSink {
            lines,
            partial: Vec::new(),
        };
        let server = Server::start(connection, limits, Box::new(sink))?;
        let (input, chunks) = mpsc::channel();
        let served = thread::spawn(move || {
            let mut input = BufReader::new(ChannelInput {
                chunks,
                pending: Vec::new(),
                at: 0,
            });
            server.serve(&mut input);
        });
        Ok(Self {
            input: Some(input),
            output,
            served: Some(served),
        })
    }

    /// Writes one message as one line.
    fn send(&self, message: &Value) {
        self.send_raw(format!("{message}\n").as_bytes());
    }

    /// Writes raw bytes to standard input.
    fn send_raw(&self, bytes: &[u8]) {
        self.input
            .as_ref()
            .expect("standard input is open")
            .send(bytes.to_vec())
            .expect("the server reads its input");
    }

    /// The next line of standard output, parsed.
    fn next(&self) -> Value {
        let line = self
            .output
            .recv_timeout(PATIENCE)
            .expect("the server answers");
        serde_json::from_str(&line).expect("one JSON message per line")
    }

    /// Writes a request and returns the next line of standard output.
    fn ask(&self, message: &Value) -> Value {
        self.send(message);
        self.next()
    }

    /// Whether standard output stays empty for `wait`.
    fn silent_for(&self, wait: Duration) -> bool {
        self.output.recv_timeout(wait).is_err()
    }

    /// Ends standard input, waits for the server to end and returns the lines it wrote since the
    /// last one read.
    fn close(mut self) -> Vec<String> {
        drop(self.input.take());
        if let Some(served) = self.served.take() {
            served.join().expect("the server ends");
        }
        self.output.try_iter().collect()
    }
}

/// Standard input fed by the test, chunk by chunk; it ends when the test drops its sender.
struct ChannelInput {
    chunks: Receiver<Vec<u8>>,
    pending: Vec<u8>,
    at: usize,
}

impl Read for ChannelInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        while self.at == self.pending.len() {
            match self.chunks.recv() {
                Ok(chunk) => {
                    self.pending = chunk;
                    self.at = 0;
                }
                Err(_) => return Ok(0),
            }
        }
        let count = buffer.len().min(self.pending.len() - self.at);
        buffer[..count].copy_from_slice(&self.pending[self.at..self.at + count]);
        self.at += count;
        Ok(count)
    }
}

/// Standard output read by the test, line by line.
struct LineSink {
    lines: Sender<String>,
    partial: Vec<u8>,
}

impl Write for LineSink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for &byte in bytes {
            if byte == b'\n' {
                let line = String::from_utf8(std::mem::take(&mut self.partial))
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                let _ = self.lines.send(line);
            } else {
                self.partial.push(byte);
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
