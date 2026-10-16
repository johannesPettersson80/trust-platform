//! F401RE memory map: the 72 KiB measured allocator arena and the reserved MSP
//! window. The allocator and the measurements live in `trust_board_fixture`.
use core::mem::MaybeUninit;
use trust_board_fixture::memory::{MeasuredHeap, StackWindow};

const HEAP_BYTES: usize = 72 * 1024;
/// Reserved main-stack RAM (`0x2001_4000..0x2001_8000`), measured from the bottom up.
pub const STACK_WINDOW: StackWindow = StackWindow::new(0x2001_4000, 0x2001_8000);

#[repr(C, align(8))]
struct HeapMemory([MaybeUninit<u8>; HEAP_BYTES]);
static mut HEAP_MEMORY: HeapMemory = HeapMemory([MaybeUninit::uninit(); HEAP_BYTES]);

#[global_allocator]
pub static HEAP: MeasuredHeap = MeasuredHeap::empty();

pub fn initialize() {
    // SAFETY: called exactly once before any allocation, single core. The
    // private static is exclusively owned by the allocator for the whole boot.
    unsafe { HEAP.init(core::ptr::addr_of_mut!(HEAP_MEMORY) as usize, HEAP_BYTES) };
}
