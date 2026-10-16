//! Control-plane replies for the bounded resource command queue (specification 11 sections 6.6 and
//! 6.9).
//!
//! A request that needs the scan thread but finds the command queue full is refused with the
//! stable code `resource_busy`, and nothing is queued. A scheduler that no longer accepts commands
//! is refused with `resource_not_running`, which names the resource state.

use crate::scheduler::{CommandSendError, ResourceState};

use super::{ControlResponse, ControlState};

/// Stable code for a request refused because the resource command queue is full.
pub(super) const RESOURCE_BUSY: &str = "resource_busy";

/// Stable code for a pause, resume, restart or reload that the resource cannot take (spec 11
/// sections 6.6 and 6.9).
pub(super) const RESOURCE_NOT_RUNNING: &str = "resource_not_running";

/// Maps a command the queue did not take to its reply. A full queue is `resource_busy`; a closed
/// scheduler is the refusal that names the resource state.
pub(super) fn queue_refusal(
    id: u64,
    error: CommandSendError,
    state: &ControlState,
) -> ControlResponse {
    match error {
        CommandSendError::Full => busy_refusal(id),
        CommandSendError::Closed => resource_not_running(id, state.resource.state()),
    }
}

/// The refusal for a request that found the resource command queue full. Nothing was queued.
pub(super) fn busy_refusal(id: u64) -> ControlResponse {
    ControlResponse::error_with_code(
        id,
        "resource command queue is full; the request was not queued".to_string(),
        RESOURCE_BUSY,
    )
}

/// Refuses a pause, resume, restart or reload with the stable code, naming the resource state.
pub(super) fn resource_not_running(id: u64, resource: ResourceState) -> ControlResponse {
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

/// A read of the scan thread that produced no report. It keeps its message, and the stable code
/// when the refusal has one.
#[derive(Debug)]
pub(in crate::control) struct ResourceReadError {
    code: Option<&'static str>,
    message: String,
}

impl ResourceReadError {
    /// A read that failed for a reason with no stable code.
    pub(in crate::control) fn failed(message: String) -> Self {
        Self {
            code: None,
            message,
        }
    }

    /// A read whose command the queue did not take. A full queue carries `resource_busy`.
    pub(in crate::control) fn from_send(context: &str, error: CommandSendError) -> Self {
        let code = match error {
            CommandSendError::Full => Some(RESOURCE_BUSY),
            CommandSendError::Closed => None,
        };
        Self {
            code,
            message: format!("{context}: {error}"),
        }
    }

    /// The failure text, for a warning list.
    pub(in crate::control) fn message(&self) -> &str {
        &self.message
    }

    /// The control reply for the failed read.
    pub(in crate::control) fn into_response(self, id: u64) -> ControlResponse {
        match self.code {
            Some(code) => ControlResponse::error_with_code(id, self.message, code),
            None => ControlResponse::error(id, self.message),
        }
    }
}
