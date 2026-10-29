//! The run targets of the extension and its requests to runtimes (specification 25 §4, §5.1,
//! §6.4 and §6.5; specification 35 A2). The extension sends what it holds (its settings, the debug
//! session's configuration, the lifecycle snapshot, the user's choice) and shows what these
//! commands answer; every rule about a target is here.
//!
//! - `trust-lsp.runtimeRequest` `{ action: "request" | "probe", endpoint, … }`: one control request
//!   to a runtime, its answer decoded, or whether the runtime's control endpoint accepts a
//!   connection (runtime protocol annex E0.1).
//! - `trust-lsp.runTargets` `{ action, … }`: the status of the configured target (§5.1), the
//!   sidebar's run targets and the selected one's action (§5.1, §6.1), what selecting a target
//!   does to the runtime settings and what Stop does without a session (§5.1), the managed
//!   runtimes of a fleet root (§6.5), the status
//!   that Live Values shows with the operations the session may use (§4.1, §4.2), the speed that a
//!   simulated runtime reports (specification 35 B9.2), the outcome of a Connect (§5.1, §6.4) and
//!   the kind of a lifecycle failure (§5.10).
//!
//! Each answer is an object with `ok`; a refused request answers `ok: false` and an `error` text.

use serde::de::DeserializeOwned;
use serde_json::{json, Value};

mod connect;
mod endpoint;
mod live_values;
mod managed;
mod request;
mod selection;
mod target;

/// One control request to a runtime, or a probe of its endpoint.
pub const RUNTIME_REQUEST_COMMAND: &str = "trust-lsp.runtimeRequest";
/// The run targets and their rules.
pub const RUN_TARGETS_COMMAND: &str = "trust-lsp.runTargets";

/// The run-target commands, in the order the server advertises them. Each name is one camel-case
/// segment after `trust-lsp.`, so it never collides with an extension command.
pub const RUN_TARGET_COMMANDS: [&str; 2] = [RUNTIME_REQUEST_COMMAND, RUN_TARGETS_COMMAND];

/// Whether `command` is a run-target command.
pub(super) fn is_command(command: &str) -> bool {
    RUN_TARGET_COMMANDS.contains(&command)
}

/// The answer of the run-target command `command` to `arguments` (one object).
pub(super) fn execute(command: &str, arguments: Vec<Value>) -> Value {
    let Ok([argument]) = <[Value; 1]>::try_from(arguments) else {
        return refused(&format!("{command} expects one argument object."));
    };
    if !argument.is_object() {
        return refused(&format!("{command} expects one argument object."));
    }
    let answer = match command {
        RUNTIME_REQUEST_COMMAND => request::answer(argument),
        RUN_TARGETS_COMMAND => run_targets(argument),
        _ => Err(format!("{command} is not a run-target command.")),
    };
    answer.unwrap_or_else(|message| refused(&message))
}

/// `trust-lsp.runTargets` by its `action`.
fn run_targets(argument: Value) -> Result<Value, String> {
    let action = argument
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    match action.as_str() {
        "target" => target::answer(argument),
        "selection" => selection::answer(argument),
        "select" => selection::select(argument),
        "stop" => selection::stop(argument),
        "managed" | "managedOutcome" | "managedLogs" | "managedStopEndsSession" => {
            managed::answer(&action, argument)
        }
        "liveValues" | "speed" => live_values::answer(&action, argument),
        "connect" | "failure" => connect::answer(&action, argument),
        "" => Err("trust-lsp.runTargets needs an action.".to_owned()),
        other => Err(format!("trust-lsp.runTargets has no action {other}.")),
    }
}

/// The arguments of a command, decoded into `T`.
fn decode<T: DeserializeOwned>(argument: Value) -> Result<T, String> {
    serde_json::from_value(argument).map_err(|error| format!("Invalid arguments: {error}."))
}

/// A refused request.
fn refused(message: &str) -> Value {
    json!({ "ok": false, "error": message })
}

/// `value` trimmed, `None` when nothing is left.
fn non_blank(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod fake_runtime;
#[cfg(test)]
mod tests;
