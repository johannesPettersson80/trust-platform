//! Supervisor decisions. The restart limiter and backoff tests moved from trust-runtime
//! (`scheduler/runner_loop.rs` and `scheduler/runner_loop/restart_generation_backoff_tests.rs`).

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::error::ExecutionLimitKind;

use super::*;

fn watchdog(enabled: bool, action: WatchdogAction) -> WatchdogPolicy {
    WatchdogPolicy {
        enabled,
        timeout: Duration::from_millis(10),
        action,
    }
}

fn limit() -> RuntimeError {
    RuntimeError::ExecutionLimit {
        kind: ExecutionLimitKind::AllocationBytes,
        used: 1,
        requested: 2,
        limit: 2,
    }
}

#[test]
fn automatic_restart_limiter_escalates_after_retry_budget() {
    let mut limiter = AutomaticRestartLimiter::default();
    let reason = RuntimeError::IoTransport("persistent failure".into());

    assert_eq!(limiter.next_attempt(&reason), Ok(1));
    assert_eq!(limiter.next_attempt(&reason), Ok(2));
    assert_eq!(limiter.next_attempt(&reason), Ok(3));
    let err = limiter
        .next_attempt(&reason)
        .expect_err("fourth persistent failure should escalate");

    assert!(matches!(
        err,
        RuntimeError::RestartLimitExceeded { attempts: 3, .. }
    ));
    assert_eq!(
        err.to_string(),
        "automatic restart limit exceeded after 3 attempts: i/o transport error 'persistent failure'"
    );
    assert!(
        limiter.next_attempt(&reason).is_err(),
        "the budget stays spent"
    );

    limiter.reset();
    assert_eq!(
        limiter.next_attempt(&reason),
        Ok(1),
        "a reset starts a new budget"
    );
}

#[test]
fn automatic_restart_backoff_is_bounded_and_monotonic() {
    let first = automatic_restart_backoff(Duration::from_millis(1), 1);
    let second = automatic_restart_backoff(Duration::from_millis(1), 2);
    let capped = automatic_restart_backoff(Duration::from_secs(60), 10);

    assert!(first.as_nanos() > 0);
    assert!(second >= first);
    assert_eq!(capped.as_nanos(), AUTOMATIC_RESTART_BACKOFF_CAP_NANOS);
}

/// Spec 11 §6.6: the backoff base is clamped to the inclusive range from one millisecond through
/// one second, so a zero or sub-millisecond cycle interval waits one millisecond and a long cycle
/// interval waits one second.
#[test]
fn automatic_restart_backoff_base_is_clamped_to_one_millisecond_through_one_second() {
    assert_eq!(
        automatic_restart_backoff(Duration::ZERO, 1).as_nanos(),
        1_000_000
    );
    assert_eq!(
        automatic_restart_backoff(Duration::from_nanos(1), 1).as_nanos(),
        1_000_000
    );
    assert_eq!(
        automatic_restart_backoff(Duration::from_secs(60), 1).as_nanos(),
        1_000_000_000
    );
}

/// Spec 11 §6.6: later automatic attempts increase exponentially from the base, and the resulting
/// delay never exceeds one second.
#[test]
fn automatic_restart_backoff_doubles_per_attempt_and_stops_at_one_second() {
    let delays: Vec<i64> = (1..=5)
        .map(|attempt| automatic_restart_backoff(Duration::from_millis(10), attempt).as_nanos())
        .collect();
    assert_eq!(
        delays,
        vec![10_000_000, 20_000_000, 40_000_000, 80_000_000, 160_000_000]
    );
    assert_eq!(
        automatic_restart_backoff(Duration::from_millis(600), 2).as_nanos(),
        1_000_000_000
    );
}

/// Spec 11 §6.6: every consecutive retry delay is nonzero, monotonic non-decreasing and within the
/// one millisecond to one second bounds, for any configured cycle interval.
#[test]
fn automatic_restart_backoff_is_nonzero_monotonic_and_bounded_for_every_attempt() {
    for interval in [
        Duration::ZERO,
        Duration::from_nanos(1),
        Duration::from_millis(1),
        Duration::from_millis(250),
        Duration::from_secs(1),
        Duration::from_secs(60),
    ] {
        let interval_ns = interval.as_nanos();
        let mut previous = 0_i64;
        for attempt in 1..=12 {
            let delay = automatic_restart_backoff(interval, attempt).as_nanos();
            assert!(
                (1_000_000..=1_000_000_000).contains(&delay),
                "delay {delay} ns is outside 1 ms..=1 s for cycle interval {interval_ns} ns, attempt {attempt}"
            );
            assert!(
                delay >= previous,
                "delay {delay} ns fell below the previous attempt's {previous} ns for cycle interval {interval_ns} ns"
            );
            previous = delay;
        }
    }
}

#[test]
fn watchdog_errors_follow_the_watchdog_action() {
    for error in [
        RuntimeError::WatchdogTimeout,
        RuntimeError::ExecutionTimeout,
    ] {
        assert_eq!(
            cycle_error_action(
                watchdog(true, WatchdogAction::Restart),
                FaultPolicy::Halt,
                error.clone()
            ),
            CycleErrorAction::Restart(RuntimeError::WatchdogTimeout),
            "{error:?} becomes a watchdog timeout"
        );
        for action in [WatchdogAction::Halt, WatchdogAction::SafeHalt] {
            assert_eq!(
                cycle_error_action(watchdog(true, action), FaultPolicy::Restart, error.clone()),
                CycleErrorAction::WatchdogHalt(RuntimeError::WatchdogTimeout)
            );
        }
    }
    assert_eq!(
        cycle_error_action(
            watchdog(true, WatchdogAction::Restart),
            FaultPolicy::Halt,
            limit()
        ),
        CycleErrorAction::Restart(limit()),
        "an execution limit keeps its own error"
    );
    assert_eq!(
        cycle_error_action(
            watchdog(true, WatchdogAction::SafeHalt),
            FaultPolicy::Halt,
            limit()
        ),
        CycleErrorAction::WatchdogHalt(limit())
    );
}

#[test]
fn other_errors_follow_the_fault_policy_and_a_panic_never_restarts() {
    let division = RuntimeError::DivisionByZero;
    assert_eq!(
        cycle_error_action(
            watchdog(false, WatchdogAction::Restart),
            FaultPolicy::Restart,
            division.clone()
        ),
        CycleErrorAction::Restart(division.clone())
    );
    for policy in [FaultPolicy::Halt, FaultPolicy::SafeHalt] {
        assert_eq!(
            cycle_error_action(
                watchdog(true, WatchdogAction::Restart),
                policy,
                division.clone()
            ),
            CycleErrorAction::Halt(division.clone())
        );
    }
    let panic = RuntimeError::ResourcePanic("boom".into());
    assert_eq!(
        cycle_error_action(
            watchdog(true, WatchdogAction::Restart),
            FaultPolicy::Restart,
            panic.clone()
        ),
        CycleErrorAction::Halt(panic)
    );
    assert_eq!(
        cycle_error_action(
            watchdog(false, WatchdogAction::Restart),
            FaultPolicy::Restart,
            RuntimeError::WatchdogTimeout
        ),
        CycleErrorAction::Restart(RuntimeError::WatchdogTimeout),
        "a disabled watchdog leaves a timeout to the fault policy"
    );
}

#[test]
fn an_overrun_needs_an_enabled_watchdog_and_more_than_its_timeout() {
    let ten_ms = core::time::Duration::from_millis(10);
    let over = core::time::Duration::from_nanos(10_000_001);
    assert_eq!(
        watchdog_overrun_action(watchdog(false, WatchdogAction::Halt), over),
        None
    );
    assert_eq!(
        watchdog_overrun_action(watchdog(true, WatchdogAction::Halt), ten_ms),
        None,
        "exactly the timeout is within it"
    );
    assert_eq!(
        watchdog_overrun_action(watchdog(true, WatchdogAction::Restart), over),
        Some(OverrunAction::Restart)
    );
    for action in [WatchdogAction::Halt, WatchdogAction::SafeHalt] {
        assert_eq!(
            watchdog_overrun_action(watchdog(true, action), over),
            Some(OverrunAction::Halt)
        );
    }
    assert_eq!(
        watchdog_overrun_action(
            watchdog(true, WatchdogAction::Halt),
            core::time::Duration::MAX
        ),
        Some(OverrunAction::Halt),
        "an elapsed time beyond the nanosecond range saturates"
    );
}

#[test]
fn a_stop_leaves_stopped_or_faulted_with_the_first_failure() {
    let safe = RuntimeError::IoTransport("safe output rejected".into());
    let retain = RuntimeError::IoTransport("retain store full".into());
    let cases = vec![
        (Ok(()), Ok(()), (ResourceState::Stopped, None)),
        (
            Err(safe.clone()),
            Ok(()),
            (ResourceState::Faulted, Some(safe.clone())),
        ),
        (
            Ok(()),
            Err(retain.clone()),
            (ResourceState::Faulted, Some(retain.clone())),
        ),
        (
            Err(safe.clone()),
            Err(retain),
            (ResourceState::Faulted, Some(safe)),
        ),
    ];
    for (safe_state, retain_save, expected) in cases {
        assert_eq!(stop_outcome(safe_state, retain_save), expected);
    }
}
