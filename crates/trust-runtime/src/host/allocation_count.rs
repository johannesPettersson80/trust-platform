//! The allocation count of a computer (specification 34, "The allocation count"; checklist
//! C1-19f).
//!
//! The trust-runtime program's global allocator calls
//! [`count_operation`](crate::allocation_count::count_operation) at each allocation, reallocation
//! and free, which counts the operations of each thread, and the hosted execution services read the
//! count of the thread that runs the scans
//! ([`operations`](crate::allocation_count::operations)). A program that embeds the runtime with an
//! allocator that does not count has no count, and the runtime reports `null` for it, never zero.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

thread_local! {
    /// This thread's allocator operations.
    static OPERATIONS: Cell<u64> = const { Cell::new(0) };
}

/// Whether the program's allocator counts: its first counted operation says so.
static COUNTING: AtomicBool = AtomicBool::new(false);

/// Count one allocator operation of this thread. A global allocator calls it for each allocation,
/// reallocation and free. It touches only an atomic and a const-initialized thread-local `Cell`,
/// so it never allocates, blocks or unwinds; while the thread's locals are torn down the operation
/// is not counted.
pub fn count_operation() {
    if !COUNTING.load(Ordering::Relaxed) {
        COUNTING.store(true, Ordering::Relaxed);
    }
    let _ = OPERATIONS.try_with(|count| count.set(count.get().wrapping_add(1)));
}

/// The allocator operations this thread has made so far, when the program's allocator counts
/// them; `None` when it does not. Reading it allocates nothing.
#[must_use]
pub fn operations() -> Option<u64> {
    if !COUNTING.load(Ordering::Relaxed) {
        return None;
    }
    OPERATIONS.try_with(Cell::get).ok()
}
