use crate::config::ControlMode;
use crate::error::RuntimeError;
use crate::scheduler::{ReloadClaim, ResourceCommand, ResourceState};
use crate::security::AccessRole;
use crate::RestartMode;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde_json::json;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use super::resource_queue::{queue_refusal, resource_not_running, RESOURCE_NOT_RUNNING};
use super::types::{BytecodeReloadParams, PairClaimParams, PairRevokeParams, RestartParams};
use super::{ControlResponse, ControlState};

#[derive(Debug, Clone, Copy)]
pub(super) enum StepKind {
    In,
    Over,
    Out,
}

const HUNG_MESSAGE: &str =
    "resource scan thread did not return; the runtime process must be restarted";

/// Refuse a request that needs the scan thread while hosted hang supervision reports that the
/// scan thread is inside a call that did not return. Only a new runtime process recovers it.
fn hung_refusal(id: u64) -> ControlResponse {
    ControlResponse::error_with_code(id, HUNG_MESSAGE.to_string(), RESOURCE_NOT_RUNNING)
}

fn control_mode(state: &ControlState) -> ControlMode {
    state
        .control_mode
        .lock()
        .map(|value| *value)
        .unwrap_or(ControlMode::Production)
}

/// A faulted or stopped resource has no scheduler that takes pause or resume commands.
fn accepts_pause_resume(resource: ResourceState) -> bool {
    !matches!(resource, ResourceState::Faulted | ResourceState::Stopped)
}

/// The reply `paused` or `running` means the resource accepted the request (spec 11 section 6.9).
/// A refusal changes nothing: no scheduler command and no debugger action is applied.
pub(super) fn handle_pause(id: u64, state: &ControlState) -> ControlResponse {
    if state.resource.is_hung() {
        return hung_refusal(id);
    }
    let resource = state.resource.state();
    if !accepts_pause_resume(resource) {
        return resource_not_running(id, resource);
    }
    if matches!(control_mode(state), ControlMode::Debug) {
        let _ = state
            .debug
            .apply_action(crate::debug::ControlAction::Pause(None));
    } else if let Err(error) = state.resource.pause() {
        return queue_refusal(id, error, state);
    }
    ControlResponse::ok(id, json!({"status": "paused"}))
}

pub(super) fn handle_resume(id: u64, state: &ControlState) -> ControlResponse {
    if state.resource.is_hung() {
        return hung_refusal(id);
    }
    let resource = state.resource.state();
    if !accepts_pause_resume(resource) {
        return resource_not_running(id, resource);
    }
    // The scheduler command is sent before the debugger hold is released, so a refused
    // resume leaves the debugger state as it was.
    if !matches!(control_mode(state), ControlMode::Debug)
        && let Err(error) = state.resource.resume()
    {
        return queue_refusal(id, error, state);
    }
    // Either mode releases a debugger hold, so the scan returns to cyclic execution.
    let _ = state
        .debug
        .apply_action(crate::debug::ControlAction::Continue);
    ControlResponse::ok(id, json!({"status": "running"}))
}

const STEP_REQUIRES_HOLD_MESSAGE: &str =
    "stepping requires the scan to be held by the debugger (a breakpoint or a debug-mode pause)";

/// A step is armed only while the debugger holds the scan, in either control mode.
/// Without a hold the request is refused before anything is armed.
pub(super) fn handle_step(id: u64, state: &ControlState, kind: StepKind) -> ControlResponse {
    if state.debug.hold_reason().is_none() {
        return ControlResponse::error(id, STEP_REQUIRES_HOLD_MESSAGE.into());
    }
    let action = match kind {
        StepKind::In => crate::debug::ControlAction::StepIn(None),
        StepKind::Over => crate::debug::ControlAction::StepOver(None),
        StepKind::Out => crate::debug::ControlAction::StepOut(None),
    };
    let _ = state.debug.apply_action(action);
    ControlResponse::ok(id, json!({"status": "stepping"}))
}

pub(super) fn handle_shutdown(id: u64, _state: &ControlState) -> ControlResponse {
    ControlResponse::shutdown(id)
}

pub(super) fn handle_restart(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
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
    if state.resource.is_hung() {
        return hung_refusal(id);
    }
    // A live scan thread takes the request: a running or paused resource at its next scan
    // boundary, a faulted one in the faulted wait. A stopped resource has no thread left.
    let resource = state.resource.state();
    if matches!(resource, ResourceState::Stopped) {
        return resource_not_running(id, resource);
    }
    if let Ok(mut guard) = state.pending_restart.lock() {
        // Requests that arrive before the scan thread takes the pending one coalesce into it. A
        // pending cold restart keeps its precedence over a later warm one (spec 11 section 6.9).
        let next = match *guard {
            Some(RestartMode::Cold) => RestartMode::Cold,
            _ => mode,
        };
        *guard = Some(next);
    }
    ControlResponse::ok(id, json!({"status": "restart queued"}))
}

pub(super) fn handle_bytecode_reload(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: BytecodeReloadParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let bytes = match BASE64_STANDARD.decode(params.bytes.as_bytes()) {
        Ok(bytes) => bytes,
        Err(err) => return ControlResponse::error(id, format!("invalid bytecode: {err}")),
    };
    if state.resource.is_hung() {
        return hung_refusal(id);
    }
    // A stopped resource has no scan thread to apply the change, so the request is refused
    // instead of waiting for a reply that cannot come.
    let resource = state.resource.state();
    if matches!(resource, ResourceState::Stopped) {
        return resource_not_running(id, resource);
    }
    reload_on_resource(id, state, bytes, RELOAD_WAIT)
}

/// How long a reload request waits for the scan thread to take it, and then for its commit result
/// once it has been taken (specification 11 section 6.7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReloadWait {
    pub(super) take: Duration,
    pub(super) commit: Duration,
}

pub(super) const RELOAD_WAIT: ReloadWait = ReloadWait {
    take: Duration::from_secs(5),
    commit: Duration::from_secs(60),
};

/// How a reload request's wait ended.
#[derive(Debug)]
pub(super) enum ReloadOutcome {
    /// The scan thread took the request and reported its commit or preparation result. Boxed,
    /// because the metadata is far larger than the other outcomes.
    Applied(Box<Result<crate::RuntimeMetadata, RuntimeError>>),
    /// The request was withdrawn before the scan thread took it, so it is never applied.
    Withdrawn,
    /// The scan thread dropped the request unapplied, because it stopped first.
    Stopped,
    /// The scan thread took the request, but no result arrived within the commit bound.
    Unknown,
}

/// Queues one reload for the scan thread and waits for its outcome.
pub(super) fn reload_on_resource(
    id: u64,
    state: &ControlState,
    bytes: Vec<u8>,
    wait: ReloadWait,
) -> ControlResponse {
    let claim = ReloadClaim::default();
    let (reply_tx, reply_rx) = std::sync::mpsc::channel();
    let command = ResourceCommand::ReloadBytecode {
        bytes,
        claim: claim.clone(),
        respond_to: reply_tx,
    };
    if let Err(error) = state.resource.send_command(command) {
        return queue_refusal(id, error, state);
    }
    let outcome = await_reload(&claim, &reply_rx, wait);
    reload_reply(id, outcome, state)
}

/// Waits for the outcome of one queued reload. A reload the scan thread has not taken within
/// `wait.take` is withdrawn, so it cannot be applied after its caller was told that it timed out.
/// A reload that was taken already reports its own result.
pub(super) fn await_reload(
    claim: &ReloadClaim,
    replies: &Receiver<Result<crate::RuntimeMetadata, RuntimeError>>,
    wait: ReloadWait,
) -> ReloadOutcome {
    match replies.recv_timeout(wait.take) {
        Ok(result) => ReloadOutcome::Applied(Box::new(result)),
        Err(RecvTimeoutError::Timeout) => {
            if claim.withdraw() {
                return ReloadOutcome::Withdrawn;
            }
            match replies.recv_timeout(wait.commit) {
                Ok(result) => ReloadOutcome::Applied(Box::new(result)),
                Err(_) => ReloadOutcome::Unknown,
            }
        }
        Err(RecvTimeoutError::Disconnected) => {
            if claim.withdraw() {
                ReloadOutcome::Stopped
            } else {
                ReloadOutcome::Unknown
            }
        }
    }
}

fn reload_reply(id: u64, outcome: ReloadOutcome, state: &ControlState) -> ControlResponse {
    match outcome {
        ReloadOutcome::Applied(result) => match *result {
            Ok(metadata) => {
                if let Ok(mut guard) = state.metadata.lock() {
                    *guard = metadata;
                }
                if let Err(error) =
                    super::ads_handlers::refresh_ads_server_runtime_after_online_change(state)
                {
                    return ControlResponse::error(id, error);
                }
                ControlResponse::ok(id, json!({ "status": "reloaded" }))
            }
            Err(err) => ControlResponse::error(id, err.to_string()),
        },
        ReloadOutcome::Withdrawn => ControlResponse::error(id, "reload timeout".into()),
        ReloadOutcome::Stopped => ControlResponse::error_with_code(
            id,
            "resource stopped before the reload was applied".into(),
            RESOURCE_NOT_RUNNING,
        ),
        ReloadOutcome::Unknown => {
            ControlResponse::error(id, "reload commit outcome is unknown".into())
        }
    }
}

pub(super) fn handle_pair_start(id: u64, state: &ControlState) -> ControlResponse {
    let Some(store) = state.pairing.as_ref() else {
        return ControlResponse::error(id, "pairing unavailable".into());
    };
    let code = store.start_pairing();
    ControlResponse::ok(
        id,
        json!({ "code": code.code, "expires_at": code.expires_at }),
    )
}

pub(super) fn handle_pair_claim(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: PairClaimParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let Some(store) = state.pairing.as_ref() else {
        return ControlResponse::error(id, "pairing unavailable".into());
    };
    let requested_role = match params.role.as_deref() {
        Some(text) => match AccessRole::parse(text) {
            Some(role) => Some(role),
            None => return ControlResponse::error(id, "invalid role".into()),
        },
        None => None,
    };
    match store.claim(&params.code, requested_role) {
        Some(token) => ControlResponse::ok(id, json!({ "token": token })),
        None => ControlResponse::error(id, "invalid or expired code".into()),
    }
}

pub(super) fn handle_pair_list(id: u64, state: &ControlState) -> ControlResponse {
    let Some(store) = state.pairing.as_ref() else {
        return ControlResponse::error(id, "pairing unavailable".into());
    };
    let tokens = store.list();
    ControlResponse::ok(id, json!({ "tokens": tokens }))
}

pub(super) fn handle_pair_revoke(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: PairRevokeParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let Some(store) = state.pairing.as_ref() else {
        return ControlResponse::error(id, "pairing unavailable".into());
    };
    if params.id == "all" {
        let count = store.revoke_all();
        return ControlResponse::ok(id, json!({ "status": "revoked", "count": count }));
    }
    if store.revoke(&params.id) {
        ControlResponse::ok(id, json!({ "status": "revoked", "id": params.id }))
    } else {
        ControlResponse::error(id, "unknown token id".into())
    }
}
