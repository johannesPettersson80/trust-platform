//! Whether Live mode's operating view ends, and why (specification 39, §11 item 6). The extension
//! reports what happens while it operates the deployed HMI of the selected run target, and acts on
//! the verdict.
//!
//! `trust-lsp.hmiLiveOperation` `{ event, … }` answers `{ ok: true, proceed, end }`: `proceed`
//! whether what the event is about goes on (a request goes out, a reply is shown), and `end` the
//! reason operation then ends, or `null`. The events:
//! - `request` `{ dirty }`, before a request goes out: unsaved HMI changes end operation.
//! - `reply` `{ method, status, body?, bundle }`, the web server's answer and the operated bundle:
//!   a page read (`GET`) answered from another bundle ends it before the reply is shown; an answer
//!   `409`, the named session's target changed or gone, is shown and then ends it.
//! - `streamUnavailable`, `streamClosed`, `unreachable`, `targetChanged` and `targetUnreachable`
//!   end it with what each names.
//!
//! Arguments it cannot read answer `{ ok: false, error }`.

use serde::Deserialize;
use serde_json::{json, Value};

use super::hmi_files::single_argument;
use super::HMI_LIVE_OPERATION_COMMAND;

#[cfg(test)]
mod tests;

/// What happened while the operating view runs.
#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "camelCase", deny_unknown_fields)]
enum Event {
    Request {
        dirty: bool,
    },
    Reply {
        method: String,
        status: u16,
        #[serde(default)]
        body: Value,
        bundle: String,
    },
    StreamUnavailable,
    StreamClosed,
    Unreachable,
    TargetChanged,
    TargetUnreachable,
}

/// Why operation ends when the HMI has unsaved changes; the gate's own reason (§11 item 6).
const CHANGED: &str = "The HMI changed. Save and deploy it before operating again.";
const DEPLOYED_CHANGED: &str =
    "The deployed HMI changed. Check and deploy the matching project before operating again.";
/// What every ending through a lost connection says of the commands in flight (§11 item 6:
/// reconnect never resends a command).
const NOT_RESENT: &str = "Any unfinished command has an unknown outcome; none will be resent.";
const STREAM_UNAVAILABLE: &str = "The runtime stream is unavailable. Reopen Live mode to connect.";
const TARGET_CHANGED: &str = "The selected run target changed. Reopen Live mode to connect.";
const TARGET_UNREACHABLE: &str =
    "The selected run target cannot be reached. Reopen Live mode to connect.";

/// The answer of `trust-lsp.hmiLiveOperation`.
pub(super) fn live_operation_value(args: Vec<Value>) -> Value {
    match single_argument::<Event>(args, HMI_LIVE_OPERATION_COMMAND) {
        Ok(event) => {
            let (proceed, end) = verdict(&event);
            json!({ "ok": true, "proceed": proceed, "end": end })
        }
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

/// Whether what `event` is about goes on, and why operation then ends, if it does.
fn verdict(event: &Event) -> (bool, Option<String>) {
    match event {
        Event::Request { dirty: true } => (false, Some(CHANGED.to_owned())),
        Event::Request { dirty: false } => (true, None),
        Event::Reply {
            method,
            status,
            body,
            bundle,
        } => {
            let other_bundle = method == "GET"
                && body
                    .get("bundle")
                    .is_some_and(|served| served.as_str() != Some(bundle.as_str()));
            if other_bundle {
                (false, Some(DEPLOYED_CHANGED.to_owned()))
            } else if *status == 409 {
                (
                    true,
                    Some(format!(
                        "The run target changed or disconnected. {NOT_RESENT}"
                    )),
                )
            } else {
                (true, None)
            }
        }
        Event::StreamUnavailable => (false, Some(STREAM_UNAVAILABLE.to_owned())),
        Event::StreamClosed => (
            false,
            Some(format!("The runtime disconnected. {NOT_RESENT}")),
        ),
        Event::Unreachable => (
            false,
            Some(format!("The runtime cannot be reached. {NOT_RESENT}")),
        ),
        Event::TargetChanged => (false, Some(TARGET_CHANGED.to_owned())),
        Event::TargetUnreachable => (false, Some(TARGET_UNREACHABLE.to_owned())),
    }
}
