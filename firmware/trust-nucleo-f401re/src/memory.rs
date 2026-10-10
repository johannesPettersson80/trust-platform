//! Bounded bring-up allocator and startup-painted MSP measurements.
use core::{
    alloc::{GlobalAlloc, Layout},
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};
use embedded_alloc::LlffHeap;

pub const HEAP_BYTES: usize = 72 * 1024;
const STACK_BOTTOM: usize = 0x2001_4000;
const STACK_TOP: usize = 0x2001_8000;
static STACK_PEAK: AtomicUsize = AtomicUsize::new(0);

#[repr(C, align(8))]
struct HeapMemory([MaybeUninit<u8>; HEAP_BYTES]);
static mut HEAP_MEMORY: HeapMemory = HeapMemory([MaybeUninit::uninit(); HEAP_BYTES]);

pub struct MeasuredHeap {
    inner: LlffHeap,
    used: AtomicUsize,
    peak: AtomicUsize,
    allocations: AtomicUsize,
    failures: AtomicUsize,
}

#[global_allocator]
pub static HEAP: MeasuredHeap = MeasuredHeap {
    inner: LlffHeap::empty(),
    used: AtomicUsize::new(0),
    peak: AtomicUsize::new(0),
    allocations: AtomicUsize::new(0),
    failures: AtomicUsize::new(0),
};

// SAFETY: the inner allocator owns the only heap region, serializes allocation
// through the Cortex-M critical-section implementation, and receives unchanged
// pointer/layout pairs. Instrumentation never allocates or panics.
unsafe impl GlobalAlloc for MeasuredHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc's caller supplies a valid allocation layout.
        let pointer = unsafe { self.inner.alloc(layout) };
        self.allocations.fetch_add(1, Ordering::Relaxed);
        if pointer.is_null() {
            self.failures.fetch_add(1, Ordering::Relaxed);
        } else {
            let used = self.inner.used();
            self.used.store(used, Ordering::Relaxed);
            self.peak.fetch_max(used, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: delegated unchanged from the GlobalAlloc caller.
        unsafe { self.inner.dealloc(pointer, layout) };
        self.used.store(self.inner.used(), Ordering::Relaxed);
    }
}

pub fn initialize() {
    // SAFETY: called exactly once before any allocation, single core. The
    // private static is exclusively owned by the allocator for the whole boot.
    unsafe {
        HEAP.inner
            .init(core::ptr::addr_of_mut!(HEAP_MEMORY) as usize, HEAP_BYTES)
    };
}

pub fn begin_phase() {
    HEAP.peak.store(HEAP.inner.used(), Ordering::Relaxed);
    HEAP.allocations.store(0, Ordering::Relaxed);
    HEAP.failures.store(0, Ordering::Relaxed);
}

pub struct Measurement {
    pub used: usize,
    pub peak: usize,
    pub allocations: usize,
    pub failures: usize,
    pub stack_used: usize,
    pub stack_remaining: usize,
}

pub fn measure() -> Measurement {
    // cortex-m-rt paint-stack fills unused RAM before calling Rust. Scan only
    // the independently reserved MSP region. Volatile reads observe interrupt
    // stack use; this remains a measured high-water, not a worst-case proof.
    let mut untouched = 0;
    for address in (STACK_BOTTOM..STACK_TOP).step_by(4) {
        // SAFETY: aligned readable SRAM reserved exclusively for the MSP.
        if unsafe { core::ptr::read_volatile(address as *const u32) } != 0xcccc_cccc {
            break;
        }
        untouched += 4;
    }
    let stack_used = STACK_TOP - STACK_BOTTOM - untouched;
    STACK_PEAK.fetch_max(stack_used, Ordering::Relaxed);
    Measurement {
        used: HEAP.inner.used(),
        peak: HEAP.peak.load(Ordering::Relaxed),
        allocations: HEAP.allocations.load(Ordering::Relaxed),
        failures: HEAP.failures.load(Ordering::Relaxed),
        stack_used,
        stack_remaining: untouched,
    }
}

/// Preserve boot-wide high-water before resetting only demonstrably inactive MSP
/// memory. The 256-byte guard includes this helper's active frames. The loop has
/// no calls, allocations or interrupts and never paints at/above the sampled MSP.
pub fn repaint_inactive_stack() {
    let _ = measure();
    let interrupts_were_enabled = cortex_m::register::primask::read().is_active();
    cortex_m::interrupt::disable();
    let end = (cortex_m::register::msp::read() as usize)
        .saturating_sub(256)
        .min(STACK_TOP)
        & !3;
    for address in (STACK_BOTTOM..end).step_by(4) {
        // SAFETY: linker-reserved MSP SRAM below the current active stack and
        // guard; IRQs disabled, no called routine can claim these addresses.
        unsafe { core::ptr::write_volatile(address as *mut u32, 0xcccc_cccc) };
    }
    if interrupts_were_enabled {
        // SAFETY: restores this function's entry IRQ state, after all painting.
        unsafe { cortex_m::interrupt::enable() };
    }
}

pub fn boot_stack_peak() -> usize {
    let _ = measure();
    STACK_PEAK.load(Ordering::Relaxed)
}

/// Lock-free failure evidence even when the panic came from inside the allocator.
pub fn emergency_metrics() -> (usize, usize, usize, usize) {
    (
        HEAP.used.load(Ordering::Relaxed),
        HEAP.peak.load(Ordering::Relaxed),
        HEAP.allocations.load(Ordering::Relaxed),
        HEAP.failures.load(Ordering::Relaxed),
    )
}
