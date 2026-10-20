//! Specification 34, "The allocation count", and specification 11 §6.8.1 on a computer: the
//! program's allocator counts each thread's allocator operations, each allocation, reallocation and
//! free once, and the runtime reports the scans' operations, the engine's apart from the
//! services'. This test binary installs the counting allocator as the trust-runtime program does.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::{Arc, Mutex};

use trust_runtime::allocation_count::{count_operation, operations};
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::value::Duration;
use trust_runtime::Runtime;
use trust_runtime_core::vm::PreparationLimits;

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
    assert_eq!(after - before, 3);
}

#[test]
fn a_scalar_programs_scans_report_no_engine_operation() {
    // The engine makes no allocator operation in a scan (specification 34, ARCH-05), on the
    // executor a computer runs; what the hosted services make in and around the scan is reported
    // apart, so the engine's total stays zero over 20 scans while the services' may grow. The wrong
    // behaviour: no count reported, the services' operations counted as the engine's, or a scan
    // of the program allocating.
    let source = "VAR_GLOBAL count : DINT := DINT#0; END_VAR\n\
        PROGRAM Main\ncount := count + DINT#1;\nEND_PROGRAM\n\
        CONFIGURATION Conf RESOURCE Controller ON PLC TASK Tick (INTERVAL := T#10ms, PRIORITY := 0); \
        PROGRAM Plant WITH Tick : Main; END_RESOURCE END_CONFIGURATION";
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted ST source")
        .encode()
        .expect("encode STBC");
    let mut runtime =
        Runtime::from_bytes(&bytes, PreparationLimits::default()).expect("admit application");
    let metrics = Arc::new(Mutex::new(RuntimeMetrics::default()));
    runtime.set_metrics_sink(metrics.clone());
    for _ in 0..20 {
        runtime.advance_time(Duration::from_millis(10));
        runtime
            .execute_cycle()
            .expect("a scan of the scalar program");
    }
    let counted = metrics
        .lock()
        .expect("metrics lock")
        .snapshot()
        .scan_allocations
        .expect("the scans were counted");
    assert_eq!(counted.engine, 0);
}
