//! The supervisor's decisions in a resource's scan loop (specification 11, §6.6): what follows a
//! cycle error or a watchdog overrun, the bounded automatic restart and its backoff, and the state
//! a stop request leaves.
//!
//! The platform runs the loop, the waits, the restarts and the fault actions; these functions
//! decide which of them happen.

use alloc::string::ToString;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Duration;
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogAction, WatchdogPolicy};

/// Automatic restarts in a row before the fault escalates to `RestartLimitExceeded`.
pub const AUTOMATIC_RESTART_MAX_ATTEMPTS: u32 = 3;

/// Shortest automatic restart backoff in nanoseconds: one millisecond.
pub const AUTOMATIC_RESTART_BACKOFF_FLOOR_NANOS: i64 = 1_000_000;

/// Longest automatic restart backoff in nanoseconds: one second.
pub const AUTOMATIC_RESTART_BACKOFF_CAP_NANOS: i64 = 1_000_000_000;

/// Poll period of the faulted wait: a faulted resource takes a restart or a stop at the end of
/// each period, so the wait never spins (specification 11, §6.6).
pub const FAULTED_WAIT_POLL: core::time::Duration = core::time::Duration::from_millis(10);

/// Counts the automatic restarts in a row.
#[derive(Debug, Default)]
pub struct AutomaticRestartLimiter {
    attempts: u32,
}

impl AutomaticRestartLimiter {
    /// Starts a new budget, after a cycle that ran cleanly or a recovery.
    pub fn reset(&mut self) {
        self.attempts = 0;
    }

    /// Counts one more automatic restart for `reason` and returns its number, from 1.
    ///
    /// # Errors
    ///
    /// `RuntimeError::RestartLimitExceeded`, naming `reason`, once
    /// [`AUTOMATIC_RESTART_MAX_ATTEMPTS`] restarts were made in a row.
    pub fn next_attempt(&mut self, reason: &RuntimeError) -> Result<u32, RuntimeError> {
        if self.attempts >= AUTOMATIC_RESTART_MAX_ATTEMPTS {
            return Err(RuntimeError::RestartLimitExceeded {
                attempts: self.attempts,
                reason: SmolStr::new(reason.to_string()),
            });
        }
        self.attempts += 1;
        Ok(self.attempts)
    }
}

/// The wait before automatic restart `attempt`, counted from 1.
///
/// The base is the cycle interval clamped to one millisecond through one second; it doubles with
/// each later attempt and never exceeds one second.
#[must_use]
pub fn automatic_restart_backoff(cycle_interval: Duration, attempt: u32) -> Duration {
    let base = cycle_interval.as_nanos().clamp(
        AUTOMATIC_RESTART_BACKOFF_FLOOR_NANOS,
        AUTOMATIC_RESTART_BACKOFF_CAP_NANOS,
    );
    let multiplier = 1_i64 << attempt.saturating_sub(1).min(8);
    Duration::from_nanos(
        base.saturating_mul(multiplier)
            .min(AUTOMATIC_RESTART_BACKOFF_CAP_NANOS),
    )
}

/// What follows a scan cycle that returned an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CycleErrorAction {
    /// Try an automatic warm restart for this reason; when the budget is spent or the restart
    /// fails, the resource faults and waits for recovery.
    Restart(RuntimeError),
    /// Record this watchdog fault through the watchdog's fault action, then halt.
    WatchdogHalt(RuntimeError),
    /// Halt with this fault.
    Halt(RuntimeError),
}

/// What follows a cycle that returned `error`.
///
/// With the watchdog enabled, a timeout or an execution limit follows the watchdog action: a
/// limit keeps its own error, a timeout becomes `WatchdogTimeout`. Any other error follows the
/// fault policy's automatic restart, never for a contained panic, and otherwise halts.
#[must_use]
pub fn cycle_error_action(
    watchdog: WatchdogPolicy,
    fault_policy: FaultPolicy,
    error: RuntimeError,
) -> CycleErrorAction {
    if watchdog.enabled
        && matches!(
            error,
            RuntimeError::WatchdogTimeout
                | RuntimeError::ExecutionTimeout
                | RuntimeError::ExecutionLimit { .. }
        )
    {
        let watchdog_error = match error {
            RuntimeError::ExecutionLimit { .. } => error,
            _ => RuntimeError::WatchdogTimeout,
        };
        if matches!(watchdog.action, WatchdogAction::Restart) {
            return CycleErrorAction::Restart(watchdog_error);
        }
        return CycleErrorAction::WatchdogHalt(watchdog_error);
    }
    if matches!(fault_policy, FaultPolicy::Restart)
        && !matches!(error, RuntimeError::ResourcePanic(_))
    {
        return CycleErrorAction::Restart(error);
    }
    CycleErrorAction::Halt(error)
}

/// What follows a cycle that returned but overran its active-execution budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrunAction {
    /// Try an automatic warm restart for `WatchdogTimeout`.
    Restart,
    /// Record the watchdog timeout and halt.
    Halt,
}

/// The watchdog action for a cycle that was active for `active_elapsed`, or `None` when the
/// watchdog is disabled or the cycle stayed within its timeout.
#[must_use]
pub fn watchdog_overrun_action(
    watchdog: WatchdogPolicy,
    active_elapsed: core::time::Duration,
) -> Option<OverrunAction> {
    if !watchdog.enabled {
        return None;
    }
    // The nanoseconds in 64 bits, saturating beyond `i64` as the 128-bit count did: a chip
    // multiplies 128 bits in software at every scan (checklist P1-6).
    let elapsed = active_elapsed
        .as_secs()
        .checked_mul(1_000_000_000)
        .and_then(|nanos| nanos.checked_add(u64::from(active_elapsed.subsec_nanos())))
        .and_then(|nanos| i64::try_from(nanos).ok())
        .unwrap_or(i64::MAX);
    if elapsed <= watchdog.timeout.as_nanos() {
        return None;
    }
    if matches!(watchdog.action, WatchdogAction::Restart) {
        Some(OverrunAction::Restart)
    } else {
        Some(OverrunAction::Halt)
    }
}

/// The state a stop request leaves after the safe state and the retain save both ran: `Stopped`,
/// or `Faulted` with the first failure.
///
/// A stop request ends the scan at once, so a failure here is recorded without the faulted wait.
#[must_use]
pub fn stop_outcome(
    safe_state: Result<(), RuntimeError>,
    retain_save: Result<(), RuntimeError>,
) -> (ResourceState, Option<RuntimeError>) {
    match safe_state.and(retain_save) {
        Ok(()) => (ResourceState::Stopped, None),
        Err(error) => (ResourceState::Faulted, Some(error)),
    }
}

#[cfg(test)]
mod contract_tests;
