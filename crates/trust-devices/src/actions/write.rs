//! The one write path of the panel's connection actions (design §4.3 B7; specification 25 §5.9 and
//! §5.14; decision O16): resolve the runtime that a payload names from what the session last
//! showed, and write one `comm.apply` to it — in process with
//! [`offline_comm_apply_json`](trust_runtime::control::offline_comm_apply_json) for the open
//! project's own files, or over the runtime's control endpoint when it answers.
//!
//! One place decides which runtime gets a write and when a write is refused, so that `config`'s form
//! actions and `plans`' connection plans cannot drift apart (design §4.3 B7, R2): a runtime the
//! session does not know, and one that answers nowhere while it is not the open project's own
//! runtime, is a refusal, and nothing is written.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use trust_runtime::control::offline_comm_apply_json;
use trust_runtime_client::{ControlClient, ControlEndpoint, ControlError, RESPONSE_LINE_LIMIT};

use crate::request::{ActRequest, Token};
use crate::session::{KnownRuntime, Session};
use crate::texts::forms as texts;

/// The wait for one `comm.apply` on a runtime (design §1.5: 2,000 ms for each further request).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Where one `comm.apply` is applied.
pub(crate) enum Writes {
    /// The runtime's control endpoint, which answered.
    Endpoint(String),
    /// The open project's files, in process.
    Project,
}

/// How a runtime is written to (design §4.3 B7; specification 25 §5.14): its control endpoint when
/// it answered, else the open project's files when it is the project's own runtime. `None` when it
/// answers nowhere and owns no files.
#[must_use]
pub(crate) fn writes(runtime: &KnownRuntime) -> Option<Writes> {
    let endpoint = runtime
        .control_endpoint
        .as_deref()
        .map(str::trim)
        .filter(|endpoint| !endpoint.is_empty());
    if runtime.answers
        && let Some(endpoint) = endpoint
    {
        return Some(Writes::Endpoint(endpoint.to_owned()));
    }
    runtime.project.then_some(Writes::Project)
}

/// The runtime that an action writes to, resolved from what the session last showed, and how it is
/// reached.
pub(crate) struct Target<'a> {
    /// The runtime, as the last view showed it.
    pub(crate) runtime: &'a KnownRuntime,
    /// Where its change is applied.
    pub(crate) writes: Writes,
}

impl Target<'_> {
    /// Whether the runtime answered this round, so that `Test` and `Apply to running runtime` can
    /// act on it (specification 25 §5.13).
    #[must_use]
    pub(crate) fn reachable(&self) -> bool {
        matches!(self.writes, Writes::Endpoint(_))
    }
}

/// The target that the payload names, resolved from what the session last showed: the project's own
/// runtime over its control endpoint when it answers, else the open project's files in process; any
/// other runtime over its control endpoint when it answers, else a refusal that names it. An ID the
/// session does not know, or a session that has shown no view yet, is a refusal (design §4.3 B7;
/// specification 25 §5.14).
///
/// # Errors
///
/// The payload named no runtime, the session has not shown it, or it answered nowhere while it is
/// not the open project's own runtime.
pub(crate) fn target<'a>(session: &'a Session, request: &ActRequest) -> Result<Target<'a>, String> {
    let Some(id) = super::payload_text(&request.op.payload, "runtime") else {
        return Err(texts::NO_RUNTIME.to_owned());
    };
    let Some(runtime) = session.runtime(&id) else {
        return Err(texts::runtime_not_shown(&id));
    };
    let Some(writes) = writes(runtime) else {
        return Err(texts::runtime_did_not_answer(&runtime.name));
    };
    Ok(Target { runtime, writes })
}

/// Writes one `comm.apply` at `writes`: the runtime's control endpoint, or the open project's files
/// (design §4.3 B7; specification 25 §5.9 and §5.14).
///
/// # Errors
///
/// The runtime refused the change, or it could not be reached.
pub(crate) fn apply(
    request: &ActRequest,
    writes: &Writes,
    params: &Value,
) -> Result<Value, String> {
    match writes {
        Writes::Endpoint(endpoint) => send_control(request, endpoint, "comm.apply", params),
        Writes::Project => send_to_project(request.project.as_deref(), params),
    }
}

/// Sends one control request `entry` (`comm.apply` or `comm.test`) to `endpoint`, with the token the
/// extension stored for it. Secrets on a plain channel are refused by the runtime itself, which
/// stamps the channel (decision O16).
///
/// # Errors
///
/// `endpoint` is not a control endpoint, the client cannot connect or times out, or the runtime
/// refused the request.
pub(crate) fn send_control(
    request: &ActRequest,
    endpoint: &str,
    entry: &str,
    params: &Value,
) -> Result<Value, String> {
    let Some(address) = ControlEndpoint::parse(endpoint) else {
        return Err(format!("{endpoint} is not a control endpoint."));
    };
    let token = request.host.token(endpoint).map(Token::secret);
    let mut client = ControlClient::connect(&address, token, REQUEST_TIMEOUT)
        .map_err(|error| error.to_string())?
        .with_response_limit(RESPONSE_LINE_LIMIT);
    client
        .set_timeout(REQUEST_TIMEOUT)
        .map_err(|error| error.to_string())?;
    client
        .request(entry, Some(params.clone()))
        .map_err(|error| match error {
            ControlError::Refused { error, .. } => error,
            other => other.to_string(),
        })
}

/// Applies `params` to the open project's files, where no runtime is needed.
fn send_to_project(project: Option<&Path>, params: &Value) -> Result<Value, String> {
    let Some(project) = project else {
        return Err(texts::NO_PROJECT_FOLDER.to_owned());
    };
    offline_comm_apply_json(project, params.clone())
}
