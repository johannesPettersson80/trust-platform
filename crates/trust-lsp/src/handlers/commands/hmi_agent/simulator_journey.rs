//! The HMI agent's simulator journey (specification 43, AT-08): bounded steps on the deployed pages
//! of a named local simulator, run with the cookie of that named session.
//!
//! Before each step the journey checks that its session is still open and unexpired and that
//! `checkTarget` still accepts the target. The step reads the deployed page and compares its model
//! with the draft's, key order aside and numbers by value, and follows the page's readback stream.
//! A step with a control sends its command with a new command ID and stops the journey unless the
//! answer is `applied`, `pending` or `confirmed`; then the step waits at most 5 seconds for the
//! command's confirmation and, with a readback path, for that path's value, the expected one when it
//! is given (a path the runtime answers as unknown, `null`, has none). Each step adds its outcome:
//! `observed`, or `cancelled`, `disconnected` or `timed-out`, which end the journey. The answer holds
//! every outcome and the error that ended the journey, if any, and has passed exactly when no error
//! ended it and its last outcome is `observed`. A command is never resent.
//!
//! Port of `executeSimulatorJourney` in `editors/vscode/src/hmiBuilder/agent/simulator.ts`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::{json, Map, Number, Value};

use super::draft_records::new_uuid;
use super::error::{AgentError, AgentResult, CANCELLED};
use super::simulator_http::{SimulatorHmi, REQUEST_TIMEOUT};
use super::simulator_stream::{Readback, Received};
use super::target::is_local_simulator;

/// What JavaScript's `encodeURIComponent` leaves as it is: letters, digits and `-_.!~*'()`.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// One step of a journey, as `hmi_preview_states` names it (AT-08): a deployed page, an optional
/// control with its action and value, and an optional readback path with its expected value.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Step {
    page: String,
    object: Option<String>,
    action: Option<String>,
    value: Option<Value>,
    path: Option<String>,
    expect: Option<Value>,
}

/// How long a journey waits: for a step's confirmation and readback, between two reads of the
/// stream, for one request, and for the stream to open.
#[derive(Debug, Clone, Copy)]
pub(super) struct Timing {
    pub(super) wait: Duration,
    pub(super) poll: Duration,
    pub(super) request: Duration,
    pub(super) stream: Duration,
}

impl Timing {
    /// The product's: 5 seconds, 50 milliseconds, 15 seconds and 5 seconds, the extension's.
    pub(super) const PRODUCT: Self = Self {
        wait: Duration::from_secs(5),
        poll: Duration::from_millis(50),
        request: REQUEST_TIMEOUT,
        stream: Duration::from_secs(5),
    };
}

/// A journey to run: its session's ID and name, the target (`{ simulator, url, endpoint, token }`),
/// the session's cookie, the steps and the draft's page models by page name.
pub(super) struct Journey<'a> {
    pub(super) session_id: &'a str,
    pub(super) name: &'a str,
    pub(super) target: &'a Value,
    pub(super) cookie: &'a str,
    pub(super) steps: &'a [Step],
    pub(super) models: &'a Map<String, Value>,
}

/// Runs `journey`. `live` answers whether its session is still open and unexpired; `cancelled` is
/// set when the journey is cancelled. The answer is `{ origin, sessionId, name, outcomes, passed }`,
/// with `error` when an error ended the journey.
pub(super) fn run(
    journey: &Journey<'_>,
    live: &dyn Fn() -> bool,
    cancelled: &AtomicBool,
    timing: Timing,
) -> Value {
    let token = journey.target.get("token").and_then(Value::as_str);
    let mut runner = Runner {
        hmi: SimulatorHmi {
            base: journey
                .target
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            token: token.filter(|token| !token.is_empty()),
            cookie: journey.cookie,
            timeout: timing.request,
        },
        journey,
        cancelled,
        timing,
        outcomes: Vec::new(),
    };
    let error = runner.steps(live).err();
    let last = runner.outcomes.last();
    let passed = error.is_none() && last.is_some_and(|outcome| outcome["outcome"] == "observed");
    let mut answer = json!({
        "origin": "simulator",
        "sessionId": journey.session_id,
        "name": journey.name,
        "outcomes": runner.outcomes,
        "passed": passed,
    });
    if let Some(error) = error {
        answer["error"] = Value::String(error.to_string());
    }
    answer
}

/// A running journey and the outcomes its steps added.
struct Runner<'a> {
    hmi: SimulatorHmi<'a>,
    journey: &'a Journey<'a>,
    cancelled: &'a AtomicBool,
    timing: Timing,
    outcomes: Vec<Value>,
}

impl Runner<'_> {
    fn steps(&mut self, live: &dyn Fn() -> bool) -> AgentResult<()> {
        let steps = self.journey.steps;
        for step in steps {
            if self.cancelled.load(Ordering::SeqCst) {
                return Err(AgentError::new(
                    CANCELLED,
                    "simulator journey cancelled; completed commands are retained in outcomes and \
                     are not undone.",
                ));
            }
            if !live() || !is_local_simulator(self.journey.target) {
                return Err(AgentError::conflict(
                    "the simulator target or named session changed.",
                ));
            }
            if self.step(step)? {
                break;
            }
        }
        Ok(())
    }

    /// One step; `true` when its outcome ends the journey.
    fn step(&mut self, step: &Step) -> AgentResult<bool> {
        let page = utf8_percent_encode(&step.page, COMPONENT).to_string();
        let (status, answer) = self.hmi.send(&format!("/api/hmi/page?name={page}"), None)?;
        let deployed = answer.get("page").and_then(|page| page.get("model"));
        let draft = self.journey.models.get(&step.page);
        let matching = deployed
            .zip(draft)
            .filter(|(deployed, draft)| same_model(deployed, draft));
        let Some((deployed, _)) = matching.filter(|_| status == 200) else {
            return Err(AgentError::conflict(
                "the deployed simulator page does not match this draft; apply and deploy explicitly \
                 before exercising it.",
            ));
        };
        let path = format!("/ws/hmi?page={page}");
        let mut stream = Readback::open(&self.hmi, &path, self.timing.stream)?;
        let ended = self.exercise(&mut stream, step, &answer, deployed);
        stream.close();
        ended
    }

    /// The step's command, if it has a control, and its wait; `true` when an outcome ends the
    /// journey.
    fn exercise(
        &mut self,
        stream: &mut Readback,
        step: &Step,
        page: &Value,
        deployed: &Value,
    ) -> AgentResult<bool> {
        let mut seen = Seen::default();
        let mut command = None;
        if let Some(object) = &step.object {
            let controls = deployed.get("object").and_then(Value::as_array);
            let is_object = |control: &Value| {
                control.get("id").and_then(Value::as_str) == Some(object.as_str())
            };
            if !controls.is_some_and(|controls| controls.iter().any(is_object)) {
                return Err(AgentError::invalid(
                    "simulator step names an unknown control.",
                ));
            }
            let id = new_uuid("a simulator command identifier")?;
            let mut body = Map::new();
            body.insert("id".into(), json!(id));
            if let Some(bundle) = page.get("bundle") {
                body.insert("bundle".into(), bundle.clone());
            }
            body.insert("page".into(), json!(step.page));
            body.insert("object".into(), json!(object));
            if let Some(action) = &step.action {
                body.insert("action".into(), json!(action));
            }
            if let Some(value) = &step.value {
                body.insert("value".into(), value.clone());
            }
            let (status, answer) = self
                .hmi
                .send("/api/hmi/command", Some(&Value::Object(body)))?;
            let mut outcome = Map::new();
            outcome.insert("page".into(), json!(step.page));
            outcome.insert("object".into(), json!(object));
            outcome.insert("commandId".into(), json!(id));
            outcome.insert("status".into(), json!(status));
            for key in ["outcome", "readback", "reason"] {
                if let Some(value) = answer.get(key) {
                    outcome.insert(key.into(), value.clone());
                }
            }
            self.outcomes.push(Value::Object(outcome));
            let named = answer.get("outcome").and_then(Value::as_str);
            let accepted =
                named.is_some_and(|named| matches!(named, "applied" | "pending" | "confirmed"));
            if status != 200 || !accepted {
                return Ok(true);
            }
            seen.commands.insert(id.clone(), answer);
            command = Some(id);
        }
        Ok(self.wait(stream, step, command.as_deref(), &mut seen))
    }

    /// Waits for the command's confirmation and the readback; `true` when the step's outcome ends
    /// the journey.
    fn wait(
        &mut self,
        stream: &mut Readback,
        step: &Step,
        command: Option<&str>,
        seen: &mut Seen,
    ) -> bool {
        let deadline = Instant::now() + self.timing.wait;
        loop {
            let ended = if self.cancelled.load(Ordering::SeqCst) {
                Some("cancelled")
            } else if seen.disconnected {
                Some("disconnected")
            } else if Instant::now() >= deadline {
                Some("timed-out")
            } else {
                None
            };
            if let Some(ended) = ended {
                let mut outcome = json!({ "page": step.page, "outcome": ended });
                if let Some(command) = command {
                    outcome["commandId"] = json!(command);
                }
                self.outcomes.push(outcome);
                return true;
            }
            let confirmed = command.is_none_or(|command| {
                let answer = seen.commands.get(command);
                answer
                    .and_then(|answer| answer.get("outcome"))
                    .and_then(Value::as_str)
                    == Some("confirmed")
            });
            let readback = match &step.path {
                None => Some(None),
                Some(path) => seen
                    .value_of(path)
                    .filter(|value| {
                        step.expect
                            .as_ref()
                            .is_none_or(|expect| same_model(value, expect))
                    })
                    .map(Some),
            };
            if confirmed && let Some(value) = readback {
                let mut outcome = json!({ "page": step.page, "outcome": "observed" });
                if let Some(command) = command {
                    outcome["commandId"] = json!(command);
                }
                if let Some(path) = &step.path {
                    outcome["path"] = json!(path);
                    outcome["value"] = value.unwrap_or(Value::Null);
                }
                self.outcomes.push(outcome);
                return false;
            }
            let wait = self
                .timing
                .poll
                .min(deadline.saturating_duration_since(Instant::now()));
            match stream.next(wait) {
                Received::Text(text) => seen.absorb(&text),
                Received::Idle => {}
                Received::Closed => seen.disconnected = true,
            }
        }
    }
}

/// What a step's stream has said: each path's last entry, each command's last answer, and whether
/// the stream ended or sent what is not JSON.
#[derive(Default)]
struct Seen {
    values: Map<String, Value>,
    commands: BTreeMap<String, Value>,
    disconnected: bool,
}

impl Seen {
    /// Takes in one message: `{ values }` entries by path and a `{ command }` answer by its ID.
    fn absorb(&mut self, text: &str) {
        let Ok(message) = serde_json::from_str::<Value>(text) else {
            self.disconnected = true;
            return;
        };
        if let Some(values) = message.get("values").and_then(Value::as_object) {
            for (path, entry) in values {
                self.values.insert(path.clone(), entry.clone());
            }
        }
        if let Some(command) = message.get("command")
            && let Some(id) = command
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
        {
            self.commands.insert(id.to_owned(), command.clone());
        }
    }

    /// The value the stream gave for `path`: none while it gave nothing, or gave `null`, the entry
    /// of a path the runtime does not know.
    fn value_of(&self, path: &str) -> Option<Value> {
        let entry = self.values.get(path).filter(|entry| !entry.is_null())?;
        Some(entry.get("value").cloned().unwrap_or(Value::Null))
    }
}

/// Whether two models are the same: objects whatever the order of their keys, arrays item by item,
/// numbers by their value (`sameModel` in `pageDocument.ts`). A draft's model passes through the
/// extension, where `5.0` becomes `5`; the deployed page keeps `5.0`.
pub(super) fn same_model(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => same_number(left, right),
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same_model(left, right))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, value)| {
                    right.get(key).is_some_and(|other| same_model(value, other))
                })
        }
        _ => left == right,
    }
}

/// Whether two numbers have the same value: exactly as integers, otherwise as floating point.
fn same_number(left: &Number, right: &Number) -> bool {
    if let (Some(left), Some(right)) = (left.as_i64(), right.as_i64()) {
        return left == right;
    }
    if let (Some(left), Some(right)) = (left.as_u64(), right.as_u64()) {
        return left == right;
    }
    left.as_f64() == right.as_f64()
}

#[cfg(test)]
#[path = "simulator_journey_tests.rs"]
mod tests;
