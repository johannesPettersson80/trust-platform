//! The simulator session and journey commands of the HMI agent (specification 43, AT-08):
//! `trust-lsp.hmiAgent.openSimulatorSession`, `closeSimulatorSession`, `simulatorSessions`,
//! `admitSimulatorLogin`, `runSimulatorJourney` and `cancelSimulatorJourney`.
//!
//! Each takes one argument object and answers the authoring envelope, `{ ok: true, data, revisions,
//! warnings }` or `{ ok: false, code, message, retryable }`. This module decodes the arguments; the
//! rules are the sessions' (`simulator_sessions`) and the journey's (`simulator_journey`). The
//! extension keeps the named login and the session's cookie (specification 39 §11 item 6) and passes
//! the cookie with each journey, which holds it only while it runs. No answer, refusal or log line
//! names the cookie or the target's access token.
//!
//! `runSimulatorJourney` starts the journey on a thread of its own and answers at once, so that a
//! journey never holds one of the language server's request slots and a cancel never waits behind
//! journeys. The client names each journey (`journeyId`); the language client gets the journey's
//! answer, with that ID, as the notification `trust-lsp/simulatorJourneyEnded` when it ends, also
//! when the journey's thread panics. A cancel names the journey it cancels.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;
use std::time::SystemTime;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tower_lsp::lsp_types::notification::Notification;
use tower_lsp::Client;

use super::boundary::{self, Success};
use super::error::{AgentError, AgentResult, PROJECT_UNAVAILABLE};
use super::schema::MAX_PREVIEW;
use super::simulator_journey::{self, Journey, Step, Timing};
use super::simulator_sessions::SimulatorSessions;

/// Opens a named session for a workspace, a journey's name and a target.
pub(super) const OPEN_SESSION: &str = "trust-lsp.hmiAgent.openSimulatorSession";
/// Closes a session, as after a login the simulator refused, cancelling its running journey.
pub(super) const CLOSE_SESSION: &str = "trust-lsp.hmiAgent.closeSimulatorSession";
/// The longest journey ID a client may give.
const MAX_JOURNEY_ID: usize = 128;
/// Lists a workspace's unexpired sessions, for the agent's context.
pub(super) const LIST_SESSIONS: &str = "trust-lsp.hmiAgent.simulatorSessions";
/// Answers whether the simulator login for a session may go to a target.
pub(super) const ADMIT_LOGIN: &str = "trust-lsp.hmiAgent.admitSimulatorLogin";
/// Starts a journey with its session's cookie.
pub(super) const RUN_JOURNEY: &str = "trust-lsp.hmiAgent.runSimulatorJourney";
/// Cancels a journey of a session, by its ID.
pub(super) const CANCEL_JOURNEY: &str = "trust-lsp.hmiAgent.cancelSimulatorJourney";

/// The simulator commands.
const COMMANDS: [&str; 6] = [
    OPEN_SESSION,
    CLOSE_SESSION,
    LIST_SESSIONS,
    ADMIT_LOGIN,
    RUN_JOURNEY,
    CANCEL_JOURNEY,
];

/// The notification that gives the language client a journey's answer when the journey ends:
/// `{ origin, sessionId, journeyId, name, outcomes, passed, error? }`.
pub(super) enum SimulatorJourneyEnded {}

impl Notification for SimulatorJourneyEnded {
    type Params = Value;
    const METHOD: &'static str = "trust-lsp/simulatorJourneyEnded";
}

/// What gives the language client a journey's answer when the journey ends.
pub(super) type Report = Box<dyn FnOnce(Value) + Send>;
/// The clock the sessions and the journeys read.
pub(super) type Clock = Arc<dyn Fn() -> SystemTime + Send + Sync>;

/// The named simulator sessions of this language server.
static SERVER_SESSIONS: SimulatorSessions = SimulatorSessions::new();

/// Whether `name` is a simulator command.
#[must_use]
pub(super) fn is_command(name: &str) -> bool {
    COMMANDS.contains(&name)
}

/// Runs the simulator command `name` on its argument. `runSimulatorJourney` reports the journey's
/// end to the language client, so it runs only through [`start_for_client`].
#[must_use]
pub(super) fn execute(name: &str, arguments: Vec<Value>) -> Value {
    answer(&SERVER_SESSIONS, name, arguments, &system_clock())
}

/// `runSimulatorJourney` from the language client: the journey starts on a thread of its own and
/// the answer comes at once; the client gets the journey's answer as
/// `trust-lsp/simulatorJourneyEnded` when it ends.
#[must_use]
pub(super) fn start_for_client(client: &Client, arguments: Vec<Value>) -> Value {
    let client = client.clone();
    let runtime = tokio::runtime::Handle::current();
    let report: Report = Box::new(move |answer| {
        runtime.block_on(client.send_notification::<SimulatorJourneyEnded>(answer));
    });
    let started = start(
        &SERVER_SESSIONS,
        arguments,
        system_clock(),
        Timing::PRODUCT,
        report,
    );
    envelope(started)
}

fn system_clock() -> Clock {
    Arc::new(SystemTime::now)
}

/// The envelope answer of one command on `sessions`, at the times `clock` gives.
fn answer(sessions: &SimulatorSessions, name: &str, arguments: Vec<Value>, clock: &Clock) -> Value {
    let result = one_argument(arguments).and_then(|argument| {
        match name {
        OPEN_SESSION => {
            let request: OpenArgument = decode(argument)?;
            required("workspace", &request.workspace)?;
            required("name", &request.name)?;
            sessions.open(&request.workspace, &request.name, &request.target, clock())
        }
        CLOSE_SESSION => {
            let request: SessionArgument = decode(argument)?;
            Ok(json!({ "closed": sessions.close(&request.session_id) }))
        }
        LIST_SESSIONS => {
            let request: WorkspaceArgument = decode(argument)?;
            required("workspace", &request.workspace)?;
            Ok(json!({ "sessions": sessions.list(&request.workspace, clock()) }))
        }
        ADMIT_LOGIN => {
            let request: AdmitArgument = decode(argument)?;
            let admitted = sessions.admits(&request.session_id, &request.target, clock());
            Ok(json!({ "admitted": admitted }))
        }
        CANCEL_JOURNEY => {
            let request: CancelArgument = decode(argument)?;
            journey_id(&request.journey_id)?;
            let cancelled = sessions.cancel(&request.session_id, &request.journey_id, clock());
            Ok(json!({ "cancelled": cancelled }))
        }
        RUN_JOURNEY => Err(AgentError::invalid(
            "runSimulatorJourney reports the journey's end to the language client, so it runs only \
             through the language server.",
        )),
        _ => Err(AgentError::invalid("unknown simulator command.")),
    }
    });
    envelope(result)
}

fn envelope(result: AgentResult<Value>) -> Value {
    match result {
        Ok(data) => boundary::success_envelope(&Success {
            data,
            revisions: BTreeMap::new(),
            warnings: Vec::new(),
        }),
        Err(error) => boundary::failure_envelope(&error),
    }
}

/// `runSimulatorJourney`: refuses the journey, or begins it on its session and runs it on a thread
/// of its own, answering `{ started: true, sessionId, journeyId }` at once. `report` gets the
/// journey's answer when it ends, while the session still counts it as running, so that the answer
/// is queued for the client before the session can take its next journey. A journey whose thread
/// panics still reports an answer, with the error that says so.
fn start(
    sessions: &'static SimulatorSessions,
    arguments: Vec<Value>,
    clock: Clock,
    timing: Timing,
    report: Report,
) -> AgentResult<Value> {
    let mut argument = one_argument(arguments)?;
    // The cookie leaves the argument first, so that no refusal of the other fields can quote it.
    let cookie = argument
        .as_object_mut()
        .and_then(|fields| fields.remove("cookie"));
    let request: RunArgument = decode(argument)?;
    required("workspace", &request.workspace)?;
    required("sessionId", &request.session_id)?;
    journey_id(&request.journey_id)?;
    if request.steps.is_empty() || request.steps.len() > MAX_PREVIEW {
        return Err(AgentError::limit(format!(
            "use 1 to {MAX_PREVIEW} bounded simulator steps."
        )));
    }
    // The runtime takes the action `invoke` for a capability request, not a control's command
    // (`Commands::execute` in trust-runtime's web/hmi_command.rs).
    if request
        .steps
        .iter()
        .any(|step| step.action() == Some("invoke"))
    {
        return Err(AgentError::invalid(
            "a method command (`invoke`) is not a simulator journey step; a step's command writes, \
             sets, resets, toggles or pulses.",
        ));
    }
    let running = sessions.begin(
        &request.workspace,
        &request.session_id,
        &request.journey_id,
        &request.target,
        clock(),
    )?;
    let cookie = named_cookie(cookie.as_ref())?.to_owned();
    let (session_id, journey_id) = (request.session_id.clone(), request.journey_id.clone());
    thread::Builder::new()
        .name(format!("hmi-journey-{session_id}"))
        .spawn(move || {
            // Dropped after `ending`, so that the answer is queued before the session is free.
            let running = running;
            let ending = Ending::new(
                report,
                &request.session_id,
                &request.journey_id,
                running.name(),
            );
            let journey = Journey {
                session_id: &request.session_id,
                name: running.name(),
                journey_id: &request.journey_id,
                target: &request.target,
                cookie: &cookie,
                steps: &request.steps,
                models: &request.models,
            };
            let live = || sessions.is_live(&request.session_id, clock());
            let answer = simulator_journey::run(&journey, &live, running.cancellation(), timing);
            tracing::debug!(
                session = %request.session_id,
                passed = %answer["passed"],
                "HMI agent simulator journey ended"
            );
            ending.send(answer);
        })
        .map_err(|error| {
            AgentError::new(
                PROJECT_UNAVAILABLE,
                format!("the simulator journey could not start: {error}"),
            )
        })?;
    Ok(json!({ "started": true, "sessionId": session_id, "journeyId": journey_id }))
}

/// The end of a journey's thread: it sends the journey's answer, or, when the thread panicked before
/// it could, an answer with the error that says so, so that the client's wait always ends.
struct Ending {
    report: Option<Report>,
    session_id: String,
    journey_id: String,
    name: String,
}

impl Ending {
    fn new(report: Report, session_id: &str, journey_id: &str, name: &str) -> Self {
        Self {
            report: Some(report),
            session_id: session_id.to_owned(),
            journey_id: journey_id.to_owned(),
            name: name.to_owned(),
        }
    }

    /// Sends `answer`.
    fn send(mut self, answer: Value) {
        if let Some(report) = self.report.take() {
            report(answer);
        }
    }
}

impl Drop for Ending {
    fn drop(&mut self) {
        let Some(report) = self.report.take() else {
            return;
        };
        report(json!({
            "origin": "simulator",
            "sessionId": self.session_id,
            "journeyId": self.journey_id,
            "name": self.name,
            "outcomes": [],
            "passed": false,
            "error": format!(
                "{PROJECT_UNAVAILABLE}: the simulator journey stopped unexpectedly; the outcomes of \
                 its steps are not known."
            ),
        }));
    }
}

/// The one argument object of a command.
fn one_argument(arguments: Vec<Value>) -> AgentResult<Value> {
    let [argument] = <[Value; 1]>::try_from(arguments)
        .map_err(|_| AgentError::invalid("the simulator command expects one argument object."))?;
    Ok(argument)
}

/// The argument decoded as `T`.
fn decode<T: DeserializeOwned>(argument: Value) -> AgentResult<T> {
    serde_json::from_value(argument)
        .map_err(|error| AgentError::invalid(format!("the argument is malformed: {error}.")))
}

/// Refuses a blank journey ID, or one longer than 128 characters.
fn journey_id(id: &str) -> AgentResult<()> {
    required("journeyId", id)?;
    if id.chars().count() > MAX_JOURNEY_ID {
        return Err(AgentError::invalid(format!(
            "journeyId has at most {MAX_JOURNEY_ID} characters."
        )));
    }
    Ok(())
}

/// Refuses a blank text field.
fn required(field: &str, text: &str) -> AgentResult<()> {
    if text.trim().is_empty() {
        return Err(AgentError::invalid(format!("{field} is required.")));
    }
    Ok(())
}

/// The named session's cookie as the HMI bridge keeps it, `trust_hmi_session=` and the session's
/// token in visible characters other than `;`. The refusal never quotes it.
fn named_cookie(cookie: Option<&Value>) -> AgentResult<&str> {
    let token = |cookie: &&str| {
        cookie
            .strip_prefix("trust_hmi_session=")
            .is_some_and(|token| {
                !token.is_empty()
                    && token
                        .bytes()
                        .all(|byte| byte.is_ascii_graphic() && byte != b';')
            })
    };
    cookie.and_then(Value::as_str).filter(token).ok_or_else(|| {
        AgentError::invalid(
            "the journey needs the cookie of its named session; connect the simulator journey \
             again.",
        )
    })
}

/// The argument of `openSimulatorSession`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenArgument {
    workspace: String,
    name: String,
    target: Value,
}

/// The argument of `closeSimulatorSession`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SessionArgument {
    session_id: String,
}

/// The argument of `cancelSimulatorJourney`: the session and the journey it cancels.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CancelArgument {
    session_id: String,
    journey_id: String,
}

/// The argument of `admitSimulatorLogin`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct AdmitArgument {
    session_id: String,
    target: Value,
}

/// The argument of `simulatorSessions`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceArgument {
    workspace: String,
}

/// The argument of `runSimulatorJourney`, without its cookie.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RunArgument {
    workspace: String,
    session_id: String,
    journey_id: String,
    target: Value,
    steps: Vec<Step>,
    models: Map<String, Value>,
}

#[cfg(test)]
#[path = "simulator_commands_tests.rs"]
mod tests;
