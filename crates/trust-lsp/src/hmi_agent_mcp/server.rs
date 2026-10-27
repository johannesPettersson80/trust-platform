//! The MCP server of one connection (specification 43, §3.1). The reading thread answers every
//! message except a tool call itself, in order. Each tool call runs on its own thread, so ping and
//! cancellation are answered while calls run.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use parking_lot::Mutex;
use serde_json::{json, Map, Value};

use super::bridge::{self, Bridge, ExchangeError};
use super::connection::Connection;
use super::lines::{self, Line};
use super::protocol::{self, Message};
use super::Limits;

/// The argument that names the workspace of a tool call.
const WORKSPACE_ID: &str = "workspaceId";

/// Why the server cannot start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartError {
    /// The loopback did not answer the tool list.
    Bridge,
    /// The loopback's tool list is not a list of named tools.
    Tools,
    /// No random call-id prefix could be drawn.
    Random,
}

/// What reaches a tool call in progress.
enum Event {
    /// The agent cancelled the call.
    Cancel,
    /// The loopback answered the call, or the exchange failed.
    Answered(Result<Value, ExchangeError>),
}

/// A tool call accepted for the loopback.
struct Call {
    /// The agent's request id.
    id: Value,
    /// The request id as JSON text: its key among the calls in progress.
    key: String,
    /// The id the loopback knows the call by.
    bridge_id: String,
    /// The tool's name.
    name: String,
    /// The tool's arguments, with the connection's workspace.
    arguments: Map<String, Value>,
}

/// The server of one connection.
pub(crate) struct Server {
    bridge: Bridge,
    workspace_id: String,
    /// The loopback's tools, as it listed them when the server started.
    tools: Vec<Value>,
    limits: Limits,
    /// The prefix of the call ids this server gives the loopback, drawn at random so that two
    /// servers on one connection never share an id.
    prefix: String,
    next_call: AtomicU64,
    /// The calls in progress, by request key: where each one's events go.
    calls: Mutex<HashMap<String, Sender<Event>>>,
    /// Standard output: protocol messages only, one per line.
    output: Mutex<Box<dyn Write + Send>>,
}

impl Server {
    /// Starts the server of a connection: it asks the loopback for its tools first, so that a
    /// connection that cannot answer fails before the agent uses it.
    pub(crate) fn start(
        connection: Connection,
        limits: Limits,
        output: Box<dyn Write + Send>,
    ) -> Result<Arc<Self>, StartError> {
        let bridge = Bridge::new(
            connection.address,
            &connection.capability,
            limits.bridge_time,
            limits.answer_bytes,
        );
        let answer = bridge
            .post(bridge::TOOLS, &json!({}))
            .map_err(|_| StartError::Bridge)?;
        let tools = tool_list(answer).ok_or(StartError::Tools)?;
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|_| StartError::Random)?;
        Ok(Arc::new(Self {
            bridge,
            workspace_id: connection.workspace_id,
            tools,
            limits,
            prefix: hex::encode(random),
            next_call: AtomicU64::new(1),
            calls: Mutex::new(HashMap::new()),
            output: Mutex::new(output),
        }))
    }

    /// Serves the agent until its input ends, then waits until the calls in progress have
    /// answered.
    pub(crate) fn serve(self: &Arc<Self>, input: &mut impl BufRead) {
        let mut running: Vec<JoinHandle<()>> = Vec::new();
        while let Ok(Some(line)) = lines::read_line(input, self.limits.line_bytes) {
            running.retain(|call| !call.is_finished());
            match line {
                Line::TooLong => self.write(&protocol::unavailable(Value::Null)),
                Line::Text(text) => running.extend(self.receive(&text)),
            }
        }
        for call in running {
            let _ = call.join();
        }
    }

    /// Answers one line, or starts its tool call.
    fn receive(self: &Arc<Self>, text: &[u8]) -> Option<JoinHandle<()>> {
        let Ok(message) = serde_json::from_slice::<Value>(text) else {
            self.write(&protocol::refusal(
                Value::Null,
                protocol::PARSE_ERROR,
                "Invalid JSON.",
            ));
            return None;
        };
        match protocol::classify(message) {
            Message::Request { id, method, params } => return self.request(id, &method, &params),
            Message::Notification { method, params } => {
                if method == protocol::CANCELLED {
                    self.cancel(&params);
                }
            }
            Message::Invalid { id, reason } => {
                self.write(&protocol::refusal(id, protocol::INVALID_REQUEST, reason));
            }
        }
        None
    }

    /// Answers a request, or starts its tool call.
    fn request(
        self: &Arc<Self>,
        id: Value,
        method: &str,
        params: &Map<String, Value>,
    ) -> Option<JoinHandle<()>> {
        let result = match method {
            "initialize" => protocol::initialize(params),
            "ping" => json!({}),
            "tools/list" => json!({ "tools": self.tools }),
            "tools/call" => return self.call(id, params),
            _ => {
                self.write(&protocol::refusal(
                    id,
                    protocol::METHOD_NOT_FOUND,
                    "Unknown method.",
                ));
                return None;
            }
        };
        self.write(&protocol::answer(id, result));
        None
    }

    /// Starts a tool call on its own thread, or refuses it.
    fn call(self: &Arc<Self>, id: Value, params: &Map<String, Value>) -> Option<JoinHandle<()>> {
        let (name, arguments) = match self.checked_call(params) {
            Ok(checked) => checked,
            Err(reason) => {
                self.write(&protocol::refusal(id, protocol::INVALID_PARAMS, reason));
                return None;
            }
        };
        let key = id.to_string();
        let (events, inbox) = mpsc::channel();
        if !self.register(&key, events.clone()) {
            self.write(&protocol::refusal(
                id,
                protocol::INVALID_REQUEST,
                "Duplicate request id or too many active requests.",
            ));
            return None;
        }
        let number = self.next_call.fetch_add(1, Ordering::Relaxed);
        let call = Call {
            id: id.clone(),
            key: key.clone(),
            bridge_id: format!("{}-{number}", self.prefix),
            name,
            arguments,
        };
        let server = Arc::clone(self);
        let started = thread::Builder::new()
            .name("hmi-agent-call".to_owned())
            .spawn(move || server.run(call, &inbox, events));
        match started {
            Ok(running) => Some(running),
            Err(_) => {
                self.calls.lock().remove(&key);
                self.write(&protocol::unavailable(id));
                None
            }
        }
    }

    /// The checks of a tool call: a listed tool, an argument object, and no workspace other than
    /// the connection's. The arguments come back with the connection's workspace.
    fn checked_call(
        &self,
        params: &Map<String, Value>,
    ) -> Result<(String, Map<String, Value>), &'static str> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| self.is_tool(name))
            .ok_or("Unknown authoring tool.")?;
        let Some(Value::Object(arguments)) = params.get("arguments") else {
            return Err("Tool arguments must be an object.");
        };
        match arguments.get(WORKSPACE_ID) {
            None => {}
            Some(Value::String(workspace)) if *workspace == self.workspace_id => {}
            Some(_) => return Err("The tool belongs to another workspace."),
        }
        let mut arguments = arguments.clone();
        arguments.insert(
            WORKSPACE_ID.to_owned(),
            Value::String(self.workspace_id.clone()),
        );
        Ok((name.to_owned(), arguments))
    }

    /// Whether the loopback lists a tool with this name.
    fn is_tool(&self, name: &str) -> bool {
        self.tools
            .iter()
            .any(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
    }

    /// Records a call in progress, unless its id is in progress already or the most calls run.
    fn register(&self, key: &str, events: Sender<Event>) -> bool {
        let mut calls = self.calls.lock();
        let free = !calls.contains_key(key) && calls.len() < self.limits.active_calls;
        if free {
            calls.insert(key.to_owned(), events);
        }
        free
    }

    /// Runs a call at the loopback and answers it. The exchange runs on its own thread, so that the
    /// agent's cancellation and the call's time limit reach the loopback while it waits.
    fn run(&self, call: Call, inbox: &Receiver<Event>, events: Sender<Event>) {
        let bridge = self.bridge.clone();
        let body = json!({ "id": call.bridge_id, "name": call.name, "arguments": call.arguments });
        let exchange = thread::Builder::new()
            .name("hmi-agent-exchange".to_owned())
            .spawn(move || {
                let _ = events.send(Event::Answered(bridge.post(bridge::CALL, &body)));
            });
        let answer = match exchange {
            Ok(_) => self.await_answer(&call.bridge_id, inbox),
            Err(_) => Err(ExchangeError::Unreachable),
        };
        // The id is free again before the agent reads the answer.
        self.calls.lock().remove(&call.key);
        self.write(&match answer {
            Ok(result) => protocol::answer(call.id, result),
            Err(_) => protocol::unavailable(call.id),
        });
    }

    /// Waits for the loopback's answer to a call. A cancellation by the agent, or the end of the
    /// call's time, cancels the call at the loopback once; the tool then answers its cancellation
    /// through the call. Without an answer in the exchange's time, the call fails.
    fn await_answer(
        &self,
        bridge_id: &str,
        inbox: &Receiver<Event>,
    ) -> Result<Value, ExchangeError> {
        let started = Instant::now();
        let cancel_at = started + self.limits.call_time;
        let give_up = started + self.limits.bridge_time;
        let mut cancelled = false;
        loop {
            let until = if cancelled {
                give_up
            } else {
                cancel_at.min(give_up)
            };
            match inbox.recv_timeout(until.saturating_duration_since(Instant::now())) {
                Ok(Event::Answered(answer)) => return answer,
                Err(RecvTimeoutError::Disconnected) => return Err(ExchangeError::Unreachable),
                Err(RecvTimeoutError::Timeout) if Instant::now() >= give_up => {
                    return Err(ExchangeError::TimedOut);
                }
                // The agent cancelled the call, or the call's time is up.
                Ok(Event::Cancel) | Err(RecvTimeoutError::Timeout) => {}
            }
            if !cancelled {
                cancelled = true;
                let _ = self
                    .bridge
                    .post(bridge::CANCEL, &json!({ "id": bridge_id }));
            }
        }
    }

    /// `notifications/cancelled`: cancels the call in progress with the request id it names.
    fn cancel(&self, params: &Map<String, Value>) {
        let Some(id) = params
            .get("requestId")
            .filter(|id| protocol::is_request_id(id))
        else {
            return;
        };
        if let Some(events) = self.calls.lock().get(&id.to_string()) {
            let _ = events.send(Event::Cancel);
        }
    }

    /// Writes one message as one line of standard output.
    fn write(&self, message: &Value) {
        let mut line = message.to_string().into_bytes();
        line.push(b'\n');
        let mut output = self.output.lock();
        let _ = output.write_all(&line).and_then(|()| output.flush());
    }
}

/// The tools that change the editor's draft or the project (specification 43 §3.1); every other
/// tool only reads.
const CHANGING_TOOLS: [&str; 4] = [
    "hmi_compose_page",
    "hmi_edit_draft",
    "hmi_apply_draft",
    "hmi_preview_states",
];

/// The one tool that writes the project's files (specification 43 §3.1).
const DESTRUCTIVE_TOOL: &str = "hmi_apply_draft";

/// `tool` with the annotations a client reads before it asks the user (specification 43 §3.1):
/// whether it only reads, whether it writes the project, and that it reaches nothing beyond the
/// workspace.
fn annotated(mut tool: Value) -> Value {
    let name = tool
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    tool["annotations"] = json!({
        "readOnlyHint": !CHANGING_TOOLS.contains(&name.as_str()),
        "destructiveHint": name == DESTRUCTIVE_TOOL,
        "openWorldHint": false,
    });
    tool
}

/// The tools of the loopback's answer `{ "tools": [...] }`, each one an object with a name, with
/// their annotations.
fn tool_list(answer: Value) -> Option<Vec<Value>> {
    let Value::Object(mut answer) = answer else {
        return None;
    };
    let Some(Value::Array(tools)) = answer.remove("tools") else {
        return None;
    };
    let named = tools
        .iter()
        .all(|tool| tool.get("name").is_some_and(Value::is_string));
    named.then(|| tools.into_iter().map(annotated).collect())
}
