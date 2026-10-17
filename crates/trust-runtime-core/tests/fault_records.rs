//! Golden fault contract for the core VM (specification 34 MEM-05): every `VmTrap` keeps its
//! stable code and its exact text, a VM fault is built without allocating, and the fixed-size
//! `FaultRecord` stays `Copy` and at most 48 bytes.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::fmt;

use trust_runtime_core::error::{RuntimeError, StableErrorCode};
use trust_runtime_core::fault::{FaultDetail, FaultRecord, FaultSite};
use trust_runtime_core::vm::VmTrap;

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
            "invalid bytecode 'vm call stack overflow'",
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

/// The fault record is `Copy` and stays within its 48-byte bound on this target.
#[test]
fn the_fault_record_is_copy_and_at_most_48_bytes() {
    let record = FaultRecord {
        code: StableErrorCode::BytecodeInvalidOpcode,
        detail: FaultDetail::Opcode(0xFF),
        site: FaultSite { pou: 7, pc: 9 },
        task: 2,
        logical_time: 30,
    };
    let copied = record;
    assert_eq!(copied, record, "the record is copied, not moved");
    assert!(
        core::mem::size_of::<FaultRecord>() <= 48,
        "FaultRecord is {} bytes, above the 48-byte bound",
        core::mem::size_of::<FaultRecord>()
    );
}
