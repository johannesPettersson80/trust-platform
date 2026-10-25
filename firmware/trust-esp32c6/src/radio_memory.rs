//! The radio's memory (checklist L3; specification 34 D.2): the C allocator symbols that the Wi-Fi
//! driver's closed libraries and its scheduler call, over an arena of their own. The arena is one
//! block of the heap, taken at boot only when the device settings name a Wi-Fi network, so that a
//! chip without one keeps that memory for its programs and the memory a program may use is measured
//! with the radio's taken. The block lies in HP SRAM, which the radio's DMA reaches. The driver's
//! buffers so never share the PLC heap's free lists, and the driver's own allocations never change
//! the count of the heap's operations that the runtime reads around each scan (specification 34,
//! "The allocation count"). The allocator's rules are `trust_platform_memory::c_heap`, proven on the
//! host.

use alloc::vec;

use trust_platform_memory::c_heap::CHeap;
use trust_platform_memory::memory::MeasuredHeap;

/// Bytes of the radio's arena: the bench's peak of 47,864 bytes, while a client polled the control
/// connection 95 times a second for a minute beside ping and while a program was installed over the
/// network (25 October 2026), with a fifth more (specification 34 D.2). `capabilities` reports the
/// peak and the failures as `driver_memory`.
const ARENA_BYTES: usize = 56 * 1024;

/// The arena's allocator. It is not the global allocator: its operations are not the scan thread's.
pub static RADIO_HEAP: MeasuredHeap = MeasuredHeap::empty();

/// The C allocator over the arena.
static RADIO: CHeap = CHeap::new(&RADIO_HEAP);

/// A piece of the arena, aligned to 128 bytes: more than the program region's blocks
/// (`BLOCK_ALIGN`, 64), so that the heap places the arena in its data regions and not in the program
/// region, which the CPU executes and which holds the programs' code.
#[derive(Clone, Copy)]
#[repr(C, align(128))]
struct Piece([u8; 128]);

/// Gives the radio its arena, a block of the heap that stays the radio's for the boot. Call once,
/// before the radio starts.
pub fn initialize() {
    let pieces: &'static mut [Piece] = vec![Piece([0; 128]); ARENA_BYTES / 128].leak();
    // SAFETY: the leaked block is valid for reads and writes of its `ARENA_BYTES` bytes for the rest
    // of the boot, nothing else holds it, and `init` runs once, before the radio's first allocation.
    unsafe {
        let arena = core::slice::from_raw_parts_mut(pieces.as_mut_ptr().cast::<u8>(), ARENA_BYTES);
        RADIO_HEAP.init(arena, None, None);
    }
}

/// `malloc` of the driver's libraries.
#[unsafe(no_mangle)]
pub extern "C" fn malloc(size: usize) -> *mut u8 {
    RADIO.malloc(size)
}

/// `malloc_internal` of esp-radio and esp-rtos: internal (HP) SRAM, which the arena is.
#[unsafe(no_mangle)]
pub extern "C" fn malloc_internal(size: usize) -> *mut u8 {
    RADIO.malloc(size)
}

/// `calloc` of the driver's libraries.
#[unsafe(no_mangle)]
pub extern "C" fn calloc(number: u32, size: usize) -> *mut u8 {
    RADIO.calloc(number as usize, size)
}

/// `calloc_internal` of esp-radio.
#[unsafe(no_mangle)]
pub extern "C" fn calloc_internal(number: u32, size: usize) -> *mut u8 {
    RADIO.calloc(number as usize, size)
}

/// `realloc` of the driver's libraries.
///
/// # Safety
///
/// The driver passes null or a block of this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(block: *mut u8, size: usize) -> *mut u8 {
    // SAFETY: the caller's contract above.
    unsafe { RADIO.realloc(block, size) }
}

/// `realloc_internal` of esp-radio.
///
/// # Safety
///
/// The driver passes null or a block of this allocator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc_internal(block: *mut u8, size: usize) -> *mut u8 {
    // SAFETY: the caller's contract above.
    unsafe { RADIO.realloc(block, size) }
}

/// `free` of the driver's libraries.
///
/// # Safety
///
/// The driver passes null or a live block of this allocator, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(block: *mut u8) {
    // SAFETY: the caller's contract above.
    unsafe { RADIO.free(block) }
}

/// `free_internal` of esp-radio and esp-rtos.
///
/// # Safety
///
/// The caller passes null or a live block of this allocator, freed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free_internal(block: *mut u8) {
    // SAFETY: the caller's contract above.
    unsafe { RADIO.free(block) }
}

/// The arena's free bytes, which the driver reads as the free internal heap.
#[unsafe(no_mangle)]
pub extern "C" fn get_free_internal_heap_size() -> usize {
    RADIO.free_bytes()
}
