//! MWDT0 supervision and the reset cookie in RTC fast memory, which survives MWDT0 and USB resets.
use super::BoardError;
use crate::{cookie::ResetCookie, profile::WATCHDOG_MS};
use core::ptr::{read_volatile, write_volatile};
use esp_hal::{
    peripherals::TIMG0,
    system::reset_reason,
    time::Duration,
    timer::timg::{MwdtStage, MwdtStageAction, TimerGroup, Wdt},
};

/// Largest accepted MWDT0 stage-0 interval in milliseconds.
const MAX_WATCHDOG_MS: u32 = 60_000;
/// Reason code reported when the hardware cannot determine the boot reset.
const REASON_UNKNOWN: u8 = 0xff;

/// Main watchdog of TIMG0 (MWDT0), stage 0 resetting the system.
pub struct Watchdog {
    inner: Wdt<TIMG0<'static>>,
}

impl Watchdog {
    /// Configure the stage-0 interval and action. The watchdog stays disabled until `start_ms`.
    pub(super) fn new(timg0: TIMG0<'static>) -> Self {
        let mut inner = TimerGroup::new(timg0).wdt;
        inner.set_timeout(
            MwdtStage::Stage0,
            Duration::from_millis(u64::from(WATCHDOG_MS)),
        );
        inner.set_stage_action(MwdtStage::Stage0, MwdtStageAction::ResetSystem);
        Self { inner }
    }

    /// Enable MWDT0 with a stage-0 interval in 1..=60,000 ms. Only a reset stops it afterwards.
    pub fn start_ms(&mut self, milliseconds: u32) -> Result<(), BoardError> {
        if !(1..=MAX_WATCHDOG_MS).contains(&milliseconds) {
            return Err(BoardError::WatchdogInterval);
        }
        self.inner.set_timeout(
            MwdtStage::Stage0,
            Duration::from_millis(u64::from(milliseconds)),
        );
        self.inner
            .set_stage_action(MwdtStage::Stage0, MwdtStageAction::ResetSystem);
        self.inner.enable();
        Ok(())
    }

    /// Refresh the watchdog once the firmware supervisor has accepted the completed work.
    pub fn feed(&mut self) {
        self.inner.feed();
    }
}

/// Boot reset reason, plus the two-word cookie in RTC fast memory.
pub struct ResetControl {
    reason: u8,
}

impl ResetControl {
    /// Capture the raw reset reason of this boot, or `0xff` when it cannot be determined.
    pub(super) fn new() -> Self {
        Self {
            reason: reset_reason().map_or(REASON_UNKNOWN, |reason| reason as u8),
        }
    }

    /// Raw boot reset reason, for example `RESET_WATCHDOG`, `RESET_USB` or `RESET_POWER_ON`.
    pub fn reason(&self) -> u8 {
        self.reason
    }

    /// The stored cookie, decoded. It is only meaningful together with the boot reason:
    /// `profile::boot_decision(reason(), cookie())` applies the D.2 rule.
    pub fn cookie(&self) -> Option<ResetCookie> {
        ResetCookie::decode(read_words())
    }

    /// Store a nonzero phase. The old pair is invalidated first and the stored words are read
    /// back before success is reported.
    pub fn set_cookie(&mut self, cookie: ResetCookie) -> Result<(), BoardError> {
        if cookie.phase == 0 {
            return Err(BoardError::ResetCookie);
        }
        let words = cookie.words();
        write_word(1, 0);
        write_word(0, words[0]);
        write_word(1, words[1]);
        if read_words() == words {
            Ok(())
        } else {
            Err(BoardError::ResetCookie)
        }
    }

    /// Erase the cookie. Both words must read back as zero.
    pub fn clear_cookie(&mut self) -> Result<(), BoardError> {
        write_word(1, 0);
        write_word(0, 0);
        if read_words() == [0, 0] {
            Ok(())
        } else {
            Err(BoardError::ResetCookie)
        }
    }
}

/// Two RTC fast-memory words that keep their value across MWDT0 and USB-Serial/JTAG resets.
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut COOKIE: [u32; 2] = [0; 2];

/// Volatile store of one cookie word; `index` is 0 or 1.
fn write_word(index: usize, value: u32) {
    assert!(index < 2, "cookie index out of range");
    let words = (&raw mut COOKIE).cast::<u32>();
    // SAFETY: `index` was checked against the two-word static above, so `words.add(index)` stays
    // inside `COOKIE`. Only `ResetControl` writes the cookie, and volatile stores keep each word
    // in RTC memory in program order.
    unsafe { write_volatile(words.add(index), value) };
}

/// Volatile load of both cookie words in index order.
fn read_words() -> [u32; 2] {
    let words = (&raw const COOKIE).cast::<u32>();
    // SAFETY: `words` addresses the two-word `COOKIE` static, so both loads are in bounds. The
    // loads are volatile, so the value comes from RTC memory and not a cached copy.
    unsafe { [read_volatile(words), read_volatile(words.add(1))] }
}
