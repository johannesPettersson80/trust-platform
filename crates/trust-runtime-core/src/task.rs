//! Portable task configuration records.

mod config;
mod readiness;
mod state;

pub use config::TaskConfig;
pub use readiness::{evaluate_task_readiness, TaskReadiness};
pub use state::TaskState;

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
    fn task_state_new_preserves_time_and_clears_event_and_overrun_history() {
        let current_time = Duration::from_nanos(-17);
        let state = TaskState::new(current_time);

        assert!(!state.last_single);
        assert_eq!(state.last_run, current_time);
        assert_eq!(state.overrun_count, 0);
    }

    #[test]
    fn task_readiness_tracks_periodic_due_time_and_overrun() {
        let mut one_period = TaskState::new(Duration::ZERO);
        let exact = evaluate_task_readiness(
            &mut one_period,
            Duration::from_millis(10),
            false,
            Duration::from_millis(10),
        );

        assert_eq!(exact.due_at, Some(Duration::from_millis(10)));
        assert_eq!(exact.missed_intervals, 0);
        assert_eq!(one_period.overrun_count, 0);
        assert_eq!(one_period.last_run, Duration::from_millis(10));

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

        let mut disabled = TaskState {
            last_single: false,
            last_run: Duration::from_millis(5),
            overrun_count: 7,
        };
        for interval in [Duration::ZERO, Duration::from_nanos(-1)] {
            let readiness =
                evaluate_task_readiness(&mut disabled, interval, false, Duration::from_millis(100));
            assert_eq!(readiness.due_at, None);
            assert_eq!(readiness.missed_intervals, 0);
            assert_eq!(disabled.last_run, Duration::from_millis(5));
            assert_eq!(disabled.overrun_count, 7);
        }
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
        assert_eq!(state.overrun_count, 0);
        assert!(state.last_single);

        let held_high = evaluate_task_readiness(
            &mut state,
            Duration::from_millis(10),
            true,
            Duration::from_millis(30),
        );
        assert_eq!(held_high.due_at, None);
        assert_eq!(held_high.missed_intervals, 0);
        assert_eq!(state.last_run, Duration::ZERO);
        assert_eq!(state.overrun_count, 0);
    }

    #[test]
    fn task_readiness_ignores_backward_clock_step_until_prior_baseline() {
        let mut state = TaskState::new(Duration::from_millis(100));

        let backward = evaluate_task_readiness(
            &mut state,
            Duration::from_millis(10),
            false,
            Duration::from_millis(20),
        );
        let baseline = evaluate_task_readiness(
            &mut state,
            Duration::from_millis(10),
            false,
            Duration::from_millis(100),
        );
        let before_deadline = evaluate_task_readiness(
            &mut state,
            Duration::from_millis(10),
            false,
            Duration::from_millis(109),
        );
        let deadline = evaluate_task_readiness(
            &mut state,
            Duration::from_millis(10),
            false,
            Duration::from_millis(110),
        );

        assert_eq!(backward.due_at, None);
        assert_eq!(backward.missed_intervals, 0);
        assert_eq!(baseline.due_at, None);
        assert_eq!(before_deadline.due_at, None);
        assert_eq!(deadline.due_at, Some(Duration::from_millis(110)));
        assert_eq!(deadline.missed_intervals, 0);
        assert_eq!(state.last_run, Duration::from_millis(110));
        assert_eq!(state.overrun_count, 0);
    }

    #[test]
    fn task_readiness_coalesces_forward_jump_after_host_pause() {
        let mut state = TaskState::new(Duration::ZERO);

        let resumed = evaluate_task_readiness(
            &mut state,
            Duration::from_millis(10),
            false,
            Duration::from_millis(55),
        );

        assert_eq!(resumed.due_at, Some(Duration::from_millis(10)));
        assert_eq!(resumed.missed_intervals, 4);
        assert_eq!(state.overrun_count, 4);
        assert_eq!(state.last_run, Duration::from_millis(55));

        let mut saturating = TaskState {
            last_single: false,
            last_run: Duration::ZERO,
            overrun_count: u64::MAX - 1,
        };
        let saturated = evaluate_task_readiness(
            &mut saturating,
            Duration::from_millis(10),
            false,
            Duration::from_millis(35),
        );
        assert_eq!(saturated.due_at, Some(Duration::from_millis(10)));
        assert_eq!(saturated.missed_intervals, 2);
        assert_eq!(saturating.overrun_count, u64::MAX);
        assert_eq!(saturating.last_run, Duration::from_millis(35));
    }
}
