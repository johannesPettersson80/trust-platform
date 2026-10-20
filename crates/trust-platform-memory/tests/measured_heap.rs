//! The two-region measured heap: the host tests of R1 (design S1b §1.1 and §4.3).

use core::alloc::{GlobalAlloc, Layout};
use trust_platform_memory::memory::MeasuredHeap;

/// A fresh region of `len` zeroed bytes the allocator owns for the whole test.
fn region(len: usize) -> &'static mut [u8] {
    vec![0u8; len].leak()
}

/// A layout the test has worked out by hand; `align` is a power of two.
fn layout(size: usize, align: usize) -> Layout {
    Layout::from_size_align(size, align).expect("the literal size and alignment are valid")
}

#[test]
fn allocations_spill_into_the_second_region() {
    // R1 and design S1b §4.3: a first region of 64 bytes cannot take a 100-byte request, so the
    // allocator takes it from the 256-byte second region, and its measurements count both regions.
    // The wrong behaviour: refusing a request the second region can take, placing it in the first
    // region, and counting only one region's bytes.
    let main = region(64);
    let second = region(256);
    let main_start = main.as_ptr() as usize;
    let second_start = second.as_ptr() as usize;
    let heap = MeasuredHeap::empty();
    // SAFETY: both slices outlive the test, are used by nothing else, and `init` runs once before
    // the first allocation.
    unsafe { heap.init(main, Some(second)) };
    assert_eq!(heap.capacity(), 64 + 256);

    let small = layout(16, 8);
    let large = layout(100, 8);
    // SAFETY: both layouts are valid.
    let first = unsafe { heap.alloc(small) };
    assert!(!first.is_null());
    let first_address = first as usize;
    assert!(first_address >= main_start && first_address < main_start + 64);
    // SAFETY: both layouts are valid.
    let second_pointer = unsafe { heap.alloc(large) };
    assert!(!second_pointer.is_null());
    let second_address = second_pointer as usize;
    assert!(second_address >= second_start && second_address < second_start + 256);

    // The 100-byte allocation spilled into the second region and both regions' bytes are counted:
    // 16 and 100 requested bytes plus the allocator's rounding, which one region alone cannot reach.
    assert!(heap.used() >= 16 + 100);
    assert!(heap.used() <= heap.capacity());
    assert_eq!(heap.emergency_metrics().2, 2);
    assert_eq!(heap.emergency_metrics().3, 0);

    // SAFETY: both pointers came from this allocator with the same layouts.
    unsafe { heap.dealloc(first, small) };
    // SAFETY: both pointers came from this allocator with the same layouts.
    unsafe { heap.dealloc(second_pointer, large) };
    assert_eq!(heap.used(), 0);
}

#[test]
fn a_full_heap_refuses_and_counts_the_failure() {
    // R1: an allocation neither region can take is refused and counted, and the bytes it asked for
    // are kept as the last failure (0 before the first). The wrong behaviour: a null result that no
    // failure counter records, a failure counter that also counts a successful allocation, and a
    // failure whose size is not kept, or kept from a later successful allocation.
    let heap = MeasuredHeap::empty();
    // SAFETY: the slice outlives the test, is used by nothing else, and `init` runs once before the
    // first allocation.
    unsafe { heap.init(region(64), None) };
    assert_eq!(heap.last_failure(), 0);

    // SAFETY: the layout is valid.
    let pointer = unsafe { heap.alloc(layout(65, 8)) };
    assert!(pointer.is_null());
    assert_eq!(heap.used(), 0);
    assert_eq!(heap.remaining(), 64);
    assert_eq!(heap.emergency_metrics(), (0, 0, 1, 1));
    assert_eq!(heap.last_failure(), 65);

    // SAFETY: the layout is valid; the block is freed below with the same layout.
    let taken = unsafe { heap.alloc(layout(8, 8)) };
    assert!(!taken.is_null());
    assert_eq!(
        heap.last_failure(),
        65,
        "a successful allocation keeps the last failure"
    );
    // SAFETY: `taken` came from this heap with this layout.
    unsafe { heap.dealloc(taken, layout(8, 8)) };
}

#[test]
fn every_allocator_operation_counts_once_and_a_reallocation_keeps_its_bytes() {
    // Specification 34, "The allocation count": each allocation, reallocation and free counts once,
    // the count is never reset, and a reallocation moves the bytes it keeps. Worked by hand: one
    // allocation of 8 bytes, its reallocation to 40 bytes, an allocation that fails (257 bytes, more
    // than the 256-byte heap holds) and the free make 4 operations; the allocation calls are 3, the
    // reallocation's own allocation among them, and 1 of them failed. The wrong behaviour: a
    // reallocation counted as two operations, a free left out, or the bytes not moved.
    let heap = MeasuredHeap::empty();
    // SAFETY: the slice outlives the test, is used by nothing else, and `init` runs once before the
    // first allocation.
    unsafe { heap.init(region(256), None) };
    assert_eq!(heap.operations(), 0);

    let small = layout(8, 8);
    // SAFETY: the layout is valid.
    let pointer = unsafe { heap.alloc(small) };
    assert!(!pointer.is_null());
    // SAFETY: the pointer is valid for the 8 bytes just allocated.
    unsafe { core::ptr::copy_nonoverlapping(b"truST-8!".as_ptr(), pointer, 8) };
    // SAFETY: the pointer/layout pair came from this allocator and 40 is a valid new size.
    let moved = unsafe { heap.realloc(pointer, small, 40) };
    assert!(!moved.is_null());
    // SAFETY: the reallocation keeps the first 8 bytes of the block.
    assert_eq!(
        unsafe { core::slice::from_raw_parts(moved, 8) },
        b"truST-8!"
    );
    assert_eq!(heap.operations(), 2);

    // SAFETY: the layout is valid.
    assert!(unsafe { heap.alloc(layout(257, 8)) }.is_null());
    assert_eq!(heap.operations(), 3);

    // SAFETY: the pointer came from this allocator with this layout.
    unsafe { heap.dealloc(moved, layout(40, 8)) };
    assert_eq!(heap.operations(), 4);
    assert_eq!(heap.used(), 0);
    assert_eq!(heap.emergency_metrics().2, 3);
    assert_eq!(heap.emergency_metrics().3, 1);
}
