//! The faults that end a scan (specification 44 §6.5, §8.6 and Appendix A).

use core::num::NonZeroU32;

/// A fault that ends a scan, as a module's functions return it up to the entry point. The fault's
/// record is in [`Cx::fault`](crate::Cx::fault).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct Fault(NonZeroU32);

impl Fault {
    /// The fault of `code`.
    #[must_use]
    pub const fn new(code: FaultCode) -> Self {
        match NonZeroU32::new(code as u32) {
            Some(raw) => Self(raw),
            None => Self(NonZeroU32::MIN),
        }
    }

    /// The fault whose code is `raw`, which a kernel function returned; `None` for 0, which is no
    /// fault.
    #[must_use]
    pub const fn from_raw(raw: u32) -> Option<Self> {
        match NonZeroU32::new(raw) {
            Some(raw) => Some(Self(raw)),
            None => None,
        }
    }

    /// The fault's code as the entry point returns it.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0.get()
    }
}

/// The codes of the faults a module raises, the stable runtime errors of specification 10 §10.2
/// that compiled code can meet, and the kernel's own (specification 44 Appendix A). The numbers are
/// part of the interface: a kernel maps each to its runtime error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FaultCode {
    /// An integer division by zero (IEC §6.6.2.5.8): `runtime_division_by_zero`.
    DivisionByZero = 1,
    /// `MOD` by zero: `runtime_modulo_by_zero`.
    ModuloByZero = 2,
    /// An integer or duration result outside its type (specification 05 §3): `runtime_overflow`.
    Overflow = 3,
    /// An array or character index outside its bounds (IEC §6.4.4.5); the detail is the index and
    /// the bounds: `runtime_index_out_of_bounds`.
    IndexOutOfBounds = 4,
    /// A value outside a subrange (IEC §6.4.4.4); the detail is the value and the bounds:
    /// `runtime_subrange_violation`.
    SubrangeViolation = 5,
    /// A dereference of `NULL` (IEC §6.4.4.10.3): `runtime_null_reference`.
    NullReference = 6,
    /// A `FOR` loop whose step is zero (specification 10 §10): `runtime_for_step_zero`.
    ForStepZero = 7,
    /// An integer that is no value of the enumeration it converts to: `runtime_type_mismatch`.
    TypeMismatch = 8,
    /// A date or time outside its range: `runtime_date_time_range`.
    DateTimeRange = 9,
    /// A text longer than the 65,535 the implementation allows (specification 07 §8.1):
    /// `runtime_string_capacity_exceeded`.
    StringCapacityExceeded = 10,
    /// A reference whose target is no longer there (specification 44 §7.3):
    /// `runtime_reference_lifetime`.
    ReferenceLifetime = 11,
    /// A non-finite real where a finite one is required: `runtime_non_finite_value`.
    NonFiniteValue = 12,
    /// The activation used up its statement budget (specification 44 §8.4).
    InfiniteLoop = 13,
    /// The task stack reached its margin (specification 44 §8.5).
    StackOverflow = 14,
    /// The CPU's memory protection stopped an access (specification 44 §10.5).
    MemoryProtection = 15,
    /// A fault that compiled code cannot raise by construction, such as an area smaller than its
    /// program's view of it (specification 44 §8.6).
    ModuleInternal = 16,
    /// A test's comparison or truth assertion failed: the operands' bytes and the assertion with
    /// the operands' types (specification 44 Appendix A): `runtime_assertion_failed`.
    AssertionFailed = 17,
    /// A test's `ASSERT_NEAR` failed: the expected, the actual and the delta in binary64:
    /// `runtime_assertion_failed`.
    AssertionNear = 18,
    /// A recursive call found no call depth left (specification 44 §6.4): the bytecode engine's
    /// call beyond its call levels, `call depth exceeded`.
    CallDepth = 19,
}

impl FaultCode {
    /// Every code, in the order of their numbers.
    pub const ALL: [Self; 19] = [
        Self::DivisionByZero,
        Self::ModuloByZero,
        Self::Overflow,
        Self::IndexOutOfBounds,
        Self::SubrangeViolation,
        Self::NullReference,
        Self::ForStepZero,
        Self::TypeMismatch,
        Self::DateTimeRange,
        Self::StringCapacityExceeded,
        Self::ReferenceLifetime,
        Self::NonFiniteValue,
        Self::InfiniteLoop,
        Self::StackOverflow,
        Self::MemoryProtection,
        Self::ModuleInternal,
        Self::AssertionFailed,
        Self::AssertionNear,
        Self::CallDepth,
    ];

    /// The code numbered `raw`, or `None` for a number no code has.
    #[must_use]
    pub fn from_raw(raw: u32) -> Option<Self> {
        Self::ALL.iter().copied().find(|code| *code as u32 == raw)
    }
}

/// The code that a `__CATCH` variable takes for `fault`, raised by a statement inside `__TRY`: one of
/// CODESYS's `__SYSTEM.ExceptionCode` values (specification 06 §15). `None` for a fault that ends the
/// scan whatever `__TRY` says: the statement budget, the task stack's margin, the CPU's memory
/// protection, a module's internal fault and a test's failed assertion, so that a looping, exhausted
/// or damaged program cannot hide behind `__TRY` and a test still fails.
#[must_use]
pub fn exception_code(fault: Fault) -> Option<u32> {
    /// `RTSEXCPT_ACCESS_VIOLATION`.
    const ACCESS_VIOLATION: u32 = 0x0000_0051;
    /// `RTSEXCPT_STACK_OVERFLOW`.
    const STACK_OVERFLOW: u32 = 0x0000_0054;
    /// `RTSEXCPT_ARRAYBOUNDS`.
    const ARRAY_BOUNDS: u32 = 0x0000_0101;
    /// `RTSEXCPT_DIVIDEBYZERO`.
    const DIVIDE_BY_ZERO: u32 = 0x0000_0102;
    /// `RTSEXCPT_OVERFLOW`.
    const OVERFLOW: u32 = 0x0000_0103;
    /// `RTSEXCPT_FPU_INVALID_OPERATION`.
    const FPU_INVALID_OPERATION: u32 = 0x0000_0154;
    /// `RTSEXCPT_UNKNOWN`.
    const UNKNOWN: u32 = 0xFFFF_FFFF;
    match FaultCode::from_raw(fault.raw())? {
        FaultCode::DivisionByZero | FaultCode::ModuloByZero => Some(DIVIDE_BY_ZERO),
        FaultCode::Overflow | FaultCode::DateTimeRange => Some(OVERFLOW),
        FaultCode::IndexOutOfBounds => Some(ARRAY_BOUNDS),
        FaultCode::NullReference | FaultCode::ReferenceLifetime => Some(ACCESS_VIOLATION),
        FaultCode::CallDepth => Some(STACK_OVERFLOW),
        FaultCode::NonFiniteValue => Some(FPU_INVALID_OPERATION),
        FaultCode::SubrangeViolation
        | FaultCode::ForStepZero
        | FaultCode::TypeMismatch
        | FaultCode::StringCapacityExceeded => Some(UNKNOWN),
        FaultCode::InfiniteLoop
        | FaultCode::StackOverflow
        | FaultCode::MemoryProtection
        | FaultCode::ModuleInternal
        | FaultCode::AssertionFailed
        | FaultCode::AssertionNear => None,
    }
}

/// The record of the fault that ended a call: its code, the statement that raised it and up to
/// three numbers that describe it (the index and the bounds, the value and the bounds).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FaultRecord {
    /// The fault's code, 0 while no fault ended the call.
    pub code: u32,
    /// The id of the statement that raised it.
    pub stmt: u32,
    /// The fault's numbers, as its code says.
    pub detail: [i64; 3],
}
