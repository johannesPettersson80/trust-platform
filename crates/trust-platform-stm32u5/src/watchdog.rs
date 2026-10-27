//! The STM32U5A9's independent watchdog (specification 34, D.5; design §1.5; RM0456 and ST's
//! `stm32u5xx_hal_iwdg.c`). It counts the 32 kHz LSI down from a 12-bit reload, behind a prescaler of
//! 4 to 256, and it keeps counting in Stop and Standby. Only a reset stops it, so the scan feeds it
//! when a pass is accepted (S1b-9u). The interval-to-register rule is pure and host-tested; the
//! registers are compiled for the chip only.
//!
//! This module never sets the debug-freeze bit (`DBG_IWDG_STOP` in DBGMCU), so a probe halt longer
//! than the interval resets the chip. That is the bench's watchdog test (S1b §2.5), as on the STM32F407.

/// The frequency of the LSI, the clock the watchdog counts, in hertz (design §1.5).
pub const LSI_HZ: u32 = 32_000;
/// The longest interval the adapter accepts, in nominal milliseconds (the STM32F407 limit). The U5's
/// hardware maximum is 32.768 s, 4,096 × 256 ticks at 32 kHz (design §1.5), so this limit is inside it.
pub const MAX_INTERVAL_MS: u32 = 32_767;
/// LSI ticks per millisecond: 32 at 32 kHz.
const TICKS_PER_MS: u32 = LSI_HZ / 1_000;
/// The counts the 12-bit reload holds: a reload value of 4,095 is 4,096 counts (design §1.5).
const RELOAD_COUNTS: u32 = 4_096;
/// The IWDG_PR codes with their dividers of the LSI: code 0 divides by 4 and code 6 by 256 (RM0456).
const PRESCALERS: [(u8, u32); 7] = [
    (0, 4),
    (1, 8),
    (2, 16),
    (3, 32),
    (4, 64),
    (5, 128),
    (6, 256),
];

/// What the watchdog's registers take for one interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    /// The value of IWDG_PR, 0 to 6 (a divider of 4 to 256).
    pub prescaler: u8,
    /// The value of IWDG_RLR: the counter reloads to it, 0 to 4,095.
    pub reload: u16,
}

/// Why the watchdog cannot be started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogError {
    /// The interval is not 1 to 32,767 nominal milliseconds.
    Interval,
    /// The registers did not take their new values in time (ST's `HAL_IWDG_Init` timeout).
    Timeout,
}

/// The prescaler and the reload for an interval of `milliseconds` nominal milliseconds.
///
/// The counter ticks 32 times per millisecond. The smallest prescaler whose counts fit the 12-bit
/// reload is chosen, so the counter takes the finest step. The reload then rounds the interval up to
/// whole counts, so the watchdog never fires before the interval has passed on the nominal LSI. The
/// rounding is less than one count of that divider: at most 8 ms, at divider 256.
///
/// # Errors
///
/// [`WatchdogError::Interval`] when `milliseconds` is 0 or above 32,767.
pub fn timing_for_ms(milliseconds: u32) -> Result<Timing, WatchdogError> {
    if !(1..=MAX_INTERVAL_MS).contains(&milliseconds) {
        return Err(WatchdogError::Interval);
    }
    // At most 32,767 × 32 = 1,048,544 ticks, which fits 32 bits.
    let ticks = milliseconds * TICKS_PER_MS;
    for (prescaler, divider) in PRESCALERS {
        let counts = ticks.div_ceil(divider);
        if counts <= RELOAD_COUNTS {
            // At least one count (the interval is at least 1 ms), at most 4,096: the reload fits 12 bits.
            let reload = u16::try_from(counts - 1).map_err(|_| WatchdogError::Interval)?;
            return Ok(Timing { prescaler, reload });
        }
    }
    // Unreachable for an interval of at most 32,767 ms: divider 256 always fits 4,096 counts.
    Err(WatchdogError::Interval)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub use hardware::Watchdog;

#[cfg(all(target_arch = "arm", target_os = "none"))]
mod hardware {
    use stm32u5::stm32u5a9 as pac;

    use super::{timing_for_ms, Timing, WatchdogError};

    /// IWDG_KR key that starts the counter; the LSI is forced on with it (RM0456).
    const KEY_START: u16 = 0xCCCC;
    /// IWDG_KR key that unlocks IWDG_PR and IWDG_RLR for writing (RM0456).
    const KEY_ACCESS: u16 = 0x5555;
    /// IWDG_KR key that reloads the counter from IWDG_RLR: the feed (RM0456).
    const KEY_RELOAD: u16 = 0xAAAA;
    /// Polls of IWDG_SR before an update is taken as lost. The status needs up to five LSI periods
    /// (about 156 µs), so this bound is far longer than the update takes.
    const STATUS_POLLS: u32 = 1 << 16;

    /// The independent watchdog, started once and fed by the scan.
    pub struct Watchdog {
        iwdg: pac::IWDG,
    }

    impl Watchdog {
        /// Takes the watchdog. It does not run until [`Watchdog::start`].
        #[must_use]
        pub fn new(iwdg: pac::IWDG) -> Self {
            Self { iwdg }
        }

        /// Starts the counter for `milliseconds` nominal milliseconds, as ST's `HAL_IWDG_Init` does: the
        /// start key, the unlock key, the prescaler and the reload, a wait until the registers have taken
        /// them, and a first reload. Once started, only a reset stops the counter.
        ///
        /// # Errors
        ///
        /// [`WatchdogError::Interval`] before any register is written, or [`WatchdogError::Timeout`]
        /// when the registers do not take their values in time.
        pub fn start(&mut self, milliseconds: u32) -> Result<(), WatchdogError> {
            let timing = timing_for_ms(milliseconds)?;
            self.write_key(KEY_START);
            self.write_key(KEY_ACCESS);
            self.write_timing(timing);
            self.wait_for_update()?;
            self.feed();
            Ok(())
        }

        /// Reloads the counter, so that it counts a whole interval again. The scan calls it only when
        /// it has accepted the work of the pass.
        pub fn feed(&mut self) {
            self.write_key(KEY_RELOAD);
        }

        /// Writes a key to IWDG_KR.
        fn write_key(&self, key: u16) {
            // SAFETY: IWDG_KR takes any 16-bit key. The keys are ST's three (0xCCCC, 0x5555, 0xAAAA), and
            // only this module writes IWDG_KR.
            self.iwdg.kr().write(|w| unsafe { w.bits(key) });
        }

        /// Writes the prescaler and the reload, after the unlock key.
        fn write_timing(&self, timing: Timing) {
            // SAFETY: IWDG_PR takes a 4-bit prescaler code, and `timing_for_ms` returns only codes 0 to 6.
            self.iwdg
                .pr()
                .write(|w| unsafe { w.bits(u16::from(timing.prescaler)) });
            // SAFETY: IWDG_RLR takes a 12-bit reload, and `timing_for_ms` returns at most 4,095.
            self.iwdg.rlr().write(|w| unsafe { w.bits(timing.reload) });
        }

        /// Waits until the prescaler, reload, window and early-wakeup registers have taken their values
        /// (IWDG_SR), as `HAL_IWDG_Init` waits.
        fn wait_for_update(&self) -> Result<(), WatchdogError> {
            for _ in 0..STATUS_POLLS {
                let status = self.iwdg.sr().read();
                let pending = status.pvu().bit()
                    || status.rvu().bit()
                    || status.wvu().bit()
                    || status.ewu().bit();
                if !pending {
                    return Ok(());
                }
            }
            Err(WatchdogError::Timeout)
        }
    }
}
