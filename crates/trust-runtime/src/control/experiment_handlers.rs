use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

use super::{parse_value, ControlResponse, ControlState};
use crate::debug::DebugControl;
use crate::io::{validate_process_image_address, IoAddress, IoSnapshot, IoSnapshotValue};
use crate::value::Value;
use trust_hir::TypeId;

#[derive(Clone, Debug, Default)]
pub struct ControlExperimentService {
    active: Arc<Mutex<Option<ExperimentRecord>>>,
}

#[derive(Clone, Debug)]
struct ExperimentRecord {
    id: String,
    source: String,
    kind: String,
    address: IoAddress,
    restoration_value: Value,
    reviewed_scan: u64,
    started_scan: u64,
    started: Instant,
    duration: Duration,
    status: ExperimentStatus,
    stop_reason: Option<&'static str>,
    restoration: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExperimentStatus {
    Running,
    Completed,
    Aborted,
    Failed,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StartParams {
    experiment_id: String,
    source: String,
    kind: String,
    address: String,
    value: JsonValue,
    restoration_value: JsonValue,
    #[serde(default)]
    value_type: Option<String>,
    minimum: f64,
    maximum: f64,
    duration_ms: u64,
    expected_scan: u64,
    stop_on_stale_ms: u64,
    #[serde(default)]
    controller_mode: Option<String>,
    #[serde(default)]
    physical_approval: Option<PhysicalApproval>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PhysicalApproval {
    user_approved: bool,
    topology_reviewed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExperimentIdParams {
    experiment_id: String,
}

pub(super) fn handle_start(
    id: u64,
    params: Option<JsonValue>,
    state: &ControlState,
) -> ControlResponse {
    let params: StartParams = match decode_params(params) {
        Ok(value) => value,
        Err(error) => return ControlResponse::error_with_code(id, error, "invalid_experiment"),
    };
    if params.experiment_id.trim().is_empty()
        || params.duration_ms == 0
        || params.duration_ms > 300_000
        || params.stop_on_stale_ms == 0
        || !params.minimum.is_finite()
        || !params.maximum.is_finite()
        || params.minimum >= params.maximum
    {
        return error(
            id,
            "invalid experiment duration, freshness, or limits",
            "invalid_experiment",
        );
    }
    if params.source != "simulation" && params.source != "physical" {
        return error(id, "unsupported experiment source", "invalid_experiment");
    }
    if params.kind != "setpoint_step" && params.kind != "output_step" {
        return error(id, "unsupported experiment kind", "invalid_experiment");
    }
    if params.kind == "output_step" && params.controller_mode.as_deref() != Some("Manual") {
        return error(
            id,
            "output steps require confirmed Manual controller mode",
            "controller_mode_blocked",
        );
    }
    if params.source == "physical"
        && !params
            .physical_approval
            .as_ref()
            .is_some_and(|approval| approval.user_approved && approval.topology_reviewed)
    {
        return error(
            id,
            "physical experiment requires immediate approval and a reviewed topology",
            "physical_approval_required",
        );
    }

    let address = match IoAddress::parse(&params.address) {
        Ok(address) => address,
        Err(parse_error) => return error(id, parse_error.to_string(), "invalid_address"),
    };
    if let Err(error_value) = validate_process_image_address(&address) {
        return error(id, error_value.to_string(), "invalid_address");
    }
    let declared_type = match params
        .value_type
        .as_deref()
        .map(experiment_type_id)
        .transpose()
    {
        Ok(value_type) => value_type,
        Err(message) => return error(id, message, "invalid_value"),
    };
    let value = match experiment_value(&params.value, params.value_type.as_deref()) {
        Ok(value) => value,
        Err(message) => return error(id, message, "invalid_value"),
    };
    let restoration_value =
        match experiment_value(&params.restoration_value, params.value_type.as_deref()) {
            Ok(value) => value,
            Err(message) => return error(id, message, "invalid_value"),
        };
    let Some(requested) = numeric_value(&value).filter(|value| value.is_finite()) else {
        return error(
            id,
            "experiment value must be finite and numeric",
            "invalid_value",
        );
    };
    if requested < params.minimum || requested > params.maximum {
        return error(
            id,
            "requested value is outside declared limits",
            "outside_limits",
        );
    }

    let snapshot = match state.io_snapshot.lock() {
        Ok(snapshot) => match snapshot.clone() {
            Some(snapshot) => snapshot,
            None => return error(id, "no current I/O snapshot", "io_unavailable"),
        },
        Err(_) => return error(id, "I/O snapshot unavailable", "io_unavailable"),
    };
    let snapshot_seen_ms = state
        .io_snapshot_seen_ms
        .load(std::sync::atomic::Ordering::Relaxed);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);
    if snapshot.scan.is_none()
        || snapshot_seen_ms == 0
        || now_ms.saturating_sub(snapshot_seen_ms) > params.stop_on_stale_ms
    {
        return error(id, "runtime I/O snapshot is stale", "stale_scan");
    }
    if snapshot.forced.iter().any(|forced| forced == &address) {
        return error(id, "experiment signal is forced", "forced_signal");
    }
    let snapshot_type = snapshot
        .inputs
        .iter()
        .chain(&snapshot.outputs)
        .chain(&snapshot.memory)
        .find(|entry| entry.address == address)
        .and_then(|entry| entry.value_type);
    if declared_type.is_some() && snapshot_type != declared_type {
        return error(
            id,
            "declared experiment type does not match the runtime binding",
            "incompatible_binding",
        );
    }
    if snapshot_value(&snapshot, &address) != Some(&restoration_value) {
        return error(
            id,
            "restoration value does not match the current runtime scan",
            "restoration_mismatch",
        );
    }

    let (write_value, restoration_write_value) = match declared_type {
        Some(value_type) => {
            let write_value = match crate::io::coerce_to_io(value, value_type, address.size) {
                Ok(value) => value,
                Err(error_value) => {
                    return error(id, error_value.to_string(), "incompatible_binding")
                }
            };
            let restoration_write_value =
                match crate::io::coerce_to_io(restoration_value, value_type, address.size) {
                    Ok(value) => value,
                    Err(error_value) => {
                        return error(id, error_value.to_string(), "incompatible_binding")
                    }
                };
            (write_value, restoration_write_value)
        }
        None => (value, restoration_value),
    };

    let record = ExperimentRecord {
        id: params.experiment_id.clone(),
        source: params.source,
        kind: params.kind,
        address: address.clone(),
        restoration_value: restoration_write_value,
        reviewed_scan: params.expected_scan,
        started_scan: snapshot.scan.expect("fresh snapshot has a scan"),
        started: Instant::now(),
        duration: Duration::from_millis(params.duration_ms),
        status: ExperimentStatus::Running,
        stop_reason: None,
        restoration: "pending",
    };
    {
        let mut active = match state.experiments.active.lock() {
            Ok(active) => active,
            Err(_) => {
                return error(
                    id,
                    "experiment state unavailable",
                    "experiment_state_unavailable",
                )
            }
        };
        if active
            .as_ref()
            .is_some_and(|current| current.status == ExperimentStatus::Running)
        {
            return error(
                id,
                "another experiment is already running",
                "experiment_active",
            );
        }
        *active = Some(record);
    }

    state.debug.enqueue_io_write(address, write_value);
    spawn_guard(
        state.experiments.clone(),
        state.debug.clone(),
        state.io_snapshot.clone(),
        state.io_snapshot_seen_ms.clone(),
        params.experiment_id.clone(),
        Duration::from_millis(params.stop_on_stale_ms),
    );
    ControlResponse::ok(
        id,
        json!({
            "experiment_id": params.experiment_id,
            "status": "running",
            "restoration": "pending"
        }),
    )
}

pub(super) fn handle_status(
    id: u64,
    params: Option<JsonValue>,
    state: &ControlState,
) -> ControlResponse {
    let params: ExperimentIdParams = match decode_params(params) {
        Ok(value) => value,
        Err(message) => return error(id, message, "invalid_experiment"),
    };
    let active = match state.experiments.active.lock() {
        Ok(active) => active,
        Err(_) => {
            return error(
                id,
                "experiment state unavailable",
                "experiment_state_unavailable",
            )
        }
    };
    let Some(record) = active
        .as_ref()
        .filter(|record| record.id == params.experiment_id)
    else {
        return error(id, "experiment was not found", "experiment_not_found");
    };
    ControlResponse::ok(id, record_json(record))
}

pub(super) fn handle_abort(
    id: u64,
    params: Option<JsonValue>,
    state: &ControlState,
) -> ControlResponse {
    let params: ExperimentIdParams = match decode_params(params) {
        Ok(value) => value,
        Err(message) => return error(id, message, "invalid_experiment"),
    };
    match finish(
        &state.experiments,
        &state.debug,
        params.experiment_id.as_str(),
        ExperimentStatus::Aborted,
        "user_abort",
    ) {
        Ok(result) => ControlResponse::ok(id, result),
        Err((message, code)) => error(id, message, code),
    }
}

fn spawn_guard(
    service: ControlExperimentService,
    debug: DebugControl,
    snapshot: Arc<Mutex<Option<IoSnapshot>>>,
    snapshot_seen_ms: Arc<std::sync::atomic::AtomicU64>,
    experiment_id: String,
    stale_after: Duration,
) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(20));
        let (elapsed, duration, address) = match service.active.lock() {
            Ok(active) => match active.as_ref().filter(|record| {
                record.id == experiment_id && record.status == ExperimentStatus::Running
            }) {
                Some(record) => (
                    record.started.elapsed(),
                    record.duration,
                    record.address.clone(),
                ),
                None => return,
            },
            Err(_) => return,
        };
        if elapsed >= duration {
            let _ = finish(
                &service,
                &debug,
                &experiment_id,
                ExperimentStatus::Completed,
                "duration_complete",
            );
            return;
        }
        let forced = snapshot
            .lock()
            .ok()
            .and_then(|snapshot| snapshot.clone())
            .is_some_and(|snapshot| snapshot.forced.iter().any(|forced| forced == &address));
        if forced {
            let _ = finish(
                &service,
                &debug,
                &experiment_id,
                ExperimentStatus::Failed,
                "signal_forced",
            );
            return;
        }
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let seen_ms = snapshot_seen_ms.load(std::sync::atomic::Ordering::Relaxed);
        if seen_ms > 0 && now_ms.saturating_sub(seen_ms) > stale_after.as_millis() as u64 {
            let _ = finish(
                &service,
                &debug,
                &experiment_id,
                ExperimentStatus::Failed,
                "stale_runtime_data",
            );
            return;
        }
    });
}

fn finish(
    service: &ControlExperimentService,
    debug: &DebugControl,
    experiment_id: &str,
    status: ExperimentStatus,
    stop_reason: &'static str,
) -> Result<JsonValue, (String, &'static str)> {
    let mut active = service.active.lock().map_err(|_| {
        (
            "experiment state unavailable".to_string(),
            "experiment_state_unavailable",
        )
    })?;
    let record = active
        .as_mut()
        .filter(|record| record.id == experiment_id)
        .ok_or_else(|| {
            (
                "experiment was not found".to_string(),
                "experiment_not_found",
            )
        })?;
    if record.status != ExperimentStatus::Running {
        return Ok(record_json(record));
    }
    // Queue restoration before publishing a terminal state to observers.
    debug.enqueue_io_write(record.address.clone(), record.restoration_value.clone());
    record.status = status;
    record.stop_reason = Some(stop_reason);
    record.restoration = "restored";
    Ok(record_json(record))
}

fn record_json(record: &ExperimentRecord) -> JsonValue {
    json!({
        "experiment_id": record.id,
        "source": record.source,
        "kind": record.kind,
        "status": match record.status {
            ExperimentStatus::Running => "running",
            ExperimentStatus::Completed => "completed",
            ExperimentStatus::Aborted => "aborted",
            ExperimentStatus::Failed => "failed",
        },
        "elapsed_ms": record.started.elapsed().as_millis() as u64,
        "duration_ms": record.duration.as_millis() as u64,
        "reviewed_scan": record.reviewed_scan,
        "started_scan": record.started_scan,
        "stop_reason": record.stop_reason,
        "restoration": record.restoration,
    })
}

fn snapshot_value<'a>(snapshot: &'a IoSnapshot, address: &IoAddress) -> Option<&'a Value> {
    snapshot
        .inputs
        .iter()
        .chain(&snapshot.outputs)
        .chain(&snapshot.memory)
        .find(|entry| &entry.address == address)
        .and_then(|entry| match &entry.value {
            IoSnapshotValue::Value(value) => Some(value),
            IoSnapshotValue::Error(_) | IoSnapshotValue::Unresolved => None,
        })
}

fn experiment_value(value: &JsonValue, value_type: Option<&str>) -> Result<Value, String> {
    if let Some(value_type) = value_type {
        return typed_numeric_value(value, value_type);
    }
    match value {
        JsonValue::Number(number) if number.is_i64() => {
            Ok(Value::LInt(number.as_i64().expect("checked i64")))
        }
        JsonValue::Number(number) => number
            .as_f64()
            .filter(|value| value.is_finite())
            .map(Value::LReal)
            .ok_or_else(|| "experiment value must be finite".to_string()),
        JsonValue::Bool(value) => Ok(Value::Bool(*value)),
        JsonValue::String(value) => {
            parse_value(value).map_err(|error_value| error_value.to_string())
        }
        _ => Err("experiment value must be numeric or boolean".to_string()),
    }
}

fn typed_numeric_value(value: &JsonValue, value_type: &str) -> Result<Value, String> {
    let invalid = || {
        format!(
            "experiment value is not a valid {}",
            value_type.to_ascii_uppercase()
        )
    };
    let signed = || value.as_i64().ok_or_else(&invalid);
    let unsigned = || value.as_u64().ok_or_else(&invalid);
    let real = || {
        value
            .as_f64()
            .filter(|number| number.is_finite())
            .ok_or_else(&invalid)
    };
    match value_type.trim().to_ascii_uppercase().as_str() {
        "SINT" => i8::try_from(signed()?)
            .map(Value::SInt)
            .map_err(|_| invalid()),
        "INT" => i16::try_from(signed()?)
            .map(Value::Int)
            .map_err(|_| invalid()),
        "DINT" => i32::try_from(signed()?)
            .map(Value::DInt)
            .map_err(|_| invalid()),
        "LINT" => signed().map(Value::LInt),
        "USINT" => u8::try_from(unsigned()?)
            .map(Value::USInt)
            .map_err(|_| invalid()),
        "UINT" => u16::try_from(unsigned()?)
            .map(Value::UInt)
            .map_err(|_| invalid()),
        "UDINT" => u32::try_from(unsigned()?)
            .map(Value::UDInt)
            .map_err(|_| invalid()),
        "ULINT" => unsigned().map(Value::ULInt),
        "REAL" => {
            let number = real()?;
            let narrowed = number as f32;
            narrowed
                .is_finite()
                .then_some(Value::Real(narrowed))
                .ok_or_else(invalid)
        }
        "LREAL" => real().map(Value::LReal),
        _ => Err(format!("unsupported experiment value type '{value_type}'")),
    }
}

fn experiment_type_id(value_type: &str) -> Result<TypeId, String> {
    match value_type.trim().to_ascii_uppercase().as_str() {
        "SINT" => Ok(TypeId::SINT),
        "INT" => Ok(TypeId::INT),
        "DINT" => Ok(TypeId::DINT),
        "LINT" => Ok(TypeId::LINT),
        "USINT" => Ok(TypeId::USINT),
        "UINT" => Ok(TypeId::UINT),
        "UDINT" => Ok(TypeId::UDINT),
        "ULINT" => Ok(TypeId::ULINT),
        "REAL" => Ok(TypeId::REAL),
        "LREAL" => Ok(TypeId::LREAL),
        _ => Err(format!("unsupported experiment value type '{value_type}'")),
    }
}

fn numeric_value(value: &Value) -> Option<f64> {
    match value {
        Value::SInt(value) => Some(f64::from(*value)),
        Value::Int(value) => Some(f64::from(*value)),
        Value::DInt(value) => Some(f64::from(*value)),
        Value::LInt(value) => Some(*value as f64),
        Value::USInt(value) => Some(f64::from(*value)),
        Value::UInt(value) => Some(f64::from(*value)),
        Value::UDInt(value) => Some(f64::from(*value)),
        Value::ULInt(value) => Some(*value as f64),
        Value::Real(value) => Some(f64::from(*value)),
        Value::LReal(value) => Some(*value),
        _ => None,
    }
}

fn decode_params<T: for<'de> Deserialize<'de>>(params: Option<JsonValue>) -> Result<T, String> {
    serde_json::from_value(params.ok_or_else(|| "missing params".to_string())?)
        .map_err(|error_value| format!("invalid params: {error_value}"))
}

fn error(id: u64, message: impl Into<String>, code: &'static str) -> ControlResponse {
    ControlResponse::error_with_code(id, message.into(), code)
}
