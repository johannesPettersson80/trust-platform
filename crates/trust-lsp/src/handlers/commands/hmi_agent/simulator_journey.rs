//! The HMI agent's simulator journey (specification 43, AT-08): bounded steps on the deployed pages
//! of a named local simulator, run with the cookie of that named session.
//!
//! Before each step, and again right before a step's command, the journey checks that its session
//! is still open and unexpired and that `checkTarget` still accepts the target, then that it was not
//! cancelled. The step reads the deployed page and compares its model with the draft's, key order
//! aside and numbers by value, and follows the page's readback stream. Before a command, the step
//! reads the stream until it says whether the target runs in simulation, and stops with `CONFLICT`
//! when it no longer does.
//!
//! A step with a control sends its command with a new command ID and stops the journey unless the
//! answer is `applied`, `pending` or `confirmed`. A `confirmed` command is complete, and so is an
//! `applied` one the runtime does not follow, as the runtime's own plan of it says
//! (`Plan::is_followed`, specification 39 §12.8 item 5). A followed `applied` command waits for its
//! later outcome on the stream, its confirmation or, for a remote pulse without one, its end, for its
//! control's confirmation time plus two stream periods (5.5 seconds by default), so that the
//! runtime's own outcome arrives inside the wait. A `pending` one (a repeated command ID) waits the
//! same way for a later `applied` or `confirmed`. A later `timed-out`, `refused` or `unknown` ends the
//! step and the journey with that outcome, and a later outcome that does not come within the wait is
//! `timed-out`, a false failure and never a false pass. With a readback path the step waits, within
//! at least 5 seconds, for that path's value, the expected one when it is given (a path the runtime
//! answers as unknown, `null`, has none). Each step adds its outcome: `observed`, or `cancelled`,
//! `disconnected`, `timed-out`, `refused` or `unknown`, which end the journey. The answer holds every
//! outcome and the error that ended the journey, if any, and has passed exactly when no error ended
//! it and its last outcome is `observed`. A command is never resent.
//!
//! Port of `executeSimulatorJourney` in `editors/vscode/src/hmiBuilder/agent/simulator.ts`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use serde::Deserialize;
use serde_json::{json, Map, Number, Value};

use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::commands::plan::{plan, DEFAULT_TIMEOUT};
use trust_runtime::hmi::page_files::HmiObject;

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

impl Step {
    /// The action the step's command sends, if it has one.
    pub(super) fn action(&self) -> Option<&str> {
        self.action.as_deref()
    }
}

/// How long a journey waits: for a step's readback, between two reads of the stream, for one
/// request, for the stream to open, and for one stream period until the stream announces its own.
#[derive(Debug, Clone, Copy)]
pub(super) struct Timing {
    pub(super) wait: Duration,
    pub(super) poll: Duration,
    pub(super) request: Duration,
    pub(super) stream: Duration,
    pub(super) period: Duration,
}

impl Timing {
    /// The product's: 5 seconds, 50 milliseconds, 15 seconds and 5 seconds, the extension's, and the
    /// runtime's shortest stream period, 250 milliseconds.
    pub(super) const PRODUCT: Self = Self {
        wait: Duration::from_secs(5),
        poll: Duration::from_millis(50),
        request: REQUEST_TIMEOUT,
        stream: Duration::from_secs(5),
        period: Duration::from_millis(250),
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
        live,
        cancelled,
        timing,
        outcomes: Vec::new(),
    };
    let error = runner.steps().err();
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
    live: &'a dyn Fn() -> bool,
    cancelled: &'a AtomicBool,
    timing: Timing,
    outcomes: Vec<Value>,
}

impl Runner<'_> {
    fn steps(&mut self) -> AgentResult<()> {
        let steps = self.journey.steps;
        for step in steps {
            self.halted()?;
            if self.step(step)? {
                break;
            }
        }
        Ok(())
    }

    /// Refuses to go on when the session closed or expired or the target is no longer a local
    /// simulator (`CONFLICT`), or when the journey was cancelled (`CANCELLED`).
    fn halted(&self) -> AgentResult<()> {
        if !(self.live)() || !is_local_simulator(self.journey.target) {
            return Err(AgentError::conflict(
                "the simulator target or named session changed.",
            ));
        }
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(AgentError::new(
                CANCELLED,
                "simulator journey cancelled; completed commands are retained in outcomes and are \
                 not undone.",
            ));
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
            let is_object = |control: &&Value| {
                control.get("id").and_then(Value::as_str) == Some(object.as_str())
            };
            let Some(control) = controls.and_then(|controls| controls.iter().find(is_object))
            else {
                return Err(AgentError::invalid(
                    "simulator step names an unknown control.",
                ));
            };
            self.settle(stream, &mut seen);
            if seen.disconnected {
                self.outcomes
                    .push(json!({ "page": step.page, "outcome": "disconnected" }));
                return Ok(true);
            }
            if seen.simulation == Some(false) {
                return Err(AgentError::conflict(
                    "the simulator target no longer runs in simulation mode; connect the simulator \
                     journey again.",
                ));
            }
            self.halted()?;
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
            let answered = answer.get("outcome").and_then(Value::as_str);
            let answered = answered.unwrap_or_default().to_owned();
            if status != 200 || !matches!(answered.as_str(), "applied" | "pending" | "confirmed") {
                return Ok(true);
            }
            let following = following(control, step);
            command = Some(Sent {
                id,
                answered,
                follow: following.follow,
                confirmation: following.confirmation,
            });
        }
        Ok(self.wait(stream, step, command.as_ref(), &mut seen))
    }

    /// Reads the stream until it has said whether the target runs in simulation, for at most one
    /// stream period: the runtime says it as soon as the stream opens.
    fn settle(&self, stream: &mut Readback, seen: &mut Seen) {
        let deadline = Instant::now() + seen.period.unwrap_or(self.timing.period);
        while seen.simulation.is_none() && !seen.disconnected {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            seen.read(stream, self.timing.poll.min(left));
        }
    }

    /// Waits for the command's later outcome and the readback; `true` when the step's outcome ends
    /// the journey.
    fn wait(
        &mut self,
        stream: &mut Readback,
        step: &Step,
        command: Option<&Sent>,
        seen: &mut Seen,
    ) -> bool {
        let started = Instant::now();
        let mut deadline = started + self.timing.wait;
        if let Some(command) = command
            && !command.complete(None)
        {
            let period = seen.period.unwrap_or(self.timing.period);
            deadline = deadline.max(started + command.confirmation + period * 2);
        }
        loop {
            // The later outcome the stream reported for the command, after its answer.
            let report = command.and_then(|command| seen.commands.get(&command.id));
            let reported = report
                .and_then(|report| report.get("outcome"))
                .and_then(Value::as_str);
            let failed = reported
                .filter(|reported| matches!(*reported, "timed-out" | "refused" | "unknown"));
            let ended = if self.cancelled.load(Ordering::SeqCst) {
                Some("cancelled")
            } else if seen.disconnected {
                Some("disconnected")
            } else if failed.is_some() {
                failed
            } else if Instant::now() >= deadline {
                Some("timed-out")
            } else {
                None
            };
            if let Some(ended) = ended {
                let mut outcome = json!({ "page": step.page, "outcome": ended });
                if let Some(command) = command {
                    outcome["commandId"] = json!(command.id);
                }
                if let Some(report) = report.filter(|_| failed == Some(ended)) {
                    for key in ["readback", "reason"] {
                        if let Some(value) = report.get(key) {
                            outcome[key] = value.clone();
                        }
                    }
                }
                self.outcomes.push(outcome);
                return true;
            }
            let confirmed = command.is_none_or(|command| command.complete(reported));
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
                    outcome["commandId"] = json!(command.id);
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
            seen.read(stream, wait);
        }
    }
}

/// A command a step sent: its ID, the outcome it was answered with, what the runtime does with it
/// after that answer, and its control's confirmation time.
struct Sent {
    id: String,
    answered: String,
    follow: Follow,
    confirmation: Duration,
}

impl Sent {
    /// Whether the command is complete, given the later outcome the stream reported for it, if any:
    /// a `confirmed` answer, or an `applied` one the runtime does not follow, at once; a followed
    /// `applied` one at its later confirmation, or for a remote pulse without one at its later end
    /// (`applied`); a `pending` one at its later `applied` or `confirmed`.
    fn complete(&self, reported: Option<&str>) -> bool {
        match (self.answered.as_str(), self.follow) {
            ("confirmed", _) | ("applied", Follow::None) => true,
            ("applied", Follow::Confirmation) => reported == Some("confirmed"),
            _ => matches!(reported, Some("applied" | "confirmed")),
        }
    }
}

/// What the runtime does with a command after answering it `applied` (specification 39, §12.8
/// item 5): nothing, so the answer is final; follow it to its confirmation; or, for a pulse to a
/// remote runtime's point without a confirmation, follow it to the pulse's end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Follow {
    None,
    Confirmation,
    PulseEnd,
}

/// What the runtime does with a command, and its control's confirmation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Following {
    follow: Follow,
    confirmation: Duration,
}

/// What the runtime does with the command `step` sends to `control`, a deployed page's object, as
/// the runtime's own plan of that command says (`Plan::is_followed`, `Plan::timeout`). A control
/// whose plan cannot be read is followed to a confirmation within the runtime's default time: an
/// outcome that never comes reports `timed-out`, never a false pass.
fn following(control: &Value, step: &Step) -> Following {
    let planned = serde_json::from_value::<HmiObject>(control.clone())
        .ok()
        .and_then(|object| {
            let component = Catalogue::shipped().component(&object.kind)?;
            let action = step.action.as_deref().unwrap_or_default();
            plan(&object, component, action, step.value.as_ref(), None).ok()
        });
    let follow = match &planned {
        Some(plan) if !plan.is_followed() => Follow::None,
        Some(plan) if plan.confirm.is_none() => Follow::PulseEnd,
        _ => Follow::Confirmation,
    };
    let confirmation = planned.map_or(DEFAULT_TIMEOUT, |plan| plan.timeout);
    Following {
        follow,
        confirmation,
    }
}

/// What a step's stream has said: each path's last entry, each command's later outcome, whether the
/// target runs in simulation, the stream's period, and whether the stream ended or sent what is not
/// JSON.
#[derive(Default)]
struct Seen {
    values: Map<String, Value>,
    commands: BTreeMap<String, Value>,
    simulation: Option<bool>,
    period: Option<Duration>,
    disconnected: bool,
}

impl Seen {
    /// Reads the stream's next message, waiting at most `wait`.
    fn read(&mut self, stream: &mut Readback, wait: Duration) {
        match stream.next(wait) {
            Received::Text(text) => self.absorb(&text),
            Received::Idle => {}
            Received::Closed => self.disconnected = true,
        }
    }

    /// Takes in one message: `{ values }` entries by path, a `{ command }` outcome by its ID, the
    /// `{ simulation }` state and the `{ period_ms }` the stream announces.
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
        if let Some(simulation) = message.get("simulation") {
            self.simulation = simulation.get("enabled").and_then(Value::as_bool);
        }
        if let Some(period) = message.get("period_ms").and_then(Value::as_u64) {
            self.period = Some(Duration::from_millis(period));
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
