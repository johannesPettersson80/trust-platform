//! Actual allocations on the hosted shared reference path (not budget estimates).
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::value::Value;
use trust_runtime_core::vm::hosted::{
    dispatch_refs,
    module::{VmModule, VmRef},
    FrameStack,
};

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
fn hosted_field_load_and_store_do_not_allocate_reference_paths() {
    let session = CompileSession::from_source("TYPE Pair : STRUCT number : INT; END_STRUCT; END_TYPE VAR_GLOBAL item : Pair; END_VAR PROGRAM Main item.number := INT#7; END_PROGRAM");
    let mut runtime = session.build_runtime().unwrap();
    let bytecode = session.build_bytecode_module().unwrap();
    let module = VmModule::from_validated(&bytecode.view().validated().unwrap()).unwrap();
    let index = module
        .refs()
        .iter()
        .position(|reference| matches!(reference, VmRef::Global { path, .. } if !path.is_empty()))
        .unwrap() as u32;
    let mut frames = FrameStack::default();
    // Prepare any host lookup caches before measuring the ordinary repeated path.
    dispatch_refs::store_ref(&mut runtime, &module, &mut frames, index, Value::Int(7)).unwrap();
    let _ = dispatch_refs::load_ref(&runtime, &module, &frames, index).unwrap();
    ALLOCATIONS.with(|count| count.set(0));
    ENABLED.with(|enabled| enabled.set(true));
    let outcome = (|| {
        for _ in 0..128 {
            dispatch_refs::store_ref(&mut runtime, &module, &mut frames, index, Value::Int(7))?;
            assert_eq!(
                dispatch_refs::load_ref(&runtime, &module, &frames, index)?,
                Value::Int(7)
            );
        }
        Ok::<(), trust_runtime_core::vm::VmTrap>(())
    })();
    ENABLED.with(|enabled| enabled.set(false));
    outcome.unwrap();
    assert_eq!(
        ALLOCATIONS.with(Cell::get),
        0,
        "borrowed policy/traversal must not materialize paths"
    );
}
