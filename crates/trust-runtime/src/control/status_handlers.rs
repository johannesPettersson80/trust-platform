use std::sync::atomic::Ordering;

use crate::io::{IoDriverHealth, IoDriverStatus};
use crate::linux_rt::{LinuxRtConfig, LinuxRtRuntimeStatus};
use serde_json::json;

use super::types::{runtime_event_to_json, HistorianAlertsParams, HistorianQueryParams};
use super::{ControlResponse, ControlState};

pub(super) fn handle_status(id: u64, state: &ControlState) -> ControlResponse {
    let status = state.resource.state();
    let error = state.resource.last_error().map(|err| err.to_string());
    let settings = state.settings.lock().ok().map(|guard| guard.clone());
    let simulation = settings.as_ref().map(|cfg| cfg.simulation.clone());
    let io_health = state
        .io_health
        .lock()
        .ok()
        .map(|guard| guard.iter().map(io_health_to_json).collect::<Vec<_>>())
        .unwrap_or_default();
    let metrics = state
        .metrics
        .lock()
        .ok()
        .map(|guard| guard.snapshot())
        .unwrap_or_default();
    let realtime = state
        .realtime_status
        .lock()
        .ok()
        .map(|guard| guard.clone())
        .unwrap_or_else(|| LinuxRtRuntimeStatus::from_config(LinuxRtConfig::default()));
    // Runtime settings are the single source of truth for selected backend mode/source.
    let execution_backend = settings
        .as_ref()
        .map(|cfg| cfg.execution_backend.as_str())
        .unwrap_or("vm");
    let execution_backend_source = settings
        .as_ref()
        .map(|cfg| cfg.execution_backend_source.as_str())
        .unwrap_or("default");
    ControlResponse::ok(
        id,
        json!({
            "state": format!("{status:?}").to_ascii_lowercase(),
            "fault": error,
            "resource": state.resource_name.as_str(),
            "plc_name": state.resource_name.as_str(),
            "uptime_ms": metrics.uptime_ms,
            "debug_enabled": state.debug_enabled.load(Ordering::Relaxed),
            "control_mode": state
                .control_mode
                .lock()
                .map(|mode| format!("{:?}", *mode).to_ascii_lowercase())
                .unwrap_or_else(|_| "production".to_string()),
            "execution_backend": execution_backend,
            "execution_backend_source": execution_backend_source,
            "simulation_mode": simulation
                .as_ref()
                .map(|cfg| cfg.mode_label.as_str())
                .unwrap_or("production"),
            "simulation_enabled": simulation.as_ref().map(|cfg| cfg.enabled).unwrap_or(false),
            "simulation_time_scale": simulation.as_ref().map(|cfg| cfg.time_scale).unwrap_or(1),
            "simulation_warning": simulation
                .as_ref()
                .map(|cfg| cfg.warning.as_str())
                .unwrap_or(""),
            "hmi_read_only": true,
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

pub(super) fn handle_health(id: u64, state: &ControlState) -> ControlResponse {
    let status = state.resource.state();
    let error = state.resource.last_error().map(|err| err.to_string());
    let io_health = state
        .io_health
        .lock()
        .ok()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let has_faulted_driver = io_health
        .iter()
        .any(|entry| matches!(entry.health, IoDriverHealth::Faulted { .. }));
    let ok = matches!(
        status,
        crate::scheduler::ResourceState::Running
            | crate::scheduler::ResourceState::Ready
            | crate::scheduler::ResourceState::Paused
    ) && error.is_none()
        && !has_faulted_driver;
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
    let metrics = state
        .metrics
        .lock()
        .ok()
        .map(|guard| guard.snapshot())
        .unwrap_or_default();
    let tasks = metrics
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
    let limit = params
        .and_then(|value| value.get("limit").cloned())
        .and_then(|value| value.as_u64())
        .unwrap_or(50) as usize;
    let events = state
        .events
        .lock()
        .map(|guard| guard.iter().rev().take(limit).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
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
    let limit = params
        .and_then(|value| value.get("limit").cloned())
        .and_then(|value| value.as_u64())
        .unwrap_or(50) as usize;
    let events = state
        .events
        .lock()
        .map(|guard| guard.iter().rev().take(limit).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let faults = events
        .into_iter()
        .filter(|event| matches!(event, crate::debug::RuntimeEvent::Fault { .. }))
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
        Some(value) => match serde_json::from_value::<HistorianQueryParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => HistorianQueryParams::default(),
    };
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
        Some(value) => match serde_json::from_value::<HistorianAlertsParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => HistorianAlertsParams::default(),
    };
    let items = historian.alerts(params.limit.unwrap_or(200));
    ControlResponse::ok(id, json!({ "items": items }))
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
