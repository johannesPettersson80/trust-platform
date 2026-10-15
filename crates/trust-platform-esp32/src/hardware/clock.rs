//! SYSTIMER time, the 1 kHz tick instrumentation and the CPU cycle counter.
use super::BoardError;
use core::{
    cell::RefCell,
    hint,
    sync::atomic::{AtomicU32, Ordering},
};
use critical_section::Mutex;
use esp_hal::{
    handler,
    time::{Duration, Instant},
    timer::{systimer::Alarm, PeriodicTimer},
    Blocking,
};

static IRQ_COUNT: AtomicU32 = AtomicU32::new(0);
static TICK: Mutex<RefCell<Option<PeriodicTimer<'static, Blocking>>>> =
    Mutex::new(RefCell::new(None));

/// 1 kHz SYSTIMER alarm. It counts the interrupt and clears the alarm, and nothing else.
#[handler]
fn tick() {
    IRQ_COUNT.fetch_add(1, Ordering::Relaxed);
    critical_section::with(|cs| {
        if let Some(timer) = TICK.borrow_ref_mut(cs).as_mut() {
            timer.clear_interrupt();
        }
    });
}

/// Main-context clock owner. The alarm lives in a static only so the handler can clear it.
pub struct Monotonic {
    _private: (),
}

impl Monotonic {
    pub(super) fn new(alarm: Alarm<'static>) -> Self {
        let mut timer = PeriodicTimer::new(alarm);
        timer.set_interrupt_handler(tick);
        critical_section::with(|cs| *TICK.borrow_ref_mut(cs) = Some(timer));
        Self { _private: () }
    }

    /// Microseconds since boot from SYSTIMER (16 MHz, 52 bits): no rollover extension is needed.
    pub fn micros(&self) -> u64 {
        now_us()
    }

    /// Poll SYSTIMER until `deadline` (microseconds since boot). This is deliberately a busy
    /// wait: no wake interrupt is required and no low-power claim is made.
    pub fn wait_until_us(&self, deadline: u64) {
        while now_us() < deadline {
            hint::spin_loop();
        }
    }

    /// Wrapping CPU cycle counter (PCCR, CSR 0x7E2). Compare intervals with `wrapping_sub`;
    /// one interval must stay below 2^32 cycles, about 26.8 s at 160 MHz.
    pub fn cycle_count(&self) -> u32 {
        cycles_now()
    }

    /// Start (`true`) or stop (`false`) the 1 kHz tick. The cumulative count is not reset.
    pub fn enable_tick(&mut self, enable: bool) -> Result<(), BoardError> {
        critical_section::with(|cs| {
            let mut slot = TICK.borrow_ref_mut(cs);
            let Some(timer) = slot.as_mut() else {
                return Err(BoardError::TickTimer);
            };
            if enable {
                timer.listen();
                timer
                    .start(Duration::from_millis(1))
                    .map_err(|_| BoardError::TickTimer)
            } else {
                timer.unlisten();
                // `TimerInactive` only reports that the alarm was already stopped.
                let _ = timer.cancel();
                Ok(())
            }
        })
    }

    /// Tick interrupts counted since boot, wrapping at `u32::MAX`.
    pub fn irq_count(&self) -> u32 {
        IRQ_COUNT.load(Ordering::Relaxed)
    }
}

/// SYSTIMER microseconds since boot.
pub(super) fn now_us() -> u64 {
    Instant::now().duration_since_epoch().as_micros()
}

/// CPU cycles since boot from the machine performance counter (CSR 0x7E2), read the same way
/// esp-hal's RNG driver reads it.
pub(super) fn cycles_now() -> u32 {
    let cycles: u32;
    // SAFETY: `csrrs rd, 0x7e2, x0` only reads the performance counter CSR. It writes no CSR,
    // touches no memory and has no side effect besides the register it fills.
    unsafe {
        core::arch::asm!("csrrs {0}, 0x7e2, x0", out(reg) cycles, options(nomem, nostack));
    }
    cycles
}
