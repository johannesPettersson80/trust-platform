//! Specification 34 MEM-12 and checklist C1's acceptance ("values read and written in place, no
//! heap allocation for a value"; C1-22): constructing a program builds the default of a global and of
//! a program variable, each an array of 5,000 DINTs (20,000 bytes in the persistent block), in the
//! variable's own bytes, so no allocation as large as a value of either array, 5,000 values, is made.
//! A computer then reads every variable back once for its tools (C1-19d, decision O3 (a)), which
//! builds one value of each array: those are the only two such allocations there. The wrong
//! behaviour caught: a default built as a value and then stored, which on a 32-bit chip needs
//! 5,000 x 32 = 160,000 bytes at once for an array of 20,000 bytes, more than the ESP32-C6's heap of
//! 139,264 bytes holds.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use trust_runtime_core::value::Value;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

/// The fixture: `Table : ARRAY[0..4999] OF DINT` as a global and `Samples : ARRAY[0..4999] OF DINT`
/// as a variable of the program `Fill`, both declared without an initial value.
const LARGE_ARRAYS: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/construction/large-array/program.stbc");

/// The elements of each array.
const ELEMENTS: usize = 5_000;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static AT_LEAST: Cell<usize> = const { Cell::new(usize::MAX) };
    static LARGE: Cell<usize> = const { Cell::new(0) };
}

/// The system's allocator, counting the allocations of this thread of at least `AT_LEAST` bytes
/// while it is armed.
struct Counting;

// SAFETY: every call goes unchanged to the system allocator; the counting touches only
// const-initialized thread-local cells, which neither allocate nor panic.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ARMED.try_with(|armed| {
            if armed.get() && layout.size() >= AT_LEAST.with(Cell::get) {
                LARGE.with(|large| large.set(large.get() + 1));
            }
        });
        // SAFETY: the caller's layout is passed on unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout come from `alloc` above.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

#[test]
fn a_large_array_is_built_in_its_own_bytes_without_a_value_of_it() {
    let prepared = PreparedModule::from_bytes(LARGE_ARRAYS, PreparationLimits::default())
        .expect("the fixture prepares");
    AT_LEAST.with(|at_least| at_least.set(ELEMENTS * std::mem::size_of::<Value>()));
    LARGE.with(|large| large.set(0));
    ARMED.with(|armed| armed.set(true));
    let state = prepared.instantiate(0).expect("the fixture constructs");
    ARMED.with(|armed| armed.set(false));
    drop(state);
    let expected = if cfg!(feature = "std") { 2 } else { 0 };
    assert_eq!(
        LARGE.with(Cell::get),
        expected,
        "the allocations of a value of 5,000 elements while the program is constructed"
    );
}
