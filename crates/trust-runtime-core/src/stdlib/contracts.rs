//! Versioned execution contracts of the built-in imports (spec 34 §5.2.1, LOAD-06, PORT-02).
//!
//! The standard-library registration stays the one list of registered functions: each family
//! in `registration.rs` names its contract here. Conversions, date/time splits, the runtime
//! clocks and the standard function blocks keep their existing recognizers, and every kind
//! they recognize maps to one contract below. Preparation refuses an import without a
//! contract or one that does not match it (`vm::prepared::admission`).

use super::fbs::{self, BuiltinFbKind};
use super::{conversions, registration, time, StdParams};
use crate::error_code::StableErrorCode;
use crate::error_code::StableErrorCode as Code;
use FbParameterType::{Bool, Count, Duration};
use ImportAllocation::{NoText, TextResult};
use ImportWork::{Binding, TextArguments};

/// Execution contract of one family of built-in imports.
///
/// No built-in import blocks: each returns without waiting for a device, a lock or a service
/// other than the clock it is given. A failing call returns one of `failures` as a typed
/// runtime error. The engine's binding and reference checks (specification 12 §7.7 and §7.8)
/// apply to every call besides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportContract {
    /// Contract version; a change to any rule of a contract issues a new version.
    pub version: u16,
    /// Work the call charges beyond its own instructions (spec 34 TIME-08).
    pub work: ImportWork,
    /// Allocation the call charges for its result (TIME-08).
    pub allocation: ImportAllocation,
    /// Every stable fault the implementation can return for a call that matches its signature.
    pub failures: &'static [StableErrorCode],
    /// Capability the execution profile must supply before the import runs.
    pub capability: Option<ImportCapability>,
}

/// Work a built-in call charges beyond its instructions (TIME-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportWork {
    /// Its argument binding only; the body charges nothing.
    Binding,
    /// Its argument binding, plus 1 unit per byte of its STRING and WSTRING arguments before
    /// the body runs.
    TextArguments,
}

/// Allocation a built-in call charges (TIME-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportAllocation {
    /// The call returns no new text and charges no allocation.
    NoText,
    /// A STRING or WSTRING result is charged as a new value: 1 unit and its bytes of
    /// allocation.
    TextResult,
}

/// A platform capability an import requires from the execution profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportCapability {
    /// The platform wall clock, read by `CURRENT_DT` (spec 34 §5.2).
    WallClock,
}

/// Parameters a built-in function call must match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionSignature {
    /// A registered standard function: the parameters of its registration.
    Registered(&'static StdParams),
    /// A type conversion: one input `IN`.
    Conversion,
    /// A date/time split: the input `IN`, then its writable outputs.
    Split(&'static [&'static str]),
    /// A runtime clock: no parameters.
    Clock,
}

/// A built-in function import and its contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionImport {
    /// Parameters the call must match.
    pub signature: FunctionSignature,
    /// Execution contract of the import's family.
    pub contract: &'static ImportContract,
}

/// Type of a standard function-block parameter, resolved per variant by
/// [`FunctionBlockImport::primitive`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FbParameterType {
    /// BOOL.
    Bool,
    /// A counter's PV and CV: the width the variant's name gives, or the generic `ANY_INT`
    /// counter state of `CTU`, `CTD` and `CTUD` (specification 12 §11.5.4).
    Count,
    /// A timer's PT and ET: TIME for the `_TIME` variants, LTIME for the `_LTIME` variants, and the
    /// native duration slot of the overloaded `TP`, `TON` and `TOF` (specification 34 §6.2).
    Duration,
}

/// One declared parameter of a standard function block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FbParameter {
    /// IEC formal name.
    pub name: &'static str,
    /// True for an output (STBC direction 1), false for an input (direction 0).
    pub output: bool,
    /// Declared type.
    pub ty: FbParameterType,
}

/// A standard function-block import and its contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionBlockImport {
    /// Native implementation that executes the block.
    pub kind: BuiltinFbKind,
    /// Declared parameters in IEC order.
    pub parameters: &'static [FbParameter],
    /// Hidden native state: slot name and STBC primitive id (specification 12 §11.5.5).
    pub state: &'static [(&'static str, u16)],
    /// Execution contract of the block's family.
    pub contract: &'static ImportContract,
    count: u16,
    duration: u16,
}

impl FunctionBlockImport {
    /// STBC primitive id (specification 12 §6.2) a parameter type has in this variant.
    #[must_use]
    pub fn primitive(&self, ty: FbParameterType) -> u16 {
        match ty {
            Bool => BOOL,
            Count => self.count,
            Duration => self.duration,
        }
    }
}

/// Look up a standard-library import by its canonical uppercase name, as a native-call
/// descriptor normalizes it. Clocks, splits, conversions and registered functions are tried
/// in the order the call dispatcher runs them.
#[must_use]
pub fn function(name: &str) -> Option<FunctionImport> {
    resolve_function(name, conversions::is_conversion_name(name))
}

/// [`function`], given whether the name is a conversion, as a parsed descriptor records it.
pub(crate) fn resolve_function(name: &str, conversion: bool) -> Option<FunctionImport> {
    let (signature, contract) = if time::is_runtime_clock_name(name) {
        // Only CURRENT_DT reads the platform wall clock (`time::runtime_clock_value`).
        let contract = if name == "CURRENT_DT" {
            &WALL_CLOCK
        } else {
            &ELAPSED_CLOCK
        };
        (FunctionSignature::Clock, contract)
    } else if let Some(params) = time::split_parameter_names(name) {
        (FunctionSignature::Split(params), &SPLITS)
    } else if conversion {
        (FunctionSignature::Conversion, &CONVERSIONS)
    } else {
        let (entry, contract) = registration::find(name)?;
        (FunctionSignature::Registered(entry.params), contract)
    };
    Some(FunctionImport {
        signature,
        contract,
    })
}

/// Every registered standard function with its import, in registration order.
pub fn registered() -> impl Iterator<Item = (&'static str, FunctionImport)> {
    registration::entries().map(|(name, entry, contract)| {
        (
            name,
            FunctionImport {
                signature: FunctionSignature::Registered(entry.params),
                contract,
            },
        )
    })
}

/// Look up a standard function block by its canonical uppercase name.
#[must_use]
pub fn function_block(name: &str) -> Option<FunctionBlockImport> {
    let kind = fbs::builtin_kind_uppercase(name)?;
    let (parameters, contract) = match kind {
        BuiltinFbKind::Rs => (RS_PARAMETERS, &BISTABLES),
        BuiltinFbKind::Sr => (SR_PARAMETERS, &BISTABLES),
        BuiltinFbKind::RTrig | BuiltinFbKind::FTrig => (TRIGGER_PARAMETERS, &EDGE_DETECTION),
        BuiltinFbKind::Ctu => (CTU_PARAMETERS, &COUNTERS),
        BuiltinFbKind::Ctd => (CTD_PARAMETERS, &COUNTERS),
        BuiltinFbKind::Ctud => (CTUD_PARAMETERS, &COUNTERS),
        BuiltinFbKind::Tp | BuiltinFbKind::Ton | BuiltinFbKind::Tof => (TIMER_PARAMETERS, &TIMERS),
    };
    Some(FunctionBlockImport {
        kind,
        parameters,
        state: fbs::state::builtin_state_layout(kind),
        contract,
        count: count_primitive(name),
        duration: duration_primitive(name),
    })
}

/// Duration family a timer variant's name gives; the overloaded timers hold either family in the
/// native duration slot that preparation gives their `PT` and `ET` (specification 34 §6.2).
fn duration_primitive(name: &str) -> u16 {
    if name.ends_with("_LTIME") {
        LTIME
    } else if name.ends_with("_TIME") {
        TIME
    } else {
        ANY_DURATION_STATE
    }
}

/// Counter width a variant's name gives; the generic counters use the `ANY_INT` state.
fn count_primitive(name: &str) -> u16 {
    if name.ends_with("_ULINT") {
        ULINT
    } else if name.ends_with("_UDINT") {
        UDINT
    } else if name.ends_with("_LINT") {
        LINT
    } else if name.ends_with("_DINT") {
        DINT
    } else if name.ends_with("_INT") {
        INT
    } else {
        ANY_INT_STATE
    }
}

// STBC primitive ids (specification 12 §6.2 and §11.5.4).
const BOOL: u16 = 1;
const INT: u16 = 7;
const DINT: u16 = 8;
const LINT: u16 = 9;
const UDINT: u16 = 12;
const ULINT: u16 = 13;
const TIME: u16 = 16;
const LTIME: u16 = 17;
const ANY_INT_STATE: u16 = 0x0100;
/// The internal primitive of the native duration slot, which preparation gives the `PT` and `ET` of
/// an overloaded `TP`, `TON` and `TOF`: a `TIME` or an `LTIME` value with its family (specification
/// 34 §6.2). It is never valid in a `TYPE_TABLE` (specification 12 §11.5.4).
pub(crate) const ANY_DURATION_STATE: u16 = 0x0101;

const fn version_1(
    work: ImportWork,
    allocation: ImportAllocation,
    failures: &'static [StableErrorCode],
) -> ImportContract {
    ImportContract {
        version: 1,
        work,
        allocation,
        failures,
        capability: None,
    }
}

/// IEC 61131-3 Ed.3 Tables 28 and 29; ADD, SUB, MUL and DIV of durations and dates follow
/// Table 35, where ADD and MUL take exactly two inputs.
pub(super) static NUMERIC: ImportContract = version_1(
    TextArguments,
    TextResult,
    &[
        Code::RuntimeTypeMismatch,
        Code::RuntimeOverflow,
        Code::RuntimeDivisionByZero,
        Code::RuntimeModuloByZero,
        Code::RuntimeDateTimeRange,
        Code::RuntimeInvalidArgumentCount,
    ],
);
/// IEC Tables 30 (shift and rotate) and 31 (bitwise Boolean).
pub(super) static BIT: ImportContract =
    version_1(TextArguments, NoText, &[Code::RuntimeTypeMismatch]);
/// IEC Table 32, applied to enumerations by Table 38.
pub(super) static SELECTION: ImportContract = version_1(
    TextArguments,
    TextResult,
    &[
        Code::RuntimeTypeMismatch,
        Code::RuntimeOverflow,
        Code::RuntimeIndexOutOfBounds,
    ],
);
/// IEC Table 33, applied to enumerations by Table 38.
pub(super) static COMPARISON: ImportContract =
    version_1(TextArguments, NoText, &[Code::RuntimeTypeMismatch]);
/// IEC Table 34, with the bounded-assignment helper `__TRUST_LIMIT_STRING` (specification 07
/// §8.1). A result longer than the implementer maximum of 65,535 is an error (IEC §6.6.2.5.11).
pub(super) static STRINGS: ImportContract = version_1(
    TextArguments,
    TextResult,
    &[
        Code::RuntimeTypeMismatch,
        Code::RuntimeOverflow,
        Code::RuntimeStringCapacityExceeded,
    ],
);
/// IEC Tables 35 (named time arithmetic) and 36 (CONCAT and DAY_OF_WEEK).
pub(super) static TIME_FUNCTIONS: ImportContract = version_1(
    TextArguments,
    NoText,
    &[
        Code::RuntimeTypeMismatch,
        Code::RuntimeOverflow,
        Code::RuntimeDivisionByZero,
        Code::RuntimeDateTimeRange,
    ],
);
/// IEC Table 39.
pub(super) static VALIDATE: ImportContract =
    version_1(TextArguments, NoText, &[Code::RuntimeTypeMismatch]);
/// The truST test-workflow assertions (specification 07, not IEC).
pub(super) static ASSERTIONS: ImportContract = version_1(
    TextArguments,
    NoText,
    &[
        Code::RuntimeAssertionFailed,
        Code::RuntimeTypeMismatch,
        Code::RuntimeOverflow,
    ],
);
/// IEC Tables 22 to 27: every name a Table 22 conversion form recognizes. A STRING or WSTRING
/// result longer than the implementer maximum is an error, as for the Table 34 functions.
static CONVERSIONS: ImportContract = version_1(
    TextArguments,
    TextResult,
    &[
        Code::RuntimeTypeMismatch,
        Code::RuntimeOverflow,
        Code::RuntimeStringCapacityExceeded,
    ],
);
/// IEC Table 36 splits; each output is written once, in the target's declared width.
static SPLITS: ImportContract = version_1(
    Binding,
    NoText,
    &[Code::RuntimeTypeMismatch, Code::RuntimeOverflow],
);
/// `TIME()`: the supplied logical elapsed time (specification 07, a truST clock source).
static ELAPSED_CLOCK: ImportContract = version_1(Binding, NoText, &[]);
/// `CURRENT_DT()`: the platform wall clock (specification 07, a truST clock source). A wall
/// time that DATE_AND_TIME cannot represent faults with overflow.
static WALL_CLOCK: ImportContract = ImportContract {
    capability: Some(ImportCapability::WallClock),
    ..version_1(Binding, NoText, &[Code::RuntimeOverflow])
};
/// IEC Table 43.
static BISTABLES: ImportContract = version_1(Binding, NoText, &[Code::RuntimeTypeMismatch]);
/// IEC Table 44, with DIFU and DIFD as further names of R_TRIG and F_TRIG.
static EDGE_DETECTION: ImportContract = version_1(Binding, NoText, &[Code::RuntimeTypeMismatch]);
/// IEC Table 45; a counter stops at its type's limits instead of overflowing.
static COUNTERS: ImportContract = version_1(Binding, NoText, &[Code::RuntimeTypeMismatch]);
/// IEC Table 46.
static TIMERS: ImportContract = version_1(Binding, NoText, &[Code::RuntimeTypeMismatch]);

const fn input(name: &'static str, ty: FbParameterType) -> FbParameter {
    FbParameter {
        name,
        output: false,
        ty,
    }
}

const fn output(name: &'static str, ty: FbParameterType) -> FbParameter {
    FbParameter {
        name,
        output: true,
        ty,
    }
}

static RS_PARAMETERS: &[FbParameter] = &[input("S", Bool), input("R1", Bool), output("Q1", Bool)];
static SR_PARAMETERS: &[FbParameter] = &[input("S1", Bool), input("R", Bool), output("Q1", Bool)];
static TRIGGER_PARAMETERS: &[FbParameter] = &[input("CLK", Bool), output("Q", Bool)];
static CTU_PARAMETERS: &[FbParameter] = &[
    input("CU", Bool),
    input("R", Bool),
    input("PV", Count),
    output("Q", Bool),
    output("CV", Count),
];
static CTD_PARAMETERS: &[FbParameter] = &[
    input("CD", Bool),
    input("LD", Bool),
    input("PV", Count),
    output("Q", Bool),
    output("CV", Count),
];
static CTUD_PARAMETERS: &[FbParameter] = &[
    input("CU", Bool),
    input("CD", Bool),
    input("R", Bool),
    input("LD", Bool),
    input("PV", Count),
    output("QU", Bool),
    output("QD", Bool),
    output("CV", Count),
];
static TIMER_PARAMETERS: &[FbParameter] = &[
    input("IN", Bool),
    input("PT", Duration),
    output("Q", Bool),
    output("ET", Duration),
];
