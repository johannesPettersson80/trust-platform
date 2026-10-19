//! The hardware watchdog's timeout rule of every chip (specification 34, TIME-04; design §2.5): the
//! interval a chip's watchdog runs at, from the project's cycle watchdog timeout, and the longest
//! wait between two feeds. A chip keeps only its own peripheral driver; the rule is a pure function,
//! so the host tests pin it without a chip's HAL.

/// The shortest hardware watchdog timeout: 4 s (specification 34, TIME-04).
pub const MIN_TIMEOUT_MS: u32 = 4_000;
/// The margin above the project's cycle watchdog timeout: 2 s (specification 34, TIME-04).
pub const CYCLE_MARGIN_MS: u32 = 2_000;
/// The longest wait between two watchdog feeds: 1 s (specification 34, TIME-04).
pub const FEED_INTERVAL_MS: u32 = 1_000;

/// The hardware watchdog timeout for the project's cycle watchdog timeout `cycle_timeout_ms`: the
/// larger of 4 s ([`MIN_TIMEOUT_MS`]) and the cycle timeout plus 2 s ([`CYCLE_MARGIN_MS`]), without
/// overflow (specification 34, TIME-04).
///
/// A project without a cycle watchdog has the zero timeout of `WatchdogPolicy::default`, so the
/// hardware watchdog stays at its 4 s minimum. The hardware watchdog ends a firmware hang only; a
/// runaway scan ends at the scan's armed deadline and work budget instead (TIME-08).
#[must_use]
pub const fn watchdog_timeout_ms(cycle_timeout_ms: u32) -> u32 {
    let with_margin = cycle_timeout_ms.saturating_add(CYCLE_MARGIN_MS);
    if with_margin > MIN_TIMEOUT_MS {
        with_margin
    } else {
        MIN_TIMEOUT_MS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A project without a cycle watchdog (timeout 0) keeps the 4 s minimum. Catches a rule that
    /// returns the bare 2 s margin (0 + 2,000) and a minimum of any other value.
    #[test]
    fn a_zero_cycle_timeout_keeps_the_four_second_minimum() {
        assert_eq!(watchdog_timeout_ms(0), 4_000);
        assert_eq!(MIN_TIMEOUT_MS, 4_000);
    }

    /// Above the minimum the timeout is the cycle timeout plus 2 s: 5,000 + 2,000 = 7,000 ms. Catches
    /// a margin added to the minimum instead (9,000 ms) and a margin of another length.
    #[test]
    fn a_cycle_timeout_above_the_minimum_gets_the_two_second_margin() {
        assert_eq!(watchdog_timeout_ms(5_000), 7_000);
        assert_eq!(CYCLE_MARGIN_MS, 2_000);
    }

    /// The boundary sits at 2,000 ms: 2,000 + 2,000 = 4,000 ms is the minimum itself, and 2,001 ms
    /// already gives 4,001 ms. Catches an off-by-one boundary, such as a rule that clamps 2,001 to
    /// 4,000 or adds a margin of 2,001.
    #[test]
    fn the_minimum_applies_up_to_a_cycle_timeout_of_two_seconds() {
        assert_eq!(watchdog_timeout_ms(2_000), 4_000);
        assert_eq!(watchdog_timeout_ms(2_001), 4_001);
    }

    /// A cycle timeout near the top of `u32` saturates at `u32::MAX` instead of wrapping. Catches a
    /// plain addition, which wraps around and answers the 4 s minimum instead of the largest timeout.
    #[test]
    fn an_overflowing_cycle_timeout_saturates_instead_of_wrapping() {
        assert_eq!(watchdog_timeout_ms(u32::MAX), u32::MAX);
        assert_eq!(watchdog_timeout_ms(u32::MAX - 1_999), u32::MAX);
    }

    /// The runner feeds the watchdog once a second, the longest wait TIME-04 allows.
    #[test]
    fn the_watchdog_is_fed_at_least_once_a_second() {
        assert_eq!(FEED_INTERVAL_MS, 1_000);
    }
}
