//! Millisecond inputs at the harness protocol boundary (host/harness/protocol.rs
//! `validate_duration_millis`). Each entry point that advances logical time refuses i64::MAX
//! milliseconds as an invalid argument that names the field, before any time passes or any cycle
//! runs. The three fields are signed at every site of the Rust protocol API, so u64::MAX cannot
//! reach these entry points; i64::MAX is the largest input they can receive.

use trust_runtime::harness::{HarnessAutomation, HarnessAutomationError};
use trust_runtime::value::Value;

const COUNTER: &str = r#"
PROGRAM Main
VAR
    counter : DINT;
END_VAR
counter := counter + 1;
END_PROGRAM
"#;

fn load_counter() -> HarnessAutomation {
    let mut automation = HarnessAutomation::new();
    automation
        .load_sources(&[COUNTER.to_string()])
        .expect("counter program loads");
    automation
}

fn invalid_argument<T>(result: Result<T, HarnessAutomationError>) -> String {
    match result {
        Err(HarnessAutomationError::InvalidArgument(message)) => message,
        Err(other) => panic!("expected an invalid argument, got {other:?}"),
        Ok(_) => panic!("expected an invalid argument, got a success"),
    }
}

/// No time passed and no cycle ran beyond the one the load performs.
fn assert_untouched(automation: &mut HarnessAutomation) {
    let state = automation.snapshot(&[]).expect("snapshot after refusal");
    assert_eq!(state.cycle_count, 1);
    assert_eq!(state.elapsed_ms, 0);
}

#[test]
fn cycle_refuses_i64_max_milliseconds_before_any_time_or_cycle() {
    let mut automation = load_counter();
    assert_eq!(
        invalid_argument(automation.cycle(1, i64::MAX, &[])),
        "dt_ms must be <= 9223372036854."
    );
    assert_untouched(&mut automation);
}

#[test]
fn advance_time_refuses_i64_max_milliseconds_before_any_time_passes() {
    let mut automation = load_counter();
    assert_eq!(
        invalid_argument(automation.advance_time(i64::MAX)),
        "duration_ms must be <= 9223372036854."
    );
    assert_untouched(&mut automation);
}

#[test]
fn run_until_refuses_i64_max_milliseconds_before_matching_or_cycling() {
    let mut automation = load_counter();
    assert_eq!(
        invalid_argument(automation.run_until("counter", Value::DInt(1), i64::MAX, 0, &[])),
        "dt_ms must be <= 9223372036854."
    );
    assert_untouched(&mut automation);
}
