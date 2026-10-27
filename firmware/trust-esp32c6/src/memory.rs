//! ESP32-C6 memory map: the measured allocator's three regions (the main heap, the boot loader's
//! 64 KiB reclaimed RAM and the LP SRAM, tried in that order) and the 78 KiB main-stack window at
//! the top of the esp-hal stack region (design §1.1, R3 and specification 34, §4.3 and D.2).
use core::mem::MaybeUninit;
use trust_platform_esp32::profile::MAIN_STACK;
use trust_platform_memory::memory::{MeasuredHeap, StackWindow, TargetMemory};

/// Bytes of the main heap: the M2E 72 KiB arena less the 24 KiB given to RAM code (P1-9) and the
/// 16,320 bytes the LP SRAM region took over (the owner, 21 October 2026, "Use LP SRAM"), so the
/// three regions held the 112 KiB that the main heap and the reclaimed RAM held; the LP SRAM region
/// has since given 64 bytes to the wall clock's carried time (specification 34 D.2).
const HEAP_BYTES: usize = 32 * 1024 + 64;
/// Bytes the main heap adds to [`HEAP_BYTES`]: 64 KiB of the RAM that the bytecode engine's RAM code
/// took (specification 44 §10.3). A program's image, an install's or an online change's upload, goes
/// to the heap's program region (`PROGRAM_CODE_BYTES`), which took the rest of that RAM; an install
/// while the resource is stopped is written into the program store as it arrives (specification 34
/// §5.1).
const PROGRAM_EXTRA_HEAP_BYTES: usize = 64 * 1024;
/// Bytes of the main heap this firmware has.
const MAIN_HEAP_BYTES: usize = HEAP_BYTES + PROGRAM_EXTRA_HEAP_BYTES;
/// Bytes the boot loader leaves free after boot, reclaimed as the second heap region: esp-hal
/// 1.2.2's `dram2_seg` at 0x4086E610 to 0x4087E610, 64 KiB.
const RECLAIMED_BYTES: usize = 64 * 1024;
/// Bytes of the LP SRAM region: the C6's 16 KiB of LP SRAM less 128 bytes, which hold the reset
/// record, the boot counter and the wall clock's carried time (`.rtc_fast.persistent`, 76 bytes)
/// with their alignment.
const LOW_POWER_BYTES: usize = 16 * 1024 - 128;

/// Top of the esp-hal stack region (`_stack_start`): the end of HP SRAM below the second-stage
/// bootloader in esp-hal 1.2.2's ESP32-C6 memory map. The stack grows down from it. A fixed window
/// keeps linker symbols out of the composition; `xtask portability inspect`
/// fails if the linked image's `_stack_start` differs.
const STACK_TOP: usize = 0x4086_E610;
/// Bytes of the main-stack window: the 78 KiB of the C6's main stack (`MAIN_STACK`; specification 34
/// MEM-03, D.2). The window was 24 KiB until the C1-25 bench found that installing a program fills it
/// (21 October 2026).
const STACK_WINDOW_BYTES: usize = MAIN_STACK.window_bytes;
/// The main-stack window: the top 78 KiB of the esp-hal stack region, measured from the bottom up.
pub const STACK_WINDOW: StackWindow = StackWindow::new(STACK_TOP - STACK_WINDOW_BYTES, STACK_TOP);

/// The main heap arena, aligned for the allocator.
#[repr(C, align(8))]
struct HeapMemory([MaybeUninit<u8>; MAIN_HEAP_BYTES]);
static mut HEAP_MEMORY: HeapMemory = HeapMemory([MaybeUninit::uninit(); MAIN_HEAP_BYTES]);

/// The boot loader's reclaimed RAM, placed by `#[ram(reclaimed)]` in `.dram2_uninit` (design
/// §4.3). It is uninitialized, so the allocator owns it and nothing reads it before `initialize`.
#[esp_hal::ram(reclaimed)]
static mut RECLAIMED: [MaybeUninit<u8>; RECLAIMED_BYTES] = [MaybeUninit::uninit(); RECLAIMED_BYTES];

/// The LP SRAM, the allocator's last region, in 8-byte words for the allocator's alignment.
/// `#[ram(unstable(rtc_fast, persistent))]` places it in `.rtc_fast.persistent`, which the startup
/// zeroes only after power-on; the allocator takes it as it finds it, as it takes the reclaimed RAM.
/// The HP CPU's atomic instructions work on it, and a load or store takes 2.0 cycles there against
/// 1.25 in HP SRAM (both measured on the bench, 21 October 2026), so it is used last.
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut LOW_POWER: [u64; LOW_POWER_BYTES / 8] = [0; LOW_POWER_BYTES / 8];

#[global_allocator]
pub static HEAP: MeasuredHeap = MeasuredHeap::empty();

/// Give the allocator its three regions and paint the stack window. Call once, first in `main`:
/// esp-riscv-rt does not paint the stack at reset, so the paint must precede the first
/// measurement. An allocation is tried in the 32,832-byte main heap, then in the boot loader's
/// 64 KiB, then in the LP SRAM's 16,256 bytes.
pub fn initialize() {
    // SAFETY: called exactly once before any allocation, on a single core. Each private static is
    // exclusively owned by the allocator for the whole boot; these slices cover exactly those
    // statics, and nothing else reaches them.
    unsafe {
        let main = core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(HEAP_MEMORY) as *mut u8,
            MAIN_HEAP_BYTES,
        );
        let reclaimed = core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(RECLAIMED) as *mut u8,
            RECLAIMED_BYTES,
        );
        let low_power = core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(LOW_POWER) as *mut u8,
            LOW_POWER_BYTES,
        );
        HEAP.init(main, Some(reclaimed), Some(low_power));
        // The program region, which the CPU executes, holds the programs' code (specification 44
        // §10.3, §12.3).
        HEAP.init_program_region(trust_platform_esp32::program_region());
    }
    TargetMemory::new(&HEAP, STACK_WINDOW).paint_inactive_stack();
}
