//! The STM32U5A9's memory for the measured allocator and the main stack (specification 34, D.5; design
//! §4.3). The allocator's region A and the stack window come from the adapter's memory map; the
//! measurements are the shared `trust_platform_memory` crate's, and the firmware never runs the
//! F407 fixture's 16 KiB stack check.
use trust_platform_memory::memory::{MeasuredHeap, TargetMemory};
use trust_platform_stm32u5::memory::{HEAP_A, MAIN_STACK, STACK_WINDOW as CHIP_STACK_WINDOW};

/// The allocator for the whole boot: region A, 1 MiB of SRAM1, SRAM2 and the first 192 KiB of SRAM3.
#[global_allocator]
pub static HEAP: MeasuredHeap = MeasuredHeap::empty();

/// Region A is no section's memory (`memory.x`), so the allocator owns it for the whole boot.
const _: () = assert!(HEAP_A.end - HEAP_A.start == 1024 * 1024);
/// The stack window is the top 78 KiB of SRAM5, the main stack whose call levels the profile's
/// `MAIN_STACK` figures give, and the linker script asserts the same 78 KiB (profile.x).
const _: () = assert!(MAIN_STACK.end - MAIN_STACK.start == 78 * 1024);

/// Gives the allocator region A and paints the inactive part of the stack window, so that the first
/// measurement counts from a painted stack. Call once, first in `main`, before any allocation.
pub fn initialize() {
    let bytes = (HEAP_A.end - HEAP_A.start) as usize;
    // SAFETY: called exactly once, first in `main`, before any allocation. Region A is SRAM1, SRAM2 and
    // the first 192 KiB of SRAM3: `memory.x` places no section there, and nothing else reaches it, so
    // the slice covers exactly memory the allocator owns for the whole boot.
    unsafe {
        let region = core::slice::from_raw_parts_mut(HEAP_A.start as usize as *mut u8, bytes);
        HEAP.init(region, None);
    }
    TargetMemory::new(&HEAP, CHIP_STACK_WINDOW).paint_inactive_stack();
}
