use super::experiment_handlers::contradicted_source;
use super::{ControlResponse, ControlState};
use crate::memory::InstanceId;
use crate::value::{EnumValue, Value};
use serde::Deserialize;
use serde_json::{json, Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub struct ControllerParameterService {
    records: Arc<Mutex<HashMap<String, ControllerRecord>>>,
}
#[derive(Clone, Debug)]
struct ControllerRecord {
    instance_id: InstanceId,
    active_revision: String,
    active: Value,
    previous: Option<(String, Value)>,
    pending: Option<PendingChange>,
    /// Why the last pending write was dropped: superseded by another write, or interrupted by a new program instance
    /// (Specification 35, A6.7). A new write clears it.
    reason: Option<String>,
}
#[derive(Clone, Debug)]
struct PendingChange {
    revision: String,
    value: Value,
    completes_rollback: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusParams {
    instance_path: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyParams {
    instance_path: String,
    source: String,
    expected_revision: String,
    parameters: Map<String, JsonValue>,
    physical_approved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RollbackParams {
    instance_path: String,
    expected_active_revision: String,
    previous_revision: String,
    /// Required on a physical target (Specification 35, A2.5). The protocol annex's rollback form has no approval field,
    /// but Part A requires one, and Part A governs.
    #[serde(default)]
    physical_approved: bool,
}

pub(super) fn dispatch(
    id: u64,
    operation: &str,
    params: Option<JsonValue>,
    state: &ControlState,
) -> Option<ControlResponse> {
    Some(match operation {
        "control.pid.status" => status(id, params, state),
        "control.pid.apply" => apply(id, params, state),
        "control.pid.rollback" => rollback(id, params, state),
        _ => return None,
    })
}
fn status(id: u64, params: Option<JsonValue>, state: &ControlState) -> ControlResponse {
    let params: StatusParams = match decode(params) {
        Ok(value) => value,
        Err(error) => return invalid(id, error),
    };
    // A status request reads the record and stores nothing, so no pending or rollback record is cleared (E0.1).
    match projected_record(state, &params.instance_path) {
        Ok(record) => ControlResponse::ok(
            id,
            json!({
                "active_revision": record.active_revision,
                "parameters": parameter_json(&record.active),
                "pending_revision": record.pending.as_ref().map(|pending| pending.revision.as_str()),
                "confirmed": record.pending.is_none(),
                "reason": record.reason,
            }),
        ),
        Err(error) => ControlResponse::error_with_code(id, error, "controller_unavailable"),
    }
}
fn apply(id: u64, params: Option<JsonValue>, state: &ControlState) -> ControlResponse {
    let params: ApplyParams = match decode(params) {
        Ok(value) => value,
        Err(error) => return invalid(id, error),
    };
    if params.source != "simulation" && params.source != "physical" {
        return invalid(id, "source must be simulation or physical".into());
    }
    let simulated_target = match target_class(id, state) {
        Ok(simulated) => simulated,
        Err(response) => return response,
    };
    if let Some(message) = contradicted_source(&params.source, simulated_target) {
        return ControlResponse::error_with_code(id, message, "invalid_target");
    }
    if !simulated_target && !params.physical_approved {
        return ControlResponse::error_with_code(
            id,
            "physical apply requires separate immediate approval".into(),
            "physical_approval_required",
        );
    }
    let current = match stored_record(state, &params.instance_path) {
        Ok(value) => value,
        Err(error) => return ControlResponse::error_with_code(id, error, "controller_unavailable"),
    };
    if let Some(pending) = current.pending.as_ref() {
        return ControlResponse::error_with_code(
            id,
            format!(
                "a controller write to {} (revision {}) is pending",
                params.instance_path, pending.revision
            ),
            "controller_write_pending",
        );
    }
    if current.active_revision != params.expected_revision {
        return ControlResponse::error_with_code(
            id,
            "controller revision changed before apply".into(),
            "stale_controller_revision",
        );
    }
    let candidate = match replace_parameters(&current.active, &params.parameters) {
        Ok(value) => value,
        Err(error) => return invalid(id, error),
    };
    let pending_revision = revision(&candidate);
    // A transfer or parameter transaction that is pending owns the controller's values (A6.6).
    if let Some(label) = state.debug.pending_transaction_label() {
        return ControlResponse::error_with_code(
            id,
            format!("{label} is pending; a controller write waits for it"),
            "transaction_pending",
        );
    }
    // A live parameter write waits for a running experiment, as transfers and parameter sets do.
    super::experiment_handlers::answer_live_write(
        id,
        state,
        || {
            state.debug.enqueue_instance_write(
                current.instance_id,
                "ParametersValue",
                candidate.clone(),
            );
            state
                .controllers
                .records
                .lock()
                .expect("controller state poisoned")
                .insert(
                    params.instance_path,
                    ControllerRecord {
                        instance_id: current.instance_id,
                        active_revision: current.active_revision.clone(),
                        active: current.active.clone(),
                        previous: Some((current.active_revision.clone(), current.active)),
                        pending: Some(PendingChange {
                            revision: pending_revision.clone(),
                            value: candidate,
                            completes_rollback: false,
                        }),
                        reason: None,
                    },
                );
            Ok(
                json!({ "previous_revision": current.active_revision, "pending_revision": pending_revision }),
            )
        },
        "controller_unavailable",
    )
}
fn rollback(id: u64, params: Option<JsonValue>, state: &ControlState) -> ControlResponse {
    let params: RollbackParams = match decode(params) {
        Ok(value) => value,
        Err(error) => return invalid(id, error),
    };
    let simulated_target = match target_class(id, state) {
        Ok(simulated) => simulated,
        Err(response) => return response,
    };
    if !simulated_target && !params.physical_approved {
        return ControlResponse::error_with_code(
            id,
            "physical rollback requires separate immediate approval".into(),
            "physical_approval_required",
        );
    }
    let current = match stored_record(state, &params.instance_path) {
        Ok(value) => value,
        Err(error) => return ControlResponse::error_with_code(id, error, "controller_unavailable"),
    };
    if let Some(pending) = current.pending.as_ref() {
        return ControlResponse::error_with_code(
            id,
            format!(
                "a controller write to {} (revision {}) is pending",
                params.instance_path, pending.revision
            ),
            "controller_write_pending",
        );
    }
    if let Some(label) = state.debug.pending_transaction_label() {
        return ControlResponse::error_with_code(
            id,
            format!("{label} is pending; a controller write waits for it"),
            "transaction_pending",
        );
    }
    let (instance_id, revision, value) = {
        let records = state
            .controllers
            .records
            .lock()
            .expect("controller state poisoned");
        let Some(record) = records.get(&params.instance_path) else {
            return ControlResponse::error_with_code(
                id,
                "no applied controller is available for rollback".into(),
                "rollback_unavailable",
            );
        };
        if record.active_revision != params.expected_active_revision {
            return ControlResponse::error_with_code(
                id,
                "controller revision changed before rollback".into(),
                "stale_controller_revision",
            );
        }
        let Some((revision, value)) = record.previous.clone() else {
            return ControlResponse::error_with_code(
                id,
                "no previous controller revision".into(),
                "rollback_unavailable",
            );
        };
        if revision != params.previous_revision {
            return ControlResponse::error_with_code(
                id,
                "previous revision does not match".into(),
                "rollback_revision_mismatch",
            );
        }
        (record.instance_id, revision, value)
    };
    // The experiment lock is taken before the controller records, in the same order as apply.
    super::experiment_handlers::answer_live_write(
        id,
        state,
        || {
            let mut records = state
                .controllers
                .records
                .lock()
                .expect("controller state poisoned");
            let record = records
                .get_mut(&params.instance_path)
                .filter(|record| {
                    record.active_revision == params.expected_active_revision
                        && record.pending.is_none()
                })
                .ok_or_else(|| "controller revision changed before rollback".to_string())?;
            state
                .debug
                .enqueue_instance_write(instance_id, "ParametersValue", value.clone());
            record.pending = Some(PendingChange {
                revision: revision.clone(),
                value,
                completes_rollback: true,
            });
            record.reason = None;
            Ok(json!({ "pending_revision": revision }))
        },
        "stale_controller_revision",
    )
}
/// The target's class from the runtime's latest scan (Specification 35, A2.4). Before the first scan the runtime's target is
/// not known, so no controller write is served.
fn target_class(id: u64, state: &ControlState) -> Result<bool, ControlResponse> {
    state.experiments.simulated_target().ok_or_else(|| {
        ControlResponse::error_with_code(
            id,
            "no completed runtime scan yet; the runtime's target is not known".into(),
            "controller_unavailable",
        )
    })
}
/// Reads the instance at `path` and its current `ParametersValue` from the runtime snapshot.
fn read_controller(state: &ControlState, path: &str) -> Result<(InstanceId, Value), String> {
    let snapshot = state
        .debug
        .snapshot()
        .ok_or("runtime snapshot unavailable")?;
    let mut parts = path.split('.');
    let root = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or("instance path is empty")?;
    let Value::Instance(mut instance_id) = snapshot
        .storage
        .get_global(root)
        .ok_or("program instance was not found")?
    else {
        return Err("program binding is not an instance".into());
    };
    for part in parts {
        let Value::Instance(next) = snapshot
            .storage
            .get_instance_var_recursive(instance_id, part)
            .ok_or("controller instance was not found")?
        else {
            return Err("controller path does not resolve to an instance".into());
        };
        instance_id = *next;
    }
    let active = snapshot
        .storage
        .get_instance_var_recursive(instance_id, "ParametersValue")
        .cloned()
        .ok_or("Control.PID ParametersValue is unavailable")?;
    Ok((instance_id, active))
}
fn fresh_record(
    instance_id: InstanceId,
    active: Value,
    reason: Option<String>,
) -> ControllerRecord {
    ControllerRecord {
        instance_id,
        active_revision: revision(&active),
        active,
        previous: None,
        pending: None,
        reason,
    }
}
/// The record the runtime's values imply. A pending write that the values confirm becomes active. A pending write that
/// another write or a new program instance superseded is dropped, with the reason kept for status (Specification 35, A6.7).
fn reconcile(
    stored: Option<ControllerRecord>,
    instance_id: InstanceId,
    active: Value,
) -> ControllerRecord {
    let snapshot_revision = revision(&active);
    let Some(mut record) = stored else {
        return fresh_record(instance_id, active, None);
    };
    if record.instance_id != instance_id {
        return fresh_record(
            instance_id,
            active,
            Some(
                "the program was reloaded or replaced; its pending write and rollback record were dropped".into(),
            ),
        );
    }
    if let Some(pending) = record.pending.clone() {
        if pending.revision == snapshot_revision {
            record.active_revision = pending.revision;
            record.active = pending.value;
            record.pending = None;
            record.reason = None;
            if pending.completes_rollback {
                record.previous = None;
            }
        } else if record.active_revision != snapshot_revision {
            // An external write won the race. The queued candidate did not become active, so it is dropped and says why.
            return fresh_record(
                instance_id,
                active,
                Some(
                    "an external write changed the controller before the queued write was applied; the queued write was dropped"
                        .into(),
                ),
            );
        }
    } else if record.active_revision != snapshot_revision {
        record.active_revision = snapshot_revision;
        record.active = active;
        record.previous = None;
    }
    record
}
/// The controller record as the runtime's values show it. Nothing is stored (Specification 35, E0.1).
fn projected_record(state: &ControlState, path: &str) -> Result<ControllerRecord, String> {
    let (instance_id, active) = read_controller(state, path)?;
    let records = state
        .controllers
        .records
        .lock()
        .map_err(|_| "controller state unavailable")?;
    Ok(reconcile(records.get(path).cloned(), instance_id, active))
}
/// The controller record as the runtime's values show it, stored for the apply or rollback that asked for it.
fn stored_record(state: &ControlState, path: &str) -> Result<ControllerRecord, String> {
    let (instance_id, active) = read_controller(state, path)?;
    let mut records = state
        .controllers
        .records
        .lock()
        .map_err(|_| "controller state unavailable")?;
    let record = reconcile(records.get(path).cloned(), instance_id, active);
    records.insert(path.to_owned(), record.clone());
    Ok(record)
}
fn replace_parameters(
    current: &Value,
    parameters: &Map<String, JsonValue>,
) -> Result<Value, String> {
    let Value::Struct(value) = current else {
        return Err("Control.PID ParametersValue is not a structure".into());
    };
    let mut updated = (**value).clone();
    for (name, json_value) in parameters {
        let current = updated
            .field(name)
            .ok_or_else(|| format!("unknown Control.PID parameter '{name}'"))?;
        let replacement = match current {
            Value::LReal(_) => Value::LReal(
                json_value
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| format!("parameter '{name}' must be finite"))?,
            ),
            Value::Enum(value) if name == "AntiWindup" => {
                let variant = json_value
                    .as_str()
                    .ok_or("AntiWindup must be BackCalculation or Combined")?;
                let numeric = match variant {
                    "BackCalculation" => 0,
                    "Combined" => 1,
                    _ => return Err("AntiWindup must be BackCalculation or Combined".into()),
                };
                Value::Enum(Box::new(EnumValue::from_canonical_parts(
                    value.type_name().clone(),
                    variant.into(),
                    numeric,
                )))
            }
            _ => return Err(format!("unsupported Control.PID parameter '{name}'")),
        };
        updated.set_existing_field(name.as_str().into(), replacement);
    }
    Ok(Value::Struct(Arc::new(updated)))
}
fn revision(value: &Value) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(format!("{value:?}").as_bytes())
    )
}
fn parameter_json(value: &Value) -> JsonValue {
    let Value::Struct(value) = value else {
        return JsonValue::Null;
    };
    JsonValue::Object(
        value
            .fields()
            .iter()
            .filter_map(|(name, value)| match value {
                Value::LReal(number) => Some((name.to_string(), json!(number))),
                Value::Enum(value) => {
                    Some((name.to_string(), json!(value.variant_name().as_str())))
                }
                _ => None,
            })
            .collect(),
    )
}
fn decode<T: for<'de> Deserialize<'de>>(params: Option<JsonValue>) -> Result<T, String> {
    serde_json::from_value(params.ok_or("missing params")?)
        .map_err(|error| format!("invalid params: {error}"))
}
fn invalid(id: u64, message: String) -> ControlResponse {
    ControlResponse::error_with_code(id, message, "invalid_controller_request")
}
