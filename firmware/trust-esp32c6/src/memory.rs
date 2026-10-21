//! ESP32-C6 memory map: the measured allocator's two regions (the 48 KiB main heap and the boot
//! loader's 64 KiB reclaimed RAM) and the 78 KiB main-stack window at the top of the esp-hal
//! stack region (design §1.1, R3 and specification 34, §4.3).
use core::mem::MaybeUninit;
use trust_platform_esp32::profile::MAIN_STACK;
use trust_platform_memory::memory::{MeasuredHeap, StackWindow, TargetMemory};

/// Bytes of the main heap: the M2E 72 KiB arena less the 24 KiB given to RAM code (P1-9).
const HEAP_BYTES: usize = 48 * 1024;
/// Bytes the boot loader leaves free after boot, reclaimed as the second heap region: esp-hal
/// 1.2.2's `dram2_seg` at 0x4086E610 to 0x4087E610, 64 KiB.
const RECLAIMED_BYTES: usize = 64 * 1024;

/// Top of the esp-hal stack region (`_stack_start`): the end of HP SRAM below the second-stage
/// bootloader in esp-hal 1.2.2's ESP32-C6 memory map. The stack grows down from it. A fixed window,
/// as on the STM32F407, keeps linker symbols out of the composition; `xtask portability inspect`
/// fails if the linked image's `_stack_start` differs.
const STACK_TOP: usize = 0x4086_E610;
/// Bytes of the main-stack window: the 78 KiB of the C6's main stack, whose call levels are the C6's
/// call depth (`MAIN_STACK`; specification 34 MEM-03, D.2). The window was 24 KiB until the C1-25
/// bench found that installing any program fills it (21 October 2026). The STM32F407 keeps 16 KiB
/// (D.4).
const STACK_WINDOW_BYTES: usize = MAIN_STACK.window_bytes;
/// The main-stack window: the top 78 KiB of the esp-hal stack region, measured from the bottom up.
pub const STACK_WINDOW: StackWindow = StackWindow::new(STACK_TOP - STACK_WINDOW_BYTES, STACK_TOP);

/// The main heap arena, aligned for the allocator.
#[repr(C, align(8))]
struct HeapMemory([MaybeUninit<u8>; HEAP_BYTES]);
static mut HEAP_MEMORY: HeapMemory = HeapMemory([MaybeUninit::uninit(); HEAP_BYTES]);

/// The boot loader's reclaimed RAM, placed by `#[ram(reclaimed)]` in `.dram2_uninit` (design
/// §4.3). It is uninitialized, so the allocator owns it and nothing reads it before `initialize`.
#[esp_hal::ram(reclaimed)]
static mut RECLAIMED: [MaybeUninit<u8>; RECLAIMED_BYTES] = [MaybeUninit::uninit(); RECLAIMED_BYTES];

#[global_allocator]
pub static HEAP: MeasuredHeap = MeasuredHeap::empty();

/// Give the allocator its two regions and paint the stack window. Call once, first in `main`:
/// esp-riscv-rt does not paint the stack at reset, so the paint must precede the first
/// measurement. The first region is the 48 KiB main heap, the second the boot loader's 64 KiB.
pub fn initialize() {
    // SAFETY: called exactly once before any allocation, on a single core. Each private static is
    // exclusively owned by the allocator for the whole boot; these slices cover exactly those
    // statics, and nothing else reaches them.
    unsafe {
        let main = core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(HEAP_MEMORY) as *mut u8,
            HEAP_BYTES,
        );
        let second = core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(RECLAIMED) as *mut u8,
            RECLAIMED_BYTES,
        );
        HEAP.init(main, Some(second));
    }
    TargetMemory::new(&HEAP, STACK_WINDOW).paint_inactive_stack();
}
