//! The STM32U5A9's memory map for the shared measurement crate (specification 34, D.5; design §4.3). The
//! chip's SRAM1, SRAM2, SRAM3 and SRAM5 form one block from 0x2000_0000; SRAM4 and the backup SRAM are
//! reported and unassigned in S1. The shared `trust_platform_memory` crate measures the heap and the
//! stack (S1b §1.3); this module gives it this chip's regions and nothing else.

use trust_platform_memory::memory::StackWindow;

/// A span of the chip's address space: `start` up to, not including, `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    /// The first address of the span.
    pub start: u32,
    /// The first address after the span.
    pub end: u32,
}

/// Heap region A: SRAM1, SRAM2 and the first 192 KiB of SRAM3, 1 MiB in all (D.5; design §4.3).
pub const HEAP_A: Region = Region {
    start: 0x2000_0000,
    end: 0x2010_0000,
};

/// The statics: the rest of SRAM3, 640 KiB, for the firmware's data and bss, the link rings and the
/// runner (design §4.3).
pub const STATICS: Region = Region {
    start: 0x2010_0000,
    end: 0x201A_0000,
};

/// Heap region B: SRAM5 below the main stack, 800 KiB, for the later components (design §4.3).
pub const HEAP_B: Region = Region {
    start: 0x201A_0000,
    end: 0x2026_8000,
};

/// The main stack: the top 32 KiB of SRAM5 (D.5; raised from 16 KiB on 18 October 2026, because the C6
/// measured a 15,740-byte high-water in a 16 KiB window).
pub const MAIN_STACK: Region = Region {
    start: 0x2026_8000,
    end: 0x2027_0000,
};

/// SRAM4: 16 KiB in the SRD domain, reported and unassigned in S1 (design §4.3).
pub const SRAM4: Region = Region {
    start: 0x2800_0000,
    end: 0x2800_4000,
};

/// The backup SRAM in the VBAT domain: 2 KiB, reported and unassigned in S1 (design §4.3).
pub const BKPSRAM: Region = Region {
    start: 0x4201_6400,
    end: 0x4201_6C00,
};

/// The main-stack window that the shared crate measures and paints (`trust_platform_memory::memory`).
pub const STACK_WINDOW: StackWindow =
    StackWindow::new(MAIN_STACK.start as usize, MAIN_STACK.end as usize);
