use std::sync::atomic::Ordering;

use crate::io::{IoDriverHealth, IoDriverStatus};
use serde_json::{json, Value};

use super::types::{runtime_event_to_json, HistorianAlertsParams, HistorianQueryParams};
use super::{ControlResponse, ControlState};

#[cfg(test)]
#[path = "status_handlers/projection_contract_tests.rs"]
mod projection_contract_tests;

#[cfg(test)]
#[path = "status_handlers/event_contract_tests.rs"]
mod event_contract_tests;

#[cfg(test)]
#[path = "status_handlers/historian_contract_tests.rs"]
mod historian_contract_tests;

pub(super) fn handle_status(id: u64, state: &ControlState) -> ControlResponse {
    let status = state.resource.state();
    let error = state.resource.last_error().map(|err| err.to_string());
    let Ok(settings) = state.settings.lock().map(|guard| guard.clone()) else {
        return unavailable(id, "runtime settings");
    };
    let simulation = settings.simulation.clone();
    let Ok(io_health) = state
        .io_health
        .lock()
        .map(|guard| guard.iter().map(io_health_to_json).collect::<Vec<_>>())
    else {
        return unavailable(id, "I/O health");
    };
    let Ok(metrics) = state.metrics.lock().map(|guard| guard.snapshot()) else {
        return unavailable(id, "runtime metrics");
    };
    let Ok(realtime) = state.realtime_status.lock().map(|guard| guard.clone()) else {
        return unavailable(id, "realtime status");
    };
    let Ok(control_mode) = state
        .control_mode
        .lock()
        .map(|mode| format!("{:?}", *mode).to_ascii_lowercase())
    else {
        return unavailable(id, "control mode");
    };
    // Runtime settings are the single source of truth for selected backend mode/source.
    let execution_backend = settings.execution_backend.as_str();
    let execution_backend_source = settings.execution_backend_source.as_str();
    let openot_persistence = match openot_persistence_status_json(state) {
        Ok(status) => status,
        Err(surface) => return unavailable(id, surface),
    };
    ControlResponse::ok(
        id,
        json!({
            "state": format!("{status:?}").to_ascii_lowercase(),
            "fault": error,
            "resource": state.resource_name.as_str(),
            "plc_name": state.resource_name.as_str(),
            "uptime_ms": metrics.uptime_ms,
            "debug_enabled": state.debug_enabled.load(Ordering::Relaxed),
            "control_mode": control_mode,
            "execution_backend": execution_backend,
            "execution_backend_source": execution_backend_source,
            "simulation_mode": simulation.mode_label.as_str(),
            "simulation_enabled": simulation.enabled,
            "simulation_time_scale": simulation.time_scale,
            "simulation_warning": simulation.warning.as_str(),
            "hmi_read_only": true,
            "openot_persistence": openot_persistence,
            "metrics": {
                "cycle_ms": {
                    "min": metrics.cycle.min_ms,
                    "avg": metrics.cycle.avg_ms,
                    "max": metrics.cycle.max_ms,
                    "last": metrics.cycle.last_ms,
                    "p50": metrics.cycle_percentiles.p50_ms,
                    "p95": metrics.cycle_percentiles.p95_ms,
                    "p99": metrics.cycle_percentiles.p99_ms,
                    "window_samples": metrics.cycle_percentiles.window_samples,
                },
                "overruns": metrics.overruns,
                "faults": metrics.faults,
                "profiling": {
                    "enabled": metrics.profiling.enabled,
                    "top": metrics
                        .profiling
                        .top_contributors
                        .iter()
                        .map(|entry| {
                            json!({
                                "key": entry.key.as_str(),
                                "kind": entry.kind.as_str(),
                                "name": entry.name.as_str(),
                                "avg_cycle_ms": entry.avg_cycle_ms,
                                "cycle_pct": entry.cycle_pct,
                                "last_ms": entry.last_ms,
                                "last_cycle_pct": entry.last_cycle_pct,
                            })
                        })
                        .collect::<Vec<_>>(),
                },
                "execution_backend": execution_backend,
            },
            "realtime": {
                "profile": realtime.requested.profile_name(),
                "enabled": realtime.requested.enabled,
                "strict": realtime.requested.strict,
                "require_preempt_rt_kernel": realtime.requested.require_preempt_rt_kernel,
                "requested": {
                    "lock_memory": realtime.requested.lock_memory,
                    "scheduler": realtime.requested.scheduler.as_str(),
                    "priority": realtime.requested.priority,
                    "cpu_affinity": realtime.requested.cpu_affinity,
                },
                "observed": {
                    "kernel_realtime": realtime.kernel_realtime,
                    "scheduler": realtime.active_scheduler.map(|value| value.as_str()),
                    "priority": realtime.active_priority,
                    "cpu_affinity": realtime.active_cpu_affinity,
                    "memory_locked_kb": realtime.memory_locked_kb,
                },
                "runtime_applied": {
                    "memory_lock": realtime.memory_lock_applied,
                    "affinity": realtime.affinity_applied_by_runtime,
                    "scheduler": realtime.scheduler_applied_by_runtime,
                },
                "active": realtime.active,
                "warnings": realtime
                    .warnings
                    .iter()
                    .map(|value| value.as_str())
                    .collect::<Vec<_>>(),
                "errors": realtime
                    .errors
                    .iter()
                    .map(|value| value.as_str())
                    .collect::<Vec<_>>(),
            },
            "io_drivers": io_health,
        }),
    )
}

fn openot_persistence_status_json(state: &ControlState) -> Result<Value, &'static str> {
    Ok(match state.openot_persistence_status.as_ref() {
        Some(status) => {
            let status = status.lock().map_err(|_| "OpenOT persistence status")?;
            let mut warnings = Vec::new();
            if status.head_abs > status.cursor_abs {
                warnings.push("lag");
            }
            if status.documents_retried > 0 {
                warnings.push("retrying");
            }
            if status.unresolved > 0 {
                warnings.push("placeholder");
            }
            if status.loss_range_count > 0 {
                warnings.push("loss");
            }
            if status.remote_pending > 0 {
                warnings.push("spool_pressure");
            }
            if status.state == crate::openot_persistence::OpenOtPersistenceState::Faulted
                && status.last_error.is_some()
            {
                warnings.push("migration_or_storage_fault");
            }
            if matches!(
                status.state,
                crate::openot_persistence::OpenOtPersistenceState::Stopped
                    | crate::openot_persistence::OpenOtPersistenceState::Faulted
            ) && (status.pending > 0 || status.remote_pending > 0)
            {
                warnings.push("shutdown_pending");
            }
            json!({
                "state": status.state.as_str(),
                "backend": status.backend,
                "schema_version": status.schema_version,
                "documents_read": status.documents_read,
                "documents_committed": status.documents_committed,
                "documents_duplicated": status.documents_duplicated,
                "remote_pending": status.remote_pending,
                "projection_rows_committed": status.projection_rows_committed,
                "unclassified_event_count": status.unclassified_event_count,
                "reconciled_part_count": status.reconciled_part_count,
                "pending_part_count": status.pending_part_count,
                "documents_retried": status.documents_retried,
                "pending": status.pending,
                "rejected": status.rejected,
                "unresolved": status.unresolved,
                "loss_range_count": status.loss_range_count,
                "lost_record_count": status.lost_record_count,
                "cursor_abs": status.cursor_abs,
                "head_abs": status.head_abs,
                "cursor_lag": status.head_abs.saturating_sub(status.cursor_abs),
                "last_success_time_ns": status.last_success_time_ns,
                "last_error": status.last_error,
                "warnings": warnings,
            })
        }
        None => json!({
            "state": "disabled",
            "backend": null,
            "schema_version": null,
            "documents_read": 0,
            "documents_committed": 0,
            "documents_duplicated": 0,
            "remote_pending": 0,
            "projection_rows_committed": 0,
            "unclassified_event_count": 0,
            "reconciled_part_count": 0,
            "pending_part_count": 0,
            "documents_retried": 0,
            "pending": 0,
            "rejected": 0,
            "unresolved": 0,
            "loss_range_count": 0,
            "lost_record_count": 0,
            "cursor_abs": 0,
            "head_abs": 0,
            "cursor_lag": 0,
            "last_success_time_ns": null,
            "last_error": null,
            "warnings": [],
        }),
    })
}

pub(super) fn handle_health(id: u64, state: &ControlState) -> ControlResponse {
    let status = state.resource.state();
    let error = state.resource.last_error().map(|err| err.to_string());
    let Ok(io_health) = state.io_health.lock().map(|guard| guard.clone()) else {
        return unavailable(id, "I/O health");
    };
    let Ok(realtime) = state.realtime_status.lock().map(|guard| guard.clone()) else {
        return unavailable(id, "realtime status");
    };
    let has_faulted_driver = io_health
        .iter()
        .any(|entry| matches!(entry.health, IoDriverHealth::Faulted { .. }));
    let ok = matches!(
        status,
        crate::scheduler::ResourceState::Running
            | crate::scheduler::ResourceState::Ready
            | crate::scheduler::ResourceState::Paused
    ) && error.is_none()
        && !has_faulted_driver
        && realtime.errors.is_empty();
    ControlResponse::ok(
        id,
        json!({
            "ok": ok,
            "state": format!("{status:?}").to_ascii_lowercase(),
            "fault": error,
            "io_drivers": io_health.iter().map(io_health_to_json).collect::<Vec<_>>(),
        }),
    )
}

pub(super) fn handle_task_stats(id: u64, state: &ControlState) -> ControlResponse {
    let Ok(metrics) = state.metrics.lock().map(|guard| guard.snapshot()) else {
        return unavailable(id, "runtime metrics");
    };
    let mut tasks = metrics
        .tasks
        .iter()
        .map(|task| {
            json!({
                "name": task.name.as_str(),
                "min_ms": task.min_ms,
                "avg_ms": task.avg_ms,
                "max_ms": task.max_ms,
                "last_ms": task.last_ms,
                "overruns": task.overruns,
            })
        })
        .collect::<Vec<_>>();
    tasks.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
    let top_contributors = metrics
        .profiling
        .top_contributors
        .iter()
        .map(|entry| {
            json!({
                "key": entry.key.as_str(),
                "kind": entry.kind.as_str(),
                "name": entry.name.as_str(),
                "avg_cycle_ms": entry.avg_cycle_ms,
                "cycle_pct": entry.cycle_pct,
                "last_ms": entry.last_ms,
                "last_cycle_pct": entry.last_cycle_pct,
            })
        })
        .collect::<Vec<_>>();
    ControlResponse::ok(
        id,
        json!({
            "tasks": tasks,
            "profiling_enabled": metrics.profiling.enabled,
            "top_contributors": top_contributors,
        }),
    )
}

pub(super) fn handle_events_tail(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let limit = match parse_bounded_limit(params.as_ref(), 50, 1_000, &["limit"]) {
        Ok(limit) => limit,
        Err(error) => return invalid_params(id, error),
    };
    let Ok(events) = state
        .events
        .lock()
        .map(|guard| guard.iter().rev().take(limit).cloned().collect::<Vec<_>>())
    else {
        return unavailable(id, "event store");
    };
    let payload = events
        .into_iter()
        .map(runtime_event_to_json)
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "events": payload }))
}

pub(super) fn handle_faults(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let limit = match parse_bounded_limit(params.as_ref(), 50, 1_000, &["limit"]) {
        Ok(limit) => limit,
        Err(error) => return invalid_params(id, error),
    };
    let Ok(faults) = state.events.lock().map(|guard| {
        guard
            .iter()
            .rev()
            .filter(|event| matches!(event, crate::debug::RuntimeEvent::Fault { .. }))
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
    }) else {
        return unavailable(id, "event store");
    };
    let faults = faults
        .into_iter()
        .map(runtime_event_to_json)
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "faults": faults }))
}

pub(super) fn handle_historian_query(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let Some(historian) = state.historian.as_ref() else {
        return ControlResponse::error(id, "historian disabled".into());
    };
    let params = match params {
        Some(value) => {
            if let Err(error) = validate_object_fields(
                &value,
                &["variable", "since_ms", "limit"],
                &["variable", "since_ms", "limit"],
            ) {
                return invalid_params(id, error);
            }
            match serde_json::from_value::<HistorianQueryParams>(value) {
                Ok(parsed) => parsed,
                Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
            }
        }
        None => HistorianQueryParams::default(),
    };
    let mut params = params;
    if let Some(variable) = params.variable.as_mut() {
        *variable = variable.trim().to_string();
        if variable.is_empty() {
            return invalid_params(id, "variable must not be blank");
        }
    }
    if params
        .limit
        .is_some_and(|limit| !(1..=5_000).contains(&limit))
    {
        return invalid_params(id, "limit must be in 1..=5000");
    }
    let items = historian.query(
        params.variable.as_deref(),
        params.since_ms,
        params.limit.unwrap_or(250),
    );
    ControlResponse::ok(id, json!({ "items": items }))
}

pub(super) fn handle_historian_alerts(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let Some(historian) = state.historian.as_ref() else {
        return ControlResponse::error(id, "historian disabled".into());
    };
    let params = match params {
        Some(value) => {
            if let Err(error) = validate_object_fields(&value, &["limit"], &["limit"]) {
                return invalid_params(id, error);
            }
            match serde_json::from_value::<HistorianAlertsParams>(value) {
                Ok(parsed) => parsed,
                Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
            }
        }
        None => HistorianAlertsParams::default(),
    };
    if params
        .limit
        .is_some_and(|limit| !(1..=1_000).contains(&limit))
    {
        return invalid_params(id, "limit must be in 1..=1000");
    }
    let items = historian.alerts(params.limit.unwrap_or(200));
    ControlResponse::ok(id, json!({ "items": items }))
}

fn parse_bounded_limit(
    params: Option<&serde_json::Value>,
    default: usize,
    maximum: usize,
    allowed_fields: &[&str],
) -> Result<usize, &'static str> {
    let Some(params) = params else {
        return Ok(default);
    };
    validate_object_fields(params, allowed_fields, &["limit"])?;
    let Some(value) = params.get("limit") else {
        return Ok(default);
    };
    let Some(limit) = value.as_u64().and_then(|value| usize::try_from(value).ok()) else {
        return Err("limit must be an integer");
    };
    if !(1..=maximum).contains(&limit) {
        return Err("limit is outside the supported range");
    }
    Ok(limit)
}

fn validate_object_fields(
    value: &serde_json::Value,
    allowed_fields: &[&str],
    non_null_fields: &[&str],
) -> Result<(), &'static str> {
    let Some(object) = value.as_object() else {
        return Err("parameters must be an object");
    };
    if object
        .keys()
        .any(|key| !allowed_fields.contains(&key.as_str()))
    {
        return Err("unknown parameter field");
    }
    if non_null_fields
        .iter()
        .any(|field| object.get(*field).is_some_and(serde_json::Value::is_null))
    {
        return Err("parameter fields must not be null");
    }
    Ok(())
}

fn invalid_params(id: u64, error: impl std::fmt::Display) -> ControlResponse {
    ControlResponse::error(id, format!("invalid params: {error}"))
}

fn unavailable(id: u64, component: &str) -> ControlResponse {
    ControlResponse::error(id, format!("{component} unavailable"))
}

fn io_health_to_json(entry: &IoDriverStatus) -> serde_json::Value {
    match &entry.health {
        IoDriverHealth::Ok => json!({
            "name": entry.name.as_str(),
            "status": "ok",
        }),
        IoDriverHealth::Degraded { error } => json!({
            "name": entry.name.as_str(),
            "status": "degraded",
            "error": error.as_str(),
        }),
        IoDriverHealth::Faulted { error } => json!({
            "name": entry.name.as_str(),
            "status": "faulted",
            "error": error.as_str(),
        }),
    }
}
