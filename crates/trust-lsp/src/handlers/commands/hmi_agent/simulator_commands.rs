//! The simulator session and journey commands of the HMI agent (specification 43, AT-08):
//! `trust-lsp.hmiAgent.openSimulatorSession`, `closeSimulatorSession`, `simulatorSessions`,
//! `runSimulatorJourney` and `cancelSimulatorJourney`.
//!
//! Each takes one argument object and answers the authoring envelope, `{ ok: true, data, revisions,
//! warnings }` or `{ ok: false, code, message, retryable }`. This module decodes the arguments; the
//! rules are the sessions' (`simulator_sessions`) and the journey's (`simulator_journey`). The
//! extension keeps the named login and the session's cookie (specification 39 §11 item 6) and passes
//! the cookie with each journey, which holds it only while it runs. No answer, refusal or log line
//! names the cookie or the target's access token.

use std::collections::BTreeMap;
use std::time::SystemTime;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::boundary::{self, Success};
use super::error::{AgentError, AgentResult};
use super::schema::MAX_PREVIEW;
use super::simulator_journey::{self, Journey, Step, Timing};
use super::simulator_sessions::SimulatorSessions;

/// Opens a named session for a workspace, a journey's name and a target.
pub(super) const OPEN_SESSION: &str = "trust-lsp.hmiAgent.openSimulatorSession";
/// Closes a session, as after a login the simulator refused.
pub(super) const CLOSE_SESSION: &str = "trust-lsp.hmiAgent.closeSimulatorSession";
/// Lists a workspace's unexpired sessions, for the agent's context.
pub(super) const LIST_SESSIONS: &str = "trust-lsp.hmiAgent.simulatorSessions";
/// Runs a journey with its session's cookie.
pub(super) const RUN_JOURNEY: &str = "trust-lsp.hmiAgent.runSimulatorJourney";
/// Cancels a session's running journey.
pub(super) const CANCEL_JOURNEY: &str = "trust-lsp.hmiAgent.cancelSimulatorJourney";

/// The simulator commands.
const COMMANDS: [&str; 5] = [
    OPEN_SESSION,
    CLOSE_SESSION,
    LIST_SESSIONS,
    RUN_JOURNEY,
    CANCEL_JOURNEY,
];

/// The named simulator sessions of this language server.
static SERVER_SESSIONS: SimulatorSessions = SimulatorSessions::new();

/// Whether `name` is a simulator command.
#[must_use]
pub(super) fn is_command(name: &str) -> bool {
    COMMANDS.contains(&name)
}

/// Runs the simulator command `name` on its argument.
#[must_use]
pub(super) fn execute(name: &str, arguments: Vec<Value>) -> Value {
    answer(
        &SERVER_SESSIONS,
        name,
        arguments,
        &SystemTime::now,
        Timing::PRODUCT,
    )
}

/// The envelope answer of one command on `sessions`, at the times `clock` gives.
fn answer(
    sessions: &SimulatorSessions,
    name: &str,
    arguments: Vec<Value>,
    clock: &dyn Fn() -> SystemTime,
    timing: Timing,
) -> Value {
    let result = one_argument(arguments).and_then(|argument| match name {
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
        RUN_JOURNEY => run(sessions, argument, clock, timing),
        CANCEL_JOURNEY => {
            let request: SessionArgument = decode(argument)?;
            Ok(json!({ "cancelled": sessions.cancel(&request.session_id) }))
        }
        _ => Err(AgentError::invalid("unknown simulator command.")),
    });
    match result {
        Ok(data) => boundary::success_envelope(&Success {
            data,
            revisions: BTreeMap::new(),
            warnings: Vec::new(),
        }),
        Err(error) => boundary::failure_envelope(&error),
    }
}

/// `runSimulatorJourney`: the journey on its session, with that session's cookie.
fn run(
    sessions: &SimulatorSessions,
    mut argument: Value,
    clock: &dyn Fn() -> SystemTime,
    timing: Timing,
) -> AgentResult<Value> {
    // The cookie leaves the argument first, so that no refusal of the other fields can quote it.
    let cookie = argument
        .as_object_mut()
        .and_then(|fields| fields.remove("cookie"));
    let request: RunArgument = decode(argument)?;
    required("workspace", &request.workspace)?;
    required("sessionId", &request.session_id)?;
    if request.steps.is_empty() || request.steps.len() > MAX_PREVIEW {
        return Err(AgentError::limit(format!(
            "use 1 to {MAX_PREVIEW} bounded simulator steps."
        )));
    }
    let running = sessions.begin(
        &request.workspace,
        &request.session_id,
        &request.target,
        clock(),
    )?;
    let cookie = named_cookie(cookie.as_ref())?;
    let journey = Journey {
        session_id: &request.session_id,
        name: running.name(),
        target: &request.target,
        cookie,
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
    Ok(answer)
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

/// The argument of `closeSimulatorSession` and `cancelSimulatorJourney`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SessionArgument {
    session_id: String,
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
    target: Value,
    steps: Vec<Step>,
    models: Map<String, Value>,
}

#[cfg(test)]
#[path = "simulator_commands_tests.rs"]
mod tests;
