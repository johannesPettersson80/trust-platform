//! Timer function blocks on the logical clock (spec 11 section 6.3, spec 08 section 5, spec 34
//! TIME-07). Each test sets explicit logical instants on the installed runtime and runs executed
//! timer calls; no wall clock is used. The expected values follow the timer runtime boundary in
//! spec 08 section 5 and the restart time-base contract in spec 11 section 6.3.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};
use trust_runtime::RestartMode;

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

const TON_ONLY: &str = r#"
PROGRAM Test
VAR
    ton : TON;
    in_ton : BOOL;
    pt : TIME := T#10ms;
    q_ton : BOOL;
    et_ton : TIME;
END_VAR
ton(IN := in_ton, PT := pt, Q => q_ton, ET => et_ton);
END_PROGRAM
"#;

#[test]
fn ton_contributes_zero_across_a_backward_clock_step_and_resumes_from_the_new_baseline() {
    // Spec 08 section 5: a backward runtime clock contributes zero, establishes the new baseline and
    // never reduces ET. Elapsed time after the step runs from the new baseline (2 ms).
    let mut harness = TestHarness::from_source(TON_ONLY).unwrap();
    harness.set_input("in_ton", true);

    cycle_at(&mut harness, 0);
    harness.assert_eq("et_ton", Value::Time(Duration::ZERO));
    harness.assert_eq("q_ton", Value::Bool(false));

    cycle_at(&mut harness, 6);
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(6)));
    harness.assert_eq("q_ton", Value::Bool(false));

    cycle_at(&mut harness, 2);
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(6)));
    harness.assert_eq("q_ton", Value::Bool(false));

    cycle_at(&mut harness, 5);
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(9)));
    harness.assert_eq("q_ton", Value::Bool(false));

    cycle_at(&mut harness, 6);
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(10)));
    harness.assert_eq("q_ton", Value::Bool(true));

    harness.set_input("in_ton", false);
    cycle_at(&mut harness, 8);
    harness.assert_eq("et_ton", Value::Time(Duration::ZERO));
    harness.assert_eq("q_ton", Value::Bool(false));
}

const TOF_AND_TP: &str = r#"
PROGRAM Test
VAR
    tof : TOF;
    tp : TP;
    in_tof : BOOL;
    in_tp : BOOL;
    pt : TIME := T#10ms;
    q_tof : BOOL;
    et_tof : TIME;
    q_tp : BOOL;
    et_tp : TIME;
END_VAR
tof(IN := in_tof, PT := pt, Q => q_tof, ET => et_tof);
tp(IN := in_tp, PT := pt, Q => q_tp, ET => et_tp);
END_PROGRAM
"#;

#[test]
fn tof_and_tp_saturate_at_the_preset_across_a_forward_jump_and_hold_through_a_backward_step() {
    // Spec 08 section 5: an accumulated span beyond PT saturates at PT, TOF then holds ET at PT
    // while IN stays low, and a backward sample adds nothing. TP keeps IN high across the jump so
    // the expiry is not combined with a falling input.
    let mut harness = TestHarness::from_source(TOF_AND_TP).unwrap();
    harness.set_input("in_tof", true);
    harness.set_input("in_tp", true);
    cycle_at(&mut harness, 0);
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(true));
    harness.assert_eq("et_tp", Value::Time(Duration::ZERO));

    harness.set_input("in_tof", false);
    cycle_at(&mut harness, 0);
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(true));

    cycle_at(&mut harness, 50);
    harness.assert_eq("q_tof", Value::Bool(false));
    harness.assert_eq("et_tof", Value::Time(Duration::from_millis(10)));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("et_tp", Value::Time(Duration::from_millis(10)));

    cycle_at(&mut harness, 20);
    harness.assert_eq("q_tof", Value::Bool(false));
    harness.assert_eq("et_tof", Value::Time(Duration::from_millis(10)));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("et_tp", Value::Time(Duration::from_millis(10)));

    harness.set_input("in_tof", true);
    cycle_at(&mut harness, 25);
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("et_tp", Value::Time(Duration::from_millis(10)));
}

#[test]
fn first_timer_call_after_a_cold_restart_contributes_zero_elapsed_time() {
    // Spec 11 section 6.3: restart keeps the clock epoch, but the first executed timer call after
    // restart contributes zero and later time is measured from that call.
    let mut harness = TestHarness::from_source(TON_ONLY).unwrap();
    harness.set_input("in_ton", true);
    cycle_at(&mut harness, 0);
    cycle_at(&mut harness, 6);
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(6)));

    harness.restart(RestartMode::Cold).unwrap();
    assert_eq!(harness.current_time(), Duration::from_millis(6));
    harness.set_input("in_ton", true);
    cycle_at(&mut harness, 9);
    harness.assert_eq("et_ton", Value::Time(Duration::ZERO));
    harness.assert_eq("q_ton", Value::Bool(false));

    cycle_at(&mut harness, 12);
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(3)));
    cycle_at(&mut harness, 19);
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(10)));
    harness.assert_eq("q_ton", Value::Bool(true));
}

#[test]
fn forward_span_wider_than_the_time_range_saturates_at_the_preset() {
    // Spec 08 section 5: elapsed accumulation saturates at PT and never wraps. The span from
    // -1 ns to i64::MAX ns is wider than the signed nanosecond range, so the contribution must
    // saturate and the 10 ms preset completes. The subtraction that forms the span is
    // trust-runtime-core src/stdlib/fbs/timers.rs `elapsed_since` (line 301) and is not saturating.
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Test
VAR
    ton : TON;
    in_ton : BOOL := TRUE;
    pt : TIME := T#10ms;
    q_ton : BOOL;
    et_ton : TIME;
END_VAR
ton(IN := in_ton, PT := pt, Q => q_ton, ET => et_ton);
END_PROGRAM
"#,
    )
    .unwrap();

    harness
        .runtime_mut()
        .set_current_time(Duration::from_nanos(-1));
    assert!(harness.cycle().errors.is_empty());
    harness.assert_eq("q_ton", Value::Bool(false));

    harness
        .runtime_mut()
        .set_current_time(Duration::from_nanos(i64::MAX));
    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "a span across the full time range must not fault: {:?}",
        result.errors
    );
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(10)));
    harness.assert_eq("q_ton", Value::Bool(true));
}

#[test]
fn zero_preset_timers_follow_the_zero_preset_rules() {
    // Spec 08 section 5: a zero-preset TON completes on an executed call with IN TRUE, a zero-preset
    // TP has no positive pulse, and a zero-preset TOF expires on the falling-edge call.
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Test
VAR
    ton : TON;
    tp : TP;
    tof : TOF;
    in_zero : BOOL := TRUE;
    pt : TIME := T#0ms;
    q_ton : BOOL;
    et_ton : TIME;
    q_tp : BOOL;
    et_tp : TIME;
    q_tof : BOOL;
    et_tof : TIME;
END_VAR
ton(IN := in_zero, PT := pt, Q => q_ton, ET => et_ton);
tp(IN := in_zero, PT := pt, Q => q_tp, ET => et_tp);
tof(IN := in_zero, PT := pt, Q => q_tof, ET => et_tof);
END_PROGRAM
"#,
    )
    .unwrap();

    cycle_at(&mut harness, 0);
    harness.assert_eq("q_ton", Value::Bool(true));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("q_tof", Value::Bool(true));

    harness.set_input("in_zero", false);
    cycle_at(&mut harness, 0);
    harness.assert_eq("q_ton", Value::Bool(false));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("q_tof", Value::Bool(false));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
}
