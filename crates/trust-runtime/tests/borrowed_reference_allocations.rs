//! Actual allocations of the standard library's shared function table (not budget estimates).
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use trust_runtime_core::value::Value;

struct CountingAllocator;
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}
fn record_allocation() {
    let _ = ENABLED.try_with(|enabled| {
        if enabled.get() {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
    });
}
// SAFETY: All allocation operations forward unchanged layouts and pointers to System.
// The thread-local counters contain only Cells and cannot allocate or unwind.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: Forward the caller's unchanged allocation layout.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: Forward the original allocation pointer/layout pair.
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        // SAFETY: System owns pointer; preserve the supplied old layout and new size.
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn default_standard_library_construction_clone_and_lookup_allocate_nothing() {
    use trust_runtime_core::stdlib::StandardLibrary;

    ALLOCATIONS.with(|count| count.set(0));
    ENABLED.with(|enabled| enabled.set(true));
    let library = StandardLibrary::new();
    let clone = library.clone();
    let original = library.get("aSsErT_Greater_OR_equal");
    let copied = clone.get("ASSERT_GREATER_OR_EQUAL");
    let absent = library.get("a_very_long_function_name_which_does_not_exist");
    let sum = library.call("aDd", &[Value::Int(1), Value::Int(2)]);
    ENABLED.with(|enabled| enabled.set(false));

    let original = original.unwrap();
    let copied = copied.unwrap();
    assert!(std::ptr::eq(original.params, copied.params));
    assert!(std::ptr::fn_addr_eq(original.func, copied.func));
    assert!(absent.is_none());
    assert_eq!(sum.unwrap(), Value::Int(3));
    assert_eq!(
        ALLOCATIONS.with(Cell::get),
        0,
        "immutable default descriptors must remain shared without registry allocations"
    );
}
