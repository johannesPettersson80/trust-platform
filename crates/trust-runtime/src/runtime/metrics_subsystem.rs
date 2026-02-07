//! Metrics subsystem for runtime statistics.

use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use smol_str::SmolStr;

use crate::metrics::RuntimeMetrics;

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

    pub(super) fn start_timer(&self) -> Option<Instant> {
        self.sink.as_ref().map(|_| Instant::now())
    }

    pub(super) fn record_cycle(&self, duration: StdDuration) {
        if let Some(metrics) = self.sink.as_ref() {
            if let Ok(mut guard) = metrics.lock() {
                guard.record_cycle(duration);
            }
        }
    }

    pub(super) fn record_task(&self, name: &SmolStr, duration: StdDuration) {
        if let Some(metrics) = self.sink.as_ref() {
            if let Ok(mut guard) = metrics.lock() {
                guard.record_task(name, duration);
            }
        }
    }

    pub(super) fn record_profile_call(&self, kind: &str, name: &SmolStr, duration: StdDuration) {
        if let Some(metrics) = self.sink.as_ref() {
            if let Ok(mut guard) = metrics.lock() {
                guard.record_call(kind, name, duration);
            }
        }
    }

    pub(super) fn record_overrun(&self, name: &SmolStr, missed: u64) {
        if let Some(metrics) = self.sink.as_ref() {
            if let Ok(mut guard) = metrics.lock() {
                guard.record_overrun(name, missed);
            }
        }
    }

    pub(super) fn record_fault(&self) {
        if let Some(metrics) = self.sink.as_ref() {
            if let Ok(mut guard) = metrics.lock() {
                guard.record_fault();
            }
        }
    }
}
