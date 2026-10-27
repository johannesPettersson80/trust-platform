//! The C6's wall clock (specification 34 TARGET-06, Appendix D.2): SYSTIMER's time since boot, which
//! the 40 MHz crystal drives, plus the offset of the Unix time from it that a time source set. The
//! loop copies the time with the LP timer's ticks into RTC fast memory every 10 seconds, and a boot
//! after a reset adds the ticks since that copy (`crate::calendar`): every reset but a loss of power
//! keeps the time. The LP timer runs from the internal RC oscillator, so it carries the time over a
//! reset and does not count it while the chip runs.

use core::cell::{Cell, RefCell};
use core::ptr::{read_volatile, write_volatile};

use critical_section::{CriticalSection, Mutex};
use esp_hal::peripherals::{LP_AON, RTC_TIMER};
use esp_hal::rtc_cntl::Rtc;
use trust_plc_abi::kernel::{wall_clock, with_wall_clock, WallClock};
use trust_plc_abi::{FaultCode, KernelApi};
use trust_runtime_core::native::KERNEL_API;

use super::clock::now_us;
use crate::calendar::{ticks_of, Carry, CARRY_WORDS};

/// The Unix time minus SYSTIMER's reading, in microseconds, once a time source set it.
static OFFSET_US: Mutex<Cell<Option<i64>>> = Mutex::new(Cell::new(None));
/// The low-power domain's timer, which counts over a reset.
static RTC: Mutex<RefCell<Option<Rtc<'static>>>> = Mutex::new(RefCell::new(None));
/// SYSTIMER's reading at the last copy of the time into RTC fast memory.
static KEPT_AT_US: Mutex<Cell<u64>> = Mutex::new(Cell::new(0));

/// The time carried over a reset, in RTC fast memory, which the startup zeroes only after a
/// power-on (`crate::calendar`).
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut CARRY: [u32; CARRY_WORDS] = [0; CARRY_WORDS];

/// How often the loop copies the time into RTC fast memory.
const KEEP_EVERY_US: u64 = 10_000_000;

/// Takes the low-power domain's timer and, when RTC fast memory carries a time from before the
/// reset, the clock's time from it. Called once, by `Board::take`.
pub(super) fn start(timer: RTC_TIMER<'static>) {
    let rtc = Rtc::new(timer);
    let period = period();
    let ticks = ticks_of(rtc.time_since_power_up().as_micros(), period);
    let now = now_us();
    let offset = Carry::decode(read_carry())
        .and_then(|carry| carry.unix_at(ticks, period))
        .map(|unix| offset_of(unix, now));
    critical_section::with(|cs| {
        OFFSET_US.borrow(cs).set(offset);
        KEPT_AT_US.borrow(cs).set(now);
        *RTC.borrow_ref_mut(cs) = Some(rtc);
    });
}

/// The Unix time in milliseconds, or `None` while no time source set it.
#[must_use]
pub fn utc_ms() -> Option<u64> {
    let offset = critical_section::with(|cs| OFFSET_US.borrow(cs).get())?;
    unix_us(offset, now_us()).map(|unix| unix / 1_000)
}

/// Sets the clock to the Unix time `utc_ms` and copies it into RTC fast memory.
pub fn set(utc_ms: u64) {
    let unix = utc_ms.saturating_mul(1_000);
    critical_section::with(|cs| {
        let now = now_us();
        OFFSET_US.borrow(cs).set(Some(offset_of(unix, now)));
        keep_at(cs, now);
    });
}

/// Copies the time into RTC fast memory once the last copy is 10 seconds old; the loop calls it
/// once per pass.
pub fn keep() {
    critical_section::with(|cs| {
        let now = now_us();
        if now.saturating_sub(KEPT_AT_US.borrow(cs).get()) >= KEEP_EVERY_US {
            keep_at(cs, now);
        }
    });
}

/// Copies the time at SYSTIMER's reading `now`, when it is set, with the LP timer's ticks into RTC
/// fast memory.
fn keep_at(cs: CriticalSection<'_>, now: u64) {
    KEPT_AT_US.borrow(cs).set(now);
    let Some(offset) = OFFSET_US.borrow(cs).get() else {
        return;
    };
    if let (Some(rtc), Some(unix_us)) = (RTC.borrow_ref(cs).as_ref(), unix_us(offset, now)) {
        let ticks = ticks_of(rtc.time_since_power_up().as_micros(), period());
        write_carry(Carry { unix_us, ticks }.words());
    }
}

/// The RC oscillator's calibrated period that this boot measured, which esp-hal keeps in
/// `LP_AON_STORE1` (`crate::calendar::PERIOD_FRACTION_BITS`).
fn period() -> u32 {
    LP_AON::regs().store1().read().bits()
}

/// The carried time's words, by volatile loads from RTC fast memory.
fn read_carry() -> [u32; CARRY_WORDS] {
    let words = (&raw const CARRY).cast::<u32>();
    let mut out = [0u32; CARRY_WORDS];
    for (index, word) in out.iter_mut().enumerate() {
        // SAFETY: `words` addresses the CARRY_WORDS-word CARRY static, so every load stays inside
        // it, and the volatile load takes the value from RTC memory.
        *word = unsafe { read_volatile(words.add(index)) };
    }
    out
}

/// Stores the carried time's words into RTC fast memory.
fn write_carry(words: [u32; CARRY_WORDS]) {
    let base = (&raw mut CARRY).cast::<u32>();
    for (index, &word) in words.iter().enumerate() {
        // SAFETY: `base` addresses the CARRY_WORDS-word CARRY static, so every store stays inside
        // it. Only `keep_at` writes it, inside a critical section, and volatile stores keep the
        // words in RTC memory in program order.
        unsafe { write_volatile(base.add(index), word) };
    }
}

/// The Unix time in microseconds minus SYSTIMER's reading `now`.
fn offset_of(unix_us: u64, now: u64) -> i64 {
    i64::try_from(unix_us)
        .unwrap_or(i64::MAX)
        .saturating_sub(i64::try_from(now).unwrap_or(i64::MAX))
}

/// The Unix time in microseconds at SYSTIMER's reading `now` with the offset `offset`; `None`
/// before the epoch.
fn unix_us(offset: i64, now: u64) -> Option<u64> {
    u64::try_from(offset.saturating_add(i64::try_from(now).unwrap_or(i64::MAX))).ok()
}

/// The C6's wall clock as `CURRENT_DT` reads it (specification 44 §8.2): the Unix time, or, while
/// no time source set it, the time since boot counted from the epoch (specification 34 TARGET-06).
pub struct CalendarClock;

impl WallClock for CalendarClock {
    fn unix_millis() -> Result<i64, FaultCode> {
        let millis = utc_ms().unwrap_or_else(|| now_us() / 1_000);
        i64::try_from(millis).map_err(|_| FaultCode::Overflow)
    }
}

/// The kernel's functions on the C6: the core's, with the C6's wall clock (specification 44 §8.2).
pub static KERNEL_FUNCTIONS: KernelApi = with_wall_clock(&KERNEL_API, wall_clock::<CalendarClock>);
