//! Control-plane replies for the bounded resource command queue (specification 11 sections 6.6 and
//! 6.9).
//!
//! A request that needs the scan thread but finds the command queue full is refused with the
//! stable code `resource_busy`, and nothing is queued. A scheduler that no longer accepts commands
//! is refused with `resource_not_running`, which names the resource state.

use crate::scheduler::CommandSendError;
use trust_runtime_services::resource::replies::RESOURCE_BUSY;

use super::ControlResponse;

// The refusal replies are shared runtime code (spec 34 ARCH-11). `queue_refusal` reads the
// resource state through the host's resource port when the scheduler no longer takes commands.
pub(super) use trust_runtime_services::resource::replies::{
    queue_refusal, resource_not_running, RESOURCE_NOT_RUNNING,
};

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
