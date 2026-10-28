//! The `status`, `health`, `tasks.stats`, `events.tail` and `faults` operations (specification
//! 11, §6.8.1), which read the platform's values through [`StatusPort`] and the running resource's
//! records through [`RecordsPort`].

mod event_reads;
mod params;
mod reply;
mod unconfigured;

use alloc::format;
use alloc::string::{String, ToString};

use smol_str::SmolStr;
use trust_runtime_core::scheduler::ResourceState;

use crate::access::AccessRole;
use crate::metrics::ProgramMemory;
use crate::ports::{AccessPort, RecordsPort, ResourcePort, StatusPort};
use crate::protocol::ControlResponse;
#[cfg(test)]
pub(crate) use reply::capitalize_role;
pub(crate) use reply::AccessCapabilities;
use reply::{DebugHold, DriverHealth, Health, Metrics, ProgramMemoryReply, Status, TaskStats};

pub(crate) use event_reads::{write_event_read, EventRead};
pub use params::{
    check_number_members, invalid_params, parse_bounded_limit, unavailable, validate_object_fields,
    MemberKind,
};
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

/// The JSON form of one I/O driver's health, as `status` and `health` write it.
#[must_use]
pub fn io_health_to_json(entry: &IoDriverStatus) -> serde_json::Value {
    serde_json::to_value(DriverHealth::of(entry)).unwrap_or(serde_json::Value::Null)
}

/// `status`: the runtime's identity, stable runtime ID, version, platform, program-control
/// posture, signature posture, whether `identify` runs, state, modes, settings, metrics, realtime
/// posture, persistence, event-store capacity and I/O driver health.
///
/// A debugger hold reports `paused` unless the resource is faulted or stopped. A value that
/// cannot be read answers `<value> unavailable`; the values are read in a fixed order, so the
/// first unreadable one names the error.
#[must_use]
pub fn status<H>(id: u64, host: &H) -> ControlResponse
where
    H: StatusPort + RecordsPort + ResourcePort + AccessPort,
{
    status_for(id, host, None)
}

/// `status` as [`status`] answers it, with what `role` may change when the dispatcher admitted the
/// request with a role.
#[must_use]
pub fn status_for<H>(id: u64, host: &H, role: Option<AccessRole>) -> ControlResponse
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
    let Some(event_capacity) = host.event_capacity() else {
        return unavailable(id, "event capacity");
    };
    let Some(openot_persistence) = host.openot_persistence() else {
        return unavailable(id, "OpenOT persistence status");
    };
    ControlResponse::ok_serialized(
        id,
        &Status {
            access: role.map(AccessCapabilities::of),
            control_mode,
            debug_enabled: host.debug_enabled(),
            debug_hold: debug_hold.map(|reason| DebugHold { reason }),
            event_capacity,
            execution_backend: settings.execution_backend,
            execution_backend_source: settings.execution_backend_source,
            fault,
            identifying: crate::identify::is_identifying(host),
            io_drivers: io_drivers.iter().map(DriverHealth::of).collect(),
            metrics: Metrics::of(&metrics, settings.execution_backend),
            openot_persistence,
            platform: host.platform(),
            plc_name: host.resource_name(),
            program_control: host.program_control().as_str(),
            realtime,
            resource: host.resource_name(),
            runtime_id: host.runtime_id(),
            signature: host.signature(),
            simulation_enabled: settings.simulation_enabled,
            simulation_mode: settings.simulation_mode.as_str(),
            simulation_time_scale: settings.simulation_time_scale,
            simulation_warning: settings.simulation_warning.as_str(),
            state: state_label,
            uptime_ms: metrics.uptime_ms,
            version: env!("CARGO_PKG_VERSION"),
        },
    )
}

/// `health`: whether the runtime is healthy, with its state, fault and I/O driver health.
///
/// The runtime is healthy while it is running, ready or paused, has no fault, no faulted I/O
/// driver and no realtime posture error. A degraded driver or a realtime warning is reported but
/// stays healthy.
#[must_use]
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
    ControlResponse::ok_serialized(
        id,
        &Health {
            fault,
            io_drivers: io_drivers.iter().map(DriverHealth::of).collect(),
            ok,
            state: resource_state_label(state),
        },
    )
}

/// `tasks.stats`: each task's durations and overruns, sorted by name, and the largest
/// contributors to the cycle time.
#[must_use]
pub fn task_stats<H: RecordsPort>(id: u64, host: &H) -> ControlResponse {
    let Some(metrics) = host.metrics_snapshot() else {
        return unavailable(id, "runtime metrics");
    };
    ControlResponse::ok_serialized(id, &TaskStats::of(&metrics))
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

/// The `program_memory` block of `status`'s metrics, and of `trust-runtime build --ci`: the memory
/// figure of a program, in bytes, as a whole and part by part (specification 34 MEM-12).
#[must_use]
pub fn program_memory_json(memory: &ProgramMemory) -> serde_json::Value {
    serde_json::to_value(ProgramMemoryReply::of(memory)).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
mod contract_tests;
