//! Spec 34 §4.3 HOST-06 (line 355): accelerated simulation keeps the arithmetic,
//! bounds, reference and instruction-budget checks that a 1x run performs.
#![cfg(feature = "full")]

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::CompileSession;
use trust_runtime::scheduler::{ResourceRunner, ScaledClock};
use trust_runtime::value::Duration;

fn build(source: &str) -> trust_runtime::Runtime {
    CompileSession::from_source(source)
        .build_runtime()
        .expect("source admits")
}

/// Runs one faulting scan at 1x logical time and under a 1000x scaled clock.
fn fault_at_1x_and_accelerated(source: &str) -> (RuntimeError, RuntimeError) {
    let mut one_x = build(source);
    one_x.set_current_time(Duration::from_millis(10));
    let one_x_error = one_x
        .execute_cycle()
        .expect_err("the first scan must fault at 1x");
    assert!(one_x.faulted(), "the 1x fault must latch");

    let mut accelerated = ResourceRunner::new(
        build(source),
        ScaledClock::new(1000),
        Duration::from_millis(10),
    );
    let accelerated_error = accelerated
        .tick()
        .expect_err("the first accelerated scan must fault");
    assert!(
        accelerated.runtime().faulted(),
        "the accelerated fault must latch"
    );
    (one_x_error, accelerated_error)
}

/// Spec 34 §4.3 HOST-06 (line 355): accelerated simulation reports the same arithmetic fault as 1x.
#[test]
fn accelerated_run_keeps_the_division_check() {
    let (one_x, accelerated) = fault_at_1x_and_accelerated(
        "PROGRAM Main VAR d : DINT; x : DINT; END_VAR x := DINT#10 / d; END_PROGRAM",
    );
    assert_eq!(one_x, RuntimeError::DivisionByZero);
    assert_eq!(accelerated, one_x);
}

/// Spec 34 §4.3 HOST-06 (line 355): accelerated simulation reports the same bounds fault as 1x.
#[test]
fn accelerated_run_keeps_the_array_bounds_check() {
    let (one_x, accelerated) = fault_at_1x_and_accelerated(
        "PROGRAM Main VAR arr : ARRAY[1..3] OF DINT; i : DINT := 4; END_VAR arr[i] := 1; END_PROGRAM",
    );
    assert_eq!(
        one_x,
        RuntimeError::IndexOutOfBounds {
            index: 4,
            lower: 1,
            upper: 3,
        }
    );
    assert_eq!(accelerated, one_x);
}

/// Spec 34 §4.3 HOST-06 (line 355): accelerated simulation reports the same reference fault as 1x.
#[test]
fn accelerated_run_keeps_the_reference_check() {
    let (one_x, accelerated) = fault_at_1x_and_accelerated(
        "PROGRAM Main VAR p : REF_TO DINT; END_VAR p^ := 1; END_PROGRAM",
    );
    assert_eq!(one_x, RuntimeError::NullReference);
    assert_eq!(accelerated, one_x);
}

/// Spec 34 §4.3 HOST-06 (line 355): accelerated simulation exhausts the same instruction budget as 1x.
#[test]
fn accelerated_run_keeps_the_instruction_budget() {
    let (one_x, accelerated) = fault_at_1x_and_accelerated(
        "PROGRAM Main VAR x : DINT; END_VAR WHILE TRUE DO x := x + DINT#1; END_WHILE; END_PROGRAM",
    );
    assert!(
        matches!(one_x, RuntimeError::ExecutionLimit { .. }),
        "an endless loop must exhaust the instruction budget, got {one_x:?}"
    );
    assert_eq!(accelerated, one_x);
}
