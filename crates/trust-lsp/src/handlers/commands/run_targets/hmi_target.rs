//! The run target that the HMI Builder's Live mode and Deploy talk to (specification 39 §11 item 6
//! and §16.2 item 8): the Simulator or runtime whose debug session the extension holds, launched or
//! attached, or the configured online runtime while it answers without one. Without a control
//! endpoint there is none.
//!
//! `hmiTarget` `{ snapshot, targetLabel? }` answers `{ ok: true, target: { endpoint, label,
//! simulator } }`, or `target: null` when nothing runs or answers. The target is named by its
//! session's `targetLabel`, else `Simulator` in simulate mode, else its endpoint.

use serde::Deserialize;
use serde_json::{json, Value};

use super::{decode, non_blank, selection::Snapshot};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Args {
    #[serde(default)]
    snapshot: Snapshot,
    #[serde(default)]
    target_label: Option<String>,
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
    let label = non_blank(args.target_label.as_deref()).unwrap_or_else(|| {
        if simulator {
            "Simulator".to_owned()
        } else {
            endpoint.to_owned()
        }
    });
    Ok(json!({
        "ok": true,
        "target": { "endpoint": endpoint, "label": label, "simulator": simulator },
    }))
}

#[cfg(test)]
#[path = "hmi_target_tests.rs"]
mod tests;
