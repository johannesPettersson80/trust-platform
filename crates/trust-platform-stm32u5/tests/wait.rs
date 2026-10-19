//! The compare arithmetic of the chip's wait (specification 34, PORT-03; design §2.5). The timer and
//! its interrupt are the chip's own and are proved on the bench; the values the wait arms are checked
//! here, with each expected value worked out by hand as a literal.

use core::time::Duration;

use trust_platform_stm32u5::wait::{compare_reached, compare_value};
use trust_runtime_core::ports::MonotonicInstant;

/// The instant `micros` microseconds after the clock's origin.
fn at(micros: u64) -> MonotonicInstant {
    MonotonicInstant::from_origin(Duration::from_micros(micros))
}

/// A deadline beyond the compare window is clamped to 2^31 microseconds from now (PORT-03): with the
/// counter at 0 and a deadline of 2^33 microseconds, the compare is 2^31. The wrong behaviour: the
/// whole remaining time loaded into the 32-bit compare, where 2^33 wraps to 0 and fires at once, or
/// a window of another size, such as 2^32 - 1 or 2^31 - 1.
#[test]
fn a_deadline_beyond_the_compare_window_is_clamped() {
    assert_eq!(compare_value(at(0), at(1 << 33)), 1 << 31);
}

/// The compare is the counter's low word now plus the time left, so with the counter at 1000 and the
/// deadline at 1500 it is 1500. Across the counter's wrap, with the counter at 2^32 - 10 and the
/// deadline 20 microseconds on, it is 10. The wrong behaviour: the time left added to the counter
/// twice (2000 in the first case), or the wrap not taken, which leaves 2^32 + 10 out of the 32 bits.
#[test]
fn the_compare_is_the_counter_plus_the_time_left_across_the_wrap() {
    assert_eq!(compare_value(at(1000), at(1500)), 1500);
    assert_eq!(compare_value(at(0xFFFF_FFF6), at(0x1_0000_000A)), 10);
}

/// A compare the counter has already reached ends the wait without sleeping (PORT-03): the counter
/// equal to the compare, or one count past it, is reached, and a compare one count ahead is not. Across
/// the wrap, the counter at 5 is 11 counts past a compare of 2^32 - 6 (5 counts to 2^32 - 1, 1 to 0,
/// then 5 to 5), so it is reached. The wrong behaviour: a passed compare left to fire only after
/// the counter wraps, which is about 71 minutes late, or a compare one count ahead treated as reached,
/// which skips the sleep.
#[test]
fn a_compare_value_already_reached_ends_the_wait_without_sleeping() {
    assert!(compare_reached(1000, 999), "one count past the compare");
    assert!(
        compare_reached(1000, 1000),
        "the counter equal to the compare"
    );
    assert!(!compare_reached(999, 1000), "one count before the compare");
    assert!(
        compare_reached(5, u32::MAX - 5),
        "11 counts past the compare, across the wrap"
    );
}
