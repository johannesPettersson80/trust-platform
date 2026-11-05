//! Tests of the MCP server's seam with its tool provider (specification 43, §3.1), with a provider
//! held in memory. The rules of the connection itself are tested through the HMI authoring tools
//! (`hmi_agent_mcp/tests`); these tests show that the server takes what it says of itself, its
//! tools and their calls from its provider, and none of the HMI adapter's.

use std::collections::HashMap;
use std::io::{self, Cursor, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

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

/// A provider of three tools: `echo` answers with its call's id and arguments, `slow` answers once
/// the server cancels it, and `down` cannot be answered.
struct TestTools {
    tools: Vec<Value>,
    /// The ids of the calls the server cancelled, in order.
    cancelled: Mutex<Vec<String>>,
    /// Wakes the `slow` calls at each cancellation.
    released: Condvar,
}

impl TestTools {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            tools: vec![
                json!({ "name": "echo" }),
                json!({ "name": "slow" }),
                json!({ "name": "down" }),
            ],
            cancelled: Mutex::new(Vec::new()),
            released: Condvar::new(),
        })
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
                let mut cancelled = self.cancelled.lock().expect("the provider's lock");
                while !cancelled.iter().any(|cancelled| cancelled == id) {
                    cancelled = self.released.wait(cancelled).expect("the provider's lock");
                }
                Ok(json!({ "cancelled": id }))
            }
            _ => Err(Unavailable),
        }
    }

    fn cancel(&self, id: &str) {
        self.cancelled
            .lock()
            .expect("the provider's lock")
            .push(id.to_owned());
        self.released.notify_all();
    }
}

/// Standard output, kept for the test.
#[derive(Clone, Default)]
struct Output(Arc<Mutex<Vec<u8>>>);

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("the output").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Serves `messages`, one per line, to a server of `provider` until its input ends; its answers,
/// by their ids as JSON text.
fn serve(provider: &Arc<TestTools>, messages: &[Value]) -> HashMap<String, Value> {
    let output = Output::default();
    let server = Server::start(provider.clone(), LIMITS, Box::new(output.clone()))
        .expect("the server starts");
    let input: String = messages
        .iter()
        .map(|message| format!("{message}\n"))
        .collect();
    server.serve(&mut Cursor::new(input.into_bytes()));
    let text = String::from_utf8(output.0.lock().expect("the output").clone()).expect("UTF-8");
    text.lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("one JSON message per line"))
        .map(|answer| (answer["id"].to_string(), answer))
        .collect()
}

/// A request with `id`, `method` and `params`.
fn request(id: u32, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
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

/// What the server says of itself, its tools, its checks and its failure answer are the provider's,
/// and a call runs at the provider under an id that the server draws. Catches a server that keeps
/// the HMI adapter's name, instructions, annotations, refusals or failure text for another
/// provider, or that hands the provider the agent's request id.
#[test]
fn the_server_serves_its_providers_info_tools_and_calls() {
    let provider = TestTools::new();
    let answers = serve(
        &provider,
        &[
            request(1, "initialize", json!({ "protocolVersion": "2025-06-18" })),
            request(2, "tools/list", json!({})),
            request(
                3,
                "tools/call",
                json!({ "name": "echo", "arguments": { "text": "a" } }),
            ),
            request(4, "tools/call", json!({ "name": "down", "arguments": {} })),
            request(
                5,
                "tools/call",
                json!({ "name": "nosuch", "arguments": {} }),
            ),
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
    assert_eq!(
        answers["2"]["result"],
        json!({ "tools": [{ "name": "echo" }, { "name": "slow" }, { "name": "down" }] })
    );
    let echoed = &answers["3"]["result"];
    assert_eq!(echoed["arguments"], json!({ "text": "a" }));
    assert!(
        is_server_call_id(&echoed["id"], 1),
        "the provider runs the first call under the server's id: {}",
        echoed["id"]
    );
    assert_eq!(
        answers["4"]["error"],
        json!({ "code": -32000, "message": "Test tools unavailable." })
    );
    assert_eq!(
        answers["5"]["error"],
        json!({ "code": -32602, "message": "Unknown test tool." })
    );
}

/// The agent's cancellation reaches the provider once, for the call it names, and the call answers
/// through its own result. Catches a cancellation that never reaches the provider, or that names
/// the agent's request id instead of the provider's call.
#[test]
fn the_agents_cancellation_reaches_the_provider_once() {
    let provider = TestTools::new();
    let answers = serve(
        &provider,
        &[
            request(7, "tools/call", json!({ "name": "slow", "arguments": {} })),
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/cancelled",
                "params": { "requestId": 7 }
            }),
        ],
    );
    let cancelled = provider
        .cancelled
        .lock()
        .expect("the provider's lock")
        .clone();
    assert_eq!(cancelled.len(), 1, "one cancellation: {cancelled:?}");
    assert!(is_server_call_id(&json!(cancelled[0]), 1));
    assert_eq!(answers["7"]["result"], json!({ "cancelled": cancelled[0] }));
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
