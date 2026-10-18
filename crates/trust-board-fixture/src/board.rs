//! Interfaces a board composition provides to the shared M2E fixture. The fixture
//! owns the measured sequence; a board supplies only these operations.

pub use trust_platform_memory::board::{Measurement, Memory};

/// Monotonic time and cycle observations. Waits are bounded by the board.
pub trait Clock {
    /// Microseconds since the board's monotonic origin.
    fn micros(&self) -> u64;
    /// Wait until `deadline_us` has been reached.
    fn wait_until_us(&self, deadline_us: u64);
    /// Wrapping cycle counter; compare intervals with `wrapping_sub`.
    fn cycle_count(&self) -> u32;
    /// Observed instrumentation interrupts, wrapping.
    fn irq_count(&self) -> u32;
}

/// One physical input observation, without any injected fixture value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputSnapshot {
    /// Actual input level.
    pub pin_high: bool,
    /// Active-low button press as the board defines it.
    pub pressed: bool,
}

/// The one-bit process image: a physical input sample and one output bit.
pub trait DigitalIo {
    /// Sample the physical input once.
    fn sample(&self) -> InputSnapshot;
    /// Publish the output bit.
    fn set_output(&mut self, on: bool);
    /// Force the output to its safe state without consulting runtime state.
    fn safe_off(&mut self);
    /// Read the output back through the board's electrical path.
    fn output_readback(&self) -> bool;
}

/// Feeds the board's supervision watchdog.
pub trait Watchdog {
    /// Refresh the watchdog after an accepted scan.
    fn feed(&mut self);
}

/// The handles one fixture run borrows from its board composition.
pub struct Board<'a, C, I, K, W, M> {
    /// Monotonic clock.
    pub clock: &'a C,
    /// Process image.
    pub io: &'a mut I,
    /// Trace console.
    pub console: &'a mut K,
    /// Supervision watchdog.
    pub watchdog: &'a mut W,
    /// Memory measurement.
    pub memory: &'a M,
}
