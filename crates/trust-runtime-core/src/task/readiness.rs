//! Portable event and periodic task readiness evaluation.

use crate::value::Duration;

use super::state::TaskState;

/// Result of one task readiness evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskReadiness {
    /// Time at which the task became due, if it should run this cycle.
    pub due_at: Option<Duration>,
    /// Missed periodic intervals observed during this evaluation.
    pub missed_intervals: u64,
}

/// Evaluate event and periodic task readiness for one cycle.
pub fn evaluate_task_readiness(
    state: &mut TaskState,
    interval: Duration,
    single_now: bool,
    now: Duration,
) -> TaskReadiness {
    let event_due = !state.last_single && single_now;
    let interval_nanos = interval.as_nanos();
    let elapsed = now.as_nanos().saturating_sub(state.last_run.as_nanos());
    let periodic_due = interval_nanos > 0 && !single_now && elapsed >= interval_nanos;
    let mut due_at = None;
    let mut missed_intervals = 0;

    if event_due {
        due_at = Some(now);
    }
    if periodic_due {
        let intervals = elapsed / interval_nanos;
        if intervals > 1 {
            missed_intervals = (intervals - 1) as u64;
            state.overrun_count = state.overrun_count.saturating_add(missed_intervals);
        }
        let due_time =
            Duration::from_nanos(state.last_run.as_nanos().saturating_add(interval_nanos));
        due_at = Some(match due_at {
            Some(existing) if existing.as_nanos() <= due_time.as_nanos() => existing,
            _ => due_time,
        });
        state.last_run = now;
    }
    state.last_single = single_now;

    TaskReadiness {
        due_at,
        missed_intervals,
    }
}
