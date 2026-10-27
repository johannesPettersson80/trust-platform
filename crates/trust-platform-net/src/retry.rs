//! When a chip tries again to join its network (specification 34 PORT-06): after 1, 2, 4, 8 and 16
//! seconds, then every 30 seconds, so that a network that comes back is joined soon and one that
//! stays away costs the chip little; after an attempt that its access point did not answer, after
//! 90 seconds.

/// The wait after the first failed attempt, in microseconds.
const FIRST_US: u64 = 1_000_000;
/// The longest wait between two attempts, in microseconds.
const LONGEST_US: u64 = 30_000_000;
/// The wait after an attempt that the access point did not answer, in microseconds. Some access
/// points keep the association of a station that reset without leaving them, ignore its attempts
/// to join while they keep it, and drop it only when their check of the station goes unanswered,
/// which an attempt can put off: the Raspberry Pi's access point dropped a silent station about 45
/// seconds after its last frame (25 October 2026).
const UNANSWERED_US: u64 = 90_000_000;

/// The wait before the next attempt after `failures` failed attempts in a row, the last of them
/// `unanswered` by its access point, in microseconds; no wait before the first attempt.
#[must_use]
pub fn wait_us(failures: u32, unanswered: bool) -> u64 {
    match failures {
        0 => 0,
        _ if unanswered => UNANSWERED_US,
        failures => FIRST_US
            .saturating_mul(1 << (failures - 1).min(5))
            .min(LONGEST_US),
    }
}

#[cfg(test)]
mod tests {
    use super::wait_us;

    /// The waits double from one second and stay at 30 seconds. Catches a chip that hammers an
    /// access point that is away, and one that waits so long that a network back after a minute
    /// stays unjoined for minutes.
    #[test]
    fn the_waits_double_up_to_thirty_seconds() {
        let seconds: [u64; 9] = core::array::from_fn(|failures| {
            wait_us(u32::try_from(failures).expect("small"), false) / 1_000_000
        });
        assert_eq!(seconds, [0, 1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(wait_us(u32::MAX, false), 30_000_000);
    }

    /// After an attempt that its access point did not answer, the chip waits 90 seconds, longer
    /// than the 45 seconds an access point that ignores a station it still holds keeps it without a
    /// frame (specification 34 PORT-06). Catches a chip that, after a reset without leaving, keeps
    /// such an access point's old association alive with its own attempts and never joins again.
    #[test]
    fn an_unanswered_attempt_waits_ninety_seconds() {
        assert_eq!(wait_us(1, true), 90_000_000);
        assert_eq!(wait_us(7, true), 90_000_000);
        assert_eq!(wait_us(0, true), 0, "the first attempt does not wait");
    }
}
