//! Portable task configuration records.

use alloc::vec::Vec;
use smol_str::SmolStr;

use crate::value::{Duration, ValueRef};

/// Configuration for a task (periodic and/or event-driven).
#[derive(Debug, Clone)]
pub struct TaskConfig {
    /// Task name.
    pub name: SmolStr,
    /// Periodic interval. Zero means no periodic interval.
    pub interval: Duration,
    /// Optional event input name.
    pub single: Option<SmolStr>,
    /// Lower values run before higher values when tasks are ready together.
    pub priority: u32,
    /// Program instances executed by this task.
    pub programs: Vec<SmolStr>,
    /// Function block instances executed by this task.
    pub fb_instances: Vec<ValueRef>,
}

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

#[cfg(test)]
mod tests {
    use super::{evaluate_task_readiness, TaskConfig, TaskState};
    use crate::value::Duration;
    use alloc::{vec, vec::Vec};
    use smol_str::SmolStr;

    #[test]
    fn task_config_preserves_periodic_and_event_fields() {
        let task = TaskConfig {
            name: SmolStr::new("Fast"),
            interval: Duration::from_millis(10),
            single: Some(SmolStr::new("Start")),
            priority: 2,
            programs: vec![SmolStr::new("Main")],
            fb_instances: Vec::new(),
        };

        assert_eq!(task.name.as_str(), "Fast");
        assert_eq!(task.interval, Duration::from_millis(10));
        assert_eq!(task.single.as_deref(), Some("Start"));
        assert_eq!(task.priority, 2);
        assert_eq!(task.programs, vec![SmolStr::new("Main")]);
        assert!(task.fb_instances.is_empty());
    }

    #[test]
    fn task_readiness_tracks_periodic_due_time_and_overrun() {
        let mut state = TaskState::new(Duration::ZERO);

        let readiness = evaluate_task_readiness(
            &mut state,
            Duration::from_millis(10),
            false,
            Duration::from_millis(35),
        );

        assert_eq!(readiness.due_at, Some(Duration::from_millis(10)));
        assert_eq!(readiness.missed_intervals, 2);
        assert_eq!(state.overrun_count, 2);
        assert_eq!(state.last_run, Duration::from_millis(35));
    }

    #[test]
    fn task_readiness_tracks_event_edges_without_repeating_high_level() {
        let mut state = TaskState::new(Duration::ZERO);

        let first =
            evaluate_task_readiness(&mut state, Duration::ZERO, true, Duration::from_millis(1));
        let repeated_high =
            evaluate_task_readiness(&mut state, Duration::ZERO, true, Duration::from_millis(2));
        let low =
            evaluate_task_readiness(&mut state, Duration::ZERO, false, Duration::from_millis(3));
        let second_edge =
            evaluate_task_readiness(&mut state, Duration::ZERO, true, Duration::from_millis(4));

        assert_eq!(first.due_at, Some(Duration::from_millis(1)));
        assert_eq!(repeated_high.due_at, None);
        assert_eq!(low.due_at, None);
        assert_eq!(second_edge.due_at, Some(Duration::from_millis(4)));
    }

    #[test]
    fn task_readiness_prefers_earlier_due_time_when_event_and_periodic_overlap() {
        let mut state = TaskState::new(Duration::ZERO);

        let readiness = evaluate_task_readiness(
            &mut state,
            Duration::from_millis(10),
            true,
            Duration::from_millis(10),
        );

        assert_eq!(readiness.due_at, Some(Duration::from_millis(10)));
        assert_eq!(readiness.missed_intervals, 0);
        assert_eq!(state.last_run, Duration::ZERO);
    }
}
