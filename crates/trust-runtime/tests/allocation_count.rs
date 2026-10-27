//! Specification 34, "The allocation count", on a computer: the program's allocator counts each
//! thread's allocator operations, each allocation, reallocation and free once. This test binary
//! installs the counting allocator as the trust-runtime program does.

use std::alloc::{GlobalAlloc, Layout, System};

use trust_runtime::allocation_count::{count_operation, operations};

/// The system's allocator, counting as the trust-runtime program's does.
struct CountingAllocator;

// SAFETY: every operation forwards the caller's unchanged layout, pointer and new size to
// `System`; the counting touches only an atomic and a const-initialized thread-local `Cell`.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_operation();
        // SAFETY: the caller's valid layout of non-zero size, unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        count_operation();
        // SAFETY: the caller returns a pointer/layout pair that `System` produced.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count_operation();
        // SAFETY: the caller's pair from `System` and its valid new size, unchanged.
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn each_allocation_reallocation_and_free_of_this_thread_counts_once() {
    // Worked by hand: a vector of one element is one allocation of capacity 1, a push grows it with
    // one reallocation, and dropping it is one free: 3 operations of this thread, whatever other
    // threads allocate meanwhile. The wrong behaviour: a reallocation counted as an allocation and a
    // free, a free left out, or another thread's operations counted.
    let before = operations().expect("the test's allocator counts");
    let mut values = vec![1_u64];
    values.push(2);
    drop(std::hint::black_box(values));
    let after = operations().expect("the test's allocator counts");
    // The count is 32 bits that wrap, so the difference wraps too.
    assert_eq!(after.wrapping_sub(before), 3);
}
