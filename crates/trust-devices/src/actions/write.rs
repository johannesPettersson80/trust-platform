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
use crate::view::address;

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
/// extension stored for it. A secret that the request would carry over plain TCP to another
/// computer is never sent: the answer is the refusal of [`secret_refusal`] in the runtime's own
/// blocked shape (specification 25 §5.9). On a channel of this computer the runtime itself refuses
/// a secret it does not trust, as it stamps the channel (specification 11 §6.9.7).
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
    if matches!(entry, "comm.apply" | "comm.test")
        && let Some(field) = secret_refusal(endpoint, params.get("params").unwrap_or(&Value::Null))
    {
        return Ok(secret_blocked_answer(params, &field));
    }
    let token = request.host.token(endpoint).map(Token::secret);
    send_with_token(endpoint, token, entry, params, REQUEST_TIMEOUT)
}

/// Sends one control request `entry` to `endpoint` with `token`, waiting `timeout` for the answer.
///
/// # Errors
///
/// `endpoint` is not a control endpoint, the client cannot connect or times out, or the runtime
/// refused the request; the error is the runtime's text, or the client's.
pub(crate) fn send_with_token(
    endpoint: &str,
    token: Option<&str>,
    entry: &str,
    params: &Value,
    timeout: Duration,
) -> Result<Value, String> {
    let Some(address) = ControlEndpoint::parse(endpoint.trim()) else {
        return Err(format!("{endpoint} is not a control endpoint."));
    };
    let mut client = ControlClient::connect(&address, token, timeout)
        .map_err(|error| error.to_string())?
        .with_response_limit(RESPONSE_LINE_LIMIT);
    client
        .set_timeout(timeout)
        .map_err(|error| error.to_string())?;
    client
        .request(entry, Some(params.clone()))
        .map_err(|error| match error {
            ControlError::Refused { error, .. } => error,
            other => other.to_string(),
        })
}

/// The secret field that `params` would carry over plain TCP to another computer, when it carries
/// one (specification 25 §5.9): `endpoint` is a `tcp://` endpoint whose host is not this computer,
/// and a value under a secret key (specification 11 §6.9.7) is a text that is not blank, at any
/// depth. A Unix socket, a serial line and a loopback address stay on this computer.
#[must_use]
pub(crate) fn secret_refusal(endpoint: &str, params: &Value) -> Option<String> {
    let endpoint = endpoint.trim();
    let remote = endpoint.starts_with("tcp://")
        && address::endpoint_host(endpoint).is_some_and(|host| !address::is_loopback(&host));
    if remote {
        secret_field(params)
    } else {
        None
    }
}

/// The first key that holds a secret text that is not blank, at any depth of `params`.
fn secret_field(params: &Value) -> Option<String> {
    match params {
        Value::Object(values) => values.iter().find_map(|(key, value)| {
            let secret = trust_runtime::security::is_secret_param_key(key.trim())
                && value.as_str().is_some_and(|text| !text.trim().is_empty());
            if secret {
                Some(key.clone())
            } else {
                secret_field(value)
            }
        }),
        Value::Array(values) => values.iter().find_map(secret_field),
        _ => None,
    }
}

/// The blocked `comm.apply` or `comm.test` answer of a secret that was not sent, in the shape the
/// runtime answers its own refusal: nothing applied, and the secret's field refused with the reason.
pub(crate) fn secret_blocked_answer(request: &Value, field: &str) -> Value {
    serde_json::json!({
        "schema_version": 1,
        "protocol": request.get("protocol").cloned().unwrap_or(Value::Null),
        "driver": "",
        "action": request.get("action").cloned().unwrap_or_else(|| Value::from("upsert")),
        "applied": false,
        "ok": false,
        "lifecycle_effect": "blocked",
        "message": texts::SECRET_NOT_SENT,
        "field_errors": [{ "field": field, "message": texts::SECRET_FIELD_NOT_SENT }],
    })
}

/// Applies `params` to the open project's files, where no runtime is needed.
fn send_to_project(project: Option<&Path>, params: &Value) -> Result<Value, String> {
    let Some(project) = project else {
        return Err(texts::NO_PROJECT_FOLDER.to_owned());
    };
    offline_comm_apply_json(project, params.clone())
}
