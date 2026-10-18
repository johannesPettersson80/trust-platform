//! The decision rules of `bytecode.reload` (specification 11 section 6.7.1), shared by every runtime:
//! the params, the resource rule and the reply to each way a queued reload ends.
//!
//! The platform queues the reload and waits for its outcome, because the waits need its threads and
//! its clock. [`admit`] decides a request before it is queued, and [`reply`] answers it once the wait
//! has ended. Nothing here holds a channel, a thread or a clock.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde::Deserialize;
use serde_json::json;
use trust_runtime_core::scheduler::ResourceState;

use crate::dispatch::transfers::{take_complete, transfer_member};
use crate::ports::ControlHost;
use crate::protocol::ControlResponse;
use crate::resource::replies::{hung_refusal, resource_not_running, RESOURCE_NOT_RUNNING};
use crate::status::{check_number_members, invalid_params, MemberKind};
use crate::transfer::TransferId;

/// The params of `bytecode.reload`: the program's bytes in standard base64. Fields the rule does not
/// read are ignored.
#[derive(Debug, Deserialize)]
struct BytecodeReloadParams {
    bytes: String,
}

/// Where the program of a request comes from: its `bytes`, or the complete transfer it names
/// (specification 34, PORT-05).
enum Source {
    /// The program's bytes, decoded from `bytes`.
    Bytes(Vec<u8>),
    /// The transfer whose bytes are the program. It is taken only once the request is accepted.
    Transfer(TransferId),
}

/// Decides a `bytecode.reload` before it is queued, and returns the program bytes to queue.
///
/// The first failing check is the reply. The params come first (`missing params`,
/// `invalid params: <reason>`, `invalid bytecode: <base64 error>`), then a hung resource
/// (`hung_refusal`), then a stopped resource, which is named with its state. A `transfer` is taken
/// only after those checks pass, so a refused request leaves the transfer open; a transfer that is
/// incomplete or unknown is refused with the session's own message.
///
/// # Errors
///
/// The refusal reply. Nothing is queued for a refused request.
pub fn admit<H: ControlHost>(
    id: u64,
    params: Option<serde_json::Value>,
    host: &H,
) -> Result<Vec<u8>, ControlResponse> {
    let source = decode(id, params)?;
    if host.is_hung() {
        return Err(hung_refusal(id));
    }
    // A stopped resource has no scan thread to apply the change, so the request is refused instead
    // of waiting for a reply that cannot come.
    let resource = host.resource_state();
    if matches!(resource, ResourceState::Stopped) {
        return Err(resource_not_running(id, resource));
    }
    match source {
        Source::Bytes(bytes) => Ok(bytes),
        Source::Transfer(transfer) => take_complete(id, transfer, host),
    }
}

/// Reads the params: `bytes` or `transfer`, and not both.
fn decode(id: u64, params: Option<serde_json::Value>) -> Result<Source, ControlResponse> {
    let Some(params) = params else {
        return Err(ControlResponse::error(id, "missing params".to_string()));
    };
    let has_bytes = params.get("bytes").is_some();
    let has_transfer = params.get("transfer").is_some();
    match (has_bytes, has_transfer) {
        (false, true) => Ok(Source::Transfer(transfer_member(id, &params)?)),
        (true, true) => Err(invalid_params(
            id,
            "give either bytes or transfer, not both",
        )),
        _ => decode_bytes(id, params).map(Source::Bytes),
    }
}

/// Reads `bytes` and decodes the program bytes from their base64 text.
fn decode_bytes(id: u64, params: serde_json::Value) -> Result<Vec<u8>, ControlResponse> {
    check_number_members(id, &params, &[("bytes", MemberKind::Text)])?;
    let params: BytecodeReloadParams = match serde_json::from_value(params) {
        Ok(parsed) => parsed,
        Err(err) => return Err(ControlResponse::error(id, format!("invalid params: {err}"))),
    };
    BASE64_STANDARD
        .decode(params.bytes.as_bytes())
        .map_err(|err| ControlResponse::error(id, format!("invalid bytecode: {err}")))
}

/// How a queued reload ended, as the platform's wait reports it.
#[derive(Debug)]
pub enum ReloadOutcome<M> {
    /// The scan thread applied the reload. `M` is the metadata of the program it now runs.
    Applied(M),
    /// The scan thread refused the reload. The text is its error.
    Failed(String),
    /// The reload was withdrawn before the scan thread took it, so it was never applied.
    Withdrawn,
    /// The scan thread stopped before it applied the reload.
    Stopped,
    /// The scan thread took the reload, but its commit result did not arrive within the bound.
    Unknown,
}

/// The reply to a queued reload, from how its wait ended.
///
/// An applied reload calls `publish` once, before the reply, with the new metadata: the platform
/// stores that metadata and refreshes what depends on it there. An error from `publish` is the
/// reply, so the reload is not reported as `reloaded`. The other outcomes do not call `publish`.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversion of this value cannot fail"
)]
pub fn reply<M>(
    id: u64,
    outcome: ReloadOutcome<M>,
    publish: impl FnOnce(M) -> Result<(), String>,
) -> ControlResponse {
    match outcome {
        ReloadOutcome::Applied(metadata) => match publish(metadata) {
            Ok(()) => ControlResponse::ok(id, json!({ "status": "reloaded" })),
            Err(error) => ControlResponse::error(id, error),
        },
        ReloadOutcome::Failed(error) => ControlResponse::error(id, error),
        ReloadOutcome::Withdrawn => ControlResponse::error(id, "reload timeout".to_string()),
        ReloadOutcome::Stopped => ControlResponse::error_with_code(
            id,
            "resource stopped before the reload was applied".to_string(),
            RESOURCE_NOT_RUNNING,
        ),
        ReloadOutcome::Unknown => {
            ControlResponse::error(id, "reload commit outcome is unknown".to_string())
        }
    }
}

#[cfg(test)]
mod contract_tests;
