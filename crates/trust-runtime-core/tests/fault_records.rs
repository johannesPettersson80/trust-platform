//! Golden fault contract for the core VM (specification 34 MEM-05, STATE-04): every `VmTrap`
//! keeps its stable code and its exact text, a VM fault is built without allocating, the
//! fixed-size `FaultRecord` stays `Copy` and at most 56 bytes, and a faulting scan latches that
//! record with the innermost failing site, the task and the scan's logical time.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fmt;

use trust_runtime_core::error::{RuntimeError, StableErrorCode};
use trust_runtime_core::fault::{FaultDetail, FaultRecord, FaultSite};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{
    LogicalExecutionServices, PreparationLimits, PreparedModule, RuntimeState, VmTrap,
};

/// Allocator calls counted while a conversion runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Calls {
    alloc: u64,
    realloc: u64,
    dealloc: u64,
}

impl Calls {
    const ZERO: Self = Self {
        alloc: 0,
        realloc: 0,
        dealloc: 0,
    };
}

impl fmt::Display for Calls {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}/{}/{}",
            self.alloc, self.realloc, self.dealloc
        )
    }
}

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNTED: Cell<Calls> = const { Cell::new(Calls::ZERO) };
}

/// Count one call on this thread while it is armed. Only const-initialized `Cell`s are
/// touched, so counting never allocates, blocks or unwinds.
fn record(call: fn(&mut Calls)) {
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = COUNTED.try_with(|counted| {
                let mut calls = counted.get();
                call(&mut calls);
                counted.set(calls);
            });
        }
    });
}

struct CountingAllocator;

// SAFETY: every operation forwards the caller's unchanged layout, pointer and new size to
// `System`; the counting touches only const-initialized thread-local `Cell`s.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(|calls| calls.alloc = calls.alloc.saturating_add(1));
        // SAFETY: the caller supplies a valid allocation layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(|calls| calls.realloc = calls.realloc.saturating_add(1));
        // SAFETY: `System` returned `pointer` for `layout`; `size` is the caller's new size.
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(|calls| calls.dealloc = calls.dealloc.saturating_add(1));
        // SAFETY: `pointer` and `layout` are the pair `System` returned.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Counting on this thread from creation until drop, starting from zero.
struct Armed;

impl Armed {
    fn new() -> Self {
        COUNTED.with(|counted| counted.set(Calls::ZERO));
        ARMED.with(|armed| armed.set(true));
        Self
    }
}

impl Drop for Armed {
    fn drop(&mut self) {
        ARMED.with(|armed| armed.set(false));
    }
}

/// The calls counted since the last take.
fn take_counted() -> Calls {
    COUNTED.with(|counted| counted.replace(Calls::ZERO))
}

/// One golden row per `VmTrap` variant: the literal trap, its literal stable code and its
/// literal text, read from `into_runtime_error` before this slice changed it.
#[allow(
    clippy::too_many_lines,
    reason = "one compatibility row per VmTrap variant in one readable table"
)]
fn trap_cases() -> Vec<(VmTrap, StableErrorCode, &'static str)> {
    vec![
        (
            VmTrap::InvalidOpcode(0xFF),
            StableErrorCode::BytecodeInvalidOpcode,
            "invalid bytecode 'vm invalid opcode 0xFF'",
        ),
        (
            VmTrap::InvalidJumpTarget(17),
            StableErrorCode::BytecodeInvalidJumpTarget,
            "invalid bytecode 'vm invalid jump target 17'",
        ),
        (
            VmTrap::InvalidRefIndex(3),
            StableErrorCode::BytecodeInvalidIndex,
            "invalid bytecode 'vm invalid ref index 3'",
        ),
        (
            VmTrap::InvalidConstIndex(4),
            StableErrorCode::BytecodeInvalidIndex,
            "invalid bytecode 'vm invalid const index 4'",
        ),
        (
            VmTrap::InvalidLocalRef {
                ref_index: 2,
                start: 8,
                count: 3,
            },
            StableErrorCode::BytecodeInvalidIndex,
            "invalid bytecode 'vm invalid local ref 2 (frame local range 8..11)'",
        ),
        (
            VmTrap::StackUnderflow,
            StableErrorCode::VmStackUnderflow,
            "invalid bytecode 'vm operand stack underflow'",
        ),
        (
            VmTrap::StackOverflow,
            StableErrorCode::VmStackOverflow,
            "invalid bytecode 'vm operand stack overflow'",
        ),
        (
            VmTrap::CallStackUnderflow,
            StableErrorCode::VmCallStackUnderflow,
            "invalid bytecode 'vm call stack underflow'",
        ),
        (
            VmTrap::CallStackOverflow,
            StableErrorCode::VmCallStackOverflow,
            "call depth exceeded",
        ),
        (
            VmTrap::UnsupportedOpcode("reserved"),
            StableErrorCode::VmUnsupportedOpcode,
            "invalid bytecode 'vm unsupported opcode reserved'",
        ),
        (
            VmTrap::UnsupportedRefLocation("external"),
            StableErrorCode::VmUnsupportedReferenceLocation,
            "invalid bytecode 'vm unsupported ref location external'",
        ),
        (
            VmTrap::ConditionNotBool,
            StableErrorCode::RuntimeConditionNotBool,
            "condition is not BOOL",
        ),
        (
            VmTrap::NullReference,
            StableErrorCode::RuntimeNullReference,
            "null reference dereference",
        ),
        (
            VmTrap::DeadlineExceeded,
            StableErrorCode::RuntimeExecutionTimeout,
            "execution timed out",
        ),
        (
            VmTrap::BudgetExceeded,
            StableErrorCode::RuntimeExecutionTimeout,
            "execution timed out",
        ),
        (
            VmTrap::ForStepZero,
            StableErrorCode::RuntimeForStepZero,
            "FOR loop step cannot be zero",
        ),
        (
            VmTrap::MissingPou(9),
            StableErrorCode::BytecodeInvalidPouId,
            "invalid bytecode 'vm missing pou id 9'",
        ),
        (
            VmTrap::MissingProgram("missing".into()),
            StableErrorCode::RuntimeUndefinedProgram,
            "undefined program 'missing'",
        ),
        (
            VmTrap::MissingFunctionBlock("missing".into()),
            StableErrorCode::RuntimeUndefinedFunctionBlock,
            "undefined function block 'missing'",
        ),
        (
            VmTrap::InvalidNativeCallKind(7),
            StableErrorCode::VmInvalidNativeCall,
            "invalid bytecode 'vm invalid CALL_NATIVE kind 7'",
        ),
        (
            VmTrap::InvalidNativeSymbolIndex(8),
            StableErrorCode::VmInvalidNativeCall,
            "invalid bytecode 'vm invalid index 8 for native symbol'",
        ),
        (
            VmTrap::InvalidNativeCall("invalid".into()),
            StableErrorCode::VmInvalidNativeCall,
            "invalid bytecode 'vm invalid CALL_NATIVE payload: invalid'",
        ),
        (
            VmTrap::BytecodeDecode("invalid".into()),
            StableErrorCode::VmBytecodeDecode,
            "invalid bytecode 'invalid'",
        ),
        (
            VmTrap::Runtime(RuntimeError::TypeMismatch),
            StableErrorCode::RuntimeTypeMismatch,
            "type mismatch",
        ),
    ]
}

/// Every `VmTrap` keeps the stable code it had and renders exactly the text it had, both from
/// the trap itself and after conversion to `RuntimeError`.
#[test]
fn every_vm_trap_keeps_its_stable_code_and_exact_text() {
    for (trap, code, text) in trap_cases() {
        let before = trap.stable_code();
        let error = trap.into_runtime_error();
        assert_eq!(before, code, "the trap's stable code for {text}");
        assert_eq!(error.stable_code(), code, "the stable code of {text}");
        assert_eq!(error.to_string(), text);
    }
}

/// Converting a fixed-size VM fault builds it without a single allocator call. The native-call
/// payload keeps the caller's formatted message (census F1/F3), so its conversion still formats
/// the fixed prefix and is not asserted here.
#[test]
fn a_vm_fault_is_built_without_allocation() {
    for (trap, _, text) in trap_cases() {
        if matches!(trap, VmTrap::InvalidNativeCall(_)) {
            continue;
        }
        let (error, counted) = {
            let _armed = Armed::new();
            (trap.into_runtime_error(), take_counted())
        };
        assert_eq!(counted, Calls::ZERO, "converting to {text} allocated");
        drop(error);
    }
}

/// The fault record is `Copy` and stays within its 56-byte bound on this target.
#[test]
fn the_fault_record_is_copy_and_at_most_56_bytes() {
    let record = FaultRecord {
        code: StableErrorCode::BytecodeInvalidOpcode,
        detail: FaultDetail::Opcode(0xFF),
        site: FaultSite { pou: 7, pc: 9 },
        task: 2,
        logical_seconds: 30,
        logical_millis: 1,
    };
    let copied = record;
    assert_eq!(copied, record, "the record is copied, not moved");
    assert!(
        core::mem::size_of::<FaultRecord>() <= 56,
        "FaultRecord is {} bytes, above the 56-byte bound",
        core::mem::size_of::<FaultRecord>()
    );
}

/// The bounded-C corpus's `faults` application, generated on the builder, never written by
/// hand:
/// `cargo run --locked -p trust-runtime --example bounded_c_fixtures -- crates/trust-runtime/tests/fixtures/bounded-c`.
const FAULTS: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/faults/program.stbc");

/// The chips' admission limits a faulting case needs (`chip_limits()` in
/// `scan_allocation_zero.rs`): a work allowance and a call depth the corpus's `faults` program
/// can exhaust. The other limits stay at the hosted defaults, which admit its small artifact.
fn fault_limits() -> PreparationLimits {
    PreparationLimits {
        max_work: 50_000,
        max_call_depth: 4,
        ..PreparationLimits::default()
    }
}

/// The corpus's `faults` application after two healthy scans, at 4,000 ms and 4,020 ms, so the
/// engine is warm when the next scan raises the selected `fault_kind`.
fn faults_application<'a>(
    prepared: &'a PreparedModule,
    services: &'a LogicalExecutionServices,
) -> RuntimeState<'a> {
    let mut state = prepared
        .instantiate_with_services(0, services)
        .expect("the chips' admission accepts the corpus faults application");
    for at_ms in [4_000, 4_020] {
        state
            .execute_cycle(Duration::from_millis(at_ms))
            .expect("a healthy scan of the corpus faults application runs");
    }
    state
}

/// Select one `fault_kind` of the corpus's `faults` program and run the scan that raises it at
/// 4,036 ms. `fault_kind`'s cases are 1 division, 2 an index, 3 a NULL dereference, 4 an
/// overflow, 5 the work allowance and 6 a call chain one level too deep.
fn fault_at_4_036_ms<'a>(
    prepared: &'a PreparedModule,
    services: &'a LogicalExecutionServices,
    fault_kind: i16,
) -> (RuntimeState<'a>, RuntimeError) {
    let mut state = faults_application(prepared, services);
    state
        .write_global("fault_kind", Value::Int(fault_kind))
        .expect("fault_kind is a declared global of the corpus faults application");
    let error = state
        .execute_cycle(Duration::from_millis(4_036))
        .expect_err("the selected fault kind fails its scan");
    (state, error)
}

/// Spec 34 MEM-05 and STATE-04: a faulting scan latches the error's stable code and detail, the
/// site of the failing instruction, the task that ran and the scan's logical time. It would
/// catch a record that loses the site, the task or the detail, one that names the caller's
/// instruction instead, and a time read as one plain millisecond count.
#[test]
fn a_fault_latches_its_code_detail_site_task_and_time() {
    let prepared = PreparedModule::from_bytes(FAULTS, fault_limits())
        .expect("the chips' admission accepts the corpus faults application");
    let services = LogicalExecutionServices;
    let (state, error) = fault_at_4_036_ms(&prepared, &services, 2);
    let record = state
        .fault_record()
        .expect("the faulting scan latched a fault record");

    // `result := values[outside]` with the fixture's `outside : DINT := DINT#9` against
    // `values : ARRAY[1..4] OF DINT`.
    assert_eq!(record.code, StableErrorCode::RuntimeIndexOutOfBounds);
    assert_eq!(
        record.detail,
        FaultDetail::Index {
            index: 9,
            lower: 1,
            upper: 4,
        }
    );
    // The INDEX instruction (opcode 0x31) of that statement, at code offset 0x5C of POU 0:
    // `program.disassembly.txt` of the corpus folder.
    #[cfg(not(feature = "register-ir"))]
    assert_eq!(record.site, FaultSite { pou: 0, pc: 0x5C });
    assert_eq!(record.task, 0, "the single `Scan` task ran the program");
    // The scan's logical time is 4,036 ms: 4 whole seconds and 36 ms.
    assert_eq!((record.logical_seconds, record.logical_millis), (4, 36));

    assert_eq!(
        error.stable_code(),
        StableErrorCode::RuntimeIndexOutOfBounds,
        "the scan still returns the error"
    );
    assert_eq!(
        state.fault().map(RuntimeError::stable_code),
        Some(StableErrorCode::RuntimeIndexOutOfBounds),
        "fault() keeps returning the error as before"
    );
}

/// Spec 34 MEM-05 and STATE-04: the latched record reads the same every time and a restart
/// clears it together with the fault it belongs to. It would catch a reader that consumes or
/// recomputes the record, and a restart that keeps the previous installation's record.
#[test]
fn the_record_reads_the_same_every_time_and_clears_on_restart() {
    let prepared = PreparedModule::from_bytes(FAULTS, fault_limits())
        .expect("the chips' admission accepts the corpus faults application");
    let services = LogicalExecutionServices;
    let (mut state, _) = fault_at_4_036_ms(&prepared, &services, 2);
    let latched = state
        .fault_record()
        .expect("the faulting scan latched a fault record");

    assert_eq!(state.fault_record(), Some(latched));
    assert_eq!(
        state.fault_record(),
        Some(latched),
        "a third read repeats the same record"
    );

    state
        .restart(RestartMode::Cold)
        .expect("the cold restart of the corpus faults application is admitted");
    assert_eq!(
        state.fault_record(),
        None,
        "the restart clears the record with the fault"
    );
    assert!(
        state.fault().is_none(),
        "the restart clears the fault the record belongs to"
    );
    state
        .execute_cycle(Duration::from_millis(4_056))
        .expect("the restarted application runs a healthy scan");
    assert_eq!(
        state.fault_record(),
        None,
        "a healthy scan latches no record"
    );
}

/// Spec 34 MEM-05: a fault inside a nested call records the innermost failing instruction, not
/// the program root that started the chain and not the frame that made the call it could not
/// complete. It would catch a site recorded at the entry or at the last instruction reached.
#[test]
fn a_fault_inside_a_nested_call_records_the_innermost_site() {
    let prepared = PreparedModule::from_bytes(FAULTS, fault_limits())
        .expect("the chips' admission accepts the corpus faults application");
    let services = LogicalExecutionServices;
    // `depth := Depth4()` calls `Depth4`, `Depth3`, `Depth2` and then `Depth1`; four frames are
    // admitted, so the call from `Depth2` (POU 35) to `Depth1` fails at its CALL instruction,
    // 0x142.
    let (state, _) = fault_at_4_036_ms(&prepared, &services, 6);
    let record = state
        .fault_record()
        .expect("the faulting scan latched a fault record");

    assert_eq!(record.code, StableErrorCode::VmCallStackOverflow);
    assert_eq!(record.detail, FaultDetail::Text("vm call stack overflow"));
    // The fault log reads the record as the runtime error reads (specification 12 §7.4.1), not
    // as a damaged program's `invalid bytecode 'vm call stack overflow'`.
    assert_eq!(record.to_string(), "call depth exceeded");
    #[cfg(not(feature = "register-ir"))]
    assert_eq!(record.site, FaultSite { pou: 35, pc: 0x142 });
    assert_eq!(record.task, 0, "the single `Scan` task ran the program");
    assert_eq!((record.logical_seconds, record.logical_millis), (4, 36));
}

/// Spec 34 MEM-05: the record's logical time is whole seconds beside the millisecond within
/// the second, at the millisecond precision the host's fault log reports. It would catch a
/// lost millisecond (a coarser unit or a wrong remainder) and a second that wraps before the
/// 32-bit field is full.
#[test]
fn the_time_field_keeps_the_millisecond_and_does_not_wrap() {
    let error = RuntimeError::DivisionByZero;
    let site = FaultSite { pou: 1, pc: 2 };
    let at_1_234_567_ms = FaultRecord::latch(&error, site, 0, Duration::from_millis(1_234_567));
    assert_eq!(
        (
            at_1_234_567_ms.logical_seconds,
            at_1_234_567_ms.logical_millis
        ),
        (1_234, 567),
        "1,234,567 ms is 1,234 s and 567 ms"
    );

    // u32::MAX seconds is 136 years of uptime; the last second and its millisecond read back
    // exactly.
    let at_the_last_second = FaultRecord::latch(
        &error,
        site,
        0,
        Duration::from_millis(i64::from(u32::MAX) * 1_000 + 999),
    );
    assert_eq!(
        (
            at_the_last_second.logical_seconds,
            at_the_last_second.logical_millis
        ),
        (u32::MAX, 999),
        "the last whole second and its millisecond read back exactly"
    );
}

/// Spec 34 MEM-05: the record's time stays distinct for a century of uptime in its 32-bit
/// second field, where a 32-bit millisecond count wraps after 49.7 days. It would catch a
/// record whose time field wraps within a century, or one that stores the raw millisecond
/// count beside its seconds.
#[test]
fn the_time_field_does_not_wrap_within_a_century() {
    // 100 years of 365 days: 3,153,600,000 seconds.
    const SECONDS_IN_A_CENTURY: u32 = 3_153_600_000;
    // The old 32-bit millisecond field holds 49 days; the record's 32-bit second field holds 136
    // years, so a century of uptime stays distinct in it.
    assert_eq!(
        u32::MAX / 86_400_000,
        49,
        "a 32-bit millisecond field wraps after 49 days"
    );
    assert_eq!(
        u32::MAX / 31_536_000,
        136,
        "a 32-bit second field lasts 136 years"
    );

    let error = RuntimeError::DivisionByZero;
    let site = FaultSite { pou: 1, pc: 2 };
    let at_a_century = FaultRecord::latch(
        &error,
        site,
        0,
        Duration::from_millis(i64::from(SECONDS_IN_A_CENTURY) * 1_000 + 36),
    );
    assert_eq!(
        (at_a_century.logical_seconds, at_a_century.logical_millis),
        (SECONDS_IN_A_CENTURY, 36),
        "a century of uptime and 36 ms is a distinct time in the record"
    );
    let one_second_later = FaultRecord::latch(
        &error,
        site,
        0,
        Duration::from_millis((i64::from(SECONDS_IN_A_CENTURY) + 1) * 1_000 + 36),
    );
    assert_eq!(
        one_second_later.logical_seconds,
        SECONDS_IN_A_CENTURY + 1,
        "the next second stays distinct, so the field has not wrapped"
    );
}
