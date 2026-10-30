//! A Connect, and the kind of a lifecycle failure (specification 25 §5.1, §5.10 and §6.4).
//!
//! - `connect` `{ endpoint, authToken?, problem?, targetLabel? }`: Connect changes nothing until it
//!   succeeds. The runtime must accept a connection at its endpoint and answer `status` with the
//!   endpoint's token; then the extension attaches, or, when the runtime keeps debugging disabled,
//!   selects it without a session. The answer is `{ ok: true, outcome: "attach", sessionName }`,
//!   `{ ok: true, outcome: "debugDisabled", failure }` or `{ ok: true, outcome: "failed", failure }`,
//!   a failure naming its kind, its message, the endpoint it belongs to and, when the extension
//!   offers one, its `remedy`: `openDevices` when the runtime accepts no connection or its `status`
//!   request gets no answer, `setAuthToken` when it refuses a missing or rejected token by its code.
//!   `problem` is trust-runtime's reason why the workspace's own runtime has no endpoint now
//!   (specification 11 §6.8.0a).
//! - `failure` `{ detail }`: the kind of the failure that the text `detail` of a start, a stop or an
//!   attach names, with the message the extension shows: a missing executable, a port in use, a
//!   folder that cannot be written, a stale runtime or session, else a failed start.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Map, Value};
use trust_runtime_client::AuthFailure;

use super::endpoint::sentence_label;
use super::request::{reachable, send, Outcome};
use super::{decode, non_blank};

/// The wait of Connect's `status` request.
const STATUS_TIMEOUT: Duration = Duration::from_millis(1000);
/// The wait of Connect's probe.
const PROBE_TIMEOUT: Duration = Duration::from_millis(400);
/// The remedy of a runtime that does not answer: Devices & Connections, to start or diagnose it.
const OPEN_DEVICES: &str = "openDevices";
/// The remedy of a runtime that refuses the token: a token stored for its endpoint.
const SET_AUTH_TOKEN: &str = "setAuthToken";
/// What Live Values and the graph say of a runtime that keeps debugging disabled.
pub(super) const DEBUG_DISABLED: &str = "Remote debugging is disabled for this runtime. Set debug_enabled = true under [runtime.control] in its runtime.toml, restart the runtime, then connect again.";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectArgs {
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    auth_token: Option<String>,
    #[serde(default)]
    problem: Option<String>,
    #[serde(default)]
    target_label: Option<String>,
}

#[derive(Deserialize)]
struct FailureArgs {
    #[serde(default)]
    detail: Option<String>,
}

/// The answer of the action `action`.
pub(super) fn answer(action: &str, argument: Value) -> Result<Value, String> {
    if action == "failure" {
        let args: FailureArgs = decode(argument)?;
        let detail = args.detail.unwrap_or_default();
        return Ok(json!({ "ok": true, "failure": classify(&detail) }));
    }
    let args: ConnectArgs = decode(argument)?;
    Ok(connect(&args, &Network))
}

/// The runtime a Connect asks: the network, or a test's stand-in.
pub(super) trait Contact {
    /// Whether `endpoint` accepts a connection.
    fn reachable(&self, endpoint: &str) -> bool;
    /// The `status` request at `endpoint` with `token`.
    fn status(&self, endpoint: &str, token: Option<&str>) -> Outcome;
}

struct Network;

impl Contact for Network {
    fn reachable(&self, endpoint: &str) -> bool {
        let answer = reachable(endpoint, PROBE_TIMEOUT);
        super::live_values::remember_probe(endpoint, answer);
        answer
    }

    fn status(&self, endpoint: &str, token: Option<&str>) -> Outcome {
        send(endpoint, token, "status", None, STATUS_TIMEOUT)
    }
}

fn connect(args: &ConnectArgs, contact: &dyn Contact) -> Value {
    let endpoint = args
        .endpoint
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_owned();
    if endpoint.is_empty() {
        // The workspace's runtime that trust-runtime reports as not running is stale, not unset.
        let failure = match non_blank(args.problem.as_deref()) {
            Some(problem) => failure_json("stale_runtime", &problem, None, None),
            None => failure_json("failed_spawn", "Runtime endpoint not set.", None, None),
        };
        return json!({ "ok": true, "outcome": "failed", "failure": failure });
    }
    let failed = |kind: &str, message: String, detail: Option<&str>, remedy: Option<&str>| {
        let mut failure = failure_json(kind, &message, detail, Some(&endpoint));
        if let Some(remedy) = remedy {
            failure["remedy"] = json!(remedy);
        }
        json!({ "ok": true, "outcome": "failed", "failure": failure })
    };
    if !contact.reachable(&endpoint) {
        return failed(
            "stale_runtime",
            not_reachable(&endpoint),
            Some(&endpoint),
            Some(OPEN_DEVICES),
        );
    }
    let token = non_blank(args.auth_token.as_deref());
    let status = match contact.status(&endpoint, token.as_deref()) {
        Outcome::Result(status) => status,
        Outcome::Refused {
            auth: Some(auth), ..
        } => {
            let message = if auth == AuthFailure::Missing || token.is_none() {
                "No auth token provided — this runtime requires one."
            } else {
                "Auth token rejected — check it and try again."
            };
            return failed(
                "workspace_permission",
                message.to_owned(),
                None,
                Some(SET_AUTH_TOKEN),
            );
        }
        Outcome::Refused { error, code, .. } => {
            let text = code.map_or(error.clone(), |code| format!("{code}: {error}"));
            return failed(
                "stale_runtime",
                format!("Runtime status check failed: {text}"),
                None,
                None,
            );
        }
        // A runtime that does not answer is not reachable; one whose answer does not read did answer.
        Outcome::Transport(error) => {
            return failed(
                "stale_runtime",
                format!("Runtime status check failed: {error}"),
                None,
                Some(OPEN_DEVICES),
            );
        }
        Outcome::Malformed(error) => {
            return failed(
                "stale_runtime",
                format!("Runtime status check failed: {error}"),
                None,
                None,
            );
        }
    };
    if debug_disabled(&status) {
        // The runtime answered, so it is the target; Live Values says why it has no session.
        return json!({
            "ok": true,
            "outcome": "debugDisabled",
            "failure": failure_json("debug_disabled", DEBUG_DISABLED, None, Some(&endpoint)),
        });
    }
    let label =
        non_blank(args.target_label.as_deref()).unwrap_or_else(|| sentence_label(&endpoint));
    json!({
        "ok": true,
        "outcome": "attach",
        "sessionName": format!("truST Remote ({label})"),
    })
}

/// A failure: its kind and message, and the text it was read from and the endpoint it belongs to
/// when there are such.
fn failure_json(kind: &str, message: &str, detail: Option<&str>, endpoint: Option<&str>) -> Value {
    let mut failure = Map::new();
    failure.insert("kind".into(), json!(kind));
    failure.insert("message".into(), json!(message));
    if let Some(detail) = detail {
        failure.insert("detail".into(), json!(detail));
    }
    if let Some(endpoint) = endpoint {
        failure.insert("endpoint".into(), json!(endpoint));
    }
    Value::Object(failure)
}

/// Whether the `status` answer says that the runtime keeps debugging disabled: `debug_enabled`
/// false at its top or in its `control_status`.
fn debug_disabled(status: &Value) -> bool {
    status.get("debug_enabled").and_then(Value::as_bool) == Some(false)
        || status
            .get("control_status")
            .and_then(|control| control.get("debug_enabled"))
            .and_then(Value::as_bool)
            == Some(false)
}

/// The message of a runtime that accepts no connection: a stopped local runtime for a Unix socket,
/// else the runtime not reachable at its address.
fn not_reachable(endpoint: &str) -> String {
    if endpoint.trim().starts_with("unix://") {
        return "Local runtime is stopped. Start it to connect.".to_owned();
    }
    format!("Runtime is not reachable at {}.", sentence_label(endpoint))
}

/// The kind of the failure that `detail` names, with the extension's message for it.
fn classify(detail: &str) -> Value {
    let lower = detail.to_lowercase();
    let has = |needle: &str| lower.contains(needle);
    let (kind, message) = if has("enoent")
        || has("not found")
        || has("trust-runtime")
        || has("trust-debug")
        || has("binary")
    {
        (
            "missing_binary",
            "Required runtime/debug binary was not found.".to_owned(),
        )
    } else if has("eaddrinuse") || has("address already in use") || (has("port") && has("in use")) {
        (
            "port_conflict",
            "The runtime port is already in use.".to_owned(),
        )
    } else if has("eacces") || has("permission") || has("read-only") || has("workspace") {
        (
            "workspace_permission",
            "The workspace or runtime path is not writable.".to_owned(),
        )
    } else if has("timeout")
        || has("timed out")
        || has("stale")
        || has("zombie")
        || has("already running")
    {
        (
            "stale_runtime",
            "A stale runtime or debug session blocked startup.".to_owned(),
        )
    } else if detail.is_empty() {
        ("failed_spawn", "Runtime failed to start.".to_owned())
    } else {
        ("failed_spawn", detail.to_owned())
    };
    failure_json(kind, &message, Some(detail), None)
}

#[cfg(test)]
#[path = "connect_tests.rs"]
mod tests;
