//! What Live Values says when it has no session to read (specification 25 §4.4). A selected online
//! runtime that the lifecycle does not name connected is told to Connect, or, when that runtime
//! answered Connect but keeps debugging disabled, why it has no session, with the state `Debugging
//! disabled`. Every other target, the Simulator among them, is told to Start.
//!
//! - `unavailable` `{ snapshot }`, the lifecycle snapshot of `selection`, or that of a debug session
//!   that just ended: `{ ok: true, message, debugDisabled }`.

use serde::Deserialize;
use serde_json::{json, Value};

use super::connect::DEBUG_DISABLED;
use super::decode;
use super::selection::{debugging_disabled, Snapshot};

/// What Live Values says of a target without a session that the user starts.
const START: &str = "Start the runtime to see live values.";
/// What Live Values says of an online runtime without a session.
const CONNECT: &str = "Connect to the selected runtime to see live values.";

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    snapshot: Snapshot,
}

/// `{ ok: true, message, debugDisabled }` for `argument`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    let args: Args = decode(argument)?;
    let online =
        args.snapshot.runtime_mode == "online" && args.snapshot.runtime_state != "connected";
    let debug_disabled = online && debugging_disabled(&args.snapshot);
    let message = if debug_disabled {
        DEBUG_DISABLED
    } else if online {
        CONNECT
    } else {
        START
    };
    Ok(json!({ "ok": true, "message": message, "debugDisabled": debug_disabled }))
}

#[cfg(test)]
#[path = "unavailable_tests.rs"]
mod tests;
