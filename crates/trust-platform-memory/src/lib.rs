//! Bounded heap and stack measurement shared by the truST board compositions: the
//! measured allocator, the main-stack window, and the measurement interfaces a board
//! supplies. A board composition owns its memory map and wiring. The fixed-size reset
//! record a chip keeps in its persistent memory lives in [`reset`].
#![no_std]

/// Measurement interfaces a board composition provides to the shared runtime. The
/// board owns its memory map; these carry only the heap and stack observations.
pub mod board {
    /// Heap and stack observations for one `MEM` record.
    pub struct Measurement {
        /// Allocator bytes in use now.
        pub used: usize,
        /// Allocator bytes in use at the phase peak.
        pub peak: usize,
        /// Allocation calls in the phase.
        pub allocations: usize,
        /// Failed allocation calls in the phase.
        pub failures: usize,
        /// Measured stack bytes in use.
        pub stack_used: usize,
        /// Measured stack bytes still unused.
        pub stack_remaining: usize,
    }

    /// Board memory measurement and stack repaint.
    pub trait Memory {
        /// Start a phase: reset the phase peak and counters.
        fn begin_phase(&self);
        /// Measure the heap and the stack window now.
        fn measure(&self) -> Measurement;
        /// Preserve the boot-wide stack peak, then repaint only inactive stack.
        fn repaint_inactive_stack(&self);
        /// Boot-wide stack high-water in bytes.
        fn boot_stack_peak(&self) -> usize;
    }
}

pub mod c_heap;
pub mod memory;
pub mod program_region;
pub mod reset;
pub mod stack;
