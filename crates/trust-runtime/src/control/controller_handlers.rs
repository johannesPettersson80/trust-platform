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
    match current_record(state, &params.instance_path) {
        Ok(record) => ControlResponse::ok(
            id,
            json!({
                "active_revision": record.active_revision,
                "parameters": parameter_json(&record.active),
                "pending_revision": record.pending.as_ref().map(|pending| pending.revision.as_str()),
                "confirmed": record.pending.is_none(),
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
    if params.source == "physical" && !params.physical_approved {
        return ControlResponse::error_with_code(
            id,
            "physical apply requires separate immediate approval".into(),
            "physical_approval_required",
        );
    }
    let current = match current_record(state, &params.instance_path) {
        Ok(value) => value,
        Err(error) => return ControlResponse::error_with_code(id, error, "controller_unavailable"),
    };
    if current.pending.is_some() {
        return ControlResponse::error_with_code(
            id,
            "a controller write is awaiting scan confirmation".into(),
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
    state
        .debug
        .enqueue_instance_write(current.instance_id, "ParametersValue", candidate.clone());
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
            },
        );
    ControlResponse::ok(
        id,
        json!({ "previous_revision": current.active_revision, "pending_revision": pending_revision }),
    )
}
fn rollback(id: u64, params: Option<JsonValue>, state: &ControlState) -> ControlResponse {
    let params: RollbackParams = match decode(params) {
        Ok(value) => value,
        Err(error) => return invalid(id, error),
    };
    let current = match current_record(state, &params.instance_path) {
        Ok(value) => value,
        Err(error) => return ControlResponse::error_with_code(id, error, "controller_unavailable"),
    };
    if current.pending.is_some() {
        return ControlResponse::error_with_code(
            id,
            "a controller write is awaiting scan confirmation".into(),
            "controller_write_pending",
        );
    }
    let mut records = state
        .controllers
        .records
        .lock()
        .expect("controller state poisoned");
    let Some(record) = records.get_mut(&params.instance_path) else {
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
    state
        .debug
        .enqueue_instance_write(record.instance_id, "ParametersValue", value.clone());
    record.pending = Some(PendingChange {
        revision: revision.clone(),
        value,
        completes_rollback: true,
    });
    ControlResponse::ok(id, json!({ "pending_revision": revision }))
}
fn current_record(state: &ControlState, path: &str) -> Result<ControllerRecord, String> {
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
    let snapshot_revision = revision(&active);
    let mut records = state
        .controllers
        .records
        .lock()
        .map_err(|_| "controller state unavailable")?;
    let record = records
        .entry(path.to_owned())
        .or_insert_with(|| ControllerRecord {
            instance_id,
            active_revision: snapshot_revision.clone(),
            active: active.clone(),
            previous: None,
            pending: None,
        });
    if record.instance_id != instance_id {
        *record = ControllerRecord {
            instance_id,
            active_revision: snapshot_revision,
            active,
            previous: None,
            pending: None,
        };
        return Ok(record.clone());
    }
    if let Some(pending) = record.pending.clone() {
        if pending.revision == snapshot_revision {
            record.active_revision = pending.revision;
            record.active = pending.value;
            record.pending = None;
            if pending.completes_rollback {
                record.previous = None;
            }
        } else if record.active_revision != snapshot_revision {
            // An external write won the race. Drop local rollback history rather
            // than claiming that the queued candidate became active.
            record.active_revision = snapshot_revision;
            record.active = active;
            record.previous = None;
            record.pending = None;
        }
    } else if record.active_revision != snapshot_revision {
        record.active_revision = snapshot_revision;
        record.active = active;
        record.previous = None;
    }
    Ok(record.clone())
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
