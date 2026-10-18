//! The engine's execution services on a device (specification 34, §5.2, §7.1 and TIME-03): the
//! scan deadline on the platform's monotonic clock, which the debugger holds of the scan move
//! forward (specification 11 §6.6; specification 34 STATE-07).

use core::cell::Cell;
use core::time::Duration;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant, TimeSpan};
use trust_runtime_core::value::DateTimeValue;
use trust_runtime_core::vm::ExecutionServices;

use super::held_time::HeldTime;

/// The admission refusal of `CURRENT_DT` without a wall clock, the engine's own text.
const NO_WALL_CLOCK: &str = "CURRENT_DT requires an admitted platform wall clock";

/// The engine's execution services on a device: the scan deadline, armed for each scan on the
/// platform's monotonic clock and moved forward by the debugger holds that run after it was armed,
/// and no wall clock.
///
/// A device has no wall clock until slice T6 gives it the platform's `WallClock`, so the
/// composition is logical-only: the engine refuses a program that calls `CURRENT_DT` when it
/// constructs the program (spec 34 §5.2), and the replies carry `null` for wall-clock times
/// (TARGET-06). It has no retain store until slice C5.
pub struct DeviceServices<C> {
    clock: C,
    deadline: Cell<Option<MonotonicInstant>>,
    holds: HeldTime,
    /// The holds that had run when the deadline was armed, so that only later holds move it.
    holds_at_arm: Cell<Duration>,
}

impl<C> DeviceServices<C> {
    /// Services on `clock`, with no deadline armed.
    #[must_use]
    pub const fn new(clock: C) -> Self {
        Self {
            clock,
            deadline: Cell::new(None),
            holds: HeldTime::new(),
            holds_at_arm: Cell::new(Duration::ZERO),
        }
    }
}

impl<C: MonotonicClock> DeviceServices<C> {
    /// Arms the scan deadline at `deadline` on the clock, or disarms it with `None`. The holds that
    /// run after this call move the deadline forward; the holds before it do not.
    pub(super) fn arm(&self, deadline: Option<MonotonicInstant>) {
        self.holds_at_arm.set(self.holds.held_at(self.clock.now()));
        self.deadline.set(deadline);
    }

    /// A debugger hold starts now (specification 11 §6.9; specification 34 STATE-07).
    pub fn hold_started(&self) {
        self.holds.start(self.clock.now());
    }

    /// The debugger hold in progress ends now. Its length moves the armed deadline forward by the
    /// same amount, as trust-runtime's `deadline_with_debug_pause` does (specification 11 §6.6).
    pub fn hold_ended(&self) {
        self.holds.end(self.clock.now());
    }

    /// The debugger holds that have run on the device's clock: the completed holds, and the hold in
    /// progress up to now (specification 11 §6.6).
    pub(super) fn held_time(&self) -> TimeSpan {
        TimeSpan::new(self.holds.held_at(self.clock.now()))
    }
}

impl<C: MonotonicClock> ExecutionServices for DeviceServices<C> {
    /// Whether the clock has reached the armed deadline, moved forward by the holds that have run
    /// since it was armed. A hold in progress counts as held, so it is never an overrun. Never
    /// while none is armed.
    fn deadline_exceeded(&self) -> bool {
        let Some(deadline) = self.deadline.get() else {
            return false;
        };
        let now = self.clock.now();
        let held = self
            .holds
            .held_at(now)
            .saturating_sub(self.holds_at_arm.get());
        now >= deadline.saturating_add(TimeSpan::new(held))
    }

    fn has_wall_clock(&self) -> bool {
        false
    }

    /// Never called: the engine refuses a program that calls `CURRENT_DT` before it runs.
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::ProfileUnsupported(SmolStr::new_static(
            NO_WALL_CLOCK,
        )))
    }
}
