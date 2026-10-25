//! Logical-time scheduling contract at the portable readiness boundary.
//!
//! Sources: spec 11 section 6.2 (task readiness and overrun accounting), spec 10 section 4.3
//! (nominal-deadline rule), spec 34 SCHED-05 and COMPAT-06. Every instant is an explicit logical
//! sample; no wall clock or sleep is involved.
//! Expected release samples are computed from the nominal deadlines written in each test, never by
//! calling the scheduler under test.

use trust_runtime_core::task::{evaluate_task_readiness, TaskReadiness, TaskState};
use trust_runtime_core::value::Duration;

fn ms(value: i64) -> Duration {
    Duration::from_millis(value)
}

/// First sample on a grid of `grid_ms` at or after `deadline_ms` (both non-negative).
fn first_sample_at_or_after(deadline_ms: i64, grid_ms: i64) -> i64 {
    (deadline_ms + grid_ms - 1) / grid_ms * grid_ms
}

#[test]
fn ten_and_twenty_five_ms_tasks_release_on_their_nominal_deadlines_over_300_ms() {
    // SCHED-05 / COMPAT-06: a 25 ms task registered at 0 ms and sampled every 10 ms activates at the
    // first sample at or after each nominal deadline 25k, and reports that deadline as due_at.
    // The 10 ms task is due on every sample. Both are due together at 50, 100, 150, 200, 250, 300 ms.
    let mut fast = TaskState::new(Duration::ZERO);
    let mut slow = TaskState::new(Duration::ZERO);
    let slow_releases: Vec<(i64, i64)> = (1..=12)
        .map(|k| (first_sample_at_or_after(25 * k, 10), 25 * k))
        .collect();
    let mut slow_samples = Vec::new();
    let mut latest_slow_deadline = 0;

    for sample in (10_i64..=300).step_by(10) {
        let fast_readiness = evaluate_task_readiness(&mut fast, ms(10), false, ms(sample));
        assert_eq!(
            fast_readiness,
            TaskReadiness {
                due_at: Some(ms(sample)),
                missed_intervals: 0,
            },
            "10 ms task at {sample} ms"
        );

        let slow_readiness = evaluate_task_readiness(&mut slow, ms(25), false, ms(sample));
        match slow_releases.iter().find(|(release, _)| *release == sample) {
            Some(&(_, deadline)) => {
                assert_eq!(
                    slow_readiness,
                    TaskReadiness {
                        due_at: Some(ms(deadline)),
                        missed_intervals: 0,
                    },
                    "25 ms task at {sample} ms"
                );
                slow_samples.push(sample);
                latest_slow_deadline = deadline;
            }
            None => assert_eq!(
                slow_readiness,
                TaskReadiness {
                    due_at: None,
                    missed_intervals: 0,
                },
                "25 ms task has no deadline at {sample} ms"
            ),
        }
        assert_eq!(
            slow.last_run,
            ms(latest_slow_deadline),
            "the baseline is the latest nominal deadline, not the sample"
        );
    }

    assert_eq!(
        slow_samples,
        vec![30, 50, 80, 100, 130, 150, 180, 200, 230, 250, 280, 300]
    );
    assert_eq!(fast.last_run, ms(300));
    assert_eq!(fast.overrun_count, 0);
    assert_eq!(slow.overrun_count, 0);
}

#[test]
fn a_clock_jump_keeps_the_following_nominal_deadlines() {
    // Spec 11 section 6.2: a 10 ms task first sampled at 35 ms reports due 10 ms, one activation
    // and two missed intervals; the baseline becomes 30 ms and the next deadline is 40 ms.
    let mut state = TaskState::new(Duration::ZERO);
    let jump = evaluate_task_readiness(&mut state, ms(10), false, ms(35));
    assert_eq!(
        jump,
        TaskReadiness {
            due_at: Some(ms(10)),
            missed_intervals: 2,
        }
    );
    assert_eq!(state.overrun_count, 2);
    assert_eq!(state.last_run, ms(30));

    for sample in [36, 39] {
        assert_eq!(
            evaluate_task_readiness(&mut state, ms(10), false, ms(sample)).due_at,
            None,
            "{sample} ms is before the 40 ms nominal deadline"
        );
    }
    let next = evaluate_task_readiness(&mut state, ms(10), false, ms(40));
    assert_eq!(
        next,
        TaskReadiness {
            due_at: Some(ms(40)),
            missed_intervals: 0,
        }
    );
    let after = evaluate_task_readiness(&mut state, ms(10), false, ms(50));
    assert_eq!(
        after,
        TaskReadiness {
            due_at: Some(ms(50)),
            missed_intervals: 0,
        }
    );
    assert_eq!(
        state.overrun_count, 2,
        "later periods add no replay and no miss"
    );
}

#[test]
fn a_jump_over_many_intervals_counts_once_and_never_replays() {
    // 1 ms interval registered at 0 ms and first sampled at 2000 ms: one activation due at the
    // first nominal deadline (1 ms), 1999 missed intervals, and the baseline at the last deadline.
    let interval = ms(1);
    let mut state = TaskState::new(Duration::ZERO);
    let jump = evaluate_task_readiness(&mut state, interval, false, ms(2000));
    assert_eq!(
        jump,
        TaskReadiness {
            due_at: Some(ms(1)),
            missed_intervals: 1999,
        }
    );
    assert_eq!(state.overrun_count, 1999);
    assert_eq!(state.last_run, ms(2000));

    let just_before = Duration::from_nanos(2_000_999_999);
    assert_eq!(
        evaluate_task_readiness(&mut state, interval, false, just_before).due_at,
        None,
        "999999 ns after the baseline is less than one interval"
    );
    let next = Duration::from_nanos(2_001_000_000);
    assert_eq!(
        evaluate_task_readiness(&mut state, interval, false, next),
        TaskReadiness {
            due_at: Some(ms(2001)),
            missed_intervals: 0,
        }
    );
    assert_eq!(state.overrun_count, 1999);
}

#[test]
fn held_single_suppresses_periodic_work_and_release_reports_the_first_pending_deadline() {
    // Spec 11 section 6.2 and spec 10 section 4.3: a held TRUE SINGLE suppresses periodic readiness;
    // the FALSE sample that releases it reports the first nominal deadline still pending, and a
    // later rising edge is an event activation that leaves the periodic baseline alone.
    let mut state = TaskState::new(Duration::ZERO);

    let rise = evaluate_task_readiness(&mut state, ms(10), true, ms(10));
    assert_eq!(
        rise,
        TaskReadiness {
            due_at: Some(ms(10)),
            missed_intervals: 0,
        }
    );
    for sample in [20, 30] {
        assert_eq!(
            evaluate_task_readiness(&mut state, ms(10), true, ms(sample)),
            TaskReadiness {
                due_at: None,
                missed_intervals: 0,
            },
            "held high at {sample} ms"
        );
    }
    assert_eq!(
        state.last_run,
        Duration::ZERO,
        "suppressed periods are not yet accounted"
    );

    let release = evaluate_task_readiness(&mut state, ms(10), false, ms(35));
    assert_eq!(
        release,
        TaskReadiness {
            due_at: Some(ms(10)),
            missed_intervals: 2,
        }
    );
    assert_eq!(state.last_run, ms(30));
    assert_eq!(state.overrun_count, 2);

    let second_rise = evaluate_task_readiness(&mut state, ms(10), true, ms(36));
    assert_eq!(
        second_rise,
        TaskReadiness {
            due_at: Some(ms(36)),
            missed_intervals: 0,
        }
    );
    assert_eq!(
        state.last_run,
        ms(30),
        "an event does not move the periodic baseline"
    );
    assert_eq!(
        state.overrun_count, 2,
        "an event does not change the missed count"
    );
    assert_eq!(
        evaluate_task_readiness(&mut state, ms(10), true, ms(40)).due_at,
        None,
        "still held high at the 40 ms deadline"
    );

    let after = evaluate_task_readiness(&mut state, ms(10), false, ms(45));
    assert_eq!(
        after,
        TaskReadiness {
            due_at: Some(ms(40)),
            missed_intervals: 0,
        }
    );
    assert_eq!(state.last_run, ms(40));
}
