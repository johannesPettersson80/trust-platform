//! The `status`, `health`, `tasks.stats`, `events.tail` and `faults` operations (specification
//! 11, §6.8.1), which read the platform's values through [`StatusPort`] and the running resource's
//! records through [`RecordsPort`].

mod event_reads;
mod params;
mod unconfigured;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime_core::scheduler::ResourceState;

use crate::metrics::RuntimeMetricsSnapshot;
use crate::ports::{AccessPort, RecordsPort, ResourcePort, StatusPort};
use crate::protocol::ControlResponse;

pub(crate) use event_reads::{write_event_read, EventRead};
pub use params::{invalid_params, parse_bounded_limit, unavailable, validate_object_fields};
pub use unconfigured::{absent_realtime_profile, disabled_openot_persistence};

/// Health of one I/O driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoDriverHealth {
    /// The driver exchanges its images normally.
    Ok,
    /// The driver works with a problem.
    Degraded {
        /// What is wrong.
        error: SmolStr,
    },
    /// The driver has stopped working.
    Faulted {
        /// What is wrong.
        error: SmolStr,
    },
}

/// The health of one named I/O driver.
#[derive(Debug, Clone)]
pub struct IoDriverStatus {
    /// Driver name.
    pub name: SmolStr,
    /// Driver health.
    pub health: IoDriverHealth,
}

/// The runtime settings that `status` reports.
#[derive(Debug, Clone)]
pub struct StatusSettings {
    /// Configuration text of the execution backend.
    pub execution_backend: &'static str,
    /// Where the execution backend selection came from.
    pub execution_backend_source: &'static str,
    /// Whether the runtime runs a simulation.
    pub simulation_enabled: bool,
    /// Simulation time scale.
    pub simulation_time_scale: u32,
    /// Label of the simulation mode.
    pub simulation_mode: SmolStr,
    /// Simulation warning text, empty without one.
    pub simulation_warning: SmolStr,
}

/// The JSON form of one I/O driver's health.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn io_health_to_json(entry: &IoDriverStatus) -> serde_json::Value {
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

/// `status`: the runtime's identity, state, modes, settings, metrics, realtime posture,
/// persistence and I/O driver health.
///
/// A debugger hold reports `paused` unless the resource is faulted or stopped. A value that
/// cannot be read answers `<value> unavailable`; the values are read in a fixed order, so the
/// first unreadable one names the error.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn status<H>(id: u64, host: &H) -> ControlResponse
where
    H: StatusPort + RecordsPort + ResourcePort + AccessPort,
{
    let state = host.resource_state();
    let fault = host.last_error().as_ref().map(ToString::to_string);
    // A debugger hold keeps the scan paused even while the resource reports running; faults and
    // clean stops keep their own state.
    let debug_hold = host.debug_hold();
    let faulted_or_stopped = matches!(state, ResourceState::Faulted | ResourceState::Stopped);
    let state_label = match debug_hold {
        Some(_) if !faulted_or_stopped => String::from("paused"),
        _ => resource_state_label(state),
    };
    let Some(settings) = host.status_settings() else {
        return unavailable(id, "runtime settings");
    };
    let Some(io_drivers) = host.io_drivers() else {
        return unavailable(id, "I/O health");
    };
    let Some(metrics) = host.metrics_snapshot() else {
        return unavailable(id, "runtime metrics");
    };
    let Some(realtime) = host.realtime_status() else {
        return unavailable(id, "realtime status");
    };
    let Some(control_mode) = host.control_mode_label() else {
        return unavailable(id, "control mode");
    };
    let Some(openot_persistence) = host.openot_persistence() else {
        return unavailable(id, "OpenOT persistence status");
    };
    let io_drivers = io_drivers.iter().map(io_health_to_json).collect::<Vec<_>>();
    ControlResponse::ok(
        id,
        json!({
            "state": state_label,
            "fault": fault,
            "resource": host.resource_name(),
            "plc_name": host.resource_name(),
            "uptime_ms": metrics.uptime_ms,
            "debug_enabled": host.debug_enabled(),
            "control_mode": control_mode,
            "debug_hold": debug_hold.map(|reason| json!({"reason": reason})),
            "execution_backend": settings.execution_backend,
            "execution_backend_source": settings.execution_backend_source,
            "simulation_mode": settings.simulation_mode.as_str(),
            "simulation_enabled": settings.simulation_enabled,
            "simulation_time_scale": settings.simulation_time_scale,
            "simulation_warning": settings.simulation_warning.as_str(),
            "hmi_read_only": true,
            "openot_persistence": openot_persistence,
            "metrics": metrics_json(&metrics, settings.execution_backend),
            "realtime": realtime,
            "io_drivers": io_drivers,
        }),
    )
}

/// `health`: whether the runtime is healthy, with its state, fault and I/O driver health.
///
/// The runtime is healthy while it is running, ready or paused, has no fault, no faulted I/O
/// driver and no realtime posture error. A degraded driver or a realtime warning is reported but
/// stays healthy.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn health<H>(id: u64, host: &H) -> ControlResponse
where
    H: StatusPort + ResourcePort,
{
    let state = host.resource_state();
    let fault = host.last_error().as_ref().map(ToString::to_string);
    let Some(io_drivers) = host.io_drivers() else {
        return unavailable(id, "I/O health");
    };
    let Some(realtime_has_errors) = host.realtime_has_errors() else {
        return unavailable(id, "realtime status");
    };
    let has_faulted_driver = io_drivers
        .iter()
        .any(|entry| matches!(entry.health, IoDriverHealth::Faulted { .. }));
    let ok = matches!(
        state,
        ResourceState::Running | ResourceState::Ready | ResourceState::Paused
    ) && fault.is_none()
        && !has_faulted_driver
        && !realtime_has_errors;
    ControlResponse::ok(
        id,
        json!({
            "ok": ok,
            "state": resource_state_label(state),
            "fault": fault,
            "io_drivers": io_drivers.iter().map(io_health_to_json).collect::<Vec<_>>(),
        }),
    )
}

/// `tasks.stats`: each task's durations and overruns, sorted by name, and the largest
/// contributors to the cycle time.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn task_stats<H: RecordsPort>(id: u64, host: &H) -> ControlResponse {
    let Some(metrics) = host.metrics_snapshot() else {
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
    ControlResponse::ok(
        id,
        json!({
            "tasks": tasks,
            "profiling_enabled": metrics.profiling.enabled,
            "top_contributors": top_contributors_json(&metrics),
        }),
    )
}

/// `events.tail` and `events`: the newest events, newest first; `limit` is 1 to 1,000 and 50
/// by default.
#[must_use]
pub fn events_tail<H: RecordsPort>(
    id: u64,
    params: Option<&serde_json::Value>,
    host: &H,
) -> ControlResponse {
    event_reads::answer_event_read(EventRead::Events, id, params, host)
}

/// `faults`: the newest fault events, newest first; `limit` is 1 to 1,000 and 50 by default,
/// and counts faults only.
#[must_use]
pub fn faults<H: RecordsPort>(
    id: u64,
    params: Option<&serde_json::Value>,
    host: &H,
) -> ControlResponse {
    event_reads::answer_event_read(EventRead::Faults, id, params, host)
}

/// The lowercase name of a resource state, for example `running`.
fn resource_state_label(state: ResourceState) -> String {
    format!("{state:?}").to_ascii_lowercase()
}

/// The `metrics` block of `status`.
fn metrics_json(metrics: &RuntimeMetricsSnapshot, execution_backend: &str) -> serde_json::Value {
    json!({
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
            "top": top_contributors_json(metrics),
        },
        "execution_backend": execution_backend,
    })
}

/// The largest contributors to the cycle time, as `status` and `tasks.stats` report them.
fn top_contributors_json(metrics: &RuntimeMetricsSnapshot) -> Vec<serde_json::Value> {
    metrics
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
        .collect()
}

#[cfg(test)]
mod contract_tests;
