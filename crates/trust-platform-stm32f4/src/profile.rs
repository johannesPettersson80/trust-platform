//! Chip clock profile of the STM32F4 adapter: the system clock and the fixture's sampling period.

/// System frequency of the chip ([`crate::chip::CHIP`]): 168 MHz on the F407, from the crystal,
/// or from the HSI when there is none. The oscillator's tolerance remains the physical accuracy
/// of the clock.
pub const SYSCLK_HZ: u32 = crate::chip::CHIP.sysclk_hz;
/// Fixture sampling period; shared runtime scheduling remains engine-owned.
pub const SCAN_PERIOD_US: u64 = 10_000;
