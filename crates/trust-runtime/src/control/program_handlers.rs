use crate::error::RuntimeError;
use crate::scheduler::{ReloadClaim, ResourceCommand};
use crate::security::AccessRole;
use serde_json::json;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;
use trust_runtime_services::loading::reload as reload_rules;

use super::resource_queue::queue_refusal;
use super::types::{PairClaimParams, PairRevokeParams};
use super::{ControlResponse, ControlState};

// `pause`, `resume`, `restart` and `shutdown` follow the shared resource rules
// (`trust_runtime_services::resource::operations`), which the shared request pipeline answers.

#[derive(Debug, Clone, Copy)]
pub(super) enum StepKind {
    In,
    Over,
    Out,
}

const STEP_REQUIRES_HOLD_MESSAGE: &str =
    "stepping requires the scan to be held by the debugger (a breakpoint or a debug-mode pause)";

/// A step is armed only while the debugger holds the scan, in either control mode.
/// Without a hold the request is refused before anything is armed.
pub(super) fn handle_step(id: u64, state: &ControlState, kind: StepKind) -> ControlResponse {
    if !state.debug.accepts_step() {
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

/// The shared `shutdown` reply, for the in-crate tests that call it directly.
#[cfg(test)]
pub(super) fn handle_shutdown(id: u64, _state: &ControlState) -> ControlResponse {
    trust_runtime_services::resource::operations::shutdown(id)
}

/// Answers `bytecode.reload`. The shared rules (`trust_runtime_services::loading::reload`) decide the
/// params and the resource; this handler only queues the reload and waits for it.
pub(super) fn handle_bytecode_reload(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let bytes = match reload_rules::admit(id, params, state) {
        Ok(bytes) => bytes,
        Err(refusal) => return refusal,
    };
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
    let outcome = match outcome {
        ReloadOutcome::Applied(result) => match *result {
            Ok(metadata) => reload_rules::ReloadOutcome::Applied(metadata),
            Err(err) => reload_rules::ReloadOutcome::Failed(err.to_string()),
        },
        ReloadOutcome::Withdrawn => reload_rules::ReloadOutcome::Withdrawn,
        ReloadOutcome::Stopped => reload_rules::ReloadOutcome::Stopped,
        ReloadOutcome::Unknown => reload_rules::ReloadOutcome::Unknown,
    };
    // An applied reload stores its metadata and refreshes the ADS server before the reply, as it
    // always has. The rules own the reply text; the stores stay with the control state.
    reload_rules::reply(id, outcome, |metadata| {
        if let Ok(mut guard) = state.metadata.lock() {
            *guard = metadata;
        }
        super::ads_handlers::refresh_ads_server_runtime_after_online_change(state)
    })
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
