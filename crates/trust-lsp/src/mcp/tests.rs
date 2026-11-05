//! Tests of the MCP server's seam with its tool provider (specification 43, §3.1), with a provider
//! held in memory. The rules of the connection itself are tested through the HMI authoring tools
//! (`hmi_agent_mcp/tests`); these tests show that the server takes what it says of itself, its
//! tools and their calls from its provider, and none of the HMI adapter's, and how it keeps its
//! limits with a provider that panics, never returns or waits in its cancellation.

use std::collections::HashMap;
use std::io::{self, BufReader, Cursor, Read, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use super::lines::{read_line, Line};
use super::{Limits, Server, ServerInfo, ToolProvider, Unavailable};

/// What the test provider says of itself.
const INFO: ServerInfo = ServerInfo {
    name: "test-tools",
    version: "2.0.0",
    instructions: "Test instructions.",
    unavailable: "Test tools unavailable.",
};

/// Limits under which only the agent cancels a call.
const LIMITS: Limits = Limits {
    line_bytes: 1024,
    active_calls: 8,
    call_time: Duration::from_secs(60),
    answer_time: Duration::from_secs(60),
};

/// How long a test waits for something that must happen.
const PATIENCE: Duration = Duration::from_secs(5);

/// What the provider's calls and cancellations wait on.
#[derive(Default)]
struct State {
    /// The ids of the calls the server cancelled, in order.
    cancelled: Vec<String>,
    /// Whether the test let the `stuck` calls, and the cancellations that wait, end.
    freed: bool,
}

/// A provider of six tools:
/// - `echo` answers with its call's id and arguments;
/// - `slow` answers once the server cancels it;
/// - `stubborn` ignores its cancellation and answers after 300 ms;
/// - `stuck` answers only once the test frees it;
/// - `panics` panics;
/// - `down` cannot be answered.
///
/// With `cancel_waits`, a cancellation waits until the test frees it.
struct TestTools {
    tools: Vec<Value>,
    state: Mutex<State>,
    /// Wakes the waiting calls and cancellations at each cancellation and when the test frees them.
    woken: Condvar,
    cancel_waits: bool,
}

impl TestTools {
    fn new() -> Arc<Self> {
        Self::with(false)
    }

    fn with(cancel_waits: bool) -> Arc<Self> {
        let names = ["echo", "slow", "stubborn", "stuck", "panics", "down"];
        Arc::new(Self {
            tools: names.iter().map(|name| json!({ "name": name })).collect(),
            state: Mutex::new(State::default()),
            woken: Condvar::new(),
            cancel_waits,
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("the provider's lock")
    }

    /// Waits until `done` holds for the provider's state.
    fn wait_until(&self, done: impl Fn(&State) -> bool) {
        let mut state = self.lock();
        while !done(&state) {
            state = self.woken.wait(state).expect("the provider's lock");
        }
    }

    fn cancelled(&self) -> Vec<String> {
        self.lock().cancelled.clone()
    }

    /// Lets the `stuck` calls and the waiting cancellations end.
    fn free(&self) {
        self.lock().freed = true;
        self.woken.notify_all();
    }
}

impl ToolProvider for TestTools {
    fn info(&self) -> ServerInfo {
        INFO
    }

    fn tools(&self) -> &[Value] {
        &self.tools
    }

    fn check(
        &self,
        params: &Map<String, Value>,
    ) -> Result<(String, Map<String, Value>), &'static str> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| self.tools.iter().any(|tool| tool["name"] == *name))
            .ok_or("Unknown test tool.")?;
        let Some(Value::Object(arguments)) = params.get("arguments") else {
            return Err("Arguments must be an object.");
        };
        Ok((name.to_owned(), arguments.clone()))
    }

    fn call(
        &self,
        id: &str,
        name: &str,
        arguments: Map<String, Value>,
    ) -> Result<Value, Unavailable> {
        match name {
            "echo" => Ok(json!({ "id": id, "arguments": arguments })),
            "slow" => {
                self.wait_until(|state| state.cancelled.iter().any(|cancelled| cancelled == id));
                Ok(json!({ "cancelled": id }))
            }
            "stubborn" => {
                thread::sleep(Duration::from_millis(300));
                Ok(json!({ "answered": id }))
            }
            "stuck" => {
                self.wait_until(|state| state.freed);
                Ok(json!({ "freed": id }))
            }
            "panics" => panic!("the test tool panics"),
            _ => Err(Unavailable),
        }
    }

    fn cancel(&self, id: &str) {
        self.lock().cancelled.push(id.to_owned());
        self.woken.notify_all();
        if self.cancel_waits {
            self.wait_until(|state| state.freed);
        }
    }
}

/// Standard output, kept for the test.
#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);

impl Output {
    /// The answers written so far, by their ids as JSON text.
    fn answers(&self) -> HashMap<String, Value> {
        let text = String::from_utf8(self.0.lock().expect("the output").clone()).expect("UTF-8");
        text.lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("one JSON message per line"))
            .map(|answer| (answer["id"].to_string(), answer))
            .collect()
    }
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("the output").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Serves `lines` to a server of `provider` within `limits` until its input ends; its answers, by
/// their ids as JSON text.
fn serve(provider: &Arc<TestTools>, limits: Limits, lines: &[String]) -> HashMap<String, Value> {
    let output = Output::default();
    let server = Server::start(provider.clone(), limits, Box::new(output.clone()))
        .expect("the server starts");
    let input: String = lines.iter().map(|line| format!("{line}\n")).collect();
    server.serve(&mut Cursor::new(input.into_bytes()));
    output.answers()
}

/// Standard input fed by the test, chunk by chunk; it ends when the test drops its sender.
struct ChannelInput {
    chunks: Receiver<Vec<u8>>,
    pending: Vec<u8>,
}

impl Read for ChannelInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.pending.is_empty() {
            match self.chunks.recv() {
                Ok(chunk) => self.pending = chunk,
                Err(_) => return Ok(0),
            }
        }
        let count = buffer.len().min(self.pending.len());
        buffer[..count].copy_from_slice(&self.pending[..count]);
        self.pending.drain(..count);
        Ok(count)
    }
}

/// A running server whose input the test writes line by line.
struct Live {
    input: Option<Sender<Vec<u8>>>,
    output: Output,
    served: Option<JoinHandle<()>>,
}

impl Live {
    fn start(provider: &Arc<TestTools>, limits: Limits) -> Self {
        let output = Output::default();
        let server = Server::start(provider.clone(), limits, Box::new(output.clone()))
            .expect("the server starts");
        let (input, chunks) = mpsc::channel();
        let served = thread::spawn(move || {
            server.serve(&mut BufReader::new(ChannelInput {
                chunks,
                pending: Vec::new(),
            }));
        });
        Self {
            input: Some(input),
            output,
            served: Some(served),
        }
    }

    fn send(&self, line: &str) {
        self.input
            .as_ref()
            .expect("standard input is open")
            .send(format!("{line}\n").into_bytes())
            .expect("the server reads its input");
    }

    /// The answer to request `id`, waited for at most [`PATIENCE`].
    fn answer(&self, id: u32) -> Value {
        let deadline = Instant::now() + PATIENCE;
        loop {
            if let Some(answer) = self.output.answers().remove(&id.to_string()) {
                return answer;
            }
            assert!(Instant::now() < deadline, "no answer to request {id}");
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// Ends standard input and waits for the server to end.
    fn close(mut self) {
        drop(self.input.take());
        if let Some(served) = self.served.take() {
            served.join().expect("the server ends");
        }
    }
}

/// A request with `id`, `method` and `params`, as one line.
fn request(id: u32, method: &str, params: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string()
}

/// A call of `tool` without arguments, as one line.
fn call(id: u32, tool: &str) -> String {
    request(id, "tools/call", json!({ "name": tool, "arguments": {} }))
}

/// The agent's cancellation of request `id`, as one line.
fn cancellation(id: u32) -> String {
    json!({
        "jsonrpc": "2.0",
        "method": "notifications/cancelled",
        "params": { "requestId": id }
    })
    .to_string()
}

/// The failure answer's error, with the test provider's text.
fn failure() -> Value {
    json!({ "code": -32000, "message": "Test tools unavailable." })
}

/// Whether `id` is a call id that the server draws: 32 lowercase hexadecimal digits, a dash and
/// the call's number.
fn is_server_call_id(id: &Value, number: u64) -> bool {
    id.as_str()
        .and_then(|id| id.strip_suffix(&format!("-{number}")))
        .is_some_and(|prefix| {
            prefix.len() == 32
                && prefix
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

/// What the server says of itself, its tools, its checks and its failure answer, also to a line
/// over the line limit, are the provider's, and a call runs at the provider under an id that the
/// server draws. Catches a server that keeps the HMI adapter's name, instructions, annotations,
/// refusals or failure text for another provider, or that hands the provider the agent's request
/// id.
#[test]
fn the_server_serves_its_providers_info_tools_and_calls() {
    let provider = TestTools::new();
    let answers = serve(
        &provider,
        LIMITS,
        &[
            request(1, "initialize", json!({ "protocolVersion": "2025-06-18" })),
            request(2, "tools/list", json!({})),
            request(
                3,
                "tools/call",
                json!({ "name": "echo", "arguments": { "text": "a" } }),
            ),
            call(4, "down"),
            call(5, "nosuch"),
            "x".repeat(2000),
        ],
    );
    assert_eq!(
        answers["1"]["result"],
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "test-tools", "version": "2.0.0" },
            "instructions": "Test instructions.",
        })
    );
    let listed: Vec<&Value> = answers["2"]["result"]["tools"]
        .as_array()
        .expect("a tool list")
        .iter()
        .collect();
    assert_eq!(listed, provider.tools.iter().collect::<Vec<_>>());
    let echoed = &answers["3"]["result"];
    assert_eq!(echoed["arguments"], json!({ "text": "a" }));
    assert!(
        is_server_call_id(&echoed["id"], 1),
        "the provider runs the first call under the server's id: {}",
        echoed["id"]
    );
    assert_eq!(answers["4"]["error"], failure());
    assert_eq!(
        answers["5"]["error"],
        json!({ "code": -32602, "message": "Unknown test tool." })
    );
    assert_eq!(answers["null"]["error"], failure());
}

/// The agent's cancellation reaches the provider, for the call it names, and the call answers
/// through its own result. Catches a cancellation that never reaches the provider, or that names
/// the agent's request id instead of the provider's call.
#[test]
fn the_agents_cancellation_reaches_the_provider_for_the_call_it_names() {
    let provider = TestTools::new();
    let answers = serve(&provider, LIMITS, &[call(7, "slow"), cancellation(7)]);
    let cancelled = provider.cancelled();
    assert_eq!(cancelled.len(), 1, "one cancellation: {cancelled:?}");
    assert!(is_server_call_id(&json!(cancelled[0]), 1));
    assert_eq!(answers["7"]["result"], json!({ "cancelled": cancelled[0] }));
}

/// A call that both the agent cancels and outlives its call time is cancelled at the provider
/// once, and answers through its own result. Catches a server that cancels again at the call
/// time after the agent's cancellation.
#[test]
fn a_call_is_cancelled_at_the_provider_once() {
    let provider = TestTools::new();
    let limits = Limits {
        call_time: Duration::from_millis(50),
        answer_time: PATIENCE,
        ..LIMITS
    };
    let answers = serve(&provider, limits, &[call(8, "stubborn"), cancellation(8)]);
    let cancelled = provider.cancelled();
    assert_eq!(cancelled.len(), 1, "one cancellation: {cancelled:?}");
    assert_eq!(answers["8"]["result"], json!({ "answered": cancelled[0] }));
}

/// A provider call that panics gets the failure answer at once, not at the answer time. Catches a
/// panic answered only when the answer time ends, or never.
#[test]
fn a_provider_call_that_panics_gets_the_failure_answer_at_once() {
    let provider = TestTools::new();
    let started = Instant::now();
    let answers = serve(&provider, LIMITS, &[call(9, "panics")]);
    assert_eq!(answers["9"]["error"], failure());
    assert!(
        started.elapsed() < PATIENCE,
        "answered after {:?}",
        started.elapsed()
    );
}

/// A cancellation that waits does not hold back the failure answer at the answer time. Catches a
/// server that cancels on the thread that waits for the answer, so that a provider whose
/// cancellation waits (a loopback that does not answer it, or a lock that the call holds) delays
/// the failure answer past the answer time, or forever.
#[test]
fn a_cancellation_that_waits_does_not_hold_back_the_answer_time() {
    let provider = TestTools::with(true);
    let limits = Limits {
        call_time: Duration::from_millis(20),
        answer_time: Duration::from_millis(300),
        ..LIMITS
    };
    // Frees the call and the cancellation after a while, so that a server that waits for the
    // cancellation answers late instead of never.
    let freeing = Arc::clone(&provider);
    let release = thread::spawn(move || {
        thread::sleep(Duration::from_secs(3));
        freeing.free();
    });
    let started = Instant::now();
    let answers = serve(&provider, limits, &[call(10, "stuck")]);
    let elapsed = started.elapsed();
    assert_eq!(answers["10"]["error"], failure());
    assert!(
        elapsed < Duration::from_secs(2),
        "the failure answer came after {elapsed:?}"
    );
    release.join().expect("the test frees the provider");
}

/// A call holds its place among the active calls until its provider returns, also after its
/// failure answer at the answer time, and the place is free again once the provider returned.
/// Catches a server that frees the place when it gives up waiting, so that calls that never return
/// pile up beside new ones.
#[test]
fn a_call_holds_its_place_until_its_provider_returns() {
    let provider = TestTools::new();
    let limits = Limits {
        active_calls: 1,
        answer_time: Duration::from_millis(200),
        ..LIMITS
    };
    let live = Live::start(&provider, limits);
    live.send(&call(11, "stuck"));
    assert_eq!(live.answer(11)["error"], failure());
    live.send(&call(12, "echo"));
    assert_eq!(
        live.answer(12)["error"],
        json!({ "code": -32600, "message": "Duplicate request id or too many active requests." })
    );
    provider.free();
    let deadline = Instant::now() + PATIENCE;
    let mut id = 13;
    loop {
        live.send(&call(id, "echo"));
        let answer = live.answer(id);
        if answer.get("result").is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the place is still taken: {answer}"
        );
        thread::sleep(Duration::from_millis(20));
        id += 1;
    }
    live.close();
}

/// Catches: the line reader splitting or joining lines wrongly, keeping a long line's bytes or
/// losing a last line without a line end.
#[test]
fn lines_end_at_a_newline_and_are_limited_before_their_line_end() {
    let mut input = Cursor::new(b"abc\r\nabcdefgh\r\nabcdefghi\n\nabcdefgh\rx\nlast".to_vec());
    let mut lines = Vec::new();
    while let Some(line) = read_line(&mut input, 8).expect("read") {
        lines.push(line);
    }
    assert_eq!(
        lines,
        [
            Line::Text(b"abc".to_vec()),
            Line::Text(b"abcdefgh".to_vec()),
            Line::TooLong,
            Line::Text(Vec::new()),
            Line::TooLong,
            Line::Text(b"last".to_vec()),
        ]
    );
}
