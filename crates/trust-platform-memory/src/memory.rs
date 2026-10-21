//! Bounded bring-up allocator and startup-painted stack measurements. The board
//! composition owns the static arenas and the addresses; this module owns the logic.
#[cfg(target_os = "none")]
use crate::board::{Measurement, Memory};
use crate::count::Count64;
use core::{
    alloc::{GlobalAlloc, Layout},
    sync::atomic::{AtomicUsize, Ordering},
};
use embedded_alloc::LlffHeap;

/// The most regions a [`MeasuredHeap`] serves.
const REGIONS: usize = 3;

/// Allocator wrapper that serves one to three regions, tried in the order a composition hands them
/// in, and counts live bytes, phase peak, allocation calls, failures and every allocator operation
/// over all of them. A composition hands in the regions at [`MeasuredHeap::init`]; the wrapper knows
/// no address of its own.
pub struct MeasuredHeap {
    /// One first-fit allocator per region, in the order an allocation tries them.
    inner: [LlffHeap; REGIONS],
    /// Start and size of each region as handed in, for deallocation routing and capacity.
    bounds: [AtomicUsize; REGIONS * 2],
    /// Sum of the region sizes, in bytes.
    capacity: AtomicUsize,
    /// Allocator bytes in use over every region.
    used: AtomicUsize,
    /// Allocator bytes in use at the peak, over every region.
    peak: AtomicUsize,
    /// Allocation calls over every region.
    allocations: AtomicUsize,
    /// Failed allocation calls over every region.
    failures: AtomicUsize,
    /// The bytes the last failed allocation call asked for, 0 before the first failure: where a
    /// first-fit heap with enough free bytes in all has no single block left for a request, this
    /// names the request, which the bench reads beside the failure count.
    last_failure: AtomicUsize,
    /// Every allocator operation over every region, each allocation, reallocation and free once,
    /// never reset: the count a runtime reads around its scans (specification 34, "The allocation
    /// count"). In 64 bits, so it does not wrap on a 32-bit chip, read without a critical section.
    operations: Count64,
}

impl MeasuredHeap {
    /// An allocator without a region; [`MeasuredHeap::init`] must run before any allocation.
    pub const fn empty() -> Self {
        Self {
            inner: [const { LlffHeap::empty() }; REGIONS],
            bounds: [const { AtomicUsize::new(0) }; REGIONS * 2],
            capacity: AtomicUsize::new(0),
            used: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            allocations: AtomicUsize::new(0),
            failures: AtomicUsize::new(0),
            last_failure: AtomicUsize::new(0),
            operations: Count64::new(),
        }
    }

    /// Give the allocator its regions: the main heap and, when present, a second and a third
    /// region. An allocation is tried in them in this order and taken from the first that has a
    /// block for it, so a later region serves only what the earlier ones cannot take.
    ///
    /// # Safety
    /// Call exactly once, before any allocation. Each region must be non-empty, valid for reads and
    /// writes, aligned for the allocator, and used by nothing else for the program's lifetime.
    pub unsafe fn init(
        &self,
        main: &'static mut [u8],
        second: Option<&'static mut [u8]>,
        third: Option<&'static mut [u8]>,
    ) {
        for (index, region) in [Some(main), second, third].into_iter().enumerate() {
            if let Some(region) = region {
                let start = region.as_mut_ptr() as usize;
                let size = region.len();
                // SAFETY: the caller's contract gives the inner allocator exclusive ownership of
                // this region for the whole program.
                unsafe { self.inner[index].init(start, size) };
                self.bounds[index * 2].store(start, Ordering::Relaxed);
                self.bounds[index * 2 + 1].store(size, Ordering::Relaxed);
                self.capacity.fetch_add(size, Ordering::Relaxed);
            }
        }
    }

    /// Total bytes of the regions handed in at [`MeasuredHeap::init`].
    pub fn capacity(&self) -> usize {
        self.capacity.load(Ordering::Relaxed)
    }

    /// Allocator bytes in use over every region.
    pub fn used(&self) -> usize {
        self.used.load(Ordering::Relaxed)
    }

    /// Bytes of the regions not in use now, over all of them.
    pub fn remaining(&self) -> usize {
        self.capacity().saturating_sub(self.used())
    }

    /// Every allocator operation since the allocator began, each allocation, reallocation and
    /// free once. Reading it allocates nothing and takes no critical section.
    pub fn operations(&self) -> u64 {
        self.operations.read()
    }

    /// Count one allocator operation, in a critical section, so that no other count interleaves
    /// with its two words.
    fn count_operation(&self) {
        critical_section::with(|_| {
            self.operations.increment();
        });
    }

    /// Allocate from the first region that takes `layout`, recording the allocation call, a
    /// failure, the live bytes and the phase peak.
    ///
    /// # Safety
    /// `layout` has a non-zero size, as [`GlobalAlloc::alloc`] requires.
    unsafe fn allocate(&self, layout: Layout) -> *mut u8 {
        let mut pointer = core::ptr::null_mut();
        for heap in &self.inner {
            // SAFETY: the caller supplies a valid allocation layout of non-zero size.
            pointer = unsafe { heap.alloc(layout) };
            if !pointer.is_null() {
                break;
            }
        }
        self.allocations.fetch_add(1, Ordering::Relaxed);
        if pointer.is_null() {
            self.failures.fetch_add(1, Ordering::Relaxed);
            self.last_failure.store(layout.size(), Ordering::Relaxed);
        } else {
            let used = self.used_of_regions();
            self.used.store(used, Ordering::Relaxed);
            self.peak.fetch_max(used, Ordering::Relaxed);
        }
        pointer
    }

    /// Free `pointer` into the region that holds it and record the live bytes.
    ///
    /// # Safety
    /// `pointer` and `layout` are a pair this allocator returned, as [`GlobalAlloc::dealloc`]
    /// requires.
    unsafe fn free(&self, pointer: *mut u8, layout: Layout) {
        let index = self.region_of(pointer as usize);
        // SAFETY: the caller returns a pointer/layout pair the same allocator produced, so `index`
        // is the region that owns it.
        unsafe { self.inner[index].dealloc(pointer, layout) };
        self.used.store(self.used_of_regions(), Ordering::Relaxed);
    }

    /// The bytes the last failed allocation call asked for, 0 before the first failure.
    #[must_use]
    pub fn last_failure(&self) -> usize {
        self.last_failure.load(Ordering::Relaxed)
    }

    /// Lock-free failure evidence even when the panic came from inside the allocator.
    pub fn emergency_metrics(&self) -> (usize, usize, usize, usize) {
        (
            self.used.load(Ordering::Relaxed),
            self.peak.load(Ordering::Relaxed),
            self.allocations.load(Ordering::Relaxed),
            self.failures.load(Ordering::Relaxed),
        )
    }

    /// Allocator bytes in use, summed fresh over every region.
    fn used_of_regions(&self) -> usize {
        self.inner.iter().map(LlffHeap::used).sum()
    }

    /// The region holding `pointer`, or region 0 when no bound matches.
    fn region_of(&self, pointer: usize) -> usize {
        for index in 0..REGIONS {
            let start = self.bounds[index * 2].load(Ordering::Relaxed);
            let size = self.bounds[index * 2 + 1].load(Ordering::Relaxed);
            if size != 0 && pointer >= start && pointer - start < size {
                return index;
            }
        }
        0
    }
}

// SAFETY: each inner allocator owns its region, serializes allocation through the target's
// single-core critical-section implementation, and receives unchanged pointer/layout pairs.
// Instrumentation never allocates or panics.
unsafe impl GlobalAlloc for MeasuredHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.count_operation();
        // SAFETY: GlobalAlloc's caller supplies a valid allocation layout of non-zero size.
        unsafe { self.allocate(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        self.count_operation();
        // SAFETY: GlobalAlloc's caller returns a pair this allocator produced.
        unsafe { self.free(pointer, layout) };
    }

    /// A reallocation is one operation: the allocation, the copy and the free of
    /// `GlobalAlloc::realloc`'s own steps, counted once.
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        self.count_operation();
        // SAFETY: GlobalAlloc's caller gives a non-zero `new_size` that, rounded up to the
        // alignment of `layout`, does not overflow `isize`, so the new layout is valid.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        // SAFETY: the new layout is valid and of non-zero size.
        let new_pointer = unsafe { self.allocate(new_layout) };
        if !new_pointer.is_null() {
            // SAFETY: both blocks are valid for the smaller of their sizes and are distinct
            // allocations, and `pointer`/`layout` is a pair this allocator produced.
            unsafe {
                core::ptr::copy_nonoverlapping(pointer, new_pointer, layout.size().min(new_size));
                self.free(pointer, layout);
            }
        }
        new_pointer
    }
}

/// Main-stack window in RAM, measured from the bottom up. Each composition sets its own span: the
/// ESP32-C6 uses 24 KiB and the STM32F407 16 KiB (specification 34, D.2 and D.4).
#[derive(Debug, Clone, Copy)]
pub struct StackWindow {
    bottom: usize,
    top: usize,
}

impl StackWindow {
    /// Build the window from `bottom` up to `top`. The span is a non-empty whole number of 32-bit
    /// words, which the paint and the measurement step through; any other span fails at compile
    /// time when this is evaluated in a constant.
    pub const fn new(bottom: usize, top: usize) -> Self {
        assert!(
            top > bottom && (top - bottom).is_multiple_of(4),
            "stack window must be a non-empty whole number of words"
        );
        Self { bottom, top }
    }

    /// Lowest address of the window.
    pub const fn bottom(self) -> usize {
        self.bottom
    }

    /// One past the highest address of the window.
    pub const fn top(self) -> usize {
        self.top
    }
}

/// The target's measurement of its real heap and MSP window. It touches raw memory,
/// so it exists only on bare-metal targets and never runs on a host.
#[cfg(target_os = "none")]
pub struct TargetMemory {
    heap: &'static MeasuredHeap,
    window: StackWindow,
    stack_peak: AtomicUsize,
}

#[cfg(target_os = "none")]
impl TargetMemory {
    /// Measure `heap` and the main-stack `window`.
    pub fn new(heap: &'static MeasuredHeap, window: StackWindow) -> Self {
        Self {
            heap,
            window,
            stack_peak: AtomicUsize::new(0),
        }
    }

    /// Paint the window below the live stack without measuring first. A composition whose
    /// runtime does not paint the stack at reset (esp-riscv-rt, unlike cortex-m-rt's
    /// `paint-stack`) calls this once at boot; a measurement before any paint would count
    /// unpainted RAM as used. The stack pointer is sampled in this frame, before the paint's
    /// critical section. An exception frame pushed before interrupts are masked, and the
    /// critical-section frames themselves, lie inside the 256-byte guard below that pointer,
    /// so the paint never reaches a live frame.
    pub fn paint_inactive_stack(&self) {
        let end = stack_pointer().saturating_sub(256).min(self.window.top()) & !3;
        critical_section::with(|_| {
            for address in (self.window.bottom()..end).step_by(4) {
                // SAFETY: linker-reserved stack SRAM below the current active stack and
                // guard; the critical section masks interrupts, and no called routine
                // can claim these addresses.
                unsafe { core::ptr::write_volatile(address as *mut u32, 0xcccc_cccc) };
            }
        });
    }
}

#[cfg(target_os = "none")]
impl Memory for TargetMemory {
    fn begin_phase(&self) {
        self.heap
            .peak
            .store(self.heap.used_of_regions(), Ordering::Relaxed);
        self.heap.allocations.store(0, Ordering::Relaxed);
        self.heap.failures.store(0, Ordering::Relaxed);
    }

    fn measure(&self) -> Measurement {
        // The composition paints the window before the runner starts (cortex-m-rt's
        // paint-stack on the F407, an explicit repaint on C6). Scan only this window.
        // Volatile reads observe interrupt stack use; this remains a measured
        // high-water, not a worst-case proof.
        let mut untouched = 0;
        for address in (self.window.bottom()..self.window.top()).step_by(4) {
            // SAFETY: aligned readable SRAM reserved exclusively for the MSP.
            if unsafe { core::ptr::read_volatile(address as *const u32) } != 0xcccc_cccc {
                break;
            }
            untouched += 4;
        }
        let stack_used = self.window.top() - self.window.bottom() - untouched;
        self.stack_peak.fetch_max(stack_used, Ordering::Relaxed);
        Measurement {
            used: self.heap.used_of_regions(),
            peak: self.heap.peak.load(Ordering::Relaxed),
            allocations: self.heap.allocations.load(Ordering::Relaxed),
            failures: self.heap.failures.load(Ordering::Relaxed),
            stack_used,
            stack_remaining: untouched,
        }
    }

    /// Preserve the boot-wide high-water, then repaint only demonstrably inactive stack.
    fn repaint_inactive_stack(&self) {
        let _ = self.measure();
        self.paint_inactive_stack();
    }

    fn boot_stack_peak(&self) -> usize {
        let _ = self.measure();
        self.stack_peak.load(Ordering::Relaxed)
    }
}

/// Current Main Stack Pointer; reading it touches no memory and changes no flags.
#[cfg(all(target_os = "none", target_arch = "arm"))]
#[inline(always)]
fn stack_pointer() -> usize {
    let sp: usize;
    // SAFETY: `mrs` reads the MSP register only; it accesses no memory and leaves
    // the stack and condition flags unchanged.
    unsafe {
        core::arch::asm!(
            "mrs {}, msp",
            out(reg) sp,
            options(nomem, nostack, preserves_flags)
        )
    };
    sp
}

/// Current stack pointer on RISC-V, which uses one stack for thread and interrupt code.
#[cfg(all(target_os = "none", target_arch = "riscv32"))]
#[inline(always)]
fn stack_pointer() -> usize {
    let sp: usize;
    // SAFETY: `mv` copies the stack-pointer register only; it accesses no memory and
    // leaves the stack and flags unchanged.
    unsafe {
        core::arch::asm!(
            "mv {}, sp",
            out(reg) sp,
            options(nomem, nostack, preserves_flags)
        )
    };
    sp
}
