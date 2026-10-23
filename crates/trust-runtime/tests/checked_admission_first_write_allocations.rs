//! Spec 34 MODEL-02 and MEM-04: the first checked hosted write needs no warm-up, compilation or allocation.
#![cfg(feature = "full")]
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use trust_runtime::{
    error::{ExecutionLimitKind, RuntimeError},
    harness::CompileSession,
    value::Value,
    Runtime,
};
use trust_runtime_core::vm::PreparationLimits;

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

const INT_SOURCE: &str = "VAR_GLOBAL level : INT := INT#0; END_VAR PROGRAM Main END_PROGRAM";
const STRING_SOURCE: &str =
    "VAR_GLOBAL label : STRING[8] := 'ab'; END_VAR PROGRAM Main END_PROGRAM";

/// Spec 34 MODEL-02 and MEM-04: the first checked hosted write of a scalar global, with no earlier write or read, allocates nothing and performs no compilation.
#[test]
fn first_hosted_scalar_write_allocates_nothing_and_compiles_nothing() {
    let bytes = CompileSession::from_source(INT_SOURCE)
        .build_bytecode_bytes()
        .unwrap();
    // The default preparation profile is the admitted capacity for this artifact.
    let mut runtime = Runtime::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let reference = runtime.storage().ref_for_global("level").unwrap();
    let before = runtime.vm_tier1_specialized_executor_snapshot();

    ALLOCATIONS.with(|count| count.set(0));
    ENABLED.with(|enabled| enabled.set(true));
    let result = runtime.write_reference(&reference, Value::Int(7));
    ENABLED.with(|enabled| enabled.set(false));
    let allocations = ALLOCATIONS.with(Cell::get);
    let after = runtime.vm_tier1_specialized_executor_snapshot();

    result.unwrap();
    assert_eq!(
        allocations, 0,
        "the first checked hosted write must not allocate"
    );
    assert_eq!(
        after.compile_attempts, before.compile_attempts,
        "the first write must not compile or lower code"
    );
    assert_eq!(after.compile_successes, before.compile_successes);
    assert_eq!(runtime.storage().get_global("level"), Some(&Value::Int(7)));
}

/// Spec 34 MEM-04 (maximum admitted sizes): the first checked write of a STRING[8] at its maximum length allocates nothing and performs no compilation.
#[test]
fn first_hosted_write_at_the_maximum_admitted_string_length_allocates_nothing() {
    let bytes = CompileSession::from_source(STRING_SOURCE)
        .build_bytecode_bytes()
        .unwrap();
    let mut runtime = Runtime::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let reference = runtime.storage().ref_for_global("label").unwrap();
    let before = runtime.vm_tier1_specialized_executor_snapshot();
    let value = Value::String("abcdefgh".into());

    ALLOCATIONS.with(|count| count.set(0));
    ENABLED.with(|enabled| enabled.set(true));
    let result = runtime.write_reference(&reference, value);
    ENABLED.with(|enabled| enabled.set(false));
    let allocations = ALLOCATIONS.with(Cell::get);
    let after = runtime.vm_tier1_specialized_executor_snapshot();

    result.unwrap();
    assert_eq!(
        allocations, 0,
        "the first maximum-length string write must not allocate"
    );
    assert_eq!(after.compile_attempts, before.compile_attempts);
    assert_eq!(after.compile_successes, before.compile_successes);
    assert_eq!(
        runtime.storage().get_global("label"),
        Some(&Value::String("abcdefgh".into()))
    );
}

/// Spec 34 MEM-04 (fault/limit paths) and §4.3.1: the first checked write refused at the admitted per-entry work limit allocates nothing, reports the exact limit and leaves the destination and scan state unchanged.
#[test]
fn first_checked_write_refused_at_the_admitted_work_limit_allocates_nothing_and_changes_nothing() {
    let bytes = CompileSession::from_source(STRING_SOURCE)
        .build_bytecode_bytes()
        .unwrap();
    // `max_work` bounds one engineering write entry; construction has its own `max_construction_work`.
    let limits = PreparationLimits {
        max_work: 1,
        ..PreparationLimits::default()
    };
    let mut runtime = Runtime::from_bytes(&bytes, limits).unwrap();
    let reference = runtime.storage().ref_for_global("label").unwrap();
    let before_value = runtime.storage().get_global("label").cloned();
    let before = runtime.vm_tier1_specialized_executor_snapshot();
    let cycles = runtime.cycle_counter();
    // Text of eight characters charges two work units per character: 16 units against a limit of 1.
    let value = Value::String("abcdefgh".into());

    ALLOCATIONS.with(|count| count.set(0));
    ENABLED.with(|enabled| enabled.set(true));
    let result = runtime.write_reference(&reference, value);
    ENABLED.with(|enabled| enabled.set(false));
    let allocations = ALLOCATIONS.with(Cell::get);
    let after = runtime.vm_tier1_specialized_executor_snapshot();

    assert_eq!(
        result,
        Err(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 16,
            limit: 1,
        })
    );
    assert_eq!(
        allocations, 0,
        "the refused first write must not allocate on its error path"
    );
    assert_eq!(runtime.storage().get_global("label").cloned(), before_value);
    assert_eq!(after.compile_attempts, before.compile_attempts);
    assert_eq!(after.compile_successes, before.compile_successes);
    assert_eq!(runtime.cycle_counter(), cycles);
    assert!(!runtime.faulted());
}
