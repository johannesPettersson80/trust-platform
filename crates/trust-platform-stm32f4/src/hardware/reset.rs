//! Independent watchdog and reset-phase evidence; not a retain store.
use super::BoardError;
use crate::{pac, ResetCookie, ResetReason};
use stm32f4xx_hal::{prelude::*, rcc::Rcc, watchdog::IndependentWatchdog};
use trust_platform_memory::reset::{self, ResetCause, ResetRecord, ResetStore, RECORD_WORDS};

/// First RTC backup register holding the reset record; BKP0R and BKP1R keep the cookie.
const RECORD_FIRST_REGISTER: usize = 2;
/// RTC backup register holding the boot counter.
const BOOT_COUNT_REGISTER: usize = 14;

/// Independent hardware watchdog using the uncalibrated nominal 32kHz LSI.
pub struct Watchdog {
    inner: IndependentWatchdog,
}
impl Watchdog {
    pub(super) fn new(iwdg: pac::IWDG) -> Self {
        Self {
            inner: IndependentWatchdog::new(iwdg),
        }
    }
    pub(super) fn continue_while_debug_halted(&mut self, debug: &pac::DBGMCU) {
        self.inner.stop_on_debug(debug, false);
    }
    /// Start a nominal interval in 1..=32767ms. Once started, only reset stops it.
    /// Real LSI tolerance must be recorded from hardware evidence, not assumed exact.
    pub fn start_ms(&mut self, milliseconds: u32) -> Result<(), BoardError> {
        if !(1..=32_767).contains(&milliseconds) {
            return Err(BoardError::WatchdogInterval);
        }
        self.inner.start(milliseconds.millis());
        Ok(())
    }
    /// Refresh only after the firmware supervisor accepts the completed work.
    pub fn feed(&mut self) {
        self.inner.feed();
    }
}

/// Owns the RTC backup registers, the reset flags captured before clearing RCC_CSR, and the shared
/// reset record.
pub struct ResetControl {
    rtc: pac::RTC,
    _pwr: pac::PWR,
    reason: ResetReason,
}
impl ResetControl {
    pub(super) fn new(rtc: pac::RTC, pwr: pac::PWR, reason: ResetReason, rcc: &mut Rcc) -> Self {
        rcc.apb1enr().modify(|_, w| w.pwren().set_bit());
        let _ = rcc.apb1enr().read();
        pwr.cr().modify(|_, w| w.dbp().set_bit());
        // RM0090 section 5.4.1: read back DBP before accessing backup registers.
        let _ = pwr.cr().read();
        // Backup words are APB-accessible without running the RTC calendar.
        // Never reset the backup domain here: that would erase reset evidence.
        Self {
            rtc,
            _pwr: pwr,
            reason,
        }
    }
    /// Original boot flags; they are not re-read after the hardware flags are cleared.
    pub fn reason(&self) -> ResetReason {
        self.reason
    }
    /// Valid phase marker only on non-power-reset boots. Firmware must also
    /// require `reason().independent_watchdog()` for an intentional-IWDG result.
    pub fn cookie(&self) -> Option<ResetCookie> {
        if self.reason.power_on() || self.reason.brownout() {
            return None;
        }
        ResetCookie::decode([
            self.rtc.bkp0r().read().bkp().bits(),
            self.rtc.bkp1r().read().bkp().bits(),
        ])
    }
    /// Write a checked phase marker, invalidating the old pair before replacement.
    pub fn set_cookie(&mut self, cookie: ResetCookie) -> Result<(), BoardError> {
        if cookie.phase == 0 {
            return Err(BoardError::ResetCookie);
        }
        let words = cookie.words();
        self.rtc.bkp1r().write(|w| w.bkp().set(0));
        self.rtc.bkp0r().write(|w| w.bkp().set(words[0]));
        self.rtc.bkp1r().write(|w| w.bkp().set(words[1]));
        let observed = [
            self.rtc.bkp0r().read().bkp().bits(),
            self.rtc.bkp1r().read().bkp().bits(),
        ];
        if observed != words {
            return Err(BoardError::ResetCookie);
        }
        Ok(())
    }
    /// Invalidate the phase marker after the reset observation has been recorded.
    pub fn clear_cookie(&mut self) {
        self.rtc.bkp1r().write(|w| w.bkp().set(0));
        self.rtc.bkp0r().write(|w| w.bkp().set(0));
    }

    /// Write the record a panic or watchdog path leaves for the next boot: the cause, the panic
    /// location when there is one, and the boot counter. It lives in the RTC backup registers, so
    /// it survives the reset the same way the cookie does.
    pub fn write_reset_record(&mut self, cause: ResetCause, text: &str) {
        reset::write_record(self, cause, text);
    }

    /// Read the record the previous boot left, with its cause and text, and clear it, so a cause is
    /// reported once. A damaged record reads as `None`, never as a cause.
    pub fn take_reset_record(&mut self) -> Option<ResetRecord> {
        reset::take_record(self)
    }

    /// Count this boot in the RTC backup registers and return the new boot counter the next record
    /// is stamped with.
    pub fn note_boot(&mut self) -> u32 {
        reset::note_boot(self)
    }
}

impl ResetStore for ResetControl {
    fn record_words(&self) -> [u32; RECORD_WORDS] {
        let mut words = [0u32; RECORD_WORDS];
        for (index, word) in words.iter_mut().enumerate() {
            *word = self
                .rtc
                .bkpr(RECORD_FIRST_REGISTER + index)
                .read()
                .bkp()
                .bits();
        }
        words
    }

    fn set_record_words(&mut self, words: [u32; RECORD_WORDS]) {
        for (index, &word) in words.iter().enumerate() {
            self.rtc
                .bkpr(RECORD_FIRST_REGISTER + index)
                .write(|w| w.bkp().set(word));
        }
    }

    fn boot_count(&self) -> u32 {
        self.rtc.bkpr(BOOT_COUNT_REGISTER).read().bkp().bits()
    }

    fn set_boot_count(&mut self, count: u32) {
        self.rtc
            .bkpr(BOOT_COUNT_REGISTER)
            .write(|w| w.bkp().set(count));
    }
}
