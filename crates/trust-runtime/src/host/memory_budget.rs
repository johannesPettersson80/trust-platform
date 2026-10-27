//! The memory budget of an isolated worker where the operating system sets none (protocol annex
//! E8.1).
//!
//! On Linux the operating system limits the worker's address space, and the calls here only
//! forward. Elsewhere the program's global allocator makes each allocation through
//! [`allocate`](crate::memory_budget::allocate), each reallocation through
//! [`resize`](crate::memory_budget::resize), and tells [`release`](crate::memory_budget::release)
//! about each free, so the budget knows the bytes the program's allocations hold. Once
//! [`limit`](crate::memory_budget::limit) has set a budget, an allocation that would hold more is
//! refused: the allocator returns null and the process stops, as an allocation beyond the
//! address-space limit stops it on Linux.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// Why a program cannot limit an isolated worker: its allocator does not report to the budget.
pub const NO_LIMIT: &str =
    "isolated runs need a worker memory limit, which this program does not set";

/// The bytes a program's allocations hold, and the most they may hold.
struct Budget {
    held: AtomicUsize,
    limit: AtomicUsize,
    /// Whether the program's allocator reports to the budget: its first report says so.
    reporting: AtomicBool,
}

impl Budget {
    const fn new() -> Self {
        Self {
            held: AtomicUsize::new(0),
            limit: AtomicUsize::new(usize::MAX),
            reporting: AtomicBool::new(false),
        }
    }

    fn admit(&self, bytes: usize) -> bool {
        if !self.reporting.load(Ordering::Relaxed) {
            self.reporting.store(true, Ordering::Relaxed);
        }
        if bytes == 0 {
            return true;
        }
        let held = self
            .held
            .fetch_add(bytes, Ordering::Relaxed)
            .saturating_add(bytes);
        if held > self.limit.load(Ordering::Relaxed) {
            self.held.fetch_sub(bytes, Ordering::Relaxed);
            return false;
        }
        true
    }

    fn release(&self, bytes: usize) {
        self.held.fetch_sub(bytes, Ordering::Relaxed);
    }

    fn allocate(&self, bytes: usize, allocate: impl FnOnce() -> *mut u8) -> *mut u8 {
        if !self.admit(bytes) {
            return std::ptr::null_mut();
        }
        let pointer = allocate();
        if pointer.is_null() {
            self.release(bytes);
        }
        pointer
    }

    fn resize(&self, old: usize, new: usize, resize: impl FnOnce() -> *mut u8) -> *mut u8 {
        let pointer = self.allocate(new.saturating_sub(old), resize);
        if !pointer.is_null() {
            self.release(old.saturating_sub(new));
        }
        pointer
    }

    fn limit(&self, bytes: usize) -> Result<usize, String> {
        if !self.reporting.load(Ordering::Relaxed) {
            return Err(String::from(NO_LIMIT));
        }
        self.limit.store(bytes, Ordering::Relaxed);
        Ok(self.limit.load(Ordering::Relaxed))
    }
}

/// The budget of this program.
static PROGRAM: Budget = Budget::new();

/// Makes an allocation of `bytes` with `allocate` when the budget admits them, and returns its
/// pointer. Null when the budget refuses them, without calling `allocate`, or when the allocation
/// fails; the bytes are then not held. It touches only atomics, so it never allocates, blocks or
/// unwinds.
#[inline]
pub fn allocate(bytes: usize, allocate: impl FnOnce() -> *mut u8) -> *mut u8 {
    if cfg!(target_os = "linux") {
        allocate()
    } else {
        PROGRAM.allocate(bytes, allocate)
    }
}

/// Resizes an allocation of `old` bytes to `new` bytes with `resize`, as [`allocate`] makes one:
/// a growth must fit the budget, and a shrink gives the bytes it frees back.
#[inline]
pub fn resize(old: usize, new: usize, resize: impl FnOnce() -> *mut u8) -> *mut u8 {
    if cfg!(target_os = "linux") {
        resize()
    } else {
        PROGRAM.resize(old, new, resize)
    }
}

/// Gives back the `bytes` of a freed allocation.
#[inline]
pub fn release(bytes: usize) {
    if !cfg!(target_os = "linux") {
        PROGRAM.release(bytes);
    }
}

/// Sets the budget to `bytes` and reads it back.
///
/// # Errors
///
/// [`NO_LIMIT`] when the program's allocator does not report to the budget.
pub fn limit(bytes: usize) -> Result<usize, String> {
    PROGRAM.limit(bytes)
}

/// Whether the program's allocator reports to the budget, so that [`limit`] holds.
#[must_use]
pub fn reporting() -> bool {
    PROGRAM.reporting.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::{Budget, NO_LIMIT};

    /// A pointer that stands for a successful allocation; nothing reads it.
    fn made() -> *mut u8 {
        std::ptr::NonNull::dangling().as_ptr()
    }

    #[test]
    fn an_allocation_beyond_the_budget_is_refused_without_allocating() {
        // A budget of 1,000 bytes holding 100: 900 more reach it exactly and are admitted, one
        // more byte would hold 1,001 and is refused before the allocator is asked. Freeing 400
        // leaves 600, so 400 fit again and the next byte is refused once more. Catches a refusal
        // at the limit itself, an admission beyond it, an allocation made after a refusal, and a
        // refused allocation that is still counted.
        let budget = Budget::new();
        assert!(!budget.allocate(100, made).is_null());
        assert_eq!(budget.limit(1_000), Ok(1_000));
        assert!(!budget.allocate(900, made).is_null());
        assert!(budget
            .allocate(1, || panic!("a refused allocation is never made"))
            .is_null());
        budget.release(400);
        assert!(!budget.allocate(400, made).is_null());
        assert!(budget.allocate(1, made).is_null());
    }

    #[test]
    fn a_failed_allocation_holds_nothing() {
        // With a budget of 1,000 bytes, an allocation of 600 that the system refuses gives its
        // bytes back, so 1,000 still fit afterwards. Catches a failed allocation that keeps
        // holding its share of the budget.
        let budget = Budget::new();
        assert!(!budget.allocate(1, made).is_null());
        budget.release(1);
        assert_eq!(budget.limit(1_000), Ok(1_000));
        assert!(budget.allocate(600, std::ptr::null_mut).is_null());
        assert!(!budget.allocate(1_000, made).is_null());
    }

    #[test]
    fn a_growth_must_fit_and_a_shrink_gives_bytes_back() {
        // A budget of 1,000 bytes: 500 grow to 1,000 (500 more, admitted), then to 1,001 (one
        // more, refused before the allocator is asked). Shrinking 1,000 to 200 frees 800, so 800
        // fit again and one byte more does not. Catches a growth counted in full instead of by
        // its difference, a refused growth that is made, and a shrink that frees nothing.
        let budget = Budget::new();
        assert!(!budget.allocate(500, made).is_null());
        assert_eq!(budget.limit(1_000), Ok(1_000));
        assert!(!budget.resize(500, 1_000, made).is_null());
        assert!(budget
            .resize(1_000, 1_001, || panic!("a refused growth is never made"))
            .is_null());
        assert!(!budget.resize(1_000, 200, made).is_null());
        assert!(!budget.allocate(800, made).is_null());
        assert!(budget.allocate(1, made).is_null());
    }

    #[test]
    fn a_budget_needs_an_allocator_that_reports() {
        // A program whose allocator never reported cannot hold its worker to a budget, so the
        // limit is refused instead of claimed. Catches a worker that reports a limit nothing
        // enforces.
        assert_eq!(Budget::new().limit(1_000), Err(String::from(NO_LIMIT)));
    }
}
