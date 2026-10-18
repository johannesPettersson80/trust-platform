//! The engine's execution services on a device (specification 34, §5.2, §7.1 and TIME-03): the
//! scan deadline on the platform's monotonic clock.

use core::cell::Cell;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::value::DateTimeValue;
use trust_runtime_core::vm::ExecutionServices;

/// The admission refusal of `CURRENT_DT` without a wall clock, the engine's own text.
const NO_WALL_CLOCK: &str = "CURRENT_DT requires an admitted platform wall clock";

/// The engine's execution services on a device: the scan deadline, armed for each scan on the
/// platform's monotonic clock, and no wall clock.
///
/// A device has no wall clock until slice T6 gives it the platform's `WallClock`, so the
/// composition is logical-only: the engine refuses a program that calls `CURRENT_DT` when it
/// constructs the program (spec 34 §5.2), and the replies carry `null` for wall-clock times
/// (TARGET-06). It has no retain store until slice C5.
pub struct DeviceServices<C> {
    clock: C,
    deadline: Cell<Option<MonotonicInstant>>,
}

impl<C> DeviceServices<C> {
    /// Services on `clock`, with no deadline armed.
    #[must_use]
    pub const fn new(clock: C) -> Self {
        Self {
            clock,
            deadline: Cell::new(None),
        }
    }

    /// Arms the scan deadline at `deadline` on the clock, or disarms it with `None`.
    pub(super) fn arm(&self, deadline: Option<MonotonicInstant>) {
        self.deadline.set(deadline);
    }
}

impl<C: MonotonicClock> ExecutionServices for DeviceServices<C> {
    /// Whether the clock has reached the armed deadline; never while none is armed.
    fn deadline_exceeded(&self) -> bool {
        self.deadline
            .get()
            .is_some_and(|deadline| self.clock.now() >= deadline)
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
