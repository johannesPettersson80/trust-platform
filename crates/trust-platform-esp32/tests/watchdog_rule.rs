//! The hardware watchdog timeout rule of the ESP32-C6 adapter (design §2.5 and specification 34,
//! TIME-04): the MWDT0 interval is a pure function of the project's cycle watchdog timeout, so a
//! host test pins it without esp-hal.

use trust_platform_esp32::watchdog_rule::{
    watchdog_timeout_ms, CYCLE_MARGIN_MS, FEED_INTERVAL_MS, MIN_TIMEOUT_MS,
};

/// A project without a cycle watchdog (`WatchdogPolicy::default` has a zero timeout) leaves the
/// hardware watchdog at its 4 s minimum, and the runner feeds it once a second.
#[test]
fn watchdog_timeout_is_four_seconds_without_a_cycle_watchdog() {
    // design §2.5: "the larger of 4 s and the record's cycle watchdog timeout plus 2 s". A zero
    // cycle timeout would be 0 + 2,000 = 2,000 ms, below the 4 s minimum.
    assert_eq!(watchdog_timeout_ms(0), 4_000);
    assert_eq!(MIN_TIMEOUT_MS, 4_000);
    // design §2.5: the feed interval is 1 s, the longest the runner ever waits (TIME-04).
    assert_eq!(FEED_INTERVAL_MS, 1_000);
}

/// A cycle watchdog timeout whose margin exceeds 4 s decides the hardware timeout.
#[test]
fn watchdog_timeout_is_the_cycle_timeout_plus_two_seconds_when_larger() {
    // A 5,000 ms cycle timeout needs 5,000 + 2,000 = 7,000 ms before the scan is even late.
    assert_eq!(watchdog_timeout_ms(5_000), 7_000);
    assert_eq!(CYCLE_MARGIN_MS, 2_000);
    // The rule is inclusive and alive just above the minimum: 2,000 + 2,000 = 4,000 ms is still
    // the 4 s minimum, and 2,001 + 2,000 = 4,001 ms is already above it.
    assert_eq!(watchdog_timeout_ms(2_000), 4_000);
    assert_eq!(watchdog_timeout_ms(2_001), 4_001);
}
