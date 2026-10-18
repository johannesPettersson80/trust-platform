//! The `set`, `var.force`, `var.unforce` and `var.forced` operations (specification 11, "Live
//! Values"; specification 34 §4.3.1).
//!
//! `set` queues a global or retained write; `var.force` sets an active force on a global, retained
//! or instance target; `var.unforce` releases a force; `var.forced` projects the active variable
//! forces. All four are debug-surface operations: the request pipeline refuses them while the
//! debug surface is disabled, so they are never reached then. `eval` stays in trust-runtime,
//! because it needs the HIR expression evaluator.
//!
//! The composing platform owns the lock around the queues through [`VariableQueuePort`] and reports
//! a refusal; these operations own the request's parameters, the stable error texts and the reply
//! JSON, so a device answers them exactly as trust-runtime does.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde::Deserialize;
use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::memory::{Generation, InstanceId};
use trust_runtime_core::value::{format_user_value, Value};

use super::queues::{ForcedSnapshot, ForcedVarTarget, PendingVarTarget, RequestQueueFull};
use super::value_grammar::parse_value;
use crate::protocol::ControlResponse;

/// The Live Values variable queues that `set`, `var.force`, `var.unforce` and `var.forced` use.
///
/// The composing platform owns the lock around the queues and reports a refusal with
/// [`RequestQueueFull`]; these operations own the request's parameters, the stable error texts and
/// the reply JSON.
pub trait VariableQueuePort {
    /// The installation generation the attached runtime is in. A request that names an instance by
    /// a bare identity belongs to this generation (specification 11, debug mutation lifecycle).
    fn generation(&self) -> Generation;

    /// Queue a pending variable write to apply at the next cycle boundary.
    ///
    /// # Errors
    ///
    /// [`RequestQueueFull`] when `target` is not queued yet and the queue already holds its
    /// capacity; the refused request queues nothing.
    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull>;

    /// Set the active force on `target`, replacing an earlier force on the same target.
    ///
    /// # Errors
    ///
    /// [`RequestQueueFull`] when `target` is not forced yet and the queue already holds its
    /// capacity.
    fn force_var(&self, target: ForcedVarTarget, value: Value) -> Result<(), RequestQueueFull>;

    /// Release the active force on the global variable `name`, when one is set.
    fn release_global(&self, name: &str);

    /// Release the active force on the retained variable `name`, when one is set.
    fn release_retain(&self, name: &str);

    /// Release the active force on `instance`/`name`, in whichever generation it was forced.
    fn release_instance(&self, instance: InstanceId, name: &str);

    /// An owned copy of every active variable force, in the order the targets were first forced.
    fn forced_snapshot(&self) -> ForcedSnapshot;
}

/// The parameters of `set` and `var.force`: a target and the value to apply.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VarWriteParams {
    target: String,
    value: String,
}

/// The parameters of `var.unforce`: a target.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VarTargetParams {
    target: String,
}

/// `set`: queue a write of `value` to a global or retained variable.
#[must_use]
pub fn set<Q: VariableQueuePort>(
    id: u64,
    params: Option<serde_json::Value>,
    queues: &Q,
) -> ControlResponse {
    let params = match parse_write_params(id, params) {
        Ok(params) => params,
        Err(response) => return response,
    };
    let value = match parse_value(&params.value) {
        Ok(value) => value,
        Err(error) => return ControlResponse::error(id, error),
    };
    let target = match parse_var_target(&params.target) {
        Ok(VarTarget::Global(name)) => PendingVarTarget::Global(SmolStr::new(name)),
        Ok(VarTarget::Retain(name)) => PendingVarTarget::Retain(SmolStr::new(name)),
        // A queued write cannot name an instance; only a force can.
        Ok(VarTarget::Instance(_, _)) => {
            return ControlResponse::error(id, String::from("unsupported target"));
        }
        Err(error)
            if error.starts_with("missing global") || error.starts_with("missing retain") =>
        {
            return ControlResponse::error(id, error);
        }
        Err(_) => return ControlResponse::error(id, String::from("unsupported target")),
    };
    if let Err(error) = queues.enqueue_var_write(target, value) {
        return ControlResponse::error(id, error.to_string());
    }
    ControlResponse::ok(id, json!({"status": "queued"}))
}

/// `var.force`: hold the global, retained or instance `target` at `value`.
#[must_use]
pub fn var_force<Q: VariableQueuePort>(
    id: u64,
    params: Option<serde_json::Value>,
    queues: &Q,
) -> ControlResponse {
    let params = match parse_write_params(id, params) {
        Ok(params) => params,
        Err(response) => return response,
    };
    let target = match parse_var_target(&params.target) {
        Ok(target) => target,
        Err(error) => return ControlResponse::error(id, error),
    };
    let value = match parse_value(&params.value) {
        Ok(value) => value,
        Err(error) => return ControlResponse::error(id, error),
    };
    // A bare identity belongs to the generation installed when the request is accepted.
    let target = match target {
        VarTarget::Global(name) => ForcedVarTarget::Global(SmolStr::new(name)),
        VarTarget::Retain(name) => ForcedVarTarget::Retain(SmolStr::new(name)),
        VarTarget::Instance(id, name) => {
            ForcedVarTarget::Instance(queues.generation(), InstanceId(id), SmolStr::new(name))
        }
    };
    if let Err(error) = queues.force_var(target, value) {
        return ControlResponse::error(id, error.to_string());
    }
    ControlResponse::ok(id, json!({"status": "forced"}))
}

/// `var.unforce`: release the active force on the global, retained or instance `target`.
#[must_use]
pub fn var_unforce<Q: VariableQueuePort>(
    id: u64,
    params: Option<serde_json::Value>,
    queues: &Q,
) -> ControlResponse {
    let params: VarTargetParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, String::from("missing params")),
    };
    let target = match parse_var_target(&params.target) {
        Ok(target) => target,
        Err(error) => return ControlResponse::error(id, error),
    };
    match target {
        VarTarget::Global(name) => queues.release_global(&name),
        VarTarget::Retain(name) => queues.release_retain(&name),
        VarTarget::Instance(id, name) => queues.release_instance(InstanceId(id), &name),
    }
    ControlResponse::ok(id, json!({"status": "released"}))
}

/// `var.forced`: the active variable forces, in the order the targets were first forced.
#[must_use]
pub fn var_forced<Q: VariableQueuePort>(id: u64, queues: &Q) -> ControlResponse {
    let vars = queues
        .forced_snapshot()
        .vars
        .into_iter()
        .map(|entry| {
            let target = match entry.target {
                ForcedVarTarget::Global(name) => format!("global:{name}"),
                ForcedVarTarget::Retain(name) => format!("retain:{name}"),
                ForcedVarTarget::Instance(_, id, name) => format!("instance:{}:{name}", id.0),
            };
            json!({
                "target": target,
                "value": format_user_value(&entry.value),
            })
        })
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "vars": vars }))
}

/// The parsed `set`/`var.force` parameters, or the refusal to answer with.
fn parse_write_params(
    id: u64,
    params: Option<serde_json::Value>,
) -> Result<VarWriteParams, ControlResponse> {
    match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => Ok(parsed),
            Err(err) => Err(ControlResponse::error(id, format!("invalid params: {err}"))),
        },
        None => Err(ControlResponse::error(id, String::from("missing params"))),
    }
}

/// A parsed `set`, `var.force` or `var.unforce` target.
enum VarTarget {
    Global(String),
    Retain(String),
    Instance(u32, String),
}

/// Parse a `global:<name>`, `retain:<name>` or `instance:<id>:<name>` target, trimming the names.
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

#[cfg(test)]
mod tests;
