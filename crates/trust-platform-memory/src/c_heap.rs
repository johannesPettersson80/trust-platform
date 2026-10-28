//! The C allocator that a chip's radio driver and its scheduler call (`malloc`, `calloc`,
//! `realloc`, `free` and their `_internal` forms; specification 34 L3), over a [`MeasuredHeap`] of
//! its own: the radio's buffers never share the PLC heap's free lists, never land in a region that
//! no DMA reaches, and never change the count of the scan thread's allocator operations
//! (specification 34, "The allocation count"). A firmware owns the arena and exports the C symbols;
//! the logic is here, so that the host tests prove it.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

use crate::memory::MeasuredHeap;

/// Bytes in front of every block: the block's whole size, so that `free` and `realloc`, which get
/// no size, find it. Eight keep the caller's block 8-byte aligned.
const HEADER: usize = 8;
/// The alignment of every block: the radio's callers assume 4 (esp-rtos, esp-radio); 8 also covers
/// the 8-byte C types.
const ALIGN: usize = 8;

/// A C allocator over one [`MeasuredHeap`].
pub struct CHeap {
    heap: &'static MeasuredHeap,
}

impl CHeap {
    /// The allocator over `heap`, which must have its region before the first call.
    #[must_use]
    pub const fn new(heap: &'static MeasuredHeap) -> Self {
        Self { heap }
    }

    /// `malloc`: a block of `size` bytes, 8-byte aligned, or null when the arena has no room;
    /// `malloc(0)` gives a block of its own, which `free` takes back.
    #[must_use]
    pub fn malloc(&self, size: usize) -> *mut u8 {
        let Some(total) = size.checked_add(HEADER) else {
            return ptr::null_mut();
        };
        let Ok(layout) = Layout::from_size_align(total, ALIGN) else {
            return ptr::null_mut();
        };
        // SAFETY: `layout` has a size of at least HEADER, which is not zero.
        let block = unsafe { self.heap.alloc(layout) };
        if block.is_null() {
            return block;
        }
        // SAFETY: `block` is valid for `total` bytes, at least HEADER, and aligned to ALIGN, which
        // is the alignment of a `usize`; the caller's block begins HEADER bytes after it.
        unsafe {
            block.cast::<usize>().write(total);
            block.add(HEADER)
        }
    }

    /// `calloc`: `count * size` zeroed bytes, or null when the product overflows or the arena has no
    /// room.
    #[must_use]
    pub fn calloc(&self, count: usize, size: usize) -> *mut u8 {
        let Some(bytes) = count.checked_mul(size) else {
            return ptr::null_mut();
        };
        let block = self.malloc(bytes);
        if !block.is_null() {
            // SAFETY: `block` is valid for writes of `bytes` bytes.
            unsafe { ptr::write_bytes(block, 0, bytes) };
        }
        block
    }

    /// `free`: gives back a block of [`Self::malloc`]; null does nothing.
    ///
    /// # Safety
    ///
    /// `block` is null or a live block of this allocator, freed once.
    pub unsafe fn free(&self, block: *mut u8) {
        if block.is_null() {
            return;
        }
        // SAFETY: by the caller's contract the block's header lies HEADER bytes before it and holds
        // the whole size it was allocated with, at ALIGN.
        unsafe {
            let start = block.sub(HEADER);
            let total = start.cast::<usize>().read();
            self.heap
                .dealloc(start, Layout::from_size_align_unchecked(total, ALIGN));
        }
    }

    /// `realloc`: a block of `size` bytes that holds the first `min(old, size)` bytes of `block`,
    /// which is freed. A null `block` allocates. When the arena has no room the result is null and
    /// `block` stays as it was.
    ///
    /// # Safety
    ///
    /// `block` is null or a live block of this allocator.
    pub unsafe fn realloc(&self, block: *mut u8, size: usize) -> *mut u8 {
        if block.is_null() {
            return self.malloc(size);
        }
        // SAFETY: by the caller's contract the header holds the block's whole size.
        let old = unsafe { block.sub(HEADER).cast::<usize>().read() } - HEADER;
        let new = self.malloc(size);
        if !new.is_null() {
            // SAFETY: both blocks are live and distinct, `block` is valid for `old` bytes and `new`
            // for `size`, and the old block is freed once, after the copy.
            unsafe {
                ptr::copy_nonoverlapping(block, new, old.min(size));
                self.free(block);
            }
        }
        new
    }

    /// The arena's free bytes (`get_free_internal_heap_size`).
    #[must_use]
    pub fn free_bytes(&self) -> usize {
        self.heap.remaining()
    }
}
