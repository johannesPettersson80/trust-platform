//! The runtime that a debug session attached to (specification 25 §6.8): the extension names the
//! session's workspace folder and the endpoint it attached to, and the folder with `endpoint:
//! null` when the session ends. The server keeps it for the inline values of the folder's
//! documents ([`crate::state::simulator_controls::attached_runtimes`]). No token crosses: the server
//! finds the token of the folder's own runtime itself (specification 14 §7.1.2).

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime::control::ControlEndpoint;

use super::decode;
use crate::state::ServerState;

#[derive(Deserialize)]
struct Args {
    /// The session's workspace folder, `None` for a session without one.
    #[serde(default)]
    folder: Option<PathBuf>,
    /// The endpoint the session attached to, `None` when it ended.
    #[serde(default)]
    endpoint: Option<String>,
}

/// `{ ok: true }` once the endpoint that a session of `folder` attached to, or the session's end,
/// is kept. An endpoint that is not a control endpoint is refused.
pub(super) fn answer(state: &ServerState, argument: Value) -> Result<Value, String> {
    let Args { folder, endpoint } = decode(argument)?;
    let endpoint = endpoint
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    if let Some(text) = &endpoint {
        ControlEndpoint::parse(text)
            .map_err(|error| format!("{text} is not a control endpoint: {error}"))?;
    }
    state
        .simulator_controls()
        .attached()
        .record(folder, endpoint);
    Ok(json!({ "ok": true }))
}

#[cfg(test)]
#[path = "attached_runtime_tests.rs"]
mod tests;
