use serde_json::json;

use super::types::{EvalParams, SetParams, VarForceParams, VarTarget, VarTargetParams};
use super::{parse_value, ControlResponse, ControlState};

#[cfg(test)]
#[path = "variable_handlers/contract_tests.rs"]
mod contract_tests;

pub(super) fn handle_eval(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: EvalParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let name = params.expr.trim();
    let value = snapshot
        .storage
        .get_global(name)
        .cloned()
        .or_else(|| snapshot.storage.get_retain(name).cloned());
    match value {
        Some(value) => ControlResponse::ok(id, json!({ "value": format!("{value:?}") })),
        None => ControlResponse::error(id, "unknown identifier".into()),
    }
}

pub(super) fn handle_set(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: SetParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let value = match parse_value(&params.value) {
        Ok(value) => value,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    match parse_var_target(&params.target) {
        Ok(VarTarget::Global(name)) => state.debug.enqueue_global_write(name, value),
        Ok(VarTarget::Retain(name)) => state.debug.enqueue_retain_write(name, value),
        Ok(VarTarget::Instance(_, _)) => {
            return ControlResponse::error(id, "unsupported target".into())
        }
        Err(error)
            if error.starts_with("missing global") || error.starts_with("missing retain") =>
        {
            return ControlResponse::error(id, error)
        }
        Err(_) => return ControlResponse::error(id, "unsupported target".into()),
    }
    ControlResponse::ok(id, json!({"status": "queued"}))
}

pub(super) fn handle_var_force(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: VarForceParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let target = match parse_var_target(&params.target) {
        Ok(target) => target,
        Err(err) => return ControlResponse::error(id, err),
    };
    let value = match parse_value(&params.value) {
        Ok(value) => value,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    match target {
        VarTarget::Global(name) => state.debug.force_global(name, value),
        VarTarget::Retain(name) => state.debug.force_retain(name, value),
        VarTarget::Instance(id, name) => {
            state
                .debug
                .force_instance(crate::memory::InstanceId(id), name, value)
        }
    }
    ControlResponse::ok(id, json!({ "status": "forced" }))
}

pub(super) fn handle_var_unforce(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: VarTargetParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let target = match parse_var_target(&params.target) {
        Ok(target) => target,
        Err(err) => return ControlResponse::error(id, err),
    };
    match target {
        VarTarget::Global(name) => state.debug.release_global(&name),
        VarTarget::Retain(name) => state.debug.release_retain(&name),
        VarTarget::Instance(id, name) => state
            .debug
            .release_instance(crate::memory::InstanceId(id), &name),
    }
    ControlResponse::ok(id, json!({ "status": "released" }))
}

pub(super) fn handle_var_forced(id: u64, state: &ControlState) -> ControlResponse {
    let snapshot = state.debug.forced_snapshot();
    let vars = snapshot
        .vars
        .into_iter()
        .map(|entry| {
            let target = match entry.target {
                crate::debug::ForcedVarTarget::Global(name) => {
                    format!("global:{name}")
                }
                crate::debug::ForcedVarTarget::Retain(name) => {
                    format!("retain:{name}")
                }
                crate::debug::ForcedVarTarget::Instance(id, name) => {
                    format!("instance:{}:{name}", id.0)
                }
            };
            json!({
                "target": target,
                "value": crate::debug::dap::format_value(&entry.value),
            })
        })
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "vars": vars }))
}

fn parse_var_target(target: &str) -> Result<VarTarget, String> {
    if let Some(name) = target.strip_prefix("global:") {
        if name.trim().is_empty() {
            return Err("missing global name".into());
        }
        return Ok(VarTarget::Global(name.trim().to_string()));
    }
    if let Some(name) = target.strip_prefix("retain:") {
        if name.trim().is_empty() {
            return Err("missing retain name".into());
        }
        return Ok(VarTarget::Retain(name.trim().to_string()));
    }
    if let Some(rest) = target.strip_prefix("instance:") {
        let mut parts = rest.splitn(2, ':');
        let id_text = parts.next().unwrap_or_default();
        if id_text.is_empty() || !id_text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("invalid instance id".to_string());
        }
        let id = id_text
            .parse::<u32>()
            .map_err(|_| "invalid instance id".to_string())?;
        let name = parts.next().unwrap_or("").trim();
        if name.is_empty() {
            return Err("missing instance name".into());
        }
        return Ok(VarTarget::Instance(id, name.to_string()));
    }
    Err("unsupported target (use global:<name> or retain:<name>)".into())
}
