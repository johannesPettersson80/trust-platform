//! Extend the free-running 32-bit, one-microsecond TIM2 counter.

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
    #[test]
    fn hardware_counter_extension_preserves_rollover_and_repeated_samples() {
        let mut clock = CounterExtension::new();
        assert_eq!(clock.observe(10_000), 10_000);
        assert_eq!(clock.observe(u32::MAX - 2), u64::from(u32::MAX - 2));
        assert_eq!(clock.observe(3), (1u64 << 32) + 3);
        assert_eq!(clock.observe(3), (1u64 << 32) + 3);
        assert_eq!(clock.observe(10_003), (1u64 << 32) + 10_003);
    }
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
