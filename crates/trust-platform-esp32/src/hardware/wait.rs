//! The resource loop's clock, wait and wake on the ESP32-C6 (specification 34, PORT-03 and
//! design §2.5): the SYSTIMER monotonic clock, a one-shot SYSTIMER alarm at the wait's deadline,
//! and one wake counter that the alarm and the control link advance.
//!
//! The wait checks the wake counter and the deadline, arms the alarm at the deadline, then waits
//! for an interrupt with interrupts masked between the check and the wait. A pending interrupt
//! ends the wait even while it is masked, so a wake that arrives in that window is never lost;
//! after the wait the deadline tells whether the alarm or a wake ended it.

use core::{
    cell::RefCell,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};

use critical_section::Mutex;
use esp_hal::{
    handler,
    interrupt::wait_for_interrupt,
    time::Duration as HalDuration,
    timer::{systimer::Alarm, OneShotTimer},
    Blocking,
};
use trust_runtime_core::ports::{
    MonotonicClock, MonotonicInstant, PortError, RunnerWait, WakeReason, WakeSignal, WakeToken,
};

use super::{clock::now_us, Monotonic};

/// `PortError` code of a wait whose deadline alarm is not configured.
const WAIT_NO_ALARM: u16 = 1;
/// `PortError` code of a wait whose remaining time the alarm refused.
const WAIT_BAD_DEADLINE: u16 = 2;
/// The `PortError` device number of the one SYSTIMER the wait uses.
const WAIT_DEVICE: u16 = 0;

/// Wakes counted since boot; the deadline alarm and the control link advance it (design §2.5).
static WAKE_COUNT: AtomicU32 = AtomicU32::new(0);

/// The one-shot deadline alarm, in a static so the handler can clear its interrupt.
static ALARM: Mutex<RefCell<Option<OneShotTimer<'static, Blocking>>>> =
    Mutex::new(RefCell::new(None));

/// Advances the wake counter, so the resource loop's wait ends at once (design §2.5). The deadline
/// alarm, the control link and [`WakeSignal::wake`] all advance the one counter the wait reads.
fn advance_wake() {
    WAKE_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// The control link's view of the wake counter: every event the link handles advances the counter,
/// so the wait ends for it at once (design §2.5).
pub(super) struct LinkWake;

impl WakeSignal for LinkWake {
    fn wake(&self) {
        advance_wake();
    }
}

/// The deadline alarm's handler: it advances the wake counter and clears the interrupt.
#[handler]
fn wait_alarm() {
    advance_wake();
    critical_section::with(|cs| {
        if let Some(timer) = ALARM.borrow_ref_mut(cs).as_mut() {
            timer.clear_interrupt();
            timer.unlisten();
        }
    });
}

impl MonotonicClock for Monotonic {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(Duration::from_micros(self.micros()))
    }
}

/// The resource loop's wait on the C6: it owns the SYSTIMER deadline alarm and reads the one
/// wake counter.
pub struct Wait {
    _private: (),
}

impl Wait {
    /// Binds the wait to `alarm`, a SYSTIMER alarm the firmware keeps free for it, and installs
    /// the handler that advances the wake counter.
    #[must_use]
    pub fn new(alarm: Alarm<'static>) -> Self {
        let mut timer = OneShotTimer::new(alarm);
        timer.set_interrupt_handler(wait_alarm);
        critical_section::with(|cs| *ALARM.borrow_ref_mut(cs) = Some(timer));
        Self { _private: () }
    }
}

impl RunnerWait for Wait {
    fn wake_token(&self) -> WakeToken {
        WakeToken::new(WAKE_COUNT.load(Ordering::Relaxed))
    }

    fn wait_until(
        &mut self,
        deadline: MonotonicInstant,
        observed: WakeToken,
    ) -> Result<WakeReason, PortError> {
        critical_section::with(|cs| {
            // A wake that arrived since the loop read the token ends the wait at once.
            if WAKE_COUNT.load(Ordering::Relaxed) != observed.count() {
                return Ok(WakeReason::Event);
            }
            // So does a deadline that has already passed.
            if now_instant() >= deadline {
                return Ok(WakeReason::Deadline);
            }
            // Arm the alarm for the remaining time and wait with interrupts masked: a wake that
            // arrives between the checks and the WFI is not lost, because the pending interrupt
            // ends the WFI even while it is masked.
            let mut slot = ALARM.borrow_ref_mut(cs);
            let Some(timer) = slot.as_mut() else {
                return Err(wait_error(WAIT_NO_ALARM));
            };
            let remaining = deadline.saturating_since(now_instant()).as_duration();
            let Ok(remaining_us) = u64::try_from(remaining.as_micros()) else {
                return Err(wait_error(WAIT_BAD_DEADLINE));
            };
            if timer
                .schedule(HalDuration::from_micros(remaining_us))
                .is_err()
            {
                return Err(wait_error(WAIT_BAD_DEADLINE));
            }
            timer.listen();
            wait_for_interrupt();
            // The alarm or a wake ended the wait; the deadline tells which.
            if now_instant() >= deadline {
                Ok(WakeReason::Deadline)
            } else {
                Ok(WakeReason::Event)
            }
        })
    }
}

impl WakeSignal for Wait {
    fn wake(&self) {
        advance_wake();
    }
}

/// The monotonic clock's current reading, the same SYSTIMER epoch as [`Monotonic::micros`] and the
/// deadline the wait compares against.
pub(super) fn now_instant() -> MonotonicInstant {
    MonotonicInstant::from_origin(Duration::from_micros(now_us()))
}

/// The `PortError` of a wait mechanism that failed.
const fn wait_error(code: u16) -> PortError {
    PortError {
        code,
        device: WAIT_DEVICE,
    }
}
