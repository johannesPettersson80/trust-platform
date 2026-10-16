//! Bounded exhaustive check of periodic and event task readiness.
//!
//! Specification: `docs/specs/10-runtime-semantics.md`, task scheduling (the readiness pseudo-code
//! with saturating arithmetic, the registration baseline, and the rules for backward samples and
//! SINGLE); `docs/specs/11-runtime-engine.md`, "Task readiness and overrun accounting" (SINGLE edges,
//! non-positive intervals, nominal deadlines, and late samples).
//!
//! Production under test: `trust_runtime_core::task::evaluate_task_readiness`
//! (`crates/trust-runtime-core/src/task/readiness.rs:17`) over `TaskState::new`
//! (`crates/trust-runtime-core/src/task/state.rs:19`).
//!
//! Model. A periodic task has a registration baseline `B0`. Its nominal deadlines are `B0 + k * I`
//! for `k >= 1`. The oracle keeps `consumed`, the number of deadlines already passed (run or counted
//! as missed), so the production baseline must equal `B0 + consumed * I`. For a SINGLE-FALSE sample
//! with `I > 0`, the number of deadlines at or before `now` is `passed`: zero before the first
//! deadline, `(now - B0) div I` otherwise. A periodic run is due when `passed > consumed`. Its
//! `due_at` is the first unconsumed deadline `B0 + (consumed + 1) * I`, the missed count is
//! `passed - consumed - 1`, and `consumed` becomes `passed`. The production reaches the same
//! numbers through the saturating elapsed time and `last_run`, so the oracle is a separate
//! computation of the same contract.
//! A FALSE-to-TRUE edge of SINGLE makes the task due at the sample time and touches neither the
//! baseline nor the miss total. While SINGLE is TRUE, periodic readiness is suppressed. The saved
//! SINGLE sample starts FALSE (`TaskState::new`).
//!
//! Bounds. Interval `I` in {-1, 0, 1, 2, 3, 5} ns; negative and zero intervals disable periodic
//! work. Baseline `B0` in {-3, 0, 2, 7} ns. Samples in -2..=10 ns, which include backward steps,
//! samples inside an interval, exact deadlines, and gaps spanning several intervals. SINGLE in
//! {FALSE, TRUE}. Every sequence of four samples is run from each (I, B0) pair: 475,254 prefixes
//! per pair, 24 pairs, 11,406,096 samples in total.
//!
//! Assumptions. The domain lies far from the i64 range, so no saturation happens inside it. The
//! saturating arithmetic at the i64 extremes is checked with exact expectations at the end of this
//! file.
//!
//! Invariants checked after every sample, beyond equality with the oracle:
//! - no drift: `last_run == B0 + (runs + overrun_count) * I`, so each passed deadline is either one
//!   run or one counted miss, exactly once;
//! - every periodic `due_at` lies on the nominal grid `B0 + k * I` with `k >= 1`;
//! - every `due_at` is not later than its sample;
//! - `overrun_count` equals the total of the reported missed intervals.

use trust_runtime_core::task::{evaluate_task_readiness, TaskState};
use trust_runtime_core::value::Duration;

/// Interval values in nanoseconds. Zero and negative values disable periodic work.
const INTERVALS_NS: [i64; 6] = [-1, 0, 1, 2, 3, 5];
/// Registration baselines in nanoseconds, including a negative one.
const BASELINES_NS: [i64; 4] = [-3, 0, 2, 7];
/// Inclusive sample range in nanoseconds, starting before the baselines.
const FIRST_SAMPLE_NS: i64 = -2;
const LAST_SAMPLE_NS: i64 = 10;
/// Samples per sequence.
const SEQUENCE_LENGTH: u32 = 4;

/// Deadline-counting oracle for one task.
#[derive(Clone, Copy)]
struct Oracle {
    base: i128,
    interval: i128,
    /// Deadlines passed so far, whether they were run or counted as missed.
    consumed: i128,
    /// Periodic activations so far.
    runs: u64,
    /// Missed intervals so far.
    missed_total: u64,
    previous_single: bool,
}

impl Oracle {
    fn new(base: i64, interval: i64) -> Self {
        Self {
            base: i128::from(base),
            interval: i128::from(interval),
            consumed: 0,
            runs: 0,
            missed_total: 0,
            previous_single: false,
        }
    }

    /// Expected `(due_at, missed_intervals)` for one sample, updating the oracle.
    fn sample(&mut self, single: bool, now: i64) -> (Option<i64>, u64) {
        let now = i128::from(now);
        let mut due_at = (!self.previous_single && single).then_some(now);
        let mut missed = 0;
        if self.interval > 0 && !single {
            let passed = if now < self.base + self.interval {
                0
            } else {
                (now - self.base) / self.interval
            };
            if passed > self.consumed {
                due_at = Some(self.base + (self.consumed + 1) * self.interval);
                missed = u64::try_from(passed - self.consumed - 1).expect("missed count fits u64");
                self.consumed = passed;
                self.runs += 1;
                self.missed_total += missed;
            }
        }
        self.previous_single = single;
        let due_at = due_at.map(|nanos| i64::try_from(nanos).expect("sample domain fits i64"));
        (due_at, missed)
    }

    /// The baseline the production must hold: `B0 + consumed * I`.
    fn last_run(&self) -> i64 {
        i64::try_from(self.base + self.consumed * self.interval).expect("baseline fits i64")
    }
}

/// Runs every sample sequence of `depth` more samples below `state`, checking each sample.
fn explore(state: &TaskState, oracle: &Oracle, interval: i64, depth: u32, samples: &mut u64) {
    for single in [false, true] {
        for now in FIRST_SAMPLE_NS..=LAST_SAMPLE_NS {
            let mut next_state = state.clone();
            let mut next_oracle = *oracle;
            let readiness = evaluate_task_readiness(
                &mut next_state,
                Duration::from_nanos(interval),
                single,
                Duration::from_nanos(now),
            );
            let (due_at, missed) = next_oracle.sample(single, now);
            let base = oracle.base;

            assert_eq!(
                readiness.due_at,
                due_at.map(Duration::from_nanos),
                "due_at: interval {interval} ns, baseline {base} ns, sample {now} ns, single {single}"
            );
            assert_eq!(
                readiness.missed_intervals, missed,
                "missed: interval {interval} ns, baseline {base} ns, sample {now} ns, single {single}"
            );
            assert_eq!(
                next_state.last_run,
                Duration::from_nanos(next_oracle.last_run()),
                "no drift: interval {interval} ns, baseline {base} ns, sample {now} ns"
            );
            assert_eq!(
                next_state.overrun_count, next_oracle.missed_total,
                "overrun total: interval {interval} ns, baseline {base} ns, sample {now} ns"
            );
            assert_eq!(
                next_state.last_single, single,
                "saved SINGLE sample: interval {interval} ns, baseline {base} ns, sample {now} ns"
            );
            assert_eq!(
                i128::from(next_oracle.runs) + i128::from(next_oracle.missed_total),
                next_oracle.consumed,
                "oracle self-check: every passed deadline is one run or one miss"
            );
            if let Some(due) = readiness.due_at {
                assert!(
                    due.as_nanos() <= now,
                    "due_at is later than its sample: interval {interval} ns, baseline {base} ns, sample {now} ns"
                );
                if !single {
                    let offset = i128::from(due.as_nanos()) - base;
                    assert!(
                        interval > 0
                            && offset >= i128::from(interval)
                            && offset % i128::from(interval) == 0,
                        "periodic due_at is off the nominal grid: interval {interval} ns, baseline {base} ns, sample {now} ns"
                    );
                }
            }

            *samples += 1;
            if depth > 1 {
                explore(&next_state, &next_oracle, interval, depth - 1, samples);
            }
        }
    }
}

#[test]
fn readiness_matches_deadline_counting_on_every_four_sample_sequence() {
    let mut samples = 0;
    for interval in INTERVALS_NS {
        for base in BASELINES_NS {
            let state = TaskState::new(Duration::from_nanos(base));
            explore(
                &state,
                &Oracle::new(base, interval),
                interval,
                SEQUENCE_LENGTH,
                &mut samples,
            );
        }
    }
    let choices =
        2 * u64::try_from(LAST_SAMPLE_NS - FIRST_SAMPLE_NS + 1).expect("sample grid is non-empty");
    let prefixes: u64 = (1..=SEQUENCE_LENGTH)
        .map(|length| choices.pow(length))
        .sum();
    let pairs = u64::try_from(INTERVALS_NS.len() * BASELINES_NS.len()).expect("small grid");
    assert_eq!(
        samples,
        prefixes * pairs,
        "every sequence of the bounded domain was run"
    );
}

#[test]
fn saturated_elapsed_time_counts_only_the_intervals_its_saturated_value_reaches() {
    // Specification 10 pseudo-code: elapsed = saturating_sub(now, last_run) saturates at i64::MAX,
    // n = elapsed / interval, due_at = saturating_add(last_run, interval), missed = n - 1, and
    // last_run = saturating_add(last_run, saturating_mul(n, interval)).
    let interval = 1_000_i64;
    let last_run = i64::MIN + 5;
    let mut state = TaskState {
        last_single: false,
        last_run: Duration::from_nanos(last_run),
        overrun_count: 0,
    };
    let readiness = evaluate_task_readiness(
        &mut state,
        Duration::from_nanos(interval),
        false,
        Duration::from_nanos(i64::MAX),
    );
    let passed = i64::MAX / interval;
    let missed = u64::try_from(passed - 1).expect("passed is positive");
    assert_eq!(
        readiness.due_at,
        Some(Duration::from_nanos(last_run + interval))
    );
    assert_eq!(readiness.missed_intervals, missed);
    assert_eq!(state.overrun_count, missed);
    assert_eq!(
        state.last_run,
        Duration::from_nanos(last_run + passed * interval)
    );
}

#[test]
fn the_largest_interval_fires_once_at_its_deadline_without_overflow() {
    let mut state = TaskState::new(Duration::ZERO);
    let readiness = evaluate_task_readiness(
        &mut state,
        Duration::from_nanos(i64::MAX),
        false,
        Duration::from_nanos(i64::MAX),
    );
    assert_eq!(readiness.due_at, Some(Duration::from_nanos(i64::MAX)));
    assert_eq!(readiness.missed_intervals, 0);
    assert_eq!(state.last_run, Duration::from_nanos(i64::MAX));
    assert_eq!(state.overrun_count, 0);
}

#[test]
fn the_full_negative_range_to_a_positive_sample_counts_each_deadline_once() {
    let mut state = TaskState::new(Duration::from_nanos(i64::MIN));
    let readiness = evaluate_task_readiness(
        &mut state,
        Duration::from_nanos(1),
        false,
        Duration::from_nanos(i64::MAX),
    );
    // The saturated elapsed time reaches i64::MAX deadlines: one run and i64::MAX - 1 misses.
    let misses = u64::try_from(i64::MAX - 1).expect("positive");
    assert_eq!(readiness.due_at, Some(Duration::from_nanos(i64::MIN + 1)));
    assert_eq!(readiness.missed_intervals, misses);
    assert_eq!(state.overrun_count, misses);
    assert_eq!(state.last_run, Duration::from_nanos(i64::MIN + i64::MAX));
}

#[test]
fn non_positive_intervals_never_schedule_periodic_work_even_at_the_extremes() {
    for interval in [0, -1, i64::MIN] {
        let mut state = TaskState {
            last_single: false,
            last_run: Duration::from_nanos(i64::MIN),
            overrun_count: 9,
        };
        let readiness = evaluate_task_readiness(
            &mut state,
            Duration::from_nanos(interval),
            false,
            Duration::from_nanos(i64::MAX),
        );
        assert_eq!(readiness.due_at, None, "interval {interval} ns");
        assert_eq!(readiness.missed_intervals, 0, "interval {interval} ns");
        assert_eq!(state.last_run, Duration::from_nanos(i64::MIN));
        assert_eq!(state.overrun_count, 9);
    }
}
