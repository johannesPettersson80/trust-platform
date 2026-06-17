use crate::config::ControlMode;
use crate::scheduler::ResourceCommand;
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

pub(super) fn handle_pause(id: u64, state: &ControlState) -> ControlResponse {
    let mode = state
        .control_mode
        .lock()
        .map(|value| *value)
        .unwrap_or(ControlMode::Production);
    if matches!(mode, ControlMode::Debug) {
        let _ = state
            .debug
            .apply_action(crate::debug::ControlAction::Pause(None));
    } else {
        let _ = state.resource.pause();
    }
    ControlResponse::ok(id, json!({"status": "paused"}))
}

pub(super) fn handle_resume(id: u64, state: &ControlState) -> ControlResponse {
    let mode = state
        .control_mode
        .lock()
        .map(|value| *value)
        .unwrap_or(ControlMode::Production);
    if matches!(mode, ControlMode::Debug) {
        let _ = state
            .debug
            .apply_action(crate::debug::ControlAction::Continue);
    } else {
        let _ = state.resource.resume();
    }
    ControlResponse::ok(id, json!({"status": "running"}))
}

pub(super) fn handle_step(id: u64, state: &ControlState, kind: StepKind) -> ControlResponse {
    let action = match kind {
        StepKind::In => crate::debug::ControlAction::StepIn(None),
        StepKind::Over => crate::debug::ControlAction::StepOver(None),
        StepKind::Out => crate::debug::ControlAction::StepOut(None),
    };
    let _ = state.debug.apply_action(action);
    ControlResponse::ok(id, json!({"status": "stepping"}))
}

pub(super) fn handle_shutdown(id: u64, state: &ControlState) -> ControlResponse {
    state.resource.stop();
    ControlResponse::ok(id, json!({"status": "stopping"}))
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
