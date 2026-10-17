//! Runtime metrics collection.
//!
//! The metrics are shared runtime code (spec 34 ARCH-11, `trust_runtime_services::metrics`); the
//! host measures their uptime with `std::time::Instant`.

#![allow(missing_docs)]

use std::time::Instant;

pub use trust_runtime_services::metrics::{
    BudgetContributorSnapshot, CallStats, CallStatsSnapshot, CyclePercentilesSnapshot, CycleStats,
    ProfilingSnapshot, RuntimeMetricsSnapshot, TaskStats, TaskStatsSnapshot,
};

/// The host's monotonic clock: the time since the clock was created.
#[derive(Debug, Clone, Copy)]
pub struct HostMonotonicClock {
    origin: Instant,
}

impl Default for HostMonotonicClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl trust_runtime_core::ports::MonotonicClock for HostMonotonicClock {
    fn now(&self) -> trust_runtime_core::ports::MonotonicInstant {
        trust_runtime_core::ports::MonotonicInstant::from_origin(self.origin.elapsed())
    }
}

/// Runtime metrics whose uptime starts when they are created.
pub type RuntimeMetrics = trust_runtime_services::metrics::RuntimeMetrics<HostMonotonicClock>;

#[cfg(test)]
mod tests {
    use super::*;
    use trust_runtime_core::ports::MonotonicClock;

    #[test]
    fn host_clock_and_metrics_uptime_never_decrease() {
        let clock = HostMonotonicClock::default();
        let first = clock.now();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = clock.now();
        assert!(
            second.saturating_since(first).as_duration() >= std::time::Duration::from_millis(2)
        );

        let metrics = RuntimeMetrics::new();
        let earlier = metrics.snapshot().uptime_ms;
        std::thread::sleep(std::time::Duration::from_millis(2));
        let later = metrics.snapshot().uptime_ms;
        assert!(
            later >= earlier + 2,
            "uptime went from {earlier} to {later} ms"
        );
    }
}
