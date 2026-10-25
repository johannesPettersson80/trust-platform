//! The C allocator a chip's radio driver calls (specification 34 L3): the semantics of `malloc`,
//! `calloc`, `realloc` and `free` that the driver's C code and esp-rtos rely on, over an arena of its
//! own. Every expected value follows from the C standard's rules and the 8-byte header.

use trust_platform_memory::c_heap::CHeap;
use trust_platform_memory::memory::MeasuredHeap;

/// An allocator over an arena of `len` bytes that nothing else uses.
fn arena(len: usize) -> (CHeap, &'static MeasuredHeap) {
    let heap: &'static MeasuredHeap = Box::leak(Box::new(MeasuredHeap::empty()));
    let region: &'static mut [u8] = vec![0u8; len].leak();
    // SAFETY: the region outlives the test, nothing else uses it, and `init` runs once before the
    // first allocation.
    unsafe { heap.init(region, None, None) };
    (CHeap::new(heap), heap)
}

/// Blocks are 8-byte aligned and distinct, and `malloc(0)` gives a block that `free` takes back.
/// Catches a block that a C type of 8 bytes cannot use and two callers handed one block.
#[test]
fn malloc_gives_aligned_distinct_blocks() {
    let (heap, _) = arena(1_024);
    let first = heap.malloc(10);
    let second = heap.malloc(10);
    let empty = heap.malloc(0);
    for block in [first, second, empty] {
        assert!(!block.is_null());
        assert_eq!(block as usize % 8, 0, "8-byte aligned");
    }
    assert_ne!(first, second);
    assert!((second as usize).abs_diff(first as usize) >= 10);
    // SAFETY: each block is a live block of this allocator, freed once; null is no block.
    unsafe {
        heap.free(empty);
        heap.free(first);
        heap.free(second);
        heap.free(core::ptr::null_mut());
    }
}

/// `calloc` zeroes every byte, and a product that overflows is null rather than a short block.
/// Catches an overflowing `count * size` that would hand the driver fewer bytes than it asked.
#[test]
fn calloc_zeroes_and_refuses_an_overflowing_product() {
    let (heap, _) = arena(1_024);
    let block = heap.malloc(32);
    // SAFETY: the block is valid for 32 bytes.
    unsafe { core::ptr::write_bytes(block, 0xAA, 32) };
    // SAFETY: the block is live.
    unsafe { heap.free(block) };
    let zeroed = heap.calloc(4, 8);
    assert!(!zeroed.is_null());
    // SAFETY: the block is valid for 32 bytes.
    let bytes = unsafe { core::slice::from_raw_parts(zeroed, 32) };
    assert_eq!(bytes, [0u8; 32]);
    assert!(heap.calloc(usize::MAX, 2).is_null());
}

/// `realloc` keeps the first `min(old, new)` bytes and frees the old block; null allocates; when
/// the arena has no room it returns null and the old block keeps its bytes. Catches a lost block and
/// a copy past the smaller size.
#[test]
fn realloc_keeps_the_smaller_prefix() {
    let (heap, metrics) = arena(512);
    let block = heap.malloc(8);
    // SAFETY: the block is valid for 8 bytes.
    unsafe { core::ptr::copy_nonoverlapping([1u8, 2, 3, 4, 5, 6, 7, 8].as_ptr(), block, 8) };
    // SAFETY: the block is live.
    let grown = unsafe { heap.realloc(block, 16) };
    // SAFETY: the grown block is valid for 16 bytes.
    assert_eq!(
        unsafe { core::slice::from_raw_parts(grown, 8) },
        [1, 2, 3, 4, 5, 6, 7, 8]
    );
    // SAFETY: the block is live.
    let shrunk = unsafe { heap.realloc(grown, 3) };
    // SAFETY: the shrunk block is valid for 3 bytes.
    assert_eq!(unsafe { core::slice::from_raw_parts(shrunk, 3) }, [1, 2, 3]);
    // SAFETY: the block is live.
    let refused = unsafe { heap.realloc(shrunk, 100_000) };
    assert!(refused.is_null());
    // SAFETY: the old block is still live and valid for 3 bytes.
    assert_eq!(unsafe { core::slice::from_raw_parts(shrunk, 3) }, [1, 2, 3]);
    // SAFETY: the block is live.
    unsafe { heap.free(shrunk) };
    // SAFETY: null allocates.
    let fresh = unsafe { heap.realloc(core::ptr::null_mut(), 4) };
    assert!(!fresh.is_null());
    // SAFETY: the block is live.
    unsafe { heap.free(fresh) };
    assert_eq!(metrics.used(), 0, "every block went back");
}

/// The free bytes return to the arena's whole size once every block is freed. Catches a header that
/// frees fewer bytes than it took, which would leak the radio's arena block by block.
#[test]
fn every_freed_block_returns_its_bytes() {
    let (heap, _) = arena(2_048);
    let start = heap.free_bytes();
    let blocks: Vec<*mut u8> = (1..=10).map(|size| heap.malloc(size * 7)).collect();
    assert!(heap.free_bytes() < start);
    for block in blocks {
        // SAFETY: each block is live and freed once.
        unsafe { heap.free(block) };
    }
    assert_eq!(heap.free_bytes(), start);
}
