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

use super::lines::{self, Line};
use super::protocol::{self, Message};
use super::{Limits, ToolProvider, Unavailable};

/// Why the server cannot start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartError {
    /// No random call-id prefix could be drawn.
    Random,
}

/// What reaches a tool call in progress.
enum Event {
    /// The agent cancelled the call.
    Cancel,
    /// The provider answered the call, or could not.
    Answered(Result<Value, Unavailable>),
}

/// A tool call accepted for the provider.
struct Call {
    /// The agent's request id.
    id: Value,
    /// The request id as JSON text: its key among the calls in progress.
    key: String,
    /// The id the provider knows the call by.
    call_id: String,
    /// The tool's name.
    name: String,
    /// The tool's arguments, as the provider's checks gave them.
    arguments: Map<String, Value>,
}

/// The server of one connection.
pub struct Server {
    provider: Arc<dyn ToolProvider>,
    limits: Limits,
    /// The prefix of the call ids this server gives the provider, drawn at random so that two
    /// servers on one connection never share an id.
    prefix: String,
    next_call: AtomicU64,
    /// The calls in progress, by request key: where each one's events go.
    calls: Mutex<HashMap<String, Sender<Event>>>,
    /// Standard output: protocol messages only, one per line.
    output: Mutex<Box<dyn Write + Send>>,
}

impl Server {
    /// The server of the tools of `provider`, within `limits`, which writes its messages to
    /// `output`.
    pub fn start(
        provider: Arc<dyn ToolProvider>,
        limits: Limits,
        output: Box<dyn Write + Send>,
    ) -> Result<Arc<Self>, StartError> {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|_| StartError::Random)?;
        Ok(Arc::new(Self {
            provider,
            limits,
            prefix: hex::encode(random),
            next_call: AtomicU64::new(1),
            calls: Mutex::new(HashMap::new()),
            output: Mutex::new(output),
        }))
    }

    /// Serves the agent until its input ends, then waits until the calls in progress have
    /// answered.
    pub fn serve(self: &Arc<Self>, input: &mut impl BufRead) {
        let mut running: Vec<JoinHandle<()>> = Vec::new();
        while let Ok(Some(line)) = lines::read_line(input, self.limits.line_bytes) {
            running.retain(|call| !call.is_finished());
            match line {
                Line::TooLong => self.write(&self.unavailable(Value::Null)),
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
            "initialize" => protocol::initialize(params, &self.provider.info()),
            "ping" => json!({}),
            "tools/list" => json!({ "tools": self.provider.tools() }),
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
        let (name, arguments) = match self.provider.check(params) {
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
            call_id: format!("{}-{number}", self.prefix),
            name,
            arguments,
        };
        let server = Arc::clone(self);
        let started = thread::Builder::new()
            .name("mcp-call".to_owned())
            .spawn(move || server.run(call, &inbox, events));
        match started {
            Ok(running) => Some(running),
            Err(_) => {
                self.calls.lock().remove(&key);
                self.write(&self.unavailable(id));
                None
            }
        }
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

    /// Runs a call at the provider and answers it. The provider runs the call on its own thread,
    /// so that the agent's cancellation and the call's time limit reach the provider while it
    /// waits.
    fn run(&self, call: Call, inbox: &Receiver<Event>, events: Sender<Event>) {
        let Call {
            id,
            key,
            call_id,
            name,
            arguments,
        } = call;
        let provider = Arc::clone(&self.provider);
        let running_id = call_id.clone();
        let exchange = thread::Builder::new()
            .name("mcp-exchange".to_owned())
            .spawn(move || {
                let answer = provider.call(&running_id, &name, arguments);
                let _ = events.send(Event::Answered(answer));
            });
        let answer = match exchange {
            Ok(_) => self.await_answer(&call_id, inbox),
            Err(_) => Err(Unavailable),
        };
        // The id is free again before the agent reads the answer.
        self.calls.lock().remove(&key);
        self.write(&match answer {
            Ok(result) => protocol::answer(id, result),
            Err(Unavailable) => self.unavailable(id),
        });
    }

    /// Waits for the provider's answer to a call. A cancellation by the agent, or the end of the
    /// call's time, cancels the call at the provider once; the tool then answers its cancellation
    /// through the call. Without an answer in the answer time, the call fails.
    fn await_answer(&self, call_id: &str, inbox: &Receiver<Event>) -> Result<Value, Unavailable> {
        let started = Instant::now();
        let cancel_at = started + self.limits.call_time;
        let give_up = started + self.limits.answer_time;
        let mut cancelled = false;
        loop {
            let until = if cancelled {
                give_up
            } else {
                cancel_at.min(give_up)
            };
            match inbox.recv_timeout(until.saturating_duration_since(Instant::now())) {
                Ok(Event::Answered(answer)) => return answer,
                Err(RecvTimeoutError::Disconnected) => return Err(Unavailable),
                Err(RecvTimeoutError::Timeout) if Instant::now() >= give_up => {
                    return Err(Unavailable);
                }
                // The agent cancelled the call, or the call's time is up.
                Ok(Event::Cancel) | Err(RecvTimeoutError::Timeout) => {}
            }
            if !cancelled {
                cancelled = true;
                self.provider.cancel(call_id);
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

    /// The failure answer of request `id`, with the provider's message.
    fn unavailable(&self, id: Value) -> Value {
        protocol::unavailable(id, self.provider.info().unavailable)
    }

    /// Writes one message as one line of standard output.
    fn write(&self, message: &Value) {
        let mut line = message.to_string().into_bytes();
        line.push(b'\n');
        let mut output = self.output.lock();
        let _ = output.write_all(&line).and_then(|()| output.flush());
    }
}
