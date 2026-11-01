//! The run target that the HMI Builder's Live mode and Deploy talk to (specification 39 §11 item 6
//! and §16.2 item 8): the Simulator or runtime whose debug session the extension holds, launched or
//! attached, or the configured online runtime while it answers without one. Without a control
//! endpoint there is none.
//!
//! `hmiTarget` `{ snapshot }` answers `{ ok: true, target: { endpoint, label, simulator } }`, or
//! `target: null` when nothing runs or answers. The target has the name the status bar gives it
//! (specification 25 §6.9): the name the extension gave it, else `Simulator` in simulate mode, else
//! the status bar's name for the runtime.

use serde::Deserialize;
use serde_json::{json, Value};

use super::decode;
use super::selection::{given_name, runtime_name, Snapshot};

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    snapshot: Snapshot,
}

/// `{ ok: true, target }` for the lifecycle snapshot in `argument`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    let args: Args = decode(argument)?;
    let snapshot = &args.snapshot;
    let endpoint = snapshot.endpoint.trim();
    // A launched session is `running`, an attached one `connected`.
    let session = matches!(snapshot.runtime_state.as_str(), "running" | "connected");
    // An online runtime the editor does not debug is reported stopped, but it answers.
    let online = snapshot.runtime_mode == "online" && snapshot.endpoint_reachable;
    if endpoint.is_empty() || !(session || online) {
        return Ok(json!({ "ok": true, "target": null }));
    }
    let simulator = snapshot.runtime_mode == "simulate";
    // The project's own runtime has that name while it answers, as its entry in the target list.
    let label = if simulator && given_name(snapshot).is_none() {
        "Simulator".to_owned()
    } else {
        runtime_name(snapshot, snapshot.project_reachable)
    };
    Ok(json!({
        "ok": true,
        "target": { "endpoint": endpoint, "label": label, "simulator": simulator },
    }))
}

#[cfg(test)]
#[path = "hmi_target_tests.rs"]
mod tests;
