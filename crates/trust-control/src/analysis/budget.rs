//! The wall-time budget of an analysis operation (specification 35, A7; draft §1.5, Q4): ten seconds, checked between the steps of a
//! calculation (one frequency point, one sweep value, one Routh row, one response sample), with the operation's cancellation checked
//! at the same points (C24). The clock and the cancel flag are passed in, so the calculations stay deterministic in tests (see
//! `lib.rs`): the engine reads the real clock and its operation's flag, and a test reads a fake clock.

use std::time::Duration;

use crate::failure::{Cause, Failure, NextAction};

/// The wall time an analysis operation may take (A7): ten seconds.
pub const ANALYSIS_BUDGET: Duration = Duration::from_secs(10);

/// The sentence of an analysis that passes its budget (A7).
pub const BUDGET_EXCEEDED: &str = "Analysis exceeded the 10 second budget.";

/// The sentence of a step that a cancel stopped. The engine's operation reports the cancellation itself as cancelled, so this
/// sentence is only the work's own reason.
const CANCELLED: &str = "Analysis was cancelled.";

/// The budget of one analysis: the limit, the time spent so far, and whether the operation was cancelled.
pub struct Budget<'a> {
    limit: Duration,
    elapsed: &'a dyn Fn() -> Duration,
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a> Budget<'a> {
    /// A budget of `limit`, whose time spent and cancellation are read from `elapsed` and `cancelled`.
    #[must_use]
    pub fn new(
        limit: Duration,
        elapsed: &'a dyn Fn() -> Duration,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            limit,
            elapsed,
            cancelled,
        }
    }

    /// Checks the budget before the next step.
    ///
    /// # Errors
    /// Returns the cancellation of the operation, which is checked first, or the A7 sentence once the time spent is past the limit.
    pub fn step(&self) -> Result<(), Failure> {
        if (self.cancelled)() {
            return Err(Failure::new(
                CANCELLED,
                Cause::Unclassified,
                NextAction::None,
            ));
        }
        if (self.elapsed)() > self.limit {
            return Err(Failure::new(
                BUDGET_EXCEEDED,
                Cause::Settings,
                NextAction::None,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
