//! Portable task scheduling state.

use crate::value::Duration;

/// Scheduling state for a task.
#[derive(Debug, Clone)]
pub struct TaskState {
    /// Whether the event input was high on the previous cycle.
    pub last_single: bool,
    /// Logical time when the task last ran.
    pub last_run: Duration,
    /// Number of missed periodic intervals.
    pub overrun_count: u64,
}

impl TaskState {
    /// Create task state at the current runtime time.
    #[must_use]
    pub fn new(current_time: Duration) -> Self {
        Self {
            last_single: false,
            last_run: current_time,
            overrun_count: 0,
        }
    }
}
