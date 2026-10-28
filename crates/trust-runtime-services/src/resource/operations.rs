//! The resource operations of the control protocol: `pause`, `resume`, `restart` and `shutdown`
//! (specification 11, §6.6 and §6.9).
//!
//! A refusal changes nothing: no scheduler command, no debugger action and no restart request.

use alloc::format;
use alloc::string::String;

use serde::Deserialize;
use serde_json::json;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;

use super::replies::{hung_refusal, no_installed_program, queue_refusal, resource_not_running};
use crate::debug::replies::no_line_table;
use crate::ports::ResourcePort;
use crate::protocol::ControlResponse;
use crate::status::{invalid_params, validate_object_fields};

#[derive(Debug, Deserialize)]
struct RestartParams {
    mode: String,
}

/// Whether a resource in `state` has a scheduler that takes pause and resume commands; a
/// faulted or stopped one has not.
#[must_use]
pub fn accepts_pause_resume(state: ResourceState) -> bool {
    !matches!(state, ResourceState::Faulted | ResourceState::Stopped)
}

/// The restart request that follows a request for `requested` while `pending` waits for the
/// scan.
///
/// Requests coalesce into the pending one, and a pending cold restart keeps its precedence over a
/// later warm one.
#[must_use]
pub fn coalesce_restart(pending: Option<RestartMode>, requested: RestartMode) -> RestartMode {
    match pending {
        Some(RestartMode::Cold) => RestartMode::Cold,
        _ => requested,
    }
}

/// `pause`: holds the scan.
///
/// In debug control mode the debugger holds the scan at its next statement; otherwise the
/// scheduler pauses between cycles. The reply `paused` means the resource accepted the request. A
/// program without a line table cannot be held at a statement, so a debug-mode `pause` is refused
/// with `no_line_table` before the debugger is asked (specification 11 §6.13.3).
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn pause<H: ResourcePort>(id: u64, host: &H) -> ControlResponse {
    if host.is_hung() {
        return hung_refusal(id);
    }
    let resource = host.resource_state();
    if !accepts_pause_resume(resource) {
        return resource_not_running(id, resource);
    }
    if host.debug_control_mode() {
        if let Some(reason) = host.undebuggable() {
            return ControlResponse::error(id, reason.into());
        }
        if host.debugger_unavailable() {
            return no_line_table(id);
        }
        host.debugger_pause();
    } else if let Err(error) = host.pause_resource() {
        return queue_refusal(id, error, host);
    }
    ControlResponse::ok(id, json!({"status": "paused"}))
}

/// `resume`: returns the scan to cyclic execution.
///
/// Outside debug control mode the scheduler resumes first, so a refused resume leaves a debugger
/// hold as it was; either mode then releases the debugger hold.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn resume<H: ResourcePort>(id: u64, host: &H) -> ControlResponse {
    if host.is_hung() {
        return hung_refusal(id);
    }
    let resource = host.resource_state();
    if !accepts_pause_resume(resource) {
        return resource_not_running(id, resource);
    }
    if !host.debug_control_mode()
        && let Err(error) = host.resume_resource()
    {
        return queue_refusal(id, error, host);
    }
    host.debugger_continue();
    ControlResponse::ok(id, json!({"status": "running"}))
}

/// `restart` with `{"mode": "cold" | "warm"}`: queues a restart that the scan takes at its next
/// boundary, in the faulted wait, or in the stopped wait.
///
/// A hung resource has no scan to take it and refuses. A stopped one refuses too, unless the
/// platform starts a stopped resource again (specification 34, STATE-08: on every runtime the
/// restart starts the installed program as a new process would); a device without an installed
/// program refuses with the reason it has none.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn restart<H: ResourcePort>(
    id: u64,
    params: Option<serde_json::Value>,
    host: &H,
) -> ControlResponse {
    let params: RestartParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let mode = match params.mode.to_ascii_lowercase().as_str() {
        "cold" => RestartMode::Cold,
        "warm" => RestartMode::Warm,
        _ => return ControlResponse::error(id, "invalid restart mode".into()),
    };
    if host.is_hung() {
        return hung_refusal(id);
    }
    let resource = host.resource_state();
    if matches!(resource, ResourceState::Stopped) {
        if let Some(reason) = host.missing_program() {
            return no_installed_program(id, reason);
        }
        if !host.starts_when_stopped() {
            return resource_not_running(id, resource);
        }
    }
    host.update_pending_restart(|pending| *pending = Some(coalesce_restart(*pending, mode)));
    ControlResponse::ok(id, json!({"status": "restart queued"}))
}

/// `shutdown`: replies `stopping`; the platform stops the program after it has written the reply
/// and leaves the runtime reachable in the Stopped state (specification 11 §6.9; specification 34,
/// STATE-08).
///
/// `params` may be absent or an object whose only member is `exit`, a Boolean. `{"exit": true}` is
/// the explicit exit: the reply is the same, and the platform then also ends the runtime's process
/// where it has one. `{"exit": false}` is plain `shutdown`. Any other shape is refused with
/// `invalid params: <reason>`, and nothing stops.
#[must_use]
pub fn shutdown(id: u64, params: Option<&serde_json::Value>) -> ControlResponse {
    match exit_requested(params) {
        Ok(false) => ControlResponse::shutdown(id),
        Ok(true) => ControlResponse::shutdown_and_exit(id),
        Err(reason) => invalid_params(id, reason),
    }
}

/// Whether `params` ask for the explicit exit, or why they are refused.
fn exit_requested(params: Option<&serde_json::Value>) -> Result<bool, &'static str> {
    let Some(params) = params else {
        return Ok(false);
    };
    validate_object_fields(params, &["exit"], &["exit"])?;
    match params.get("exit") {
        None => Ok(false),
        Some(exit) => exit.as_bool().ok_or("exit must be a boolean"),
    }
}

#[cfg(test)]
mod contract_tests;
