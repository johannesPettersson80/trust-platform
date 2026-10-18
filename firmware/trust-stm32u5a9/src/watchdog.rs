//! The hardware watchdog of the resource loop: the STM32U5A9's independent watchdog (design §1.5)
//! started at the timeout the installed record needs, and fed by the runner once per feed interval
//! (specification 34, TIME-04). Only a reset stops the counter, and the chip never sets the
//! debug-freeze bit, so a probe halt longer than the timeout resets the chip (the bench's test).
use stm32u5::stm32u5a9 as pac;
use trust_platform_stm32u5::watchdog::{Watchdog as Iwdg, MAX_INTERVAL_MS};
use trust_runtime_core::ports::{HardwareWatchdog, TimeSpan};

use crate::profile::{CYCLE_MARGIN_MS, FEED_INTERVAL_MS, MIN_TIMEOUT_MS};

/// The timeout of a stopped device, which has no cycle watchdog: the 4 s minimum.
pub const STOPPED_TIMEOUT_MS: u32 = MIN_TIMEOUT_MS;

/// The hardware timeout for the record's cycle watchdog timeout `cycle_timeout_ms`: the larger of
/// 4 s and the cycle timeout plus 2 s, without overflow (specification 34, TIME-04).
///
/// # Errors
///
/// The refusal's text when the timeout is longer than the chip's 32,767 ms interval.
pub fn timeout_for(cycle_timeout_ms: u32) -> Result<u32, &'static str> {
    let milliseconds = cycle_timeout_ms
        .saturating_add(CYCLE_MARGIN_MS)
        .max(MIN_TIMEOUT_MS);
    if milliseconds > MAX_INTERVAL_MS {
        return Err(
            "the installed application's watchdog timeout is longer than the STM32U5A9's 32,767 ms",
        );
    }
    Ok(milliseconds)
}

/// The chip's independent watchdog as the runtime's hardware watchdog.
pub struct Watchdog {
    chip: Iwdg,
}

impl Watchdog {
    /// Takes the watchdog, which does not run until [`Watchdog::start`].
    pub fn new(iwdg: pac::IWDG) -> Self {
        Self {
            chip: Iwdg::new(iwdg),
        }
    }

    /// Starts the counter for `milliseconds`, a value from [`timeout_for`].
    ///
    /// # Errors
    ///
    /// The refusal's text when the chip does not take the interval.
    pub fn start(&mut self, milliseconds: u32) -> Result<(), &'static str> {
        self.chip
            .start(milliseconds)
            .map_err(|_| "the independent watchdog did not take its interval")
    }
}

impl HardwareWatchdog for Watchdog {
    fn feed_interval(&self) -> TimeSpan {
        TimeSpan::new(core::time::Duration::from_millis(u64::from(
            FEED_INTERVAL_MS,
        )))
    }

    fn feed(&mut self) {
        self.chip.feed();
    }
}
