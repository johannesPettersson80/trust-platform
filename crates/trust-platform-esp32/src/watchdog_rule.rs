//! The hardware watchdog's timeout rule (design §2.5 and specification 34, TIME-04): the
//! interval the firmware sets from the project's cycle watchdog timeout, and the interval
//! between two feeds.
//!
//! The rule is a pure function, so a host test pins it without esp-hal. The adapter's MWDT0
//! supervisor in `hardware/reset.rs` applies it, and the runner feeds the watchdog through
//! `HardwareWatchdog` once per feed interval.

/// The shortest hardware watchdog timeout: 4 s (design §2.5).
pub const MIN_TIMEOUT_MS: u32 = 4_000;
/// The margin above the project's cycle watchdog timeout: 2 s (design §2.5).
pub const CYCLE_MARGIN_MS: u32 = 2_000;
/// The longest wait between two watchdog feeds: 1 s (design §2.5 and specification 34, TIME-04).
pub const FEED_INTERVAL_MS: u32 = 1_000;

/// The hardware watchdog timeout for the project's cycle watchdog timeout `cycle_timeout_ms`:
/// the larger of 4 s (`MIN_TIMEOUT_MS`) and the cycle timeout plus 2 s (`CYCLE_MARGIN_MS`),
/// without overflow (design §2.5 and specification 34, TIME-04).
///
/// A project without a cycle watchdog has the zero timeout of `WatchdogPolicy::default`, so the
/// hardware watchdog stays at its 4 s minimum. The hardware watchdog ends a firmware hang only;
/// a runaway scan ends at the scan's armed deadline and work budget instead (TIME-08).
#[must_use]
pub const fn watchdog_timeout_ms(cycle_timeout_ms: u32) -> u32 {
    let with_margin = cycle_timeout_ms.saturating_add(CYCLE_MARGIN_MS);
    if with_margin > MIN_TIMEOUT_MS {
        with_margin
    } else {
        MIN_TIMEOUT_MS
    }
}
