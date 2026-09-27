mod abort_binding;
use abort_binding::{AbortBinding, AbortBindingParams};
mod values;
use values::{
    decode_params, decode_relay_feedback, decode_trajectory, experiment_type_id, experiment_value,
    numeric_value, snapshot_type, snapshot_value,
};

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

use super::{ControlResponse, ControlState};
use crate::debug::DebugControl;
use crate::io::{validate_process_image_address, IoAddress, IoSnapshot};
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
    abort_binding: Option<AbortBinding>,
    restoration_after_scan: Option<u64>,
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
    #[serde(default)]
    abort_binding: Option<AbortBindingParams>,
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
    let abort_binding = match params.abort_binding {
        Some(binding) => {
            match AbortBinding::decode(binding, &params.kind, &address, declared_type, &snapshot) {
                Ok(binding) => Some(binding),
                Err((message, code)) => return error(id, message, code),
            }
        }
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
                vec![(Duration::ZERO, write_value.clone())]
            }
            Ok(values) => values,
            Err(message) => return error(id, message, "invalid_experiment"),
        }
    };
    if params.kind == "mv_pulse"
        && (trajectory.len() != 2
            || trajectory[0] != (Duration::ZERO, write_value.clone())
            || trajectory[1]
                != (
                    Duration::from_millis(params.duration_ms / 2),
                    restoration_write_value.clone(),
                ))
    {
        return error(
            id,
            "MV pulses must rise at 0 ms and return to baseline halfway through duration",
            "invalid_experiment",
        );
    }

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
        abort_binding,
        restoration_after_scan: None,
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
        if active.as_ref().is_some_and(|current| {
            current.status == ExperimentStatus::Running
                || (current.abort_binding.is_some() && current.restoration == "pending")
        }) {
            return error(
                id,
                "another experiment is already running",
                "experiment_active",
            );
        }
        // Start and stop queue writes under one lifecycle lock. An abort cannot
        // be followed by a delayed first activation write from this request.
        if write_enabled {
            state
                .debug
                .enqueue_io_write(address, record.trajectory[0].1.clone());
        }
        *active = Some(record);
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
        &state.io_snapshot,
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
        let (elapsed, duration, address, minimum, maximum, relay_feedback, abort_binding) =
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
                        let observed = snapshot.lock().ok().and_then(|value| value.clone());
                        let confirmed = observed.as_ref().is_some_and(|value| {
                            if snapshot_value(value, &record.address)
                                != Some(&record.restoration_observed_value)
                            {
                                return false;
                            }
                            if let Some(abort) = record.abort_binding.as_mut() {
                                if value
                                    .scan
                                    .zip(record.restoration_after_scan)
                                    .is_none_or(|(scan, after)| scan <= after)
                                    || value.forced.contains(&record.address)
                                    || snapshot_type(value, &record.address) != Some(TypeId::BOOL)
                                {
                                    return false;
                                }
                                return abort.confirm(value);
                            }
                            true
                        });
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
                            if let Some(abort) = record.abort_binding.as_mut() {
                                abort.fail_confirmation();
                            }
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
                        record.abort_binding.clone(),
                    )
                }
                Err(_) => return,
            };
        // Legacy plans retain their established timeout precedence. A reviewed
        // abort policy checks faults before declaring normal completion.
        if elapsed >= duration && abort_binding.is_none() {
            let _ = finish(
                &service,
                &debug,
                &snapshot,
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
                &snapshot,
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
                &snapshot,
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
                &snapshot,
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
                &snapshot,
                &experiment_id,
                ExperimentStatus::Failed,
                "signal_outside_limits",
            );
            continue;
        }
        if let Some(abort) = abort_binding {
            let failure = current_snapshot
                .as_ref()
                .map_or(Some("bad_abort_binding"), |value| {
                    if snapshot_type(value, &address) != Some(TypeId::BOOL) {
                        return Some("bad_runtime_data");
                    }
                    match abort.asserted(value) {
                        Ok(false) => None,
                        Ok(true) => Some("project_abort_asserted"),
                        Err(reason) => Some(reason),
                    }
                });
            if let Some(reason) = failure {
                let _ = finish(
                    &service,
                    &debug,
                    &snapshot,
                    &experiment_id,
                    ExperimentStatus::Failed,
                    reason,
                );
                continue;
            }
        }
        if elapsed >= duration {
            let _ = finish(
                &service,
                &debug,
                &snapshot,
                &experiment_id,
                ExperimentStatus::Completed,
                "duration_complete",
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
                &snapshot,
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
                &snapshot,
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
        let high = super::relay_state::next_relay_state(
            relay_value,
            relay.center,
            relay.hysteresis,
            true,
            relay.high_active,
        );
        if high != relay.high_active {
            let value = if high {
                relay.high_value.clone()
            } else {
                relay.low_value.clone()
            };
            debug.enqueue_io_write(record.address.clone(), value);
            relay.high_active = high;
        }
    });
}

fn finish(
    service: &ControlExperimentService,
    debug: &DebugControl,
    snapshot: &Arc<Mutex<Option<IoSnapshot>>>,
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
    // Abort assertion precedes deactivation; the declared ST program owns the
    // actuator policy. Queue both while excluding late excitation writes.
    if record.write_enabled {
        if let Some(abort) = record.abort_binding.as_mut() {
            record.restoration_after_scan = Some(
                snapshot
                    .lock()
                    .ok()
                    .and_then(|value| value.as_ref().and_then(|value| value.scan))
                    .unwrap_or(record.started_scan),
            );
            if status != ExperimentStatus::Completed {
                abort.request(debug);
            }
        }
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
        "abort_binding": record.abort_binding.as_ref().map(AbortBinding::json),
    })
}

fn error(id: u64, message: impl Into<String>, code: &'static str) -> ControlResponse {
    ControlResponse::error_with_code(id, message.into(), code)
}
