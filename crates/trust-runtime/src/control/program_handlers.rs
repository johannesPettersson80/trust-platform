use crate::config::ControlMode;
use crate::scheduler::{ResourceCommand, ResourceState};
use crate::security::AccessRole;
use crate::RestartMode;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use serde_json::json;

use super::types::{BytecodeReloadParams, PairClaimParams, PairRevokeParams, RestartParams};
use super::{ControlResponse, ControlState};

#[derive(Debug, Clone, Copy)]
pub(super) enum StepKind {
    In,
    Over,
    Out,
}

/// Stable code for a pause, resume, restart or reload that the resource cannot take (spec 11
/// sections 6.6 and 6.9).
const RESOURCE_NOT_RUNNING: &str = "resource_not_running";

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

/// Refuse a pause or resume with the stable code, naming the resource state. The refusal
/// changes nothing: no scheduler command and no debugger action is applied.
fn resource_not_running(id: u64, resource: ResourceState) -> ControlResponse {
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
    } else if state.resource.pause().is_err() {
        return resource_not_running(id, state.resource.state());
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
    if !matches!(control_mode(state), ControlMode::Debug) && state.resource.resume().is_err() {
        return resource_not_running(id, state.resource.state());
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
        *guard = Some(mode);
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
    let (tx, rx) = std::sync::mpsc::channel();
    if let Err(err) = state
        .resource
        .send_command(ResourceCommand::ReloadBytecode {
            bytes,
            respond_to: tx,
        })
    {
        return ControlResponse::error(id, err.to_string());
    }
    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(Ok(metadata)) => {
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
        Ok(Err(err)) => ControlResponse::error(id, err.to_string()),
        Err(_) => ControlResponse::error(id, "reload timeout".into()),
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
