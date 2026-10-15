//! ESP32-C6 memory map: the 72 KiB measured allocator arena and the 16 KiB main-stack window at the
//! top of the esp-hal stack region. The allocator and the measurements live in `trust_board_fixture`.
use core::mem::MaybeUninit;
use trust_board_fixture::{
    memory::{MeasuredHeap, StackWindow, TargetMemory},
    stack::STACK_BYTES,
};

const HEAP_BYTES: usize = 72 * 1024;

unsafe extern "C" {
    /// Top of the esp-hal main stack region, defined by the linker script (`stack.x`). The stack
    /// grows down from this address.
    static _stack_start: u8;
}

#[repr(C, align(8))]
struct HeapMemory([MaybeUninit<u8>; HEAP_BYTES]);
static mut HEAP_MEMORY: HeapMemory = HeapMemory([MaybeUninit::uninit(); HEAP_BYTES]);

#[global_allocator]
pub static HEAP: MeasuredHeap = MeasuredHeap::empty();

/// Give the allocator its region and paint the stack window. Call once, first in `main`:
/// esp-riscv-rt does not paint the stack at reset, so the paint must precede the first
/// measurement.
pub fn initialize() {
    // SAFETY: called exactly once before any allocation, on a single core. The private static is
    // exclusively owned by the allocator for the whole boot.
    unsafe { HEAP.init(core::ptr::addr_of_mut!(HEAP_MEMORY) as usize, HEAP_BYTES) };
    TargetMemory::new(&HEAP, stack_window()).paint_inactive_stack();
}

/// The main-stack window: the top 16 KiB of the esp-hal stack region, measured from the bottom up.
pub fn stack_window() -> StackWindow {
    // Only the address of the linker symbol is taken; its value is never read.
    let top = core::ptr::addr_of!(_stack_start) as usize;
    StackWindow::new(top - STACK_BYTES, top)
}
