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
use core::cmp::Ordering;
use core::time::Duration;

use smol_str::SmolStr;

use crate::execution_backend::ExecutionBackend;
use trust_runtime_core::ports::MonotonicClock;

const CYCLE_LATENCY_WINDOW_SIZE: usize = 512;

/// Milliseconds of a duration.
fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Minimum, maximum, mean and last scan cycle duration, in milliseconds.
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
    /// Records one cycle that took `duration`.
    #[allow(clippy::cast_precision_loss)] // the running mean weighs by the sample count
    pub fn record(&mut self, duration: Duration) {
        let ms = millis(duration);
        self.last_ms = ms;
        if self.samples == 0 {
            self.min_ms = ms;
            self.max_ms = ms;
            self.avg_ms = ms;
        } else {
            if ms < self.min_ms {
                self.min_ms = ms;
            }
            if ms > self.max_ms {
                self.max_ms = ms;
            }
            let total = self.avg_ms * self.samples as f64 + ms;
            self.avg_ms = total / (self.samples as f64 + 1.0);
        }
        self.samples = self.samples.saturating_add(1);
    }

    /// Recorded cycles.
    #[must_use]
    pub const fn samples(&self) -> u64 {
        self.samples
    }
}

#[derive(Debug, Clone)]
struct CycleLatencyWindow {
    values: [f64; CYCLE_LATENCY_WINDOW_SIZE],
    len: usize,
    next: usize,
}

impl CycleLatencyWindow {
    fn record_ms(&mut self, value_ms: f64) {
        self.values[self.next] = value_ms;
        self.next = (self.next + 1) % CYCLE_LATENCY_WINDOW_SIZE;
        if self.len < CYCLE_LATENCY_WINDOW_SIZE {
            self.len += 1;
        }
    }

    fn snapshot(&self) -> CyclePercentilesSnapshot {
        if self.len == 0 {
            return CyclePercentilesSnapshot::default();
        }
        let mut values = self.values[..self.len].to_vec();
        values.sort_by(f64::total_cmp);
        CyclePercentilesSnapshot {
            p50_ms: percentile(&values, 0.50),
            p95_ms: percentile(&values, 0.95),
            p99_ms: percentile(&values, 0.99),
            max_ms: *values.last().unwrap_or(&0.0),
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
            values: [0.0; CYCLE_LATENCY_WINDOW_SIZE],
            len: 0,
            next: 0,
        }
    }
}

/// Duration statistics and missed activations of one task, in milliseconds.
#[derive(Debug, Clone, Copy, Default)]
pub struct TaskStats {
    /// Shortest recorded execution.
    pub min_ms: f64,
    /// Longest recorded execution.
    pub max_ms: f64,
    /// Mean of the recorded executions.
    pub avg_ms: f64,
    /// Most recent execution.
    pub last_ms: f64,
    /// Missed periodic activations.
    pub overruns: u64,
    samples: u64,
}

impl TaskStats {
    /// Records one execution that took `duration`.
    #[allow(clippy::cast_precision_loss)] // the running mean weighs by the sample count
    pub fn record(&mut self, duration: Duration) {
        let ms = millis(duration);
        self.last_ms = ms;
        if self.samples == 0 {
            self.min_ms = ms;
            self.max_ms = ms;
            self.avg_ms = ms;
        } else {
            if ms < self.min_ms {
                self.min_ms = ms;
            }
            if ms > self.max_ms {
                self.max_ms = ms;
            }
            let total = self.avg_ms * self.samples as f64 + ms;
            self.avg_ms = total / (self.samples as f64 + 1.0);
        }
        self.samples = self.samples.saturating_add(1);
    }

    /// Counts `missed` periodic activations.
    pub fn record_overrun(&mut self, missed: u64) {
        self.overruns = self.overruns.saturating_add(missed);
    }
}

/// Duration statistics of one profiled call, in milliseconds.
#[derive(Debug, Clone, Copy, Default)]
pub struct CallStats {
    /// Shortest recorded call.
    pub min_ms: f64,
    /// Longest recorded call.
    pub max_ms: f64,
    /// Mean of the recorded calls.
    pub avg_ms: f64,
    /// Most recent call.
    pub last_ms: f64,
    /// Recorded calls.
    pub calls: u64,
    total_ms: f64,
}

impl CallStats {
    /// Records one call that took `duration`.
    #[allow(clippy::cast_precision_loss)] // the running mean weighs by the call count
    pub fn record(&mut self, duration: Duration) {
        let ms = millis(duration);
        self.last_ms = ms;
        self.total_ms += ms;
        if self.calls == 0 {
            self.min_ms = ms;
            self.max_ms = ms;
            self.avg_ms = ms;
        } else {
            if ms < self.min_ms {
                self.min_ms = ms;
            }
            if ms > self.max_ms {
                self.max_ms = ms;
            }
            let total = self.avg_ms * self.calls as f64 + ms;
            self.avg_ms = total / (self.calls as f64 + 1.0);
        }
        self.calls = self.calls.saturating_add(1);
    }
}

/// The key of a profiled call. It orders and compares as its text `kind:name`, the key the
/// snapshot reports, so a call is found without building that text.
#[derive(Debug, Clone)]
struct CallKey {
    kind: SmolStr,
    name: SmolStr,
}

impl CallKey {
    fn text(&self) -> impl Iterator<Item = u8> + '_ {
        self.kind
            .bytes()
            .chain(core::iter::once(b':'))
            .chain(self.name.bytes())
    }
}

impl Ord for CallKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.text().cmp(other.text())
    }
}

impl PartialOrd for CallKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for CallKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for CallKey {}

#[derive(Debug, Clone)]
struct CallProfileEntry {
    /// The text `kind:name`, built once, when the call is first recorded.
    key: SmolStr,
    stats: CallStats,
}

/// Metrics of one runtime, measured with the platform clock `C`.
#[derive(Debug, Clone)]
pub struct RuntimeMetrics<C> {
    clock: C,
    execution_backend: ExecutionBackend,
    /// Scan cycle statistics.
    pub cycle: CycleStats,
    cycle_window: CycleLatencyWindow,
    /// Statistics of each task, by task name.
    pub tasks: BTreeMap<SmolStr, TaskStats>,
    /// Whether calls are profiled.
    pub profiling_enabled: bool,
    profile_calls: BTreeMap<CallKey, CallProfileEntry>,
    /// Recorded faults.
    pub faults: u64,
    /// Missed periodic activations of all tasks.
    pub overruns: u64,
}

impl<C: MonotonicClock> RuntimeMetrics<C> {
    /// Empty metrics whose uptime is the time `clock` has measured since its origin.
    #[must_use]
    pub fn with_clock(clock: C) -> Self {
        Self {
            clock,
            execution_backend: ExecutionBackend::BytecodeVm,
            cycle: CycleStats::default(),
            cycle_window: CycleLatencyWindow::default(),
            tasks: BTreeMap::new(),
            profiling_enabled: true,
            profile_calls: BTreeMap::new(),
            faults: 0,
            overruns: 0,
        }
    }

    /// Starts the metrics again in place, as [`Self::with_clock`] creates them: the uptime is
    /// `clock`'s, and every statistic, count and recorded call is cleared.
    pub fn restart(&mut self, clock: C) {
        self.clock = clock;
        self.execution_backend = ExecutionBackend::BytecodeVm;
        self.cycle = CycleStats::default();
        self.cycle_window.clear();
        self.tasks.clear();
        self.profiling_enabled = true;
        self.profile_calls.clear();
        self.faults = 0;
        self.overruns = 0;
    }

    /// Milliseconds since the metrics started: the clock's time since its origin.
    #[must_use]
    pub fn uptime_ms(&self) -> u64 {
        u64::try_from(self.clock.now().since_origin().as_millis()).unwrap_or(u64::MAX)
    }

    /// Records one scan cycle that took `duration`.
    pub fn record_cycle(&mut self, duration: Duration) {
        self.cycle.record(duration);
        self.cycle_window.record_ms(millis(duration));
    }

    /// Records one execution of task `name` that took `duration`.
    pub fn record_task(&mut self, name: &SmolStr, duration: Duration) {
        let entry = self.tasks.entry(name.clone()).or_default();
        entry.record(duration);
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

    /// Counts one fault.
    pub fn record_fault(&mut self) {
        self.faults = self.faults.saturating_add(1);
    }

    /// Records the execution backend that runs the program.
    pub fn set_execution_backend(&mut self, backend: ExecutionBackend) {
        self.execution_backend = backend;
    }

    /// Turns call profiling on or off; turning it off discards the recorded calls.
    pub fn set_profiling_enabled(&mut self, enabled: bool) {
        self.profiling_enabled = enabled;
        if !enabled {
            self.profile_calls.clear();
        }
    }

    /// Records one call of `kind` `name` that took `duration`, while profiling is on. Calls whose
    /// text `kind:name` is the same share one entry.
    pub fn record_call(&mut self, kind: &str, name: &SmolStr, duration: Duration) {
        if !self.profiling_enabled {
            return;
        }
        let key = CallKey {
            kind: SmolStr::new(kind),
            name: name.clone(),
        };
        if let Some(entry) = self.profile_calls.get_mut(&key) {
            entry.stats.record(duration);
            return;
        }
        let mut stats = CallStats::default();
        stats.record(duration);
        let text = SmolStr::new(format!("{kind}:{name}"));
        self.profile_calls
            .insert(key, CallProfileEntry { key: text, stats });
    }

    /// A snapshot of every metric.
    ///
    /// Calls are ranked by their mean contribution per cycle, ties by key; the five largest are
    /// the top contributors. Tasks are sorted by name.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // per-cycle means divide by the cycle count
    pub fn snapshot(&self) -> RuntimeMetricsSnapshot {
        let cycle_avg = self.cycle.avg_ms;
        let cycle_last = self.cycle.last_ms;
        let cycle_samples = self.cycle.samples.max(1) as f64;
        let mut calls = self
            .profile_calls
            .iter()
            .map(|(key, entry)| CallStatsSnapshot {
                key: entry.key.clone(),
                kind: key.kind.clone(),
                name: key.name.clone(),
                min_ms: entry.stats.min_ms,
                max_ms: entry.stats.max_ms,
                avg_ms: entry.stats.avg_ms,
                last_ms: entry.stats.last_ms,
                calls: entry.stats.calls,
                avg_cycle_ms: entry.stats.total_ms / cycle_samples,
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
                min_ms: stats.min_ms,
                max_ms: stats.max_ms,
                avg_ms: stats.avg_ms,
                last_ms: stats.last_ms,
                overruns: stats.overruns,
            })
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| left.name.cmp(&right.name));
        RuntimeMetricsSnapshot {
            uptime_ms: self.uptime_ms(),
            execution_backend: self.execution_backend,
            cycle: self.cycle,
            cycle_percentiles: self.cycle_window.snapshot(),
            faults: self.faults,
            overruns: self.overruns,
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

/// The value at `quantile` of `sorted_values`, by the rounded index.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)] // the index is rounded from a clamped, non-negative position
fn percentile(sorted_values: &[f64], quantile: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    let clamped = quantile.clamp(0.0, 1.0);
    let position = (sorted_values.len().saturating_sub(1) as f64) * clamped;
    let index = trust_runtime_core::numeric::math::round(position) as usize;
    sorted_values
        .get(index)
        .copied()
        .unwrap_or_else(|| *sorted_values.last().unwrap_or(&0.0))
}

#[cfg(test)]
mod contract_tests;
