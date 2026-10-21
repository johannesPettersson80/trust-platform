//! The debugger holds of a device's scans on its monotonic clock (specification 11 §6.6 and §6.9;
//! specification 34 STATE-07). A hold suspends the cycle, so its time is no active time: the scan's
//! deadline moves forward by each hold, and a hold in progress never counts as an overrun.

use core::cell::Cell;

use trust_runtime_core::ports::{MonotonicInstant, TimeSpan};

/// The holds of a device on its monotonic clock: the total length of the completed holds, and the
/// reading at which the hold in progress started. Both are spans of the clock, so that the scan's
/// deadline check, which asks for them at every sample, adds and compares integers (specification
/// 34 PERF-05).
#[derive(Debug)]
pub(super) struct HeldTime {
    completed: Cell<TimeSpan>,
    started: Cell<Option<MonotonicInstant>>,
}

impl HeldTime {
    /// No holds yet.
    pub(super) const fn new() -> Self {
        Self {
            completed: Cell::new(TimeSpan::ZERO),
            started: Cell::new(None),
        }
    }

    /// A hold starts at `now`. A hold already in progress keeps its start.
    pub(super) fn start(&self, now: MonotonicInstant) {
        if self.started.get().is_none() {
            self.started.set(Some(now));
        }
    }

    /// The hold in progress ends at `now`, and its length joins the completed holds. An end with no
    /// hold in progress adds nothing.
    pub(super) fn end(&self, now: MonotonicInstant) {
        if let Some(started) = self.started.take() {
            let held = now.saturating_since(started);
            self.completed
                .set(self.completed.get().saturating_add(held));
        }
    }

    /// The completed holds and the hold in progress up to `now`, which counts as held.
    pub(super) fn held_at(&self, now: MonotonicInstant) -> TimeSpan {
        let in_progress = self
            .started
            .get()
            .map_or(TimeSpan::ZERO, |started| now.saturating_since(started));
        self.completed.get().saturating_add(in_progress)
    }
}
