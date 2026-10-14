#![cfg(feature = "full")]
//! Retained checkpoint capture: a failed capture is an error, never an empty checkpoint.

use std::time::{Duration as PhysicalDuration, Instant};

use trust_runtime::{
    error::RuntimeError,
    harness::{CompileSession, TestHarness},
    value::Value,
};

const RETAINED_SOURCE: &str = "VAR_GLOBAL RETAIN count : DINT := DINT#3; END_VAR PROGRAM Main count := count + DINT#1; END_PROGRAM";

fn expired_deadline() -> Instant {
    Instant::now()
        .checked_sub(PhysicalDuration::from_secs(1))
        .expect("the monotonic clock is more than one second past its origin")
}

/// Spec 34 §4.3.1 retained checkpoint capture: an expired physical deadline fails the owner capture instead of returning an empty checkpoint.
#[test]
fn installed_write_expired_deadline_fails_retained_capture_instead_of_an_empty_checkpoint() {
    let mut runtime = CompileSession::from_source(RETAINED_SOURCE)
        .build_runtime()
        .unwrap();
    runtime.set_execution_deadline(Some(expired_deadline()));
    let error = match runtime.retain_snapshot() {
        Ok(_) => panic!("an expired deadline must not produce a checkpoint"),
        Err(error) => error,
    };
    assert_eq!(error, RuntimeError::ExecutionTimeout);
    assert_eq!(
        runtime.storage().get_global("count"),
        Some(&Value::DInt(3)),
        "the failed capture must not change the retained value"
    );
}

/// Spec 34 §4.3.1 retained checkpoint capture: a failed capture is a rejected management request, not a PLC execution fault, so the same owner captures normally once the deadline is cleared.
#[test]
fn installed_write_owner_captures_normally_after_a_deadline_capture_failure() {
    let mut runtime = CompileSession::from_source(RETAINED_SOURCE)
        .build_runtime()
        .unwrap();
    runtime.set_execution_deadline(Some(expired_deadline()));
    assert!(runtime.retain_snapshot().is_err());
    assert!(!runtime.faulted());
    runtime.set_execution_deadline(None);
    assert!(
        runtime.retain_snapshot().is_ok(),
        "the owner must still capture after the deadline is cleared"
    );
}

/// Spec 34 §4.3.1 retained checkpoint capture: a source reload propagates the capture failure, does not rebuild the application and keeps the running owner.
#[test]
fn installed_write_reload_propagates_a_failed_retained_capture_and_keeps_the_owner() {
    let mut harness =
        TestHarness::from_source(RETAINED_SOURCE).expect("retained source must compile");
    harness
        .runtime_mut()
        .set_execution_deadline(Some(expired_deadline()));
    let error = harness
        .reload_source(RETAINED_SOURCE)
        .expect_err("the capture failure must stop the reload");
    assert_eq!(
        error.to_string(),
        RuntimeError::ExecutionTimeout.to_string()
    );
    harness.runtime_mut().set_execution_deadline(None);
    assert_eq!(
        harness.runtime().storage().get_global("count"),
        Some(&Value::DInt(3))
    );
    assert_eq!(harness.cycle_count(), 0);
}
