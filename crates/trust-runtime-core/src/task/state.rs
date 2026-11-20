//! Portable task scheduling state.

use crate::value::Duration;

/// Scheduling state for a task.
#[derive(Debug, Clone)]
pub struct TaskState {
    /// Whether the event input was high on the previous cycle.
    pub last_single: bool,
    /// The latest nominal deadline the task has accounted for: the due time of its last
    /// periodic run, not the time it ran (specification 11, "Task readiness").
    pub last_run: Duration,
    /// Number of missed periodic intervals.
    pub overrun_count: u64,
}

impl TaskState {
    /// The state of a task registered at `current_time` with the periodic `interval`: its first
    /// nominal deadline is `current_time` itself, so that a periodic task is due at its start and
    /// at every interval after it (specification 10 §4.3; IEC 61131-3 Ed.3 §6.8.2, whose Table 63
    /// schedules run the periodic program organization units from t = 0). `last_run`, the latest
    /// deadline accounted for, therefore starts one interval before `current_time`, with signed
    /// saturation; a task without a positive interval keeps `current_time`.
    #[must_use]
    pub fn new(current_time: Duration, interval: Duration) -> Self {
        let interval = interval.as_nanos();
        let last_run = if interval > 0 {
            Duration::from_nanos(current_time.as_nanos().saturating_sub(interval))
        } else {
            current_time
        };
        Self {
            last_single: false,
            last_run,
            overrun_count: 0,
        }
    }
}
