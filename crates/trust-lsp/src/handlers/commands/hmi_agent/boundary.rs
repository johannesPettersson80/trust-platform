//! The shared request boundary of the authoring tools (specification 43, §4).
//!
//! Port of `agent/boundary.ts` (`invokeAuthoring`). Every tool call is checked here before any
//! operation runs: the envelope keys, the contract version, the workspace the request names and
//! the size of the request. The envelope never grants path authority; each operation checks its
//! own input.
//!
//! The answers carry the shared envelope: `{ ok: true, data, revisions, warnings }` for a success
//! and `{ ok: false, code, message, retryable: false }` for a refusal.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use super::error::{
    AgentError, AgentResult, CANCELLED, INVALID_ARGUMENT, PROJECT_UNAVAILABLE, UNSUPPORTED_VERSION,
};

/// The largest request the boundary accepts, measured as its encoded JSON text (8 MiB).
pub(super) const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;

/// The only keys a request may carry.
const ENVELOPE_KEYS: [&str; 3] = ["contractVersion", "workspaceId", "input"];

/// The contract version this service serves.
const SUPPORTED_VERSION: i64 = 1;

/// A successful operation's answer, before the envelope wraps it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Success {
    /// The operation's data.
    pub(super) data: Value,
    /// The revisions the operation read or produced, by name.
    pub(super) revisions: BTreeMap<String, String>,
    /// Warnings that do not refuse the operation.
    pub(super) warnings: Vec<String>,
}

/// Checks a request's envelope and, when it is valid, runs the operation on its `input`
/// (`invokeAuthoring`). `workspace_id` is the workspace this connection serves; `cancelled` is
/// the transport's cancellation of the request before it started.
///
/// # Errors
///
/// The returned envelope carries the refusal: `CANCELLED`, `INVALID_ARGUMENT` for a malformed
/// envelope, `UNSUPPORTED_VERSION`, `PROJECT_UNAVAILABLE` for a request from another workspace,
/// and `LIMIT_EXCEEDED` for a request over 8 MiB.
#[must_use]
pub(super) fn invoke(
    request: &Value,
    workspace_id: &str,
    cancelled: bool,
    operation: impl FnOnce(&Map<String, Value>) -> AgentResult<Success>,
) -> Value {
    match check_and_run(request, workspace_id, cancelled, operation) {
        Ok(success) => success_envelope(&success),
        Err(error) => failure_envelope(&error),
    }
}

/// The checks of `invokeAuthoring`, in its order, then the operation.
fn check_and_run(
    request: &Value,
    workspace_id: &str,
    cancelled: bool,
    operation: impl FnOnce(&Map<String, Value>) -> AgentResult<Success>,
) -> AgentResult<Success> {
    if cancelled {
        return Err(AgentError::new(
            CANCELLED,
            "The authoring request was cancelled.",
        ));
    }
    let shape = || {
        AgentError::new(
            INVALID_ARGUMENT,
            "Provide contractVersion, workspaceId and an input object only.",
        )
    };
    let envelope = request
        .as_object()
        .filter(|object| {
            object
                .keys()
                .all(|key| ENVELOPE_KEYS.contains(&key.as_str()))
        })
        .ok_or_else(shape)?;
    let version = envelope
        .get("contractVersion")
        .and_then(Value::as_i64)
        .ok_or_else(shape)?;
    let requested = envelope
        .get("workspaceId")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(shape)?;
    let input = envelope
        .get("input")
        .and_then(Value::as_object)
        .ok_or_else(shape)?;
    if version != SUPPORTED_VERSION {
        return Err(AgentError::new(
            UNSUPPORTED_VERSION,
            "This authoring service supports contractVersion 1.",
        ));
    }
    if workspace_id.is_empty() || requested != workspace_id {
        return Err(AgentError::new(
            PROJECT_UNAVAILABLE,
            "The request does not belong to this connected workspace.",
        ));
    }
    if request.to_string().len() > MAX_REQUEST_BYTES {
        return Err(AgentError::limit(
            "The request exceeds 8 MiB; submit a smaller batch.",
        ));
    }
    operation(input)
}

/// The success envelope of an answer.
#[must_use]
fn success_envelope(success: &Success) -> Value {
    json!({
        "ok": true,
        "data": success.data,
        "revisions": success.revisions,
        "warnings": success.warnings,
    })
}

/// The refusal envelope of an error. Refusals are never retryable as they stand.
#[must_use]
pub(super) fn failure_envelope(error: &AgentError) -> Value {
    json!({
        "ok": false,
        "code": error.code,
        "message": error.message,
        "retryable": false,
    })
}

#[cfg(test)]
#[path = "boundary_tests.rs"]
mod tests;
