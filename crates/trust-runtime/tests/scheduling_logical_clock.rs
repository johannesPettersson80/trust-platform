//! Host-level logical-time scheduling (spec 10 sections 4.3 and 4.4, spec 11 section 6.2, spec 34
//! SCHED-05, COMPAT-06, CYCLE-03). Each test sets explicit logical instants with
//! `set_current_time` and runs `cycle`; no wall clock, sleep or OS thread is involved. Expected
//! activations are derived from the nominal deadlines written in each test.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

/// First sample on a grid of `grid_ms` at or after `deadline_ms` (both non-negative).
fn first_sample_at_or_after(deadline_ms: i64, grid_ms: i64) -> i64 {
    (deadline_ms + grid_ms - 1) / grid_ms * grid_ms
}

fn cycle_at(harness: &mut TestHarness, millis: i64) {
    harness
        .runtime_mut()
        .set_current_time(Duration::from_millis(millis));
    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "cycle at {millis} ms failed: {:?}",
        result.errors
    );
}

fn dint(harness: &TestHarness, name: &str) -> i64 {
    match harness.runtime().storage().get_global(name) {
        Some(Value::DInt(value)) => i64::from(*value),
        other => panic!("expected DINT global {name}, got {other:?}"),
    }
}

fn overruns(harness: &TestHarness, task: &str) -> Option<u64> {
    harness.runtime().task_overrun_count(task)
}

const MIXED_RATE: &str = r#"
VAR_GLOBAL
    fast_runs : DINT := 0;
    slow_runs : DINT := 0;
END_VAR
CONFIGURATION C
TASK Fast (INTERVAL := T#10ms, PRIORITY := 0);
TASK Slow (INTERVAL := T#25ms, PRIORITY := 1);
PROGRAM P1 WITH Fast : FastProg;
PROGRAM P2 WITH Slow : SlowProg;
END_CONFIGURATION

PROGRAM FastProg
fast_runs := fast_runs + 1;
END_PROGRAM

PROGRAM SlowProg
slow_runs := slow_runs + 1;
END_PROGRAM
"#;

#[test]
fn ten_and_twenty_five_ms_tasks_release_on_nominal_deadlines_over_300_ms() {
    // SCHED-05 / COMPAT-06 with a coincident batch: the 25 ms task releases at the first 10 ms
    // sample at or after each deadline 25k, and both tasks are due at 50, 100, ... 300 ms.
    let mut harness = TestHarness::from_source(MIXED_RATE).unwrap();
    let mut slow_releases = Vec::new();
    let mut slow_seen = 0;

    for sample in (10_i64..=300).step_by(10) {
        cycle_at(&mut harness, sample);
        assert_eq!(
            dint(&harness, "fast_runs"),
            sample / 10,
            "10 ms releases at {sample} ms"
        );

        let mut expected_slow = 0;
        while first_sample_at_or_after(25 * (expected_slow + 1), 10) <= sample {
            expected_slow += 1;
        }
        let slow = dint(&harness, "slow_runs");
        assert_eq!(slow, expected_slow, "25 ms releases at {sample} ms");
        if slow != slow_seen {
            slow_releases.push(sample);
            slow_seen = slow;
        }
    }

    assert_eq!(
        slow_releases,
        vec![30, 50, 80, 100, 130, 150, 180, 200, 230, 250, 280, 300]
    );
    assert_eq!(overruns(&harness, "Fast"), Some(0));
    assert_eq!(overruns(&harness, "Slow"), Some(0));
}

const TIE_SAME_PRIORITY: &str = r#"
VAR_GLOBAL order : DINT := 0; END_VAR
CONFIGURATION C
TASK TaskA (INTERVAL := T#10ms, PRIORITY := 1);
TASK TaskB (INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM PA WITH TaskA : ProgA;
PROGRAM PB WITH TaskB : ProgB;
END_CONFIGURATION

PROGRAM ProgA
order := order * 10 + 1;
END_PROGRAM

PROGRAM ProgB
order := order * 10 + 2;
END_PROGRAM
"#;

#[test]
fn same_priority_and_same_due_time_run_in_declaration_order() {
    // Spec 10 section 4.6: equal priority and equal due time fall back to the stable task index.
    let mut harness = TestHarness::from_source(TIE_SAME_PRIORITY).unwrap();
    cycle_at(&mut harness, 10);
    assert_eq!(dint(&harness, "order"), 12, "TaskA (index 0) then TaskB");
}

const TIE_PRIORITY_BEFORE_INDEX: &str = r#"
VAR_GLOBAL order : DINT := 0; END_VAR
CONFIGURATION C
TASK TaskA (INTERVAL := T#10ms, PRIORITY := 5);
TASK TaskB (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM PA WITH TaskA : ProgA;
PROGRAM PB WITH TaskB : ProgB;
END_CONFIGURATION

PROGRAM ProgA
order := order * 10 + 1;
END_PROGRAM

PROGRAM ProgB
order := order * 10 + 2;
END_PROGRAM
"#;

#[test]
fn lower_priority_value_runs_first_even_when_declared_later() {
    // Spec 10 section 4.3: lower numeric priority values run first (0 is highest).
    let mut harness = TestHarness::from_source(TIE_PRIORITY_BEFORE_INDEX).unwrap();
    cycle_at(&mut harness, 10);
    assert_eq!(dint(&harness, "order"), 21, "TaskB (priority 0) then TaskA");
}

const TIE_DUE_BEFORE_INDEX: &str = r#"
VAR_GLOBAL order : DINT := 0; END_VAR
CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
END_VAR
TASK Event (SINGLE := trigger, INTERVAL := T#0ms, PRIORITY := 0);
TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM PE WITH Event : ProgEvent;
PROGRAM PP WITH Periodic : ProgPeriodic;
END_CONFIGURATION

PROGRAM ProgEvent
order := order * 10 + 1;
END_PROGRAM

PROGRAM ProgPeriodic
order := order * 10 + 2;
END_PROGRAM
"#;

#[test]
fn earlier_nominal_due_time_beats_declaration_index_within_a_priority() {
    // Spec 10 section 4.6: at 20 ms the periodic task is due at its nominal deadline 10 ms and the
    // event is due at 20 ms. The event is declared first, so only the due time can put the
    // periodic task first (sample-time due would tie and keep the event first).
    let mut harness = TestHarness::from_source(TIE_DUE_BEFORE_INDEX).unwrap();
    harness
        .runtime_mut()
        .write_global("trigger", Value::Bool(true))
        .unwrap();
    cycle_at(&mut harness, 20);
    assert_eq!(
        dint(&harness, "order"),
        21,
        "Periodic (due 10 ms) then Event (due 20 ms)"
    );
}

const JUMP_COUNTER: &str = r#"
VAR_GLOBAL count : DINT := 0; END_VAR
CONFIGURATION C
TASK Fast (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM P WITH Fast : Main;
END_CONFIGURATION

PROGRAM Main
count := count + 1;
END_PROGRAM
"#;

#[test]
fn clock_jumps_count_missed_periods_once_and_keep_the_next_nominal_deadline() {
    // Spec 11 section 6.2: the first activation after a jump is due at the first pending nominal
    // deadline; the missed intervals are counted and the next deadline is the unchanged grid.
    let mut harness = TestHarness::from_source(JUMP_COUNTER).unwrap();

    cycle_at(&mut harness, 55);
    assert_eq!(dint(&harness, "count"), 1);
    assert_eq!(
        overruns(&harness, "Fast"),
        Some(4),
        "deadlines 10, 20, 30, 40 missed"
    );

    cycle_at(&mut harness, 59);
    assert_eq!(
        dint(&harness, "count"),
        1,
        "the 60 ms deadline is not yet reached"
    );
    cycle_at(&mut harness, 60);
    assert_eq!(dint(&harness, "count"), 2, "the grid continues at 60 ms");
    cycle_at(&mut harness, 69);
    assert_eq!(dint(&harness, "count"), 2);
    cycle_at(&mut harness, 70);
    assert_eq!(dint(&harness, "count"), 3);
    assert_eq!(overruns(&harness, "Fast"), Some(4));

    cycle_at(&mut harness, 200);
    assert_eq!(
        dint(&harness, "count"),
        4,
        "one activation after a 130 ms gap"
    );
    assert_eq!(
        overruns(&harness, "Fast"),
        Some(16),
        "twelve further periods counted"
    );
    cycle_at(&mut harness, 209);
    assert_eq!(dint(&harness, "count"), 4);
    cycle_at(&mut harness, 210);
    assert_eq!(dint(&harness, "count"), 5, "the next deadline is 210 ms");
    assert_eq!(overruns(&harness, "Fast"), Some(16));
}

#[test]
fn backward_clock_step_keeps_the_nominal_baseline_and_counts_nothing() {
    // Spec 10 section 4.3: a backward sample does not move the baseline or create an overrun.
    let mut harness = TestHarness::from_source(JUMP_COUNTER).unwrap();
    cycle_at(&mut harness, 10);
    assert_eq!(dint(&harness, "count"), 1);

    cycle_at(&mut harness, 2);
    assert_eq!(dint(&harness, "count"), 1);
    assert_eq!(overruns(&harness, "Fast"), Some(0));
    cycle_at(&mut harness, 9);
    cycle_at(&mut harness, 19);
    assert_eq!(
        dint(&harness, "count"),
        1,
        "the 20 ms deadline is still ahead"
    );
    cycle_at(&mut harness, 20);
    assert_eq!(dint(&harness, "count"), 2);
    assert_eq!(overruns(&harness, "Fast"), Some(0));
}

const HELD_SINGLE: &str = r#"
VAR_GLOBAL runs : DINT := 0; END_VAR
CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := FALSE;
END_VAR
TASK Gated (SINGLE := trigger, INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM PG WITH Gated : GatedProg;
END_CONFIGURATION

PROGRAM GatedProg
runs := runs + 1;
END_PROGRAM
"#;

fn set_trigger(harness: &mut TestHarness, value: bool) {
    harness
        .runtime_mut()
        .write_global("trigger", Value::Bool(value))
        .unwrap();
}

#[test]
fn held_single_suppresses_periodic_releases_until_a_low_sample_then_rearms() {
    // Spec 10 section 4.3 and spec 11 section 6.2 for one task with SINGLE and INTERVAL: the
    // rising edge at 10 ms runs it once, the high level then suppresses its periodic releases, and
    // the release at 35 ms accounts for deadlines 10, 20 and 30 (two missed) in one run. A new
    // rising edge at 36 ms runs it again without moving the baseline (30 ms), and the release at
    // 45 ms runs for the 40 ms deadline.
    let mut harness = TestHarness::from_source(HELD_SINGLE).unwrap();

    set_trigger(&mut harness, true);
    cycle_at(&mut harness, 10);
    assert_eq!(dint(&harness, "runs"), 1, "rising edge");
    cycle_at(&mut harness, 20);
    cycle_at(&mut harness, 30);
    assert_eq!(
        dint(&harness, "runs"),
        1,
        "held high suppresses periodic releases"
    );

    set_trigger(&mut harness, false);
    cycle_at(&mut harness, 35);
    assert_eq!(
        dint(&harness, "runs"),
        2,
        "one release for the three pending deadlines"
    );
    assert_eq!(overruns(&harness, "Gated"), Some(2));

    set_trigger(&mut harness, true);
    cycle_at(&mut harness, 36);
    assert_eq!(dint(&harness, "runs"), 3, "a new rising edge runs the task");
    cycle_at(&mut harness, 40);
    assert_eq!(dint(&harness, "runs"), 3, "held high again");

    set_trigger(&mut harness, false);
    cycle_at(&mut harness, 45);
    assert_eq!(dint(&harness, "runs"), 4, "release for the 40 ms deadline");
    cycle_at(&mut harness, 50);
    assert_eq!(dint(&harness, "runs"), 5, "release for the 50 ms deadline");
    assert_eq!(overruns(&harness, "Gated"), Some(2));
}

#[test]
fn cold_restart_clears_task_baselines_without_replaying_pre_restart_periods() {
    // Spec 11 section 6.3 and spec 10 section 4.8: restart keeps the logical clock and reinitializes
    // task timing at that instant, so the first post-restart samples are not due and nothing from
    // before the restart is counted again.
    let mut harness = TestHarness::from_source(JUMP_COUNTER).unwrap();
    cycle_at(&mut harness, 95);
    assert_eq!(dint(&harness, "count"), 1);
    assert_eq!(overruns(&harness, "Fast"), Some(8));

    harness.restart(RestartMode::Cold).unwrap();
    assert_eq!(harness.current_time(), Duration::from_millis(95));
    assert_eq!(
        dint(&harness, "count"),
        0,
        "cold restart reinitializes globals"
    );
    assert_eq!(overruns(&harness, "Fast"), Some(0));

    cycle_at(&mut harness, 96);
    assert_eq!(dint(&harness, "count"), 0);
    cycle_at(&mut harness, 104);
    assert_eq!(
        dint(&harness, "count"),
        0,
        "the first deadline after restart is 105 ms"
    );
    cycle_at(&mut harness, 105);
    assert_eq!(dint(&harness, "count"), 1);
    assert_eq!(overruns(&harness, "Fast"), Some(0));
}
