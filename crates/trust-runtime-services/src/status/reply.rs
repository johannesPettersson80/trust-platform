//! The wire forms of the frequent reads, `status`, `health` and `tasks.stats` (specification 11
//! §6.8.1), written as JSON text without building a value, so that a read served between scans
//! stays short on a chip (specification 34 PORT-02). Each object's members are in the order of
//! their names, the order in which a value's object writes them, so the text is the one the values
//! gave.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use serde::Serialize;

use super::{IoDriverHealth, IoDriverStatus};
use crate::access::AccessRole;
use crate::metrics::{BudgetContributorSnapshot, ProgramMemory, RuntimeMetricsSnapshot};

/// One I/O driver's health.
#[derive(Serialize)]
pub(super) struct DriverHealth<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
    name: &'a str,
    status: &'static str,
}

impl<'a> DriverHealth<'a> {
    /// The health of `entry`: `ok`, or `degraded` or `faulted` with its error.
    pub(super) fn of(entry: &'a IoDriverStatus) -> Self {
        let (status, error) = match &entry.health {
            IoDriverHealth::Ok => ("ok", None),
            IoDriverHealth::Degraded { error } => ("degraded", Some(error.as_str())),
            IoDriverHealth::Faulted { error } => ("faulted", Some(error.as_str())),
        };
        Self {
            error,
            name: entry.name.as_str(),
            status,
        }
    }
}

/// What a role may change, which `status` reports to the client that asked: writes, forces and
/// releases of values and HMI writes need `Engineer`; a lower role carries the reason.
#[derive(Serialize)]
pub(crate) struct AccessCapabilities {
    hmi: HmiAccess,
    io: IoAccess,
    reason: Option<String>,
    role: &'static str,
}

#[derive(Serialize)]
struct HmiAccess {
    write: bool,
}

#[derive(Serialize)]
struct IoAccess {
    force: bool,
    release: bool,
    write: bool,
}

impl AccessCapabilities {
    /// What `role` may change.
    pub(crate) fn of(role: AccessRole) -> Self {
        let engineer = role.allows(AccessRole::Engineer);
        Self {
            hmi: HmiAccess { write: engineer },
            io: IoAccess {
                force: engineer,
                release: engineer,
                write: engineer,
            },
            reason: (!engineer).then(|| {
                format!(
                    "{} role — connect with an engineer token to write or force.",
                    capitalize_role(role.as_str())
                )
            }),
            role: role.as_str(),
        }
    }
}

/// `role` with its first letter in upper case.
pub(crate) fn capitalize_role(role: &str) -> String {
    let mut chars = role.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_uppercase().collect::<String>() + chars.as_str()
}

/// The reason of a debugger hold.
#[derive(Serialize)]
pub(super) struct DebugHold {
    pub(super) reason: &'static str,
}

/// The `status` result.
#[derive(Serialize)]
pub(super) struct Status<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) access: Option<AccessCapabilities>,
    pub(super) control_mode: String,
    pub(super) debug_enabled: bool,
    pub(super) debug_hold: Option<DebugHold>,
    pub(super) event_capacity: usize,
    pub(super) execution_backend: &'static str,
    pub(super) execution_backend_source: &'static str,
    pub(super) fault: Option<String>,
    pub(super) hmi_read_only: bool,
    pub(super) io_drivers: Vec<DriverHealth<'a>>,
    pub(super) metrics: Metrics<'a>,
    pub(super) openot_persistence: serde_json::Value,
    pub(super) platform: &'a str,
    pub(super) plc_name: &'a str,
    pub(super) program_control: &'static str,
    pub(super) realtime: serde_json::Value,
    pub(super) resource: &'a str,
    pub(super) runtime_id: &'a str,
    pub(super) signature: &'static str,
    pub(super) simulation_enabled: bool,
    pub(super) simulation_mode: &'a str,
    pub(super) simulation_time_scale: u32,
    pub(super) simulation_warning: &'a str,
    pub(super) state: String,
    pub(super) uptime_ms: u64,
    pub(super) version: &'static str,
}

/// The `metrics` block of `status`.
#[derive(Serialize)]
pub(super) struct Metrics<'a> {
    cycle_ms: CycleMs,
    execution_backend: &'a str,
    faults: u64,
    overruns: u64,
    profiling: Profiling<'a>,
    program_memory: Option<ProgramMemoryReply>,
    scan_allocations: Option<ScanAllocationsReply>,
}

impl<'a> Metrics<'a> {
    /// The block of `metrics` of a runtime whose backend is `execution_backend`.
    pub(super) fn of(metrics: &'a RuntimeMetricsSnapshot, execution_backend: &'a str) -> Self {
        Self {
            cycle_ms: CycleMs {
                avg: metrics.cycle.avg_ms,
                last: metrics.cycle.last_ms,
                max: metrics.cycle.max_ms,
                min: metrics.cycle.min_ms,
                p50: metrics.cycle_percentiles.p50_ms,
                p95: metrics.cycle_percentiles.p95_ms,
                p99: metrics.cycle_percentiles.p99_ms,
                window_samples: metrics.cycle_percentiles.window_samples,
            },
            execution_backend,
            faults: metrics.faults,
            overruns: metrics.overruns,
            profiling: Profiling {
                enabled: metrics.profiling.enabled,
                top: contributors(metrics),
            },
            program_memory: metrics.program_memory.as_ref().map(ProgramMemoryReply::of),
            scan_allocations: metrics.scan_allocations.map(|count| ScanAllocationsReply {
                engine: count.engine,
                services: count.services,
            }),
        }
    }
}

#[derive(Serialize)]
struct CycleMs {
    avg: f64,
    last: f64,
    max: f64,
    min: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    window_samples: u64,
}

#[derive(Serialize)]
struct Profiling<'a> {
    enabled: bool,
    top: Vec<Contributor<'a>>,
}

#[derive(Serialize)]
struct ScanAllocationsReply {
    engine: u64,
    services: u64,
}

/// The `program_memory` block: the memory figure of a program, in bytes, as a whole and part by
/// part (specification 34 MEM-12).
#[derive(Serialize)]
pub(super) struct ProgramMemoryReply {
    activation_stack: usize,
    bytes: usize,
    engine: usize,
    operand_stack: usize,
    persistent_block: usize,
    program: usize,
    retain_capture: usize,
}

impl ProgramMemoryReply {
    /// The block of `memory`.
    pub(super) fn of(memory: &ProgramMemory) -> Self {
        let figure = &memory.figure;
        Self {
            activation_stack: figure.activation_stack,
            bytes: memory.bytes(),
            engine: figure.engine,
            operand_stack: figure.operand_stack,
            persistent_block: figure.persistent_block,
            program: figure.program,
            retain_capture: memory.retain_capture(),
        }
    }
}

/// One of the largest contributors to the cycle time.
#[derive(Serialize)]
struct Contributor<'a> {
    avg_cycle_ms: f64,
    cycle_pct: f64,
    key: &'a str,
    kind: &'a str,
    last_cycle_pct: f64,
    last_ms: f64,
    name: &'a str,
}

impl<'a> Contributor<'a> {
    fn of(entry: &'a BudgetContributorSnapshot) -> Self {
        Self {
            avg_cycle_ms: entry.avg_cycle_ms,
            cycle_pct: entry.cycle_pct,
            key: entry.key.as_str(),
            kind: entry.kind.as_str(),
            last_cycle_pct: entry.last_cycle_pct,
            last_ms: entry.last_ms,
            name: entry.name.as_str(),
        }
    }
}

/// The largest contributors to the cycle time, as `status` and `tasks.stats` report them.
fn contributors(metrics: &RuntimeMetricsSnapshot) -> Vec<Contributor<'_>> {
    metrics
        .profiling
        .top_contributors
        .iter()
        .map(Contributor::of)
        .collect()
}

/// The `health` result.
#[derive(Serialize)]
pub(super) struct Health<'a> {
    pub(super) fault: Option<String>,
    pub(super) io_drivers: Vec<DriverHealth<'a>>,
    pub(super) ok: bool,
    pub(super) state: String,
}

/// The `tasks.stats` result: each task by name, and the largest contributors.
#[derive(Serialize)]
pub(super) struct TaskStats<'a> {
    profiling_enabled: bool,
    tasks: Vec<Task<'a>>,
    top_contributors: Vec<Contributor<'a>>,
}

#[derive(Serialize)]
struct Task<'a> {
    avg_ms: f64,
    last_ms: f64,
    max_ms: f64,
    min_ms: f64,
    name: &'a str,
    overruns: u64,
}

impl<'a> TaskStats<'a> {
    /// The result of `metrics`, its tasks sorted by name.
    pub(super) fn of(metrics: &'a RuntimeMetricsSnapshot) -> Self {
        let mut tasks: Vec<Task<'a>> = metrics
            .tasks
            .iter()
            .map(|task| Task {
                avg_ms: task.avg_ms,
                last_ms: task.last_ms,
                max_ms: task.max_ms,
                min_ms: task.min_ms,
                name: task.name.as_str(),
                overruns: task.overruns,
            })
            .collect();
        tasks.sort_by(|left, right| left.name.cmp(right.name));
        Self {
            profiling_enabled: metrics.profiling.enabled,
            tasks,
            top_contributors: contributors(metrics),
        }
    }
}
