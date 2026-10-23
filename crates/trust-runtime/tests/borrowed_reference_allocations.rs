//! Actual allocations on the hosted shared reference path (not budget estimates).
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::value::{RefSegment, Value};

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
    let mut reference = runtime.storage().ref_for_global("item").unwrap();
    reference.path.push(RefSegment::Field("number".into()));
    // Resolve the path before measuring and use the installed owner's checked
    // surface. STBC 2.0 does not expose a legacy raw ReferenceContext.
    runtime.write_reference(&reference, Value::Int(7)).unwrap();
    let _ = runtime.read_reference(&reference).unwrap();
    ALLOCATIONS.with(|count| count.set(0));
    ENABLED.with(|enabled| enabled.set(true));
    let outcome = (|| {
        for _ in 0..128 {
            runtime.write_reference(&reference, Value::Int(7))?;
            assert_eq!(runtime.read_reference(&reference)?, Value::Int(7));
        }
        Ok::<(), trust_runtime_core::error::RuntimeError>(())
    })();
    ENABLED.with(|enabled| enabled.set(false));
    outcome.unwrap();
    assert_eq!(
        ALLOCATIONS.with(Cell::get),
        0,
        "borrowed policy/traversal must not materialize paths"
    );
}

#[test]
fn actual_field_load_and_store_opcodes_do_not_allocate_reference_paths() {
    use trust_runtime_core::{
        bytecode::{SectionData, SectionId},
        error::RuntimeError,
        memory::VariableStorage,
        value::{DateTimeValue, Duration},
        vm::{ExecutionServices, PreparationLimits, PreparedModule},
    };

    // Instruction-boundary observation selects the actual shared dispatch path;
    // frame setup and other instructions are outside this allocation assertion.
    struct OpcodeAllocationObserver {
        code: Vec<u8>,
        armed: Cell<bool>,
        loads: Cell<usize>,
        stores: Cell<usize>,
    }
    impl ExecutionServices for OpcodeAllocationObserver {
        fn debug_active(&self) -> bool {
            true
        }
        fn deadline_exceeded(&self) -> bool {
            false
        }
        fn has_wall_clock(&self) -> bool {
            false
        }
        fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
            Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
        }
        fn on_statement(
            &self,
            _pou: u32,
            pc: usize,
            _depth: u32,
            _storage: &VariableStorage,
            _time: Duration,
        ) {
            ENABLED.with(|enabled| enabled.set(false));
            if !self.armed.get() {
                return;
            }
            // Specification 12's reference instructions, without decoding operands.
            const LOAD_REF: u8 = 0x20;
            const STORE_REF: u8 = 0x21;
            match self.code[pc] {
                LOAD_REF => self.loads.set(self.loads.get() + 1),
                STORE_REF => self.stores.set(self.stores.get() + 1),
                _ => return,
            }
            ENABLED.with(|enabled| enabled.set(true));
        }
    }
    let session = CompileSession::from_source(
        "TYPE Pair : STRUCT number : INT; END_STRUCT; END_TYPE \
         VAR_GLOBAL item : Pair; observed : INT; END_VAR \
         PROGRAM Main item.number := INT#7; observed := item.number; END_PROGRAM",
    );
    let module = session.build_bytecode_module().unwrap();
    let Some(SectionData::PouBodies(code)) = module.section(SectionId::PouBodies) else {
        panic!("the real producer must supply instruction bytes");
    };
    let services = OpcodeAllocationObserver {
        code: code.clone(),
        armed: Cell::new(false),
        loads: Cell::new(0),
        stores: Cell::new(0),
    };
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate_with_services(0, &services).unwrap();
    // Warm declared lookup/unique-value state before repeated hot-path measurement.
    state.execute_cycle(Duration::ZERO).unwrap();
    ALLOCATIONS.with(|count| count.set(0));
    services.armed.set(true);
    let outcome = (|| {
        for ms in 1..=128 {
            state.execute_cycle(Duration::from_millis(ms))?;
            ENABLED.with(|enabled| enabled.set(false));
            assert_eq!(state.storage().get_global("observed"), Some(&Value::Int(7)));
        }
        Ok::<(), RuntimeError>(())
    })();
    ENABLED.with(|enabled| enabled.set(false));
    services.armed.set(false);
    outcome.unwrap();
    assert_eq!(services.loads.get(), 128, "the real LOAD_REF path executed");
    assert_eq!(
        services.stores.get(),
        256,
        "both field/global STORE_REF paths executed"
    );
    assert_eq!(
        ALLOCATIONS.with(Cell::get),
        0,
        "actual reference instructions must not materialize paths"
    );
}

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
