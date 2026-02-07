//! Runtime metrics collection.

#![allow(missing_docs)]

use std::collections::HashMap;
use std::time::Instant;

use smol_str::SmolStr;

#[derive(Debug, Clone, Copy)]
pub struct CycleStats {
    pub min_ms: f64,
    pub max_ms: f64,
    pub avg_ms: f64,
    pub last_ms: f64,
    samples: u64,
}

impl CycleStats {
    pub fn record(&mut self, duration: std::time::Duration) {
        let ms = duration.as_secs_f64() * 1000.0;
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
}

impl Default for CycleStats {
    fn default() -> Self {
        Self {
            min_ms: 0.0,
            max_ms: 0.0,
            avg_ms: 0.0,
            last_ms: 0.0,
            samples: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TaskStats {
    pub min_ms: f64,
    pub max_ms: f64,
    pub avg_ms: f64,
    pub last_ms: f64,
    pub overruns: u64,
    samples: u64,
}

impl TaskStats {
    pub fn record(&mut self, duration: std::time::Duration) {
        let ms = duration.as_secs_f64() * 1000.0;
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

    pub fn record_overrun(&mut self, missed: u64) {
        self.overruns = self.overruns.saturating_add(missed);
    }
}

impl Default for TaskStats {
    fn default() -> Self {
        Self {
            min_ms: 0.0,
            max_ms: 0.0,
            avg_ms: 0.0,
            last_ms: 0.0,
            overruns: 0,
            samples: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CallStats {
    pub min_ms: f64,
    pub max_ms: f64,
    pub avg_ms: f64,
    pub last_ms: f64,
    pub calls: u64,
    total_ms: f64,
}

impl CallStats {
    pub fn record(&mut self, duration: std::time::Duration) {
        let ms = duration.as_secs_f64() * 1000.0;
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

impl Default for CallStats {
    fn default() -> Self {
        Self {
            min_ms: 0.0,
            max_ms: 0.0,
            avg_ms: 0.0,
            last_ms: 0.0,
            calls: 0,
            total_ms: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
struct CallProfileEntry {
    kind: SmolStr,
    name: SmolStr,
    stats: CallStats,
}

#[derive(Debug, Clone)]
pub struct RuntimeMetrics {
    start: Instant,
    pub cycle: CycleStats,
    pub tasks: HashMap<SmolStr, TaskStats>,
    pub profiling_enabled: bool,
    profile_calls: HashMap<SmolStr, CallProfileEntry>,
    pub faults: u64,
    pub overruns: u64,
}

impl RuntimeMetrics {
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            cycle: CycleStats::default(),
            tasks: HashMap::new(),
            profiling_enabled: true,
            profile_calls: HashMap::new(),
            faults: 0,
            overruns: 0,
        }
    }

    #[must_use]
    pub fn uptime_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    pub fn record_cycle(&mut self, duration: std::time::Duration) {
        self.cycle.record(duration);
    }

    pub fn record_task(&mut self, name: &SmolStr, duration: std::time::Duration) {
        let entry = self.tasks.entry(name.clone()).or_default();
        entry.record(duration);
    }

    pub fn record_overrun(&mut self, name: &SmolStr, missed: u64) {
        self.overruns = self.overruns.saturating_add(missed);
        let entry = self.tasks.entry(name.clone()).or_default();
        entry.record_overrun(missed);
    }

    pub fn record_fault(&mut self) {
        self.faults = self.faults.saturating_add(1);
    }

    pub fn set_profiling_enabled(&mut self, enabled: bool) {
        self.profiling_enabled = enabled;
        if !enabled {
            self.profile_calls.clear();
        }
    }

    pub fn record_call(&mut self, kind: &str, name: &SmolStr, duration: std::time::Duration) {
        if !self.profiling_enabled {
            return;
        }
        let key = SmolStr::new(format!("{kind}:{name}"));
        let entry = self
            .profile_calls
            .entry(key)
            .or_insert_with(|| CallProfileEntry {
                kind: SmolStr::new(kind),
                name: name.clone(),
                stats: CallStats::default(),
            });
        entry.stats.record(duration);
    }

    #[must_use]
    pub fn snapshot(&self) -> RuntimeMetricsSnapshot {
        let cycle_avg = self.cycle.avg_ms;
        let cycle_last = self.cycle.last_ms;
        let cycle_samples = self.cycle.samples.max(1) as f64;
        let mut calls = self
            .profile_calls
            .iter()
            .map(|(key, entry)| {
                let avg_cycle_ms = entry.stats.total_ms / cycle_samples;
                CallStatsSnapshot {
                    key: key.clone(),
                    kind: entry.kind.clone(),
                    name: entry.name.clone(),
                    min_ms: entry.stats.min_ms,
                    max_ms: entry.stats.max_ms,
                    avg_ms: entry.stats.avg_ms,
                    last_ms: entry.stats.last_ms,
                    calls: entry.stats.calls,
                    avg_cycle_ms,
                }
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
        let tasks = self
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
            .collect();
        RuntimeMetricsSnapshot {
            uptime_ms: self.uptime_ms(),
            cycle: self.cycle,
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

impl Default for RuntimeMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct TaskStatsSnapshot {
    pub name: SmolStr,
    pub min_ms: f64,
    pub max_ms: f64,
    pub avg_ms: f64,
    pub last_ms: f64,
    pub overruns: u64,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeMetricsSnapshot {
    pub uptime_ms: u64,
    pub cycle: CycleStats,
    pub faults: u64,
    pub overruns: u64,
    pub tasks: Vec<TaskStatsSnapshot>,
    pub profiling: ProfilingSnapshot,
}

#[derive(Debug, Clone)]
pub struct CallStatsSnapshot {
    pub key: SmolStr,
    pub kind: SmolStr,
    pub name: SmolStr,
    pub min_ms: f64,
    pub max_ms: f64,
    pub avg_ms: f64,
    pub last_ms: f64,
    pub calls: u64,
    pub avg_cycle_ms: f64,
}

#[derive(Debug, Clone)]
pub struct BudgetContributorSnapshot {
    pub key: SmolStr,
    pub kind: SmolStr,
    pub name: SmolStr,
    pub avg_cycle_ms: f64,
    pub cycle_pct: f64,
    pub last_ms: f64,
    pub last_cycle_pct: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ProfilingSnapshot {
    pub enabled: bool,
    pub calls: Vec<CallStatsSnapshot>,
    pub top_contributors: Vec<BudgetContributorSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::RuntimeMetrics;
    use smol_str::SmolStr;
    use std::time::Duration;

    #[test]
    fn profiling_records_call_entries_with_cycle_contribution() {
        let mut metrics = RuntimeMetrics::new();
        metrics.record_cycle(Duration::from_millis(10));
        metrics.record_cycle(Duration::from_millis(10));
        metrics.record_call("program", &SmolStr::new("MAIN"), Duration::from_millis(4));
        metrics.record_call("fb", &SmolStr::new("TON_1"), Duration::from_millis(2));

        let snapshot = metrics.snapshot();
        assert!(snapshot.profiling.enabled);
        assert_eq!(snapshot.profiling.calls.len(), 2);
        let main = snapshot
            .profiling
            .calls
            .iter()
            .find(|entry| entry.key.as_str() == "program:MAIN")
            .expect("missing program profile entry");
        assert_eq!(main.calls, 1);
        assert!(main.avg_cycle_ms > 0.0);
    }

    #[test]
    fn profiling_toggle_disables_and_reenables_collection() {
        let mut metrics = RuntimeMetrics::new();
        metrics.set_profiling_enabled(false);
        metrics.record_cycle(Duration::from_millis(10));
        metrics.record_call("program", &SmolStr::new("MAIN"), Duration::from_millis(3));
        let disabled = metrics.snapshot();
        assert!(!disabled.profiling.enabled);
        assert!(disabled.profiling.calls.is_empty());
        assert!(disabled.profiling.top_contributors.is_empty());

        metrics.set_profiling_enabled(true);
        metrics.record_call("program", &SmolStr::new("MAIN"), Duration::from_millis(3));
        let enabled = metrics.snapshot();
        assert!(enabled.profiling.enabled);
        assert_eq!(enabled.profiling.calls.len(), 1);
    }

    #[test]
    fn profiling_top_contributors_ranked_by_cycle_budget() {
        let mut metrics = RuntimeMetrics::new();
        for _ in 0..4 {
            metrics.record_cycle(Duration::from_millis(10));
            metrics.record_call("program", &SmolStr::new("MAIN"), Duration::from_millis(5));
            metrics.record_call("fb", &SmolStr::new("TON_1"), Duration::from_millis(2));
        }

        let snapshot = metrics.snapshot();
        let top = snapshot
            .profiling
            .top_contributors
            .first()
            .expect("expected top contributor");
        assert_eq!(top.key.as_str(), "program:MAIN");
        assert!(top.cycle_pct > 0.0);
    }
}
