//! Runtime metrics: cycle, task and call timings and the fault and overrun counters that
//! `status`, `tasks.stats` and the metrics export report.
//!
//! The platform supplies the clock that measures uptime ([`MonotonicClock`]) and records the
//! durations it measures.
//!
//! Recording a scan allocates nothing once each task and program has its entry: a task's or
//! program's first record adds it (specification 34, ARCH-05).

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::vec::Vec;
use core::time::Duration;

use smol_str::SmolStr;

use crate::execution_backend::ExecutionBackend;
use trust_runtime_core::ports::MonotonicClock;
use trust_runtime_core::vm::MemoryFigure;

const CYCLE_LATENCY_WINDOW_SIZE: usize = 512;

/// The whole nanoseconds of `duration`, saturated at `u64::MAX` (about 584 years).
fn whole_nanos(duration: Duration) -> u64 {
    duration
        .as_secs()
        .saturating_mul(1_000_000_000)
        .saturating_add(u64::from(duration.subsec_nanos()))
}

/// Milliseconds of `nanos` nanoseconds.
#[allow(clippy::cast_precision_loss)] // a reported duration, not an exact count
fn millis(nanos: u64) -> f64 {
    nanos as f64 / 1_000_000.0
}

/// Recorded durations as whole nanoseconds: the shortest, the longest, the last, their sum and their
/// count. A record compares and adds integers, and the milliseconds are computed when a snapshot
/// reads them, so that a chip records a scan without software floating point (checklist P1-6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Durations {
    min: u64,
    max: u64,
    last: u64,
    total: u64,
    count: u64,
}

impl Durations {
    fn record(&mut self, duration: Duration) {
        let nanos = whole_nanos(duration);
        self.last = nanos;
        if self.count == 0 {
            self.min = nanos;
            self.max = nanos;
        } else {
            self.min = self.min.min(nanos);
            self.max = self.max.max(nanos);
        }
        self.total = self.total.saturating_add(nanos);
        self.count = self.count.saturating_add(1);
    }

    fn min_ms(&self) -> f64 {
        millis(self.min)
    }

    fn max_ms(&self) -> f64 {
        millis(self.max)
    }

    fn last_ms(&self) -> f64 {
        millis(self.last)
    }

    fn total_ms(&self) -> f64 {
        millis(self.total)
    }

    /// The mean, 0 before the first record.
    #[allow(clippy::cast_precision_loss)] // the mean divides by the count
    fn avg_ms(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.total_ms() / self.count as f64
        }
    }
}

/// Minimum, maximum, mean and last scan cycle duration, in milliseconds, as a snapshot reports them.
#[derive(Debug, Clone, Copy, Default)]
pub struct CycleStats {
    /// Shortest recorded cycle.
    pub min_ms: f64,
    /// Longest recorded cycle.
    pub max_ms: f64,
    /// Mean of the recorded cycles.
    pub avg_ms: f64,
    /// Most recent cycle.
    pub last_ms: f64,
    samples: u64,
}

impl CycleStats {
    /// The statistics of `durations`.
    fn of(durations: &Durations) -> Self {
        Self {
            min_ms: durations.min_ms(),
            max_ms: durations.max_ms(),
            avg_ms: durations.avg_ms(),
            last_ms: durations.last_ms(),
            samples: durations.count,
        }
    }

    /// Recorded cycles.
    #[must_use]
    pub const fn samples(&self) -> u64 {
        self.samples
    }
}

/// The durations of the most recent cycles, as whole nanoseconds.
#[derive(Debug, Clone)]
struct CycleLatencyWindow {
    values: [u64; CYCLE_LATENCY_WINDOW_SIZE],
    len: usize,
    next: usize,
}

impl CycleLatencyWindow {
    fn record(&mut self, duration: Duration) {
        self.values[self.next] = whole_nanos(duration);
        self.next = (self.next + 1) % CYCLE_LATENCY_WINDOW_SIZE;
        if self.len < CYCLE_LATENCY_WINDOW_SIZE {
            self.len += 1;
        }
    }

    /// The percentiles of the window, read from a copy on the stack by selection, with no heap
    /// allocation and no sort of the whole copy.
    fn snapshot(&self) -> CyclePercentilesSnapshot {
        let recorded = &self.values[..self.len];
        let Some(max) = recorded.iter().copied().max() else {
            return CyclePercentilesSnapshot::default();
        };
        let mut copy = [0u64; CYCLE_LATENCY_WINDOW_SIZE];
        let values = &mut copy[..self.len];
        values.copy_from_slice(recorded);
        let ranks = [0.50, 0.95, 0.99].map(|quantile| rank(self.len, quantile));
        let [p50, p95, p99] = order_statistics(values, ranks);
        CyclePercentilesSnapshot {
            p50_ms: millis(p50),
            p95_ms: millis(p95),
            p99_ms: millis(p99),
            max_ms: millis(max),
            window_samples: u64::try_from(self.len).unwrap_or(u64::MAX),
        }
    }

    fn clear(&mut self) {
        self.len = 0;
        self.next = 0;
    }
}

impl Default for CycleLatencyWindow {
    fn default() -> Self {
        Self {
            values: [0; CYCLE_LATENCY_WINDOW_SIZE],
            len: 0,
            next: 0,
        }
    }
}

/// Duration statistics and missed activations of one task.
#[derive(Debug, Clone, Copy, Default)]
pub struct TaskStats {
    durations: Durations,
    /// Missed periodic activations.
    pub overruns: u64,
}

impl TaskStats {
    /// Records one execution that took `duration`.
    pub fn record(&mut self, duration: Duration) {
        self.durations.record(duration);
    }

    /// Counts `missed` periodic activations.
    pub fn record_overrun(&mut self, missed: u64) {
        self.overruns = self.overruns.saturating_add(missed);
    }

    /// Shortest recorded execution, in milliseconds.
    #[must_use]
    pub fn min_ms(&self) -> f64 {
        self.durations.min_ms()
    }

    /// Longest recorded execution, in milliseconds.
    #[must_use]
    pub fn max_ms(&self) -> f64 {
        self.durations.max_ms()
    }

    /// Mean of the recorded executions, in milliseconds.
    #[must_use]
    pub fn avg_ms(&self) -> f64 {
        self.durations.avg_ms()
    }

    /// Most recent execution, in milliseconds.
    #[must_use]
    pub fn last_ms(&self) -> f64 {
        self.durations.last_ms()
    }
}

/// Duration statistics of one profiled call.
#[derive(Debug, Clone, Copy, Default)]
pub struct CallStats {
    durations: Durations,
}

impl CallStats {
    /// Records one call that took `duration`.
    pub fn record(&mut self, duration: Duration) {
        self.durations.record(duration);
    }

    /// Recorded calls.
    #[must_use]
    pub fn calls(&self) -> u64 {
        self.durations.count
    }

    /// Shortest recorded call, in milliseconds.
    #[must_use]
    pub fn min_ms(&self) -> f64 {
        self.durations.min_ms()
    }

    /// Longest recorded call, in milliseconds.
    #[must_use]
    pub fn max_ms(&self) -> f64 {
        self.durations.max_ms()
    }

    /// Mean of the recorded calls, in milliseconds.
    #[must_use]
    pub fn avg_ms(&self) -> f64 {
        self.durations.avg_ms()
    }

    /// Most recent call, in milliseconds.
    #[must_use]
    pub fn last_ms(&self) -> f64 {
        self.durations.last_ms()
    }

    /// Sum of the recorded calls, in milliseconds.
    #[must_use]
    pub fn total_ms(&self) -> f64 {
        self.durations.total_ms()
    }
}

/// One profiled call: the kind and name it was first recorded with, its text `kind:name`, which the
/// snapshot reports and which finds it again, and its statistics.
#[derive(Debug, Clone)]
struct CallProfileEntry {
    kind: SmolStr,
    name: SmolStr,
    /// The text `kind:name`, built once, when the call is first recorded.
    key: SmolStr,
    stats: CallStats,
}

impl CallProfileEntry {
    /// Whether the entry's text is `kind:name`, compared in its three parts without building that
    /// text (checklist P1-6) and without the generic string search, which runs from the C6's flash
    /// at every call (checklist P1-7): two calls whose text is the same share one entry.
    fn is(&self, kind: &str, name: &str) -> bool {
        let Some((head, rest)) = self.key.as_bytes().split_at_checked(kind.len()) else {
            return false;
        };
        let Some((colon, tail)) = rest.split_first() else {
            return false;
        };
        head == kind.as_bytes() && *colon == b':' && tail == name.as_bytes()
    }
}

/// The allocator operations of the thread that runs the scans, each allocation, reallocation and
/// free once (specification 34, "The allocation count"): those the engine made running the
/// program, and those the services made in the calls the scan makes into them and in the
/// runtime's cycle work around the scan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanAllocations {
    /// The engine's operations.
    pub engine: u64,
    /// The services' operations.
    pub services: u64,
}

/// The memory figure of the program a runtime runs (specification 34 MEM-12): the bytes of each
/// part, which preparation computed, and whether the runtime keeps the program's retained values,
/// so that the capture of them counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramMemory {
    /// The figure, part by part.
    pub figure: MemoryFigure,
    /// Whether the runtime keeps the program's retained values.
    pub retains: bool,
}

impl ProgramMemory {
    /// The bytes of the whole figure, as admission compares them with the memory the device has
    /// for programs.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.figure.total(self.retains)
    }

    /// The bytes of the capture of the retained values: the figure's when the runtime keeps them,
    /// else none.
    #[must_use]
    pub fn retain_capture(&self) -> usize {
        if self.retains {
            self.figure.retain_capture
        } else {
            0
        }
    }
}

/// Metrics of one runtime, measured with the platform clock `C`.
#[derive(Debug, Clone)]
pub struct RuntimeMetrics<C> {
    clock: C,
    execution_backend: ExecutionBackend,
    /// Scan cycle durations.
    cycle: Durations,
    cycle_window: CycleLatencyWindow,
    /// Statistics of each task, by task name.
    pub tasks: BTreeMap<SmolStr, TaskStats>,
    /// Whether calls are profiled.
    pub profiling_enabled: bool,
    /// The profiled calls, in the order they were first recorded; the snapshot ranks them.
    profile_calls: Vec<CallProfileEntry>,
    /// Recorded faults.
    pub faults: u64,
    /// Missed periodic activations of all tasks.
    pub overruns: u64,
    /// The scans' allocator operations since the metrics started; `None` until a scan is counted,
    /// and on a runtime that does not count them.
    pub scan_allocations: Option<ScanAllocations>,
    /// The memory figure of the program the runtime runs; `None` while it runs none.
    pub program_memory: Option<ProgramMemory>,
}

impl<C: MonotonicClock> RuntimeMetrics<C> {
    /// Empty metrics whose uptime is the time `clock` has measured since its origin.
    #[must_use]
    pub fn with_clock(clock: C) -> Self {
        Self {
            clock,
            execution_backend: ExecutionBackend::BytecodeVm,
            cycle: Durations::default(),
            cycle_window: CycleLatencyWindow::default(),
            tasks: BTreeMap::new(),
            profiling_enabled: true,
            profile_calls: Vec::new(),
            faults: 0,
            overruns: 0,
            scan_allocations: None,
            program_memory: None,
        }
    }

    /// Starts the metrics again in place, as [`Self::with_clock`] creates them: the uptime is
    /// `clock`'s, and every statistic, count and recorded call is cleared.
    pub fn restart(&mut self, clock: C) {
        self.clock = clock;
        self.execution_backend = ExecutionBackend::BytecodeVm;
        self.cycle = Durations::default();
        self.cycle_window.clear();
        self.tasks.clear();
        self.profiling_enabled = true;
        self.profile_calls.clear();
        self.faults = 0;
        self.overruns = 0;
        self.scan_allocations = None;
        self.program_memory = None;
    }

    /// Milliseconds since the metrics started: the clock's time since its origin.
    #[must_use]
    pub fn uptime_ms(&self) -> u64 {
        u64::try_from(self.clock.now().since_origin().as_millis()).unwrap_or(u64::MAX)
    }

    /// Records one scan cycle that took `duration`.
    pub fn record_cycle(&mut self, duration: Duration) {
        self.cycle.record(duration);
        self.cycle_window.record(duration);
    }

    /// Scan cycle statistics.
    #[must_use]
    pub fn cycle(&self) -> CycleStats {
        CycleStats::of(&self.cycle)
    }

    /// Records one execution of task `name` that took `duration`. A task already recorded is found
    /// by its name, without a copy of the name (checklist P1-6).
    pub fn record_task(&mut self, name: &SmolStr, duration: Duration) {
        if let Some(entry) = self.tasks.get_mut(name.as_str()) {
            entry.record(duration);
            return;
        }
        self.tasks.entry(name.clone()).or_default().record(duration);
    }

    /// Counts `missed` periodic activations of task `name`; zero records nothing.
    pub fn record_overrun(&mut self, name: &SmolStr, missed: u64) {
        if missed == 0 {
            return;
        }
        self.overruns = self.overruns.saturating_add(missed);
        let entry = self.tasks.entry(name.clone()).or_default();
        entry.record_overrun(missed);
    }

    /// Adds the allocator operations of one counted scan: `engine` of the engine's and `services`
    /// of the services'. Adding allocates nothing.
    pub fn record_scan_allocations(&mut self, engine: u64, services: u64) {
        let total = self.scan_allocations.get_or_insert_default();
        total.engine = total.engine.saturating_add(engine);
        total.services = total.services.saturating_add(services);
    }

    /// Counts one fault.
    pub fn record_fault(&mut self) {
        self.faults = self.faults.saturating_add(1);
    }

    /// Records the execution backend that runs the program.
    pub fn set_execution_backend(&mut self, backend: ExecutionBackend) {
        self.execution_backend = backend;
    }

    /// Records the memory figure of the program the runtime runs now, `None` while it runs none
    /// (specification 34 MEM-12).
    pub fn set_program_memory(&mut self, memory: Option<ProgramMemory>) {
        self.program_memory = memory;
    }

    /// Turns call profiling on or off; turning it off discards the recorded calls.
    pub fn set_profiling_enabled(&mut self, enabled: bool) {
        self.profiling_enabled = enabled;
        if !enabled {
            self.profile_calls.clear();
        }
    }

    /// Records one call of `kind` `name` that took `duration`, while profiling is on. Calls whose
    /// text `kind:name` is the same share one entry. A call already recorded is found by that text,
    /// compared without building it (checklist P1-6).
    pub fn record_call(&mut self, kind: &str, name: &str, duration: Duration) {
        if !self.profiling_enabled {
            return;
        }
        if let Some(entry) = self
            .profile_calls
            .iter_mut()
            .find(|entry| entry.is(kind, name))
        {
            entry.stats.record(duration);
            return;
        }
        let mut stats = CallStats::default();
        stats.record(duration);
        self.profile_calls.push(CallProfileEntry {
            kind: SmolStr::new(kind),
            name: SmolStr::new(name),
            key: SmolStr::new(format!("{kind}:{name}")),
            stats,
        });
    }

    /// A snapshot of every metric.
    ///
    /// Calls are ranked by their mean contribution per cycle, ties by key; the five largest are
    /// the top contributors. Tasks are sorted by name.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // per-cycle means divide by the cycle count
    pub fn snapshot(&self) -> RuntimeMetricsSnapshot {
        let cycle = self.cycle();
        let cycle_avg = cycle.avg_ms;
        let cycle_last = cycle.last_ms;
        let cycle_samples = cycle.samples.max(1) as f64;
        let mut calls = self
            .profile_calls
            .iter()
            .map(|entry| CallStatsSnapshot {
                key: entry.key.clone(),
                kind: entry.kind.clone(),
                name: entry.name.clone(),
                min_ms: entry.stats.min_ms(),
                max_ms: entry.stats.max_ms(),
                avg_ms: entry.stats.avg_ms(),
                last_ms: entry.stats.last_ms(),
                calls: entry.stats.calls(),
                avg_cycle_ms: entry.stats.total_ms() / cycle_samples,
            })
            .collect::<Vec<_>>();
        calls.sort_by(|left, right| {
            right
                .avg_cycle_ms
                .total_cmp(&left.avg_cycle_ms)
                .then_with(|| left.key.cmp(&right.key))
        });
        let top_contributors = calls
            .iter()
            .take(5)
            .map(|call| BudgetContributorSnapshot {
                key: call.key.clone(),
                kind: call.kind.clone(),
                name: call.name.clone(),
                avg_cycle_ms: call.avg_cycle_ms,
                cycle_pct: if cycle_avg > 0.0 {
                    (call.avg_cycle_ms * 100.0) / cycle_avg
                } else {
                    0.0
                },
                last_ms: call.last_ms,
                last_cycle_pct: if cycle_last > 0.0 {
                    (call.last_ms * 100.0) / cycle_last
                } else {
                    0.0
                },
            })
            .collect();
        let mut tasks = self
            .tasks
            .iter()
            .map(|(name, stats)| TaskStatsSnapshot {
                name: name.clone(),
                min_ms: stats.min_ms(),
                max_ms: stats.max_ms(),
                avg_ms: stats.avg_ms(),
                last_ms: stats.last_ms(),
                overruns: stats.overruns,
            })
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| left.name.cmp(&right.name));
        RuntimeMetricsSnapshot {
            uptime_ms: self.uptime_ms(),
            execution_backend: self.execution_backend,
            cycle,
            cycle_percentiles: self.cycle_window.snapshot(),
            faults: self.faults,
            overruns: self.overruns,
            scan_allocations: self.scan_allocations,
            program_memory: self.program_memory,
            tasks,
            profiling: ProfilingSnapshot {
                enabled: self.profiling_enabled,
                calls,
                top_contributors,
            },
        }
    }
}

impl<C: MonotonicClock + Default> RuntimeMetrics<C> {
    /// Empty metrics measured with a new clock of type `C`.
    #[must_use]
    pub fn new() -> Self {
        Self::with_clock(C::default())
    }
}

impl<C: MonotonicClock + Default> Default for RuntimeMetrics<C> {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics of one task in a [`RuntimeMetricsSnapshot`].
#[derive(Debug, Clone)]
pub struct TaskStatsSnapshot {
    /// Task name.
    pub name: SmolStr,
    /// Shortest recorded execution, in milliseconds.
    pub min_ms: f64,
    /// Longest recorded execution, in milliseconds.
    pub max_ms: f64,
    /// Mean of the recorded executions, in milliseconds.
    pub avg_ms: f64,
    /// Most recent execution, in milliseconds.
    pub last_ms: f64,
    /// Missed periodic activations.
    pub overruns: u64,
}

/// A snapshot of every runtime metric.
#[derive(Debug, Clone, Default)]
pub struct RuntimeMetricsSnapshot {
    /// Milliseconds since the metrics started.
    pub uptime_ms: u64,
    /// Execution backend that runs the program.
    pub execution_backend: ExecutionBackend,
    /// Scan cycle statistics.
    pub cycle: CycleStats,
    /// Percentiles of the most recent 512 cycles.
    pub cycle_percentiles: CyclePercentilesSnapshot,
    /// Recorded faults.
    pub faults: u64,
    /// Missed periodic activations of all tasks.
    pub overruns: u64,
    /// The scans' allocator operations since the metrics started; `None` when none was counted.
    pub scan_allocations: Option<ScanAllocations>,
    /// The memory figure of the program the runtime runs; `None` while it runs none.
    pub program_memory: Option<ProgramMemory>,
    /// Statistics of each task, sorted by name.
    pub tasks: Vec<TaskStatsSnapshot>,
    /// Call profiling.
    pub profiling: ProfilingSnapshot,
}

/// Percentiles of the most recent 512 cycle durations, in milliseconds.
#[derive(Debug, Clone, Copy, Default)]
pub struct CyclePercentilesSnapshot {
    /// Median.
    pub p50_ms: f64,
    /// 95th percentile.
    pub p95_ms: f64,
    /// 99th percentile.
    pub p99_ms: f64,
    /// Longest cycle in the window.
    pub max_ms: f64,
    /// Cycles in the window.
    pub window_samples: u64,
}

/// Statistics of one profiled call in a [`ProfilingSnapshot`].
#[derive(Debug, Clone)]
pub struct CallStatsSnapshot {
    /// `kind:name` of the call.
    pub key: SmolStr,
    /// Kind of the call, for example `program`.
    pub kind: SmolStr,
    /// Name of the called unit.
    pub name: SmolStr,
    /// Shortest recorded call, in milliseconds.
    pub min_ms: f64,
    /// Longest recorded call, in milliseconds.
    pub max_ms: f64,
    /// Mean of the recorded calls, in milliseconds.
    pub avg_ms: f64,
    /// Most recent call, in milliseconds.
    pub last_ms: f64,
    /// Recorded calls.
    pub calls: u64,
    /// Mean time per cycle spent in the call, in milliseconds.
    pub avg_cycle_ms: f64,
}

/// One of the largest contributors to the cycle time.
#[derive(Debug, Clone)]
pub struct BudgetContributorSnapshot {
    /// `kind:name` of the call.
    pub key: SmolStr,
    /// Kind of the call.
    pub kind: SmolStr,
    /// Name of the called unit.
    pub name: SmolStr,
    /// Mean time per cycle spent in the call, in milliseconds.
    pub avg_cycle_ms: f64,
    /// That mean as a percentage of the mean cycle; it is not capped at 100.
    pub cycle_pct: f64,
    /// Most recent call, in milliseconds.
    pub last_ms: f64,
    /// The most recent call as a percentage of the most recent cycle.
    pub last_cycle_pct: f64,
}

/// Call profiling in a [`RuntimeMetricsSnapshot`].
#[derive(Debug, Clone, Default)]
pub struct ProfilingSnapshot {
    /// Whether calls are profiled.
    pub enabled: bool,
    /// Every profiled call, largest contribution first.
    pub calls: Vec<CallStatsSnapshot>,
    /// The five largest contributors.
    pub top_contributors: Vec<BudgetContributorSnapshot>,
}

/// The index of `quantile` among `len` values in ascending order: `round((len - 1) * quantile)`,
/// the quantile clamped to 0 and 1 (specification 11, the cycle percentiles).
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // the index is rounded from a clamped, non-negative position
fn rank(len: usize, quantile: f64) -> usize {
    let last = len.saturating_sub(1);
    let position = (last as f64) * quantile.clamp(0.0, 1.0);
    (trust_runtime_core::numeric::math::round(position) as usize).min(last)
}

/// The values at the indexes `ranks`, in ascending order and each less than the length, that
/// `values` would have sorted ascending. Each is found by a selection in the part above the one
/// found before, which holds only values no smaller, so the three percentiles of the window take
/// about two comparisons per value in all, where a sort takes about nine. `values` is reordered.
fn order_statistics<const N: usize>(values: &mut [u64], ranks: [usize; N]) -> [u64; N] {
    let mut found = [0u64; N];
    // `values[..start]` holds the `start` smallest values once a selection has run.
    let mut start = 0;
    let mut previous: Option<(usize, u64)> = None;
    for (slot, rank) in found.iter_mut().zip(ranks) {
        if let Some((_, value)) = previous.filter(|(at, _)| *at == rank) {
            *slot = value;
            continue;
        }
        let (_, value, _) = values[start..].select_nth_unstable(rank - start);
        *slot = *value;
        previous = Some((rank, *value));
        start = rank + 1;
    }
    found
}

#[cfg(test)]
mod contract_tests;
