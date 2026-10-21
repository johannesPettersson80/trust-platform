//! F407 memory map: the 72 KiB measured allocator arena in SRAM and the reserved MSP window at
//! the top of SRAM. The allocator and the measurements live in `trust_board_fixture`. The 64 KiB
//! of core-coupled memory is unassigned.
use core::mem::MaybeUninit;
use trust_board_fixture::memory::{MeasuredHeap, StackWindow};

const HEAP_BYTES: usize = 72 * 1024;
/// Reserved main-stack RAM (`0x2001_C000..0x2002_0000`), measured from the bottom up.
pub const STACK_WINDOW: StackWindow = StackWindow::new(0x2001_C000, 0x2002_0000);

#[repr(C, align(8))]
struct HeapMemory([MaybeUninit<u8>; HEAP_BYTES]);
static mut HEAP_MEMORY: HeapMemory = HeapMemory([MaybeUninit::uninit(); HEAP_BYTES]);

#[global_allocator]
pub static HEAP: MeasuredHeap = MeasuredHeap::empty();

pub fn initialize() {
    // SAFETY: called exactly once before any allocation, single core. The
    // private static is exclusively owned by the allocator for the whole
    // boot; this slice covers exactly that static and nothing else reaches it.
    unsafe {
        let region = core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(HEAP_MEMORY) as *mut u8,
            HEAP_BYTES,
        );
        HEAP.init(region, None, None);
    }
}
