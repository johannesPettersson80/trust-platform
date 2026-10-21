//! SYSTIMER time, the 1 kHz tick instrumentation and the CPU cycle counter.
use super::BoardError;
use crate::cycle_clock::CycleBase;
use crate::profile::CPU_HZ;
use core::{
    cell::{Cell, RefCell},
    hint,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
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
        CYCLES_COUNT.store(cycles_counting(), Ordering::Relaxed);
        Self { _private: () }
    }

    /// Microseconds since boot from SYSTIMER (16 MHz, 52 bits): no rollover extension is needed.
    /// While the CPU runs, a reading extends the last SYSTIMER reading by the cycle counter
    /// (`now_us`).
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

/// Whether the CPU cycle counter counts, found when the clock is made (`Monotonic::new`). When
/// it does not, every reading reads SYSTIMER.
static CYCLES_COUNT: AtomicBool = AtomicBool::new(false);

/// What the last SYSTIMER reading gives the readings after it (`crate::cycle_clock`).
static BASE: Mutex<Cell<CycleBase>> = Mutex::new(Cell::new(CycleBase::STALE));

/// CPU cycles in a microsecond.
const CYCLES_PER_MICROSECOND: u32 = CPU_HZ / 1_000_000;

/// The cycles a reading extends a SYSTIMER reading by at most: one second, far inside the
/// counter's wrap of about 26.8 s.
const REBASE_CYCLES: u32 = CPU_HZ;

/// SYSTIMER microseconds since boot, extended by the CPU cycle counter while the CPU runs
/// (checklist P1-6; `crate::cycle_clock`).
pub(super) fn now_us() -> u64 {
    if !CYCLES_COUNT.load(Ordering::Relaxed) {
        return systimer_us();
    }
    let cycles = cycles_now();
    critical_section::with(|cs| {
        let cell = BASE.borrow(cs);
        let base = cell.get();
        let (base, reading) = match base.extend(cycles, CYCLES_PER_MICROSECOND, REBASE_CYCLES) {
            Some(reading) => (base, reading),
            None => {
                let micros = systimer_us();
                (base.rebased(micros, cycles), micros)
            }
        };
        let (reading, base) = base.give(reading);
        cell.set(base);
        reading
    })
}

/// A wait stopped the CPU, whose cycle counter need not count then: the next reading reads
/// SYSTIMER.
pub(super) fn cpu_stopped() {
    critical_section::with(|cs| {
        let cell = BASE.borrow(cs);
        cell.set(cell.get().stopped());
    });
}

/// SYSTIMER microseconds since boot, read from the timer.
fn systimer_us() -> u64 {
    Instant::now().duration_since_epoch().as_micros()
}

/// Whether the CPU cycle counter counts: over 20 µs of SYSTIMER it advances at least half the 3,200
/// cycles of 160 MHz.
fn cycles_counting() -> bool {
    let before = cycles_now();
    let start = systimer_us();
    while systimer_us() < start + 20 {
        hint::spin_loop();
    }
    cycles_now().wrapping_sub(before) >= 20 * CYCLES_PER_MICROSECOND / 2
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
