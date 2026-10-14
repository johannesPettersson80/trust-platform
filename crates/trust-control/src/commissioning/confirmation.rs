//! The confirmation window of a transfer or a parameter change, and the interval of its status reads (`confirmationWindow.ts`;
//! specification 35, A7). The status request is the caller's: this module decides from the runtime's answer.

use serde_json::Value;

use super::observation::exact_f64;
use crate::limits::CONNECTION_LOSS_TIMEOUT_MS;

/// The shortest confirmation window: 10 s (A7).
const MINIMUM_WINDOW_MS: f64 = 10_000.0;
/// The fastest status read: every 25 ms.
const FASTEST_POLL_MS: f64 = 25.0;
/// The slowest status read: at most every second.
const SLOWEST_POLL_MS: f64 = 1_000.0;

/// How long a transfer or a parameter change may take to be confirmed, and how often its status is read (`ConfirmationTiming`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timing {
    /// The confirmation window, in ms of wall time.
    pub window_ms: f64,
    /// The interval between two status reads, in ms.
    pub poll_ms: f64,
}

/// The confirmation timing of a change on the loop's task, from the runtime's status answer (`confirmationTiming`, A7). `status` is
/// None when the status request failed; the task interval is then unknown and counts as zero.
///
/// The window is 10 s, or three executions of the task plus the connection-loss timeout when that is longer. The status is read
/// about four times per execution, and never faster than every 25 ms or slower than every second.
#[must_use]
pub fn confirmation_timing(status: Option<&Value>, task_name: &str) -> Timing {
    let interval_ms = task_interval_ms(status, task_name);
    Timing {
        window_ms: confirmation_window_ms(interval_ms),
        poll_ms: (interval_ms / 4.0).clamp(FASTEST_POLL_MS, SLOWEST_POLL_MS),
    }
}

/// The confirmation window after executions of a task of `interval_ms` (A7): 10 s, or three executions plus the connection-loss
/// timeout when that is longer.
#[must_use]
pub fn confirmation_window_ms(interval_ms: f64) -> f64 {
    MINIMUM_WINDOW_MS.max(3.0 * interval_ms + exact_f64(CONNECTION_LOSS_TIMEOUT_MS))
}

/// The interval of the named task in the runtime's status, in ms (`taskIntervalMs`). Zero when the status holds no task of that
/// name with a positive, finite interval; the first task of the name decides. The status is read as the runtime answers it, so a
/// malformed entry is skipped rather than refusing the whole status.
#[must_use]
fn task_interval_ms(status: Option<&Value>, task_name: &str) -> f64 {
    let Some(tasks) = status
        .and_then(|status| status.get("tasks"))
        .and_then(Value::as_array)
    else {
        return 0.0;
    };
    tasks
        .iter()
        .find(|task| task.get("name").and_then(Value::as_str) == Some(task_name))
        .and_then(|task| task.get("interval_ms"))
        .and_then(Value::as_f64)
        .filter(|interval| interval.is_finite() && *interval > 0.0)
        .unwrap_or(0.0)
}

#[cfg(test)]
#[path = "confirmation_tests.rs"]
mod tests;
