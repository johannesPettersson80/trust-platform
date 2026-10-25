//! MWDT0 supervision and the reset cookie in RTC fast memory, which survives MWDT0 and USB resets.
use super::BoardError;
use crate::{
    cookie::ResetCookie,
    watchdog_rule::{watchdog_timeout_ms, FEED_INTERVAL_MS},
};
use core::ptr::{read_volatile, write_volatile};
use esp_hal::{
    peripherals::TIMG0,
    system::reset_reason,
    time::Duration,
    timer::timg::{MwdtStage, MwdtStageAction, TimerGroup, Wdt},
};
use trust_platform_memory::reset::{self, ResetCause, ResetRecord, ResetStore, RECORD_WORDS};
use trust_runtime_core::ports::{HardwareWatchdog, TimeSpan};
use trust_runtime_services::chip::watchdog::CycleWatchdog;

/// Largest accepted MWDT0 stage-0 interval in milliseconds.
const MAX_WATCHDOG_MS: u32 = 60_000;
/// Reason code reported when the hardware cannot determine the boot reset.
const REASON_UNKNOWN: u8 = 0xff;

/// Main watchdog of TIMG0 (MWDT0), stage 0 resetting the system.
pub struct Watchdog {
    inner: Wdt<TIMG0<'static>>,
}

impl Watchdog {
    /// Configure the stage-0 interval and action. The watchdog stays disabled until `start`. The
    /// interval starts at the 4 s minimum of [`crate::watchdog_rule::watchdog_timeout_ms`], which
    /// a project without a cycle watchdog keeps.
    pub(super) fn new(timg0: TIMG0<'static>) -> Self {
        let mut inner = TimerGroup::new(timg0).wdt;
        configure(&mut inner, watchdog_timeout_ms(0));
        Self { inner }
    }

    /// Enable MWDT0 with the interval [`Self::new`] or the program's cycle watchdog set
    /// ([`CycleWatchdog`]). Only a reset stops it afterwards.
    pub fn start(&mut self) {
        self.inner.enable();
    }

    /// Refresh the watchdog once the firmware supervisor has accepted the completed work.
    pub fn feed(&mut self) {
        self.inner.feed();
    }
}

/// Set MWDT0's stage-0 interval and action. The caller has checked `milliseconds`.
fn configure(inner: &mut Wdt<TIMG0<'static>>, milliseconds: u32) {
    inner.set_timeout(
        MwdtStage::Stage0,
        Duration::from_millis(u64::from(milliseconds)),
    );
    inner.set_stage_action(MwdtStage::Stage0, MwdtStageAction::ResetSystem);
}

impl HardwareWatchdog for Watchdog {
    fn feed_interval(&self) -> TimeSpan {
        TimeSpan::new(core::time::Duration::from_millis(u64::from(
            FEED_INTERVAL_MS,
        )))
    }

    fn feed(&mut self) {
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

    /// Write the record a panic or watchdog path leaves for the next boot: the cause, the panic
    /// location when there is one, and the boot counter. It is kept in RTC fast memory next to the
    /// cookie and survives the same MWDT0 and USB-Serial/JTAG resets.
    pub fn write_reset_record(&mut self, cause: ResetCause, text: &str) {
        reset::write_record(self, cause, text);
    }

    /// Read the record the previous boot left, with its cause and text, and clear it, so a cause is
    /// reported once. A damaged record reads as `None`, never as a cause.
    pub fn take_reset_record(&mut self) -> Option<ResetRecord> {
        reset::take_record(self)
    }

    /// Count this boot in RTC memory and return the new boot counter the next record is stamped
    /// with.
    pub fn note_boot(&mut self) -> u32 {
        reset::note_boot(self)
    }
}

/// The reset record words in RTC fast memory, which survive MWDT0 and USB-Serial/JTAG resets. Kept
/// apart from [`COOKIE`], whose two words stay the D.2 phase marker.
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut RECORD: [u32; RECORD_WORDS] = [0; RECORD_WORDS];
/// The boot counter in RTC fast memory, next to the record.
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut BOOT_COUNT: u32 = 0;

impl ResetStore for ResetControl {
    fn record_words(&self) -> [u32; RECORD_WORDS] {
        let words = (&raw const RECORD).cast::<u32>();
        let mut out = [0u32; RECORD_WORDS];
        for (index, word) in out.iter_mut().enumerate() {
            // SAFETY: `words` addresses the RECORD_WORDS-word RECORD static, so every load stays
            // inside it, and the volatile load takes the value from RTC memory.
            *word = unsafe { read_volatile(words.add(index)) };
        }
        out
    }

    fn set_record_words(&mut self, words: [u32; RECORD_WORDS]) {
        let base = (&raw mut RECORD).cast::<u32>();
        for (index, &word) in words.iter().enumerate() {
            // SAFETY: `base` addresses the RECORD_WORDS-word RECORD static, so every store stays
            // inside it. Only `ResetControl` writes the record, and volatile stores keep the words
            // in RTC memory in program order.
            unsafe { write_volatile(base.add(index), word) };
        }
    }

    fn boot_count(&self) -> u32 {
        // SAFETY: `BOOT_COUNT` is a `u32` static and the volatile load reads it from RTC memory.
        unsafe { read_volatile(&raw const BOOT_COUNT) }
    }

    fn set_boot_count(&mut self, count: u32) {
        // SAFETY: `BOOT_COUNT` is a `u32` static and the volatile store keeps it in RTC memory.
        unsafe { write_volatile(&raw mut BOOT_COUNT, count) };
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

/// MWDT0's stage-0 interval follows the program's cycle watchdog (design §2.5 and specification 34,
/// TIME-04): the loader sets it from the record it loads, and `config.set` from a new timeout.
impl CycleWatchdog for Watchdog {
    fn max_interval_ms(&self) -> u32 {
        MAX_WATCHDOG_MS
    }

    fn set_interval_ms(&mut self, milliseconds: u32) -> bool {
        configure(&mut self.inner, milliseconds);
        true
    }
}

/// The mark of the boot's program load in the reset cookie, which a panic or watchdog reset keeps and
/// a power-on clears (specification 34 TARGET-08).
impl ResetControl {
    /// Whether a reset ended the last boot while it loaded the installed program. The mark is
    /// cleared, so a boot reads it once.
    pub fn take_program_load(&mut self) -> bool {
        let marked = self.cookie()
            == Some(ResetCookie {
                phase: crate::cookie::PROGRAM_LOAD_PHASE,
            });
        if marked {
            let _ = self.clear_cookie();
        }
        marked
    }
}

/// Marks that the boot begins loading the installed program (specification 34 TARGET-08).
pub fn mark_program_load() {
    let _ = ResetControl::new().set_cookie(ResetCookie {
        phase: crate::cookie::PROGRAM_LOAD_PHASE,
    });
}

/// Ends the mark of the boot's program load, which finished.
pub fn end_program_load() {
    let _ = ResetControl::new().clear_cookie();
}
