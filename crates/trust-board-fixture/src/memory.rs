//! Bounded bring-up allocator and startup-painted stack measurements. The board
//! composition owns the static arena and the addresses; this module owns the logic.
#[cfg(target_os = "none")]
use crate::board::{Measurement, Memory};
use crate::stack;
use core::{
    alloc::{GlobalAlloc, Layout},
    sync::atomic::{AtomicUsize, Ordering},
};
use embedded_alloc::LlffHeap;

/// Allocator wrapper that counts live bytes, phase peak, allocation calls and failures.
pub struct MeasuredHeap {
    inner: LlffHeap,
    used: AtomicUsize,
    peak: AtomicUsize,
    allocations: AtomicUsize,
    failures: AtomicUsize,
}

impl MeasuredHeap {
    /// An allocator without a region; [`MeasuredHeap::init`] must run before any allocation.
    pub const fn empty() -> Self {
        Self {
            inner: LlffHeap::empty(),
            used: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
            allocations: AtomicUsize::new(0),
            failures: AtomicUsize::new(0),
        }
    }

    /// Give the allocator its only heap region.
    ///
    /// # Safety
    /// Call exactly once, before any allocation. `start..start + size` must be valid for
    /// reads and writes, aligned for the allocator, and used by nothing else for the
    /// program's lifetime.
    pub unsafe fn init(&self, start: usize, size: usize) {
        // SAFETY: the caller's contract gives the inner allocator exclusive ownership of
        // this region for the whole program.
        unsafe { self.inner.init(start, size) };
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
}

// SAFETY: the inner allocator owns the only heap region, serializes allocation
// through the target's single-core critical-section implementation, and receives
// unchanged pointer/layout pairs. Instrumentation never allocates or panics.
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

/// Main-stack window in RAM, measured from the bottom up. Its span is one stack budget.
#[derive(Debug, Clone, Copy)]
pub struct StackWindow {
    bottom: usize,
    top: usize,
}

impl StackWindow {
    /// Build the window; a span other than [`stack::STACK_BYTES`] fails at compile time
    /// when this is evaluated in a constant.
    pub const fn new(bottom: usize, top: usize) -> Self {
        assert!(
            top.wrapping_sub(bottom) == stack::STACK_BYTES,
            "stack window must span STACK_BYTES"
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
            .store(self.heap.inner.used(), Ordering::Relaxed);
        self.heap.allocations.store(0, Ordering::Relaxed);
        self.heap.failures.store(0, Ordering::Relaxed);
    }

    fn measure(&self) -> Measurement {
        // The composition paints the window before the runner starts (cortex-m-rt's
        // paint-stack on F401, an explicit repaint on C6). Scan only this window.
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
            used: self.heap.inner.used(),
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
