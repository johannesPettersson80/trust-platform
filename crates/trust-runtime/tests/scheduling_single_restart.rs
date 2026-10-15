//! A held TRUE SINGLE input across a restart (spec 10 section 4.1.1: registration seeds the sampled
//! SINGLE so it creates no artificial rising edge; spec 10 section 4.8: a restart resets task
//! timing; spec 11 section 6.2: event readiness is a rising edge). Restart rebuilds the task state
//! with no sampled SINGLE and does not seed it again, so the first cycle after a warm or a cold
//! restart sees one rising edge and the event program runs exactly once. The logical clock is set
//! explicitly, so no wall clock is involved.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

const HELD_TRUE_SINGLE: &str = r#"
CONFIGURATION C
VAR_GLOBAL
    trigger : BOOL := TRUE;
    runs : DINT := 0;
END_VAR
TASK Event (SINGLE := trigger, INTERVAL := T#0ms, PRIORITY := 1);
PROGRAM P WITH Event : EventProg;
END_CONFIGURATION

PROGRAM EventProg
runs := runs + 1;
END_PROGRAM
"#;

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

/// The TRUE input seeded at registration creates no event, so the counter is still zero before
/// the restart under test.
fn started_with_held_true_input() -> TestHarness {
    let mut harness = TestHarness::from_source(HELD_TRUE_SINGLE).unwrap();
    cycle_at(&mut harness, 0);
    cycle_at(&mut harness, 10);
    harness.assert_eq("runs", Value::DInt(0));
    harness
}

#[test]
fn held_true_single_runs_its_event_once_on_the_first_cycle_after_a_warm_restart() {
    let mut harness = started_with_held_true_input();
    harness.restart(RestartMode::Warm).unwrap();

    cycle_at(&mut harness, 20);
    harness.assert_eq("runs", Value::DInt(1));
    cycle_at(&mut harness, 30);
    harness.assert_eq("runs", Value::DInt(1));
}

#[test]
fn held_true_single_runs_its_event_once_on_the_first_cycle_after_a_cold_restart() {
    let mut harness = started_with_held_true_input();
    harness.restart(RestartMode::Cold).unwrap();

    cycle_at(&mut harness, 20);
    harness.assert_eq("runs", Value::DInt(1));
    cycle_at(&mut harness, 30);
    harness.assert_eq("runs", Value::DInt(1));
}
