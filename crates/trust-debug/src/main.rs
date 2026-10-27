use std::alloc::{GlobalAlloc, Layout, System};

use tracing::info;
use trust_debug::{DebugAdapter, DebugSession};
use trust_runtime::memory_budget;

/// The program's allocator: the system's, keeping the memory budget of the isolated worker this
/// program also runs (protocol annex E8.1).
struct BudgetAllocator;

// SAFETY: every operation forwards the caller's unchanged layout, pointer and new size to
// `System` and returns its result, or refuses an allocation beyond the memory budget with null,
// as an allocator may; the budget touches only atomics, so it allocates nothing and cannot unwind.
unsafe impl GlobalAlloc for BudgetAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's valid layout of non-zero size, unchanged.
        memory_budget::allocate(layout.size(), || unsafe { System.alloc(layout) })
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's valid layout of non-zero size, unchanged.
        memory_budget::allocate(layout.size(), || unsafe { System.alloc_zeroed(layout) })
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the caller returns a pointer/layout pair that `System` produced.
        unsafe { System.dealloc(pointer, layout) };
        memory_budget::release(layout.size());
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller's pair from `System` and its valid new size, unchanged.
        memory_budget::resize(layout.size(), new_size, || unsafe {
            System.realloc(pointer, layout, new_size)
        })
    }
}

#[global_allocator]
static ALLOCATOR: BudgetAllocator = BudgetAllocator;

fn main() {
    // ControlBridge searches supervise the current host executable. Dispatch
    // the shared bounded worker before starting DAP or its logging surface.
    if std::env::args().nth(1).as_deref() == Some("control-search-worker") {
        if let Err(error) = trust_runtime::control::search::worker_stdio() {
            eprintln!("control-search-worker: {error}");
            std::process::exit(1);
        }
        return;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("Starting trust-debug adapter");
    let mut adapter = DebugAdapter::new(DebugSession::new());
    if let Err(err) = adapter.run_stdio() {
        eprintln!("trust-debug error: {err}");
        std::process::exit(1);
    }
}
