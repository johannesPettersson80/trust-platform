//! ESP32-C6 memory map: the 72 KiB measured allocator arena and the 16 KiB main-stack window at the
//! top of the esp-hal stack region. The allocator and the measurements live in `trust_board_fixture`.
use core::mem::MaybeUninit;
use trust_board_fixture::{
    memory::{MeasuredHeap, StackWindow, TargetMemory},
    stack::STACK_BYTES,
};

const HEAP_BYTES: usize = 72 * 1024;

/// Top of the esp-hal stack region (`_stack_start`): the end of HP SRAM below the second-stage
/// bootloader in esp-hal 1.2.2's ESP32-C6 memory map. The stack grows down from it. A fixed window
/// keeps linker symbols out of the composition, as on F401; `xtask portability inspect` fails if
/// the linked image's `_stack_start` differs.
const STACK_TOP: usize = 0x4086_E610;
/// The main-stack window: the top 16 KiB of the esp-hal stack region, measured from the bottom up.
pub const STACK_WINDOW: StackWindow = StackWindow::new(STACK_TOP - STACK_BYTES, STACK_TOP);

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
    TargetMemory::new(&HEAP, STACK_WINDOW).paint_inactive_stack();
}
