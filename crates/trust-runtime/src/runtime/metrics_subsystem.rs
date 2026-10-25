//! Metrics subsystem for runtime statistics.

use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use smol_str::SmolStr;

use crate::metrics::{ProgramMemory, RuntimeMetrics};

pub(super) struct MetricsSubsystem {
    sink: Option<Arc<Mutex<RuntimeMetrics>>>,
}

impl MetricsSubsystem {
    pub(super) fn new() -> Self {
        Self { sink: None }
    }

    pub(super) fn set_sink(&mut self, metrics: Arc<Mutex<RuntimeMetrics>>) {
        self.sink = Some(metrics);
    }

    /// Records the memory figure of the installed program (specification 34 MEM-12).
    pub(super) fn set_program_memory(&self, memory: ProgramMemory) {
        if let Some(metrics) = self.sink.as_ref()
            && let Ok(mut guard) = metrics.lock()
        {
            guard.set_program_memory(Some(memory));
        }
    }

    pub(super) fn start_timer(&self) -> Option<Instant> {
        self.sink.as_ref().map(|_| Instant::now())
    }

    pub(super) fn record_cycle(&self, duration: StdDuration) {
        if let Some(metrics) = self.sink.as_ref()
            && let Ok(mut guard) = metrics.lock()
        {
            guard.record_cycle(duration);
        }
    }

    pub(super) fn record_task(&self, name: &SmolStr, duration: StdDuration) {
        if let Some(metrics) = self.sink.as_ref()
            && let Ok(mut guard) = metrics.lock()
        {
            guard.record_task(name, duration);
        }
    }

    pub(super) fn record_call(&self, kind: &str, name: &str, duration: StdDuration) {
        if let Some(metrics) = self.sink.as_ref()
            && let Ok(mut guard) = metrics.lock()
        {
            guard.record_call(kind, name, duration);
        }
    }

    pub(super) fn record_overrun(&self, name: &SmolStr, missed: u64) {
        if let Some(metrics) = self.sink.as_ref()
            && let Ok(mut guard) = metrics.lock()
        {
            guard.record_overrun(name, missed);
        }
    }

    pub(super) fn record_fault(&self) {
        if let Some(metrics) = self.sink.as_ref()
            && let Ok(mut guard) = metrics.lock()
        {
            guard.record_fault();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_subsystem_is_optional_and_routes_all_runtime_measurements() {
        let mut subsystem = MetricsSubsystem::new();
        assert!(subsystem.start_timer().is_none());
        subsystem.record_cycle(StdDuration::from_millis(1));
        subsystem.record_task(&"Fast".into(), StdDuration::from_millis(2));
        subsystem.record_overrun(&"Fast".into(), 4);
        subsystem.record_fault();

        let sink = Arc::new(Mutex::new(RuntimeMetrics::new()));
        subsystem.set_sink(sink.clone());
        assert!(subsystem.start_timer().is_some());
        subsystem.record_cycle(StdDuration::from_millis(1));
        subsystem.record_task(&"Fast".into(), StdDuration::from_millis(2));
        subsystem.record_overrun(&"Fast".into(), 4);
        subsystem.record_fault();

        let snapshot = sink.lock().unwrap().snapshot();
        assert_eq!(snapshot.cycle.last_ms, 1.0);
        assert_eq!(snapshot.cycle.min_ms, 1.0);
        assert_eq!(snapshot.cycle.max_ms, 1.0);
        assert_eq!(snapshot.tasks.len(), 1);
        assert_eq!(snapshot.tasks[0].name, "Fast");
        assert_eq!(snapshot.tasks[0].overruns, 4);
        assert_eq!(snapshot.overruns, 4);
        assert_eq!(snapshot.faults, 1);
    }
}
