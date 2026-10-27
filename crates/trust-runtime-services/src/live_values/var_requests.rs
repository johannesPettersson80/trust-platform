//! The `set`, `var.force`, `var.unforce` and `var.forced` operations (specification 11, "Live
//! Values"; specification 34 §4.3.1).
//!
//! `set` queues a write of a global, a variable path or a retained variable; `var.force` sets an
//! active force on such a target or an instance's variable; `var.unforce` releases a force;
//! `var.forced` projects the active variable forces. All four are debug-surface operations: the
//! request pipeline refuses them while the debug surface is disabled, so they are never reached
//! then.
//!
//! A `global:` target is a variable path (`global:Main.Pumps[2].Speed`): a global, a program's
//! variable, an instance's member at any depth, a structure's field and an array's element with
//! constant indices, resolved at the boundary through the program's tables. The value is an IEC
//! 61131-3 literal of the target's declared type when the platform's program resolves the target
//! when the request arrives ([`VariableQueuePort::with_program_image`],
//! [`typed_value`]); otherwise, and for a text of the untyped grammar that the declared type
//! refuses, the untyped grammar ([`parse_value`]) applies and the boundary refuses what the
//! variable does not take, as it always has.
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
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::memory::{member_place, retained_place};
use trust_runtime_core::native::reflect::{resolve_for_write, PathError};
use trust_runtime_core::value::{format_user_value, Value};

use super::queues::{ForcedSnapshot, ForcedVarTarget, PendingVarTarget, RequestQueueFull};
use super::value_grammar::{parse_value, typed_value};
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

    /// Runs `read` on the image of the program the requests write, when the platform has one at
    /// hand, and returns whether it did: `set` and `var.force` resolve their target and its
    /// declared type through its tables, to admit an IEC literal of that type (specification 11,
    /// "Live Values"). A platform without one leaves the value to the untyped grammar, and the
    /// boundary to refuse what the variable does not take.
    fn with_program_image(&self, _read: &mut dyn FnMut(&Image<'_>)) -> bool {
        false
    }
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

/// `set`: queue a write of `value` to a global, a variable path or a retained variable.
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
    let parsed = parse_var_target(&params.target);
    // The value is refused before the target, as it always was; a target that `set` takes types it.
    let value = match &parsed {
        Ok(target @ (VarTarget::Global(_) | VarTarget::Retain(_))) => {
            admitted_value(queues, target, &params.value)
        }
        _ => parse_value(&params.value),
    };
    let value = match value {
        Ok(value) => value,
        Err(error) => return ControlResponse::error(id, error),
    };
    let target = match parsed {
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
    let value = match admitted_value(queues, &target, &params.value) {
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

/// The value of the text `text` for `target` (specification 11, "Live Values"): an IEC literal of
/// the target's declared type when the platform's program resolves the target; otherwise the
/// untyped grammar, whose integer takes its type at the boundary. A text of the untyped grammar
/// that the declared type refuses keeps that grammar, so the boundary refuses it as it always has.
///
/// # Errors
///
/// The refusal of a text that is no value of the target: of a constant or an instance, which no
/// write changes, of a literal the declared type refuses, or of a text that is no literal at all.
fn admitted_value<Q: VariableQueuePort>(
    queues: &Q,
    target: &VarTarget,
    text: &str,
) -> Result<Value, String> {
    let mut typed = None;
    let mut unwritable = None;
    queues.with_program_image(&mut |image| match declared_type(image, target) {
        Ok(Some(type_index)) => typed = Some(typed_value(image, type_index, text)),
        Ok(None) => {}
        Err(reason) => unwritable = Some(reason),
    });
    if let Some(reason) = unwritable {
        return Err(reason);
    }
    match typed {
        Some(Ok(value)) => Ok(value),
        Some(Err(reason)) => parse_value(text).map_err(|_| reason),
        None => parse_value(text),
    }
}

/// The declared type of the variable `target` names in `image`; `None` for a target the program
/// does not resolve, which the boundary refuses.
///
/// # Errors
///
/// The plain reason of a target no write changes: a constant, or an instance.
fn declared_type(image: &Image<'_>, target: &VarTarget) -> Result<Option<u32>, String> {
    Ok(match target {
        VarTarget::Global(path) => match resolve_for_write(image, path, &|_| None) {
            Ok(at) => Some(at.type_index),
            Err(error @ (PathError::Constant(_) | PathError::NoValue(_))) => {
                return Err(format!("{error}"));
            }
            Err(_) => None,
        },
        VarTarget::Retain(label) => retained_place(image, label)
            .ok()
            .map(|(_, type_index)| type_index),
        VarTarget::Instance(instance, name) => member_place(image, *instance, name)
            .ok()
            .map(|(_, type_index)| type_index),
    })
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
