//! `control.pid.*` (protocol annex E7.7): a `Control.PID`'s parameter record read, written and
//! rolled back. A write is a parameter set over the record's fields that the runtime commits at the
//! boundary of the task that runs the controller and confirms after that task
//! ([`crate::debug::DebugControl::control_pid_apply`]); this service keeps each controller's
//! revisions, its pending write and the revision a rollback returns to.
use super::experiment_handlers::contradicted_source;
use super::{ControlResponse, ControlState};
use crate::debug::{ControlPidOutcome, ControlPidView, ControlPidWrite};
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
    /// The controller instance's symbol in the program image, which a write of its members names
    /// (specification 44 §13.3).
    instance_id: InstanceId,
    /// The runtime epoch the record was taken in: a reload or a restart ends it, and with it the
    /// revision a rollback returns to (E9.3).
    epoch: u64,
    active_revision: String,
    active: Value,
    previous: Option<(String, Value)>,
    pending: Option<PendingChange>,
    /// Why the last write did not take effect: refused at its task's boundary, rejected because the
    /// program changed a field it wrote, or stopped by a reload or restart (E7.4, A6.7); or why a
    /// new program instance dropped it. A new write clears it.
    reason: Option<String>,
}
#[derive(Clone, Debug)]
struct PendingChange {
    /// The identity of the parameter set that carries the write (E7.7), which its outcome names.
    transaction: String,
    revision: String,
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
    // A live parameter write waits for a running experiment, as transfers and parameter sets do. The write is the
    // parameter set of E7.7, checked as a set is (mode, forcing, I/O authority, approval, bounds) and committed at the
    // boundary of the task that runs the controller.
    super::experiment_handlers::answer_live_write_coded(id, state, || {
        let transaction = state
            .debug
            .control_pid_apply(&ControlPidWrite {
                instance: &params.instance_path,
                current: &current.active,
                candidate: &candidate,
                rollback: false,
                physical_target: !simulated_target,
                physical_approved: params.physical_approved,
            })
            .map_err(|reason| (reason, "invalid_controller_request"))?;
        state
            .controllers
            .records
            .lock()
            .expect("controller state poisoned")
            .insert(
                params.instance_path.clone(),
                ControllerRecord {
                    instance_id: current.instance_id,
                    epoch: current.epoch,
                    active_revision: current.active_revision.clone(),
                    active: current.active.clone(),
                    previous: Some((current.active_revision.clone(), current.active.clone())),
                    pending: Some(PendingChange {
                        transaction,
                        revision: pending_revision.clone(),
                        completes_rollback: false,
                    }),
                    reason: None,
                },
            );
        Ok(
            json!({ "previous_revision": current.active_revision, "pending_revision": pending_revision }),
        )
    })
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
    let (revision, value) = {
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
        (revision, value)
    };
    // The experiment lock is taken before the controller records, in the same order as apply. The rollback is the
    // parameter set of E7.7 that writes the retained record back (E7.5).
    super::experiment_handlers::answer_live_write_coded(id, state, || {
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
            .ok_or_else(|| {
                (
                    "controller revision changed before rollback".to_string(),
                    "stale_controller_revision",
                )
            })?;
        let transaction = state
            .debug
            .control_pid_apply(&ControlPidWrite {
                instance: &params.instance_path,
                current: &record.active,
                candidate: &value,
                rollback: true,
                physical_target: !simulated_target,
                physical_approved: params.physical_approved,
            })
            .map_err(|reason| (reason, "invalid_controller_request"))?;
        record.pending = Some(PendingChange {
            transaction,
            revision: revision.clone(),
            completes_rollback: true,
        });
        record.reason = None;
        Ok(json!({ "pending_revision": revision }))
    })
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
fn fresh_record(
    instance_id: InstanceId,
    epoch: u64,
    active: Value,
    reason: Option<String>,
) -> ControllerRecord {
    ControllerRecord {
        instance_id,
        epoch,
        active_revision: revision(&active),
        active,
        previous: None,
        pending: None,
        reason,
    }
}
/// The record the runtime's values imply, `view` being the controller as the last scan left it with the outcome of its
/// last write. A pending write whose set was applied becomes active; one that was refused at its task's boundary,
/// rejected after the task or stopped by a reload or restart is dropped with its reason (E7.4, E7.7, A6.7). A new
/// program instance drops the record, and a new runtime epoch the revision a rollback returns to (E9.3).
fn reconcile(stored: Option<ControllerRecord>, view: ControlPidView) -> ControllerRecord {
    let ControlPidView {
        instance_id,
        record: active,
        outcome,
        epoch,
    } = view;
    let active_revision = revision(&active);
    let Some(mut record) = stored else {
        return fresh_record(instance_id, epoch, active, None);
    };
    if record.instance_id != instance_id {
        return fresh_record(
            instance_id,
            epoch,
            active,
            Some(
                "the program was reloaded or replaced; its pending write and rollback record were dropped".into(),
            ),
        );
    }
    if record.epoch != epoch {
        record.epoch = epoch;
        record.previous = None;
    }
    if let Some(pending) = record.pending.clone() {
        if let Some((_, ended)) =
            outcome.filter(|(transaction, _)| *transaction == pending.transaction)
        {
            record.pending = None;
            match ended {
                ControlPidOutcome::Applied => {
                    record.reason = None;
                    if pending.completes_rollback {
                        record.previous = None;
                    }
                }
                ControlPidOutcome::Rejected(reason) => {
                    record.reason = Some(reason);
                    record.previous = None;
                }
            }
        }
    } else if record.active_revision != active_revision {
        // The program or another writer changed the record: the revision a rollback returns to no longer follows it.
        record.previous = None;
    }
    record.active_revision = active_revision;
    record.active = active;
    record
}
/// The controller record as the runtime's values show it. Nothing is stored (Specification 35, E0.1).
fn projected_record(state: &ControlState, path: &str) -> Result<ControllerRecord, String> {
    let view = state.debug.control_pid_view(path)?;
    let records = state
        .controllers
        .records
        .lock()
        .map_err(|_| "controller state unavailable")?;
    Ok(reconcile(records.get(path).cloned(), view))
}
/// The controller record as the runtime's values show it, stored for the apply or rollback that asked for it.
fn stored_record(state: &ControlState, path: &str) -> Result<ControllerRecord, String> {
    let view = state.debug.control_pid_view(path)?;
    let mut records = state
        .controllers
        .records
        .lock()
        .map_err(|_| "controller state unavailable")?;
    let record = reconcile(records.get(path).cloned(), view);
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
        "sha256:{}",
        hex::encode(Sha256::digest(format!("{value:?}").as_bytes()))
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
