//! When a chip tries again to join its network (specification 34 ARCH-12): after 1, 2, 4, 8 and 16
//! seconds, then every 30 seconds, so that a network that comes back is joined soon and one that
//! stays away costs the chip little.

/// The wait after the first failed attempt, in microseconds.
const FIRST_US: u64 = 1_000_000;
/// The longest wait between two attempts, in microseconds.
const LONGEST_US: u64 = 30_000_000;

/// The wait before the next attempt after `failures` failed attempts in a row, in microseconds; no
/// wait before the first attempt.
#[must_use]
pub fn wait_us(failures: u32) -> u64 {
    match failures {
        0 => 0,
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
            wait_us(u32::try_from(failures).expect("small")) / 1_000_000
        });
        assert_eq!(seconds, [0, 1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(wait_us(u32::MAX), 30_000_000);
    }
}
