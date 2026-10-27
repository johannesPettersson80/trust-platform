//! Extend the free-running 32-bit, one-microsecond TIM2 counter of the STM32 chips (the STM32F407
//! and the STM32U5A9).

/// Single-owner rollover extension. Samples must be less than 2^32 microseconds
/// apart (about 71 minutes); missed complete wraps cannot be reconstructed.
#[derive(Debug, Clone, Copy, Default)]
pub struct CounterExtension {
    previous: u32,
    elapsed: u64,
}

impl CounterExtension {
    /// Create an extension for a counter just reset to zero.
    pub const fn new() -> Self {
        Self {
            previous: 0,
            elapsed: 0,
        }
    }

    /// Account for the next hardware sample, including one rollover.
    /// The u64 result saturates rather than moving backward at its own limit.
    pub fn observe(&mut self, counter: u32) -> u64 {
        self.elapsed = self
            .elapsed
            .saturating_add(u64::from(counter.wrapping_sub(self.previous)));
        self.previous = counter;
        self.elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Worked by hand: 10 000 is below the wrap; 2^32 - 3 is the last value before it; the sample 3
    // after it is 6 counts on, so the elapsed time is 2^32 + 3; a repeated sample adds nothing.
    // Wrong behaviour caught: the wrap read as a backward step, a repeated sample counted again,
    // or a wrap that is missed.
    #[test]
    fn counter_extension_keeps_time_across_the_32_bit_wrap() {
        let mut clock = CounterExtension::new();
        assert_eq!(clock.observe(10_000), 10_000);
        assert_eq!(clock.observe(u32::MAX - 2), u64::from(u32::MAX - 2));
        assert_eq!(clock.observe(3), (1u64 << 32) + 3);
        assert_eq!(clock.observe(3), (1u64 << 32) + 3);
        assert_eq!(clock.observe(10_003), (1u64 << 32) + 10_003);
    }

    // Wrong behaviour caught: an elapsed time that wraps to a small value at the u64 limit, or that
    // moves backward on the next sample.
    #[test]
    fn elapsed_time_saturates_without_reversing() {
        let mut clock = CounterExtension {
            previous: u32::MAX,
            elapsed: u64::MAX - 2,
        };
        assert_eq!(clock.observe(3), u64::MAX);
        assert_eq!(clock.observe(4), u64::MAX);
    }
}
