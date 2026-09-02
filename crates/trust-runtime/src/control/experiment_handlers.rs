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
    restoration_observed_value: Value,
    reviewed_scan: u64,
    started_scan: u64,
    started: Instant,
    duration: Duration,
    status: ExperimentStatus,
    stop_reason: Option<&'static str>,
    restoration: &'static str,
    restoration_reason: Option<&'static str>,
    restoration_deadline: Option<Instant>,
    restoration_timeout: Duration,
    minimum: f64,
    maximum: f64,
    trajectory: Vec<(Duration, Value)>,
    next_trajectory: usize,
    relay_feedback: Option<RelayFeedbackRecord>,
    write_enabled: bool,
}

#[derive(Clone, Debug)]
struct RelayFeedbackRecord {
    address: IoAddress,
    address_text: String,
    value_type: TypeId,
    center: f64,
    hysteresis: f64,
    low_value: Value,
    high_value: Value,
    low_numeric: f64,
    high_numeric: f64,
    high_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExperimentStatus {
    Running,
    Completed,
    Aborted,
    Failed,
    FailedToRestore,
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
    #[serde(default)]
    trajectory: Vec<TrajectoryPoint>,
    #[serde(default)]
    relay_feedback: Option<RelayFeedbackParams>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrajectoryPoint {
    at_ms: u64,
    value: JsonValue,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelayFeedbackParams {
    address: String,
    value_type: String,
    center: f64,
    hysteresis: f64,
    low_value: JsonValue,
    high_value: JsonValue,
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
        || params.duration_ms > 3_600_000
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
    if ![
        "observation",
        "setpoint_step",
        "output_step",
        "load_step",
        "mv_step",
        "mv_pulse",
        "relay",
        "prbs",
        "multisine",
        "project_sequence",
    ]
    .contains(&params.kind.as_str())
    {
        return error(id, "unsupported experiment kind", "invalid_experiment");
    }
    if [
        "output_step",
        "mv_step",
        "mv_pulse",
        "relay",
        "prbs",
        "multisine",
    ]
    .contains(&params.kind.as_str())
        && params.controller_mode.as_deref() != Some("Manual")
    {
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
    // An observation never writes this signal, so a changing PV between review
    // and start is normal plant behavior rather than a restoration conflict.
    if params.kind != "observation"
        && snapshot_value(&snapshot, &address) != Some(&restoration_value)
    {
        return error(
            id,
            "restoration value does not match the current runtime scan",
            "restoration_mismatch",
        );
    }

    let restoration_observed_value = restoration_value.clone();
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
    let write_enabled = params.kind != "observation";
    if (params.kind == "relay") != params.relay_feedback.is_some() {
        return error(
            id,
            "relay experiments require relay feedback and other experiments forbid it",
            "invalid_experiment",
        );
    }
    if params.kind == "relay" && !params.trajectory.is_empty() {
        return error(
            id,
            "feedback relay experiments do not accept a timed trajectory",
            "invalid_experiment",
        );
    }
    let relay_feedback = match params.relay_feedback.as_ref() {
        Some(relay) => match decode_relay_feedback(
            relay,
            &snapshot,
            params.minimum,
            params.maximum,
            params.value_type.as_deref(),
            declared_type,
            address.size,
            requested,
        ) {
            Ok(relay) => Some(relay),
            Err((message, code)) => return error(id, message, code),
        },
        None => None,
    };
    let trajectory = if let Some(relay) = relay_feedback.as_ref() {
        vec![(Duration::ZERO, relay.high_value.clone())]
    } else {
        match decode_trajectory(
            params.trajectory,
            params.duration_ms,
            params.minimum,
            params.maximum,
            params.value_type.as_deref(),
            declared_type,
            address.size,
        ) {
            Ok(values) if values.is_empty() && write_enabled => {
                vec![(Duration::ZERO, write_value)]
            }
            Ok(values) => values,
            Err(message) => return error(id, message, "invalid_experiment"),
        }
    };

    let record = ExperimentRecord {
        id: params.experiment_id.clone(),
        source: params.source,
        kind: params.kind,
        address: address.clone(),
        restoration_value: restoration_write_value,
        restoration_observed_value,
        reviewed_scan: params.expected_scan,
        started_scan: snapshot.scan.expect("fresh snapshot has a scan"),
        started: Instant::now(),
        duration: Duration::from_millis(params.duration_ms),
        status: ExperimentStatus::Running,
        stop_reason: None,
        restoration: if write_enabled {
            "pending"
        } else {
            "not_required"
        },
        restoration_reason: None,
        restoration_deadline: None,
        restoration_timeout: Duration::from_millis(params.stop_on_stale_ms),
        minimum: params.minimum,
        maximum: params.maximum,
        trajectory,
        next_trajectory: usize::from(write_enabled),
        relay_feedback,
        write_enabled,
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

    if write_enabled {
        state.debug.enqueue_io_write(
            address,
            record_first_value(&state.experiments, &params.experiment_id),
        );
    }
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
            "restoration": if write_enabled { "pending" } else { "not_required" }
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
        let (elapsed, duration, address, minimum, maximum, relay_feedback) =
            match service.active.lock() {
                Ok(mut active) => {
                    let Some(record) = active.as_mut().filter(|record| record.id == experiment_id)
                    else {
                        return;
                    };
                    if record.status != ExperimentStatus::Running {
                        if record.restoration != "pending" {
                            return;
                        }
                        let confirmed = snapshot
                            .lock()
                            .ok()
                            .and_then(|value| value.clone())
                            .and_then(|value| snapshot_value(&value, &record.address).cloned())
                            .is_some_and(|value| value == record.restoration_observed_value);
                        if confirmed {
                            record.restoration = "restored";
                            record.restoration_deadline = None;
                            return;
                        }
                        if record
                            .restoration_deadline
                            .is_some_and(|deadline| Instant::now() >= deadline)
                        {
                            record.status = ExperimentStatus::FailedToRestore;
                            record.restoration = "failed";
                            record.restoration_reason = Some("confirmation_timeout");
                            record.restoration_deadline = None;
                            return;
                        }
                        continue;
                    }
                    let elapsed = record.started.elapsed();
                    // Keep the experiment lock while queuing due values. Abort takes
                    // the same lock, so restoration can never be followed by a late
                    // excitation write from this trajectory.
                    while record.next_trajectory < record.trajectory.len()
                        && record.trajectory[record.next_trajectory].0 <= elapsed
                    {
                        debug.enqueue_io_write(
                            record.address.clone(),
                            record.trajectory[record.next_trajectory].1.clone(),
                        );
                        record.next_trajectory += 1;
                    }
                    (
                        elapsed,
                        record.duration,
                        record.address.clone(),
                        record.minimum,
                        record.maximum,
                        record.relay_feedback.clone(),
                    )
                }
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
            continue;
        }
        let current_snapshot = snapshot.lock().ok().and_then(|snapshot| snapshot.clone());
        let forced = current_snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.forced.iter().any(|forced| forced == &address));
        if forced {
            let _ = finish(
                &service,
                &debug,
                &experiment_id,
                ExperimentStatus::Failed,
                "signal_forced",
            );
            continue;
        }
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let seen_ms = snapshot_seen_ms.load(std::sync::atomic::Ordering::Relaxed);
        if seen_ms == 0 || now_ms.saturating_sub(seen_ms) > stale_after.as_millis() as u64 {
            let _ = finish(
                &service,
                &debug,
                &experiment_id,
                ExperimentStatus::Failed,
                "stale_runtime_data",
            );
            continue;
        }
        let Some(current_value) = current_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot_value(snapshot, &address))
            .and_then(numeric_value)
            .filter(|value| value.is_finite())
        else {
            let _ = finish(
                &service,
                &debug,
                &experiment_id,
                ExperimentStatus::Failed,
                "bad_runtime_data",
            );
            continue;
        };
        if current_value < minimum || current_value > maximum {
            let _ = finish(
                &service,
                &debug,
                &experiment_id,
                ExperimentStatus::Failed,
                "signal_outside_limits",
            );
            continue;
        }
        let Some(relay) = relay_feedback else {
            continue;
        };
        let relay_forced = current_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot
                .forced
                .iter()
                .any(|forced| forced == &relay.address)
        });
        if relay_forced {
            let _ = finish(
                &service,
                &debug,
                &experiment_id,
                ExperimentStatus::Failed,
                "relay_feedback_forced",
            );
            continue;
        }
        let relay_value = current_snapshot.as_ref().and_then(|snapshot| {
            (snapshot_type(snapshot, &relay.address) == Some(relay.value_type))
                .then(|| snapshot_value(snapshot, &relay.address))
                .flatten()
                .and_then(numeric_value)
                .filter(|value| value.is_finite())
        });
        let Some(relay_value) = relay_value else {
            let _ = finish(
                &service,
                &debug,
                &experiment_id,
                ExperimentStatus::Failed,
                "bad_relay_feedback",
            );
            continue;
        };
        let mut active = match service.active.lock() {
            Ok(active) => active,
            Err(_) => return,
        };
        let Some(record) = active.as_mut().filter(|record| {
            record.id == experiment_id && record.status == ExperimentStatus::Running
        }) else {
            continue;
        };
        let Some(relay) = record.relay_feedback.as_mut() else {
            continue;
        };
        // The relay changes state only after measured PV crosses a hysteresis
        // threshold. Holding the experiment lock prevents an abort restoration
        // from being followed by a late relay write.
        if relay.high_active && relay_value >= relay.center + relay.hysteresis {
            debug.enqueue_io_write(record.address.clone(), relay.low_value.clone());
            relay.high_active = false;
        } else if !relay.high_active && relay_value <= relay.center - relay.hysteresis {
            debug.enqueue_io_write(record.address.clone(), relay.high_value.clone());
            relay.high_active = true;
        }
    });
}

fn record_first_value(service: &ControlExperimentService, experiment_id: &str) -> Value {
    service
        .active
        .lock()
        .ok()
        .and_then(|active| {
            active
                .as_ref()
                .filter(|record| record.id == experiment_id)
                .map(|record| record.trajectory[0].1.clone())
        })
        .expect("new experiment retains its first trajectory value")
}

fn decode_relay_feedback(
    params: &RelayFeedbackParams,
    snapshot: &IoSnapshot,
    minimum: f64,
    maximum: f64,
    output_value_type: Option<&str>,
    output_type: Option<TypeId>,
    output_size: crate::io::IoSize,
    requested: f64,
) -> Result<RelayFeedbackRecord, (String, &'static str)> {
    if !params.center.is_finite() || !params.hysteresis.is_finite() || params.hysteresis <= 0.0 {
        return Err((
            "relay center must be finite and hysteresis must be positive".to_string(),
            "invalid_experiment",
        ));
    }
    let address = IoAddress::parse(&params.address)
        .map_err(|error| (error.to_string(), "invalid_address"))?;
    validate_process_image_address(&address)
        .map_err(|error| (error.to_string(), "invalid_address"))?;
    if snapshot.forced.iter().any(|forced| forced == &address) {
        return Err((
            "relay feedback signal is forced".to_string(),
            "forced_signal",
        ));
    }
    let value_type =
        experiment_type_id(&params.value_type).map_err(|message| (message, "invalid_value"))?;
    if snapshot_type(snapshot, &address) != Some(value_type) {
        return Err((
            "declared relay feedback type does not match the runtime binding".to_string(),
            "incompatible_binding",
        ));
    }
    snapshot_value(snapshot, &address)
        .and_then(numeric_value)
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            (
                "relay feedback must be finite and numeric".to_string(),
                "bad_runtime_data",
            )
        })?;

    let decode_output = |value: &JsonValue| -> Result<(Value, f64), (String, &'static str)> {
        let raw = experiment_value(value, output_value_type)
            .map_err(|message| (message, "invalid_value"))?;
        let numeric = numeric_value(&raw)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                (
                    "relay output must be finite and numeric".to_string(),
                    "invalid_value",
                )
            })?;
        if numeric < minimum || numeric > maximum {
            return Err((
                "relay output is outside declared limits".to_string(),
                "outside_limits",
            ));
        }
        let write_value = match output_type {
            Some(value_type) => crate::io::coerce_to_io(raw, value_type, output_size)
                .map_err(|error| (error.to_string(), "incompatible_binding")),
            None => Ok(raw),
        }?;
        Ok((write_value, numeric))
    };
    let (low_value, low) = decode_output(&params.low_value)?;
    let (high_value, high) = decode_output(&params.high_value)?;
    if low >= high || (requested - high).abs() > f64::EPSILON {
        return Err((
            "relay outputs must be ordered and the requested value must equal the high output"
                .to_string(),
            "invalid_experiment",
        ));
    }
    Ok(RelayFeedbackRecord {
        address,
        address_text: params.address.clone(),
        value_type,
        center: params.center,
        hysteresis: params.hysteresis,
        low_value,
        high_value,
        low_numeric: low,
        high_numeric: high,
        high_active: true,
    })
}

fn decode_trajectory(
    points: Vec<TrajectoryPoint>,
    duration_ms: u64,
    minimum: f64,
    maximum: f64,
    value_type: Option<&str>,
    declared_type: Option<TypeId>,
    address_size: crate::io::IoSize,
) -> Result<Vec<(Duration, Value)>, String> {
    if points.len() > 10_000 {
        return Err("experiment trajectory exceeds 10000 points".to_string());
    }
    let mut previous = None;
    let mut values = Vec::with_capacity(points.len());
    for point in points {
        if point.at_ms >= duration_ms || previous.is_some_and(|last| point.at_ms <= last) {
            return Err(
                "experiment trajectory times must be strictly increasing and before duration"
                    .to_string(),
            );
        }
        if previous.is_none() && point.at_ms != 0 {
            return Err("experiment trajectory must begin at 0 ms".to_string());
        }
        let raw = experiment_value(&point.value, value_type)?;
        let numeric = numeric_value(&raw)
            .filter(|value| value.is_finite())
            .ok_or_else(|| "trajectory value must be finite and numeric".to_string())?;
        if numeric < minimum || numeric > maximum {
            return Err("trajectory value is outside declared limits".to_string());
        }
        let value = match declared_type {
            Some(value_type) => crate::io::coerce_to_io(raw, value_type, address_size)
                .map_err(|error| error.to_string())?,
            None => raw,
        };
        values.push((Duration::from_millis(point.at_ms), value));
        previous = Some(point.at_ms);
    }
    Ok(values)
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
    if record.write_enabled {
        debug.enqueue_io_write(record.address.clone(), record.restoration_value.clone());
    }
    record.status = status;
    record.stop_reason = Some(stop_reason);
    record.restoration = if record.write_enabled {
        "pending"
    } else {
        "not_required"
    };
    record.restoration_deadline = record
        .write_enabled
        .then(|| Instant::now() + record.restoration_timeout);
    Ok(record_json(record))
}

fn record_json(record: &ExperimentRecord) -> JsonValue {
    let relay_feedback = record.relay_feedback.as_ref().map(|relay| {
        json!({
        "address": relay.address_text,
            "center": relay.center,
            "hysteresis": relay.hysteresis,
            "lower_threshold": relay.center - relay.hysteresis,
            "upper_threshold": relay.center + relay.hysteresis,
        "low_value": relay.low_numeric,
        "high_value": relay.high_numeric,
            "high_active": relay.high_active,
        })
    });
    json!({
        "experiment_id": record.id,
        "source": record.source,
        "kind": record.kind,
        "status": match record.status {
            ExperimentStatus::Running => "running",
            ExperimentStatus::Completed => "completed",
            ExperimentStatus::Aborted => "aborted",
            ExperimentStatus::Failed => "failed",
            ExperimentStatus::FailedToRestore => "failed_to_restore",
        },
        "elapsed_ms": record.started.elapsed().as_millis() as u64,
        "duration_ms": record.duration.as_millis() as u64,
        "reviewed_scan": record.reviewed_scan,
        "started_scan": record.started_scan,
        "stop_reason": record.stop_reason,
        "restoration": record.restoration,
        "restoration_reason": record.restoration_reason,
        "relay_feedback": relay_feedback,
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

fn snapshot_type(snapshot: &IoSnapshot, address: &IoAddress) -> Option<TypeId> {
    snapshot
        .inputs
        .iter()
        .chain(&snapshot.outputs)
        .chain(&snapshot.memory)
        .find(|entry| &entry.address == address)
        .and_then(|entry| entry.value_type)
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
        "BOOL" => value.as_bool().map(Value::Bool).ok_or_else(invalid),
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
        "BOOL" => Ok(TypeId::BOOL),
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
        Value::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
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
