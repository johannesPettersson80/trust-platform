//! Stable replies to requests that need the resource's scan (specification 11, §6.6 and §6.9).

use alloc::format;
use alloc::string::ToString;

use trust_runtime_core::scheduler::ResourceState;

use super::commands::CommandSendError;
use crate::ports::ResourcePort;
use crate::protocol::ControlResponse;

/// Stable code of a request refused because the resource command queue is full.
pub const RESOURCE_BUSY: &str = "resource_busy";

/// Stable code of a pause, resume, restart or reload that the resource cannot take.
pub const RESOURCE_NOT_RUNNING: &str = "resource_not_running";

const HUNG_MESSAGE: &str =
    "resource scan thread did not return; the runtime process must be restarted";

/// The refusal of a request that found the resource command queue full; nothing was queued.
#[must_use]
pub fn busy_refusal(id: u64) -> ControlResponse {
    ControlResponse::error_with_code(
        id,
        "resource command queue is full; the request was not queued".to_string(),
        RESOURCE_BUSY,
    )
}

/// Refuses a pause, resume, restart or reload, naming the resource state.
#[must_use]
pub fn resource_not_running(id: u64, resource: ResourceState) -> ControlResponse {
    let message = match resource {
        ResourceState::Faulted => "resource is faulted; restart required".to_string(),
        ResourceState::Stopped => "resource is stopped".to_string(),
        other => {
            let label = format!("{other:?}").to_ascii_lowercase();
            format!("resource is {label} and its scheduler does not accept commands")
        }
    };
    ControlResponse::error_with_code(id, message, RESOURCE_NOT_RUNNING)
}

/// Refuses a `restart` of a device without an installed program, naming why it has none
/// (specification 34, STATE-08).
#[must_use]
pub fn no_installed_program(id: u64, reason: &str) -> ControlResponse {
    ControlResponse::error_with_code(
        id,
        format!("no installed program: {reason}"),
        RESOURCE_NOT_RUNNING,
    )
}

/// The reply to a command the queue did not take.
///
/// A full queue is `resource_busy`; a closed scheduler is the refusal that names the resource
/// state, read after the refusal.
#[must_use]
pub fn queue_refusal<H: ResourcePort>(
    id: u64,
    error: CommandSendError,
    host: &H,
) -> ControlResponse {
    match error {
        CommandSendError::Full => busy_refusal(id),
        CommandSendError::Closed => resource_not_running(id, host.resource_state()),
    }
}

/// Refuses a request that needs the scan while the scan is inside a call that did not return;
/// only a new runtime process recovers it.
#[must_use]
pub fn hung_refusal(id: u64) -> ControlResponse {
    ControlResponse::error_with_code(id, HUNG_MESSAGE.to_string(), RESOURCE_NOT_RUNNING)
}
