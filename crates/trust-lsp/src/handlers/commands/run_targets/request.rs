//! One control request to a runtime, and whether a runtime's control endpoint accepts a connection
//! (runtime protocol annex E0.1). Every request of the extension to a runtime comes here: the
//! endpoint is read, the connection made and the answer decoded in one place.
//!
//! - `request` `{ endpoint, authToken?, type, params?, timeoutMs? }`: the answer is the outcome
//!   `result` with the runtime's `result`; `refused` with its `error`, its code (`error_code`), the
//!   credential failure the code names (`missing` or `rejected`, specification 11 §6.9.6; the text
//!   is never read) and the `result` a refusal may carry, such as a plant's revision conflict;
//!   `transport` when the endpoint is not a control endpoint, nothing accepts the connection or the
//!   connection breaks; and `malformed` when the answer is not a control response. Each read and
//!   write waits at most `timeoutMs`, 2000 ms when absent.
//! - `probe` `{ endpoint, timeoutMs?, waitMs? }`: `reachable` when the endpoint is a control
//!   endpoint and a connection is accepted within `timeoutMs`, 400 ms when absent. Nothing is sent.
//!   With `waitMs` the endpoint is tried again every 250 ms until it accepts or the wait ends, as
//!   right after a managed Start, whose runtime binds its endpoint a moment after its process is up.

use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime_client::{AuthFailure, ControlClient, ControlEndpoint, ControlError};

use super::decode;

/// The wait of a request when the extension names none.
const REQUEST_TIMEOUT: Duration = Duration::from_millis(2000);
/// The wait of a probe when the extension names none.
const PROBE_TIMEOUT: Duration = Duration::from_millis(400);
/// The pause between the tries of a waiting probe.
const PROBE_RETRY: Duration = Duration::from_millis(250);
/// The text of a refusal that names none.
const REFUSED_WITHOUT_TEXT: &str = "control request rejected";

#[derive(Deserialize)]
#[serde(
    tag = "action",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Args {
    Request {
        endpoint: String,
        #[serde(default)]
        auth_token: Option<String>,
        #[serde(rename = "type")]
        kind: String,
        #[serde(default)]
        params: Option<Value>,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },
    Probe {
        endpoint: String,
        #[serde(default)]
        timeout_ms: Option<u64>,
        #[serde(default)]
        wait_ms: Option<u64>,
    },
}

/// `request`: `{ ok: true, outcome, … }` as the module says. `probe`: `{ ok: true, reachable }`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    Ok(match decode(argument)? {
        Args::Request {
            endpoint,
            auth_token,
            kind,
            params,
            timeout_ms,
        } => {
            let timeout = timeout_ms.map_or(REQUEST_TIMEOUT, Duration::from_millis);
            outcome_json(&send(
                &endpoint,
                auth_token.as_deref(),
                &kind,
                params,
                timeout,
            ))
        }
        Args::Probe {
            endpoint,
            timeout_ms,
            wait_ms,
        } => {
            let timeout = timeout_ms.map_or(PROBE_TIMEOUT, Duration::from_millis);
            let deadline = Instant::now() + Duration::from_millis(wait_ms.unwrap_or(0));
            let mut answer = reachable(&endpoint, timeout);
            while !answer && Instant::now() < deadline {
                std::thread::sleep(PROBE_RETRY);
                answer = reachable(&endpoint, timeout);
            }
            super::live_values::remember_probe(endpoint.trim(), answer);
            json!({ "ok": true, "reachable": answer })
        }
    })
}

/// What one control request came to.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Outcome {
    /// The runtime answered with this result.
    Result(Value),
    /// The runtime refused the request.
    Refused {
        /// Its text, naming the failed check.
        error: String,
        /// Its code, when it gave one.
        code: Option<String>,
        /// The credential failure that the code names.
        auth: Option<AuthFailure>,
        /// The result a refusal carries, when it carries one.
        result: Option<Value>,
    },
    /// The request produced no answer of the runtime: the endpoint is not a control endpoint,
    /// nothing accepted the connection, or the connection broke or timed out.
    Transport(String),
    /// The answer is not a control response.
    Malformed(String),
}

/// Sends the request `kind` with `params` to `endpoint` with the token `auth` (a blank token is
/// never sent), waiting at most `timeout` for each read and write.
pub(super) fn send(
    endpoint: &str,
    auth: Option<&str>,
    kind: &str,
    params: Option<Value>,
    timeout: Duration,
) -> Outcome {
    let Some(parsed) = ControlEndpoint::parse(endpoint.trim()) else {
        return Outcome::Transport(format!("invalid control endpoint '{endpoint}'"));
    };
    let mut client = match ControlClient::connect(&parsed, auth, timeout) {
        Ok(client) => client,
        Err(error) => return failed(error),
    };
    match client.exchange(kind, params) {
        Ok(response) => decoded(response),
        Err(error) => failed(error),
    }
}

/// Whether `endpoint` is a control endpoint that accepts a connection within `timeout`.
pub(super) fn reachable(endpoint: &str, timeout: Duration) -> bool {
    ControlEndpoint::parse(endpoint.trim())
        .is_some_and(|parsed| ControlClient::connect(&parsed, None, timeout).is_ok())
}

/// The outcome of the response object `response`: its result when `ok` is true, else its refusal
/// with the code of `error_code` (or of `code`).
fn decoded(response: Value) -> Outcome {
    if response.get("ok").and_then(Value::as_bool) == Some(true) {
        return Outcome::Result(response.get("result").cloned().unwrap_or(Value::Null));
    }
    let text = |key: &str| super::non_blank(response.get(key).and_then(Value::as_str));
    let error = text("error").unwrap_or_else(|| REFUSED_WITHOUT_TEXT.to_owned());
    let code = text("error_code").or_else(|| text("code"));
    let auth = ControlError::Refused {
        error: error.clone(),
        code: code.clone(),
    }
    .auth_failure();
    Outcome::Refused {
        error,
        code,
        auth,
        result: response
            .get("result")
            .cloned()
            .filter(|value| !value.is_null()),
    }
}

/// The outcome of a request that has no response object.
fn failed(error: ControlError) -> Outcome {
    let auth = error.auth_failure();
    match error {
        ControlError::Transport(text) => Outcome::Transport(text),
        ControlError::Malformed(text) => Outcome::Malformed(text),
        ControlError::Refused { error, code } => Outcome::Refused {
            error,
            code,
            auth,
            result: None,
        },
    }
}

/// The JSON of `outcome`, as the extension's client reads it.
pub(super) fn outcome_json(outcome: &Outcome) -> Value {
    match outcome {
        Outcome::Result(result) => json!({ "ok": true, "outcome": "result", "result": result }),
        Outcome::Refused {
            error,
            code,
            auth,
            result,
        } => json!({
            "ok": true,
            "outcome": "refused",
            "error": error,
            "code": code,
            "authFailure": auth.map(auth_json),
            "result": result,
        }),
        Outcome::Transport(error) => json!({ "ok": true, "outcome": "transport", "error": error }),
        Outcome::Malformed(error) => json!({ "ok": true, "outcome": "malformed", "error": error }),
    }
}

/// The name of a credential failure: `missing` or `rejected`.
pub(super) fn auth_json(auth: AuthFailure) -> &'static str {
    match auth {
        AuthFailure::Missing => "missing",
        AuthFailure::Rejected => "rejected",
    }
}

#[cfg(test)]
#[path = "request_tests.rs"]
mod tests;
