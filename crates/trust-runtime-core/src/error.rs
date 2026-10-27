//! Runtime errors and configuration.

#![allow(missing_docs)]

use smol_str::SmolStr;
use thiserror::Error;

use crate::datetime::DateTimeCalcError;
use crate::value::DateTimeError;

pub use crate::error_code::StableErrorCode;

mod execution;
pub use execution::ExecutionLimitKind;
mod import;
pub use import::ImportRefusal;
mod vm_fault;
pub use vm_fault::{RuntimeText, VmFaultDetail};

/// Runtime errors for evaluation and execution.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuntimeError {
    /// Undefined variable or name.
    #[error("undefined variable '{0}'")]
    UndefinedVariable(SmolStr),

    /// Undefined function by name.
    #[error("undefined function '{0}'")]
    UndefinedFunction(SmolStr),

    /// Undefined program by name.
    #[error("undefined program '{0}'")]
    UndefinedProgram(SmolStr),

    /// Undefined function block by name.
    #[error("undefined function block '{0}'")]
    UndefinedFunctionBlock(SmolStr),

    /// Undefined field name.
    #[error("undefined field '{0}'")]
    UndefinedField(SmolStr),

    /// Invalid I/O address syntax.
    #[error("invalid I/O address '{0}'")]
    InvalidIoAddress(SmolStr),

    /// Reference outlived its activation or instance.
    #[error("reference outlived its activation or instance")]
    ReferenceLifetime,

    /// Hosted context for a rejected read-only alias, sharing the alias fault code.
    #[error("VAR_ACCESS '{0}' is read-only")]
    ReadOnlyAlias(SmolStr),

    /// Admitted execution metadata is inconsistent.
    #[error("admitted execution metadata is inconsistent")]
    InvalidExecutionState,

    /// A valid artifact requires a capability absent from the selected profile.
    #[error("unsupported execution profile: {0}")]
    ProfileUnsupported(SmolStr),

    /// An import has no built-in contract or does not match it (spec 34 §5.2.1).
    #[error("unsupported import '{import}': {reason}")]
    ImportUnsupported {
        /// Canonical uppercase name of the refused import.
        import: SmolStr,
        /// The part of the contract the import fails.
        reason: ImportRefusal,
    },

    /// A deployment record is not readable (spec 34 §5.1.1).
    #[error("invalid deployment record: {0}")]
    DeploymentRecordInvalid(crate::deployment::DeploymentRecordError),

    /// A device refused a deployment before constructing any state (spec 34 §5.1.1,
    /// DEPLOY-04).
    #[error("deployment refused: {0}")]
    DeploymentRefused(crate::deployment::AdmissionRefusal),

    /// Type mismatch between values.
    #[error("type mismatch")]
    TypeMismatch,

    /// A checked engineering boundary received a non-finite floating value.
    #[error("non-finite floating-point value")]
    NonFiniteValue,

    /// Invalid argument count for a function call.
    #[error("invalid argument count (expected {expected}, got {got})")]
    InvalidArgumentCount { expected: usize, got: usize },

    /// Invalid argument name for a call.
    #[error("invalid argument name '{0}'")]
    InvalidArgumentName(SmolStr),

    /// Assertion failure in ST test execution.
    #[error("assertion failed: {0}")]
    AssertionFailed(SmolStr),

    /// Division by zero.
    #[error("division by zero")]
    DivisionByZero,

    /// Modulo by zero.
    #[error("modulo by zero")]
    ModuloByZero,

    /// Arithmetic overflow.
    #[error("arithmetic overflow")]
    Overflow,

    /// A string function result is longer than the implementer maximum of 65,535 UTF-8 bytes
    /// (`STRING`) or UTF-16 code units (`WSTRING`), IEC 61131-3 Ed.3 §6.6.2.5.11.
    #[error("string result longer than the maximum of 65535")]
    StringCapacityExceeded,

    /// Index out of bounds.
    #[error("array index {index} out of bounds [{lower}..{upper}]")]
    IndexOutOfBounds { index: i64, lower: i64, upper: i64 },

    /// Value outside a declared subrange.
    #[error("value {value} outside declared subrange {lower}..{upper}")]
    SubrangeViolation {
        /// Actual runtime value.
        value: i128,
        /// Inclusive lower bound.
        lower: i128,
        /// Inclusive upper bound.
        upper: i128,
    },

    /// Null reference dereference.
    #[error("null reference dereference")]
    NullReference,

    /// FOR loop step cannot be zero.
    #[error("FOR loop step cannot be zero")]
    ForStepZero,

    /// Date/time value out of range.
    #[error("date/time out of range")]
    DateTimeRange(DateTimeError),

    /// Invalid frame id for debug evaluation.
    #[error("invalid frame id {0}")]
    InvalidFrame(u32),

    /// Resource is faulted and cannot execute.
    #[error("resource faulted")]
    ResourceFaulted,

    /// Resource cycle panicked inside scheduler-managed execution.
    #[error("resource panic '{0}'")]
    ResourcePanic(SmolStr),

    /// I/O driver error.
    #[error("i/o driver error '{0}'")]
    IoDriver(SmolStr),

    /// I/O transport/session error.
    #[error("i/o transport error '{0}'")]
    IoTransport(SmolStr),

    /// I/O protocol/address error.
    #[error("i/o address error '{0}'")]
    IoAddress(SmolStr),

    /// I/O input freshness error.
    #[error("i/o freshness error '{0}'")]
    IoFreshness(SmolStr),

    /// Runtime initialization failed for a variable, parameter, or instance member.
    #[error("init failed for {owner}.{variable}: {error}")]
    InitFailed {
        /// Owning POU, instance, or static scope.
        owner: SmolStr,
        /// Variable/member that failed to initialize.
        variable: SmolStr,
        /// Root initialization error.
        error: SmolStr,
    },

    /// A VM fault during a scan with bounded, allocation-free context (specification 34
    /// MEM-05). Its stable code is unchanged from the bytecode fault it replaces; it reads as
    /// [`VmFaultDetail::runtime_text`] says.
    #[error("{}", .0.runtime_text())]
    VmFault(VmFaultDetail),

    /// Thread spawn error.
    #[error("thread spawn error '{0}'")]
    ThreadSpawn(SmolStr),

    /// Watchdog timeout.
    #[error("watchdog timeout")]
    WatchdogTimeout,

    /// Automatic restart policy exhausted its bounded retry budget.
    #[error("automatic restart limit exceeded after {attempts} attempts: {reason}")]
    RestartLimitExceeded {
        /// Automatic restart attempts made before escalation.
        attempts: u32,
        /// Root fault that kept triggering restart.
        reason: SmolStr,
    },

    /// Safe-state output application failed while handling a root fault.
    #[error("safe-state failed after '{root}': {error}")]
    SafeStateFailed {
        /// Original runtime fault being handled.
        root: SmolStr,
        /// Safe-state write/application failure.
        error: SmolStr,
    },

    /// Script/test execution exceeded the configured time budget.
    #[error("execution timed out")]
    ExecutionTimeout,

    /// A logical execution allowance refused a charge before consuming it.
    /// The stable timeout identity is retained; this context separates it from
    /// physical deadline expiry without allocating a diagnostic string.
    #[error("execution timed out: {kind} limit {limit}, used {used}, requested {requested}")]
    ExecutionLimit {
        /// Allowance exhausted by the first rejected operation.
        kind: ExecutionLimitKind,
        /// Accepted demand before the rejected operation.
        used: usize,
        /// Complete demand of the rejected operation.
        requested: usize,
        /// Admitted maximum for this operation entry.
        limit: usize,
    },

    /// Scripted simulation fault injection.
    #[error("simulation fault '{0}'")]
    SimulationFault(SmolStr),

    /// Configuration error.
    #[error("invalid config '{0}'")]
    InvalidConfig(SmolStr),

    /// Runtime project folder error.
    #[error("invalid project folder '{0}'")]
    InvalidBundle(SmolStr),

    /// Retain storage error.
    #[error("retain store error '{0}'")]
    RetainStore(SmolStr),

    /// Retain data failed integrity validation.
    #[error("retain corruption '{0}'")]
    RetainCorruption(SmolStr),

    /// Retain schema migration failed or was applied explicitly.
    #[error("retain migration error '{0}'")]
    RetainMigration(SmolStr),

    /// Retained data violates a destination subrange; keep both context and cause.
    #[error("retain migration error 'invalid retained value for '{name}': value {value} outside declared subrange {lower}..{upper}'")]
    RetainSubrangeViolation {
        /// Logical checkpoint destination.
        name: SmolStr,
        /// Rejected numeric value.
        value: i128,
        /// Inclusive admitted lower bound.
        lower: i128,
        /// Inclusive admitted upper bound.
        upper: i128,
    },

    /// Control protocol error.
    #[error("control error '{0}'")]
    ControlError(SmolStr),
}

impl RuntimeError {
    /// The error of a call beyond the call levels a task's stack holds: a recursion deeper than
    /// the depth the kernel gives an activation (specification 44 §6.4, Appendix A). It reads
    /// `call depth exceeded`.
    #[must_use]
    pub fn call_depth_exceeded() -> Self {
        Self::VmFault(VmFaultDetail {
            code: StableErrorCode::VmCallStackOverflow,
            detail: crate::fault::FaultDetail::Text("vm call stack overflow"),
        })
    }

    /// Return the stable machine identifier for this runtime failure.
    #[must_use]
    pub const fn stable_code(&self) -> StableErrorCode {
        match self {
            Self::UndefinedVariable(_) => StableErrorCode::RuntimeUndefinedVariable,
            Self::UndefinedFunction(_) => StableErrorCode::RuntimeUndefinedFunction,
            Self::UndefinedProgram(_) => StableErrorCode::RuntimeUndefinedProgram,
            Self::UndefinedFunctionBlock(_) => StableErrorCode::RuntimeUndefinedFunctionBlock,
            Self::UndefinedField(_) => StableErrorCode::RuntimeUndefinedField,
            Self::InvalidIoAddress(_) => StableErrorCode::RuntimeInvalidIoAddress,
            Self::ReferenceLifetime => StableErrorCode::RuntimeReferenceLifetime,
            Self::ReadOnlyAlias(_) => StableErrorCode::RuntimeInvalidAlias,
            Self::InvalidExecutionState => StableErrorCode::RuntimeInvalidExecutionState,
            Self::ProfileUnsupported(_) => StableErrorCode::RuntimeProfileUnsupported,
            Self::ImportUnsupported { .. } => StableErrorCode::RuntimeImportUnsupported,
            Self::DeploymentRecordInvalid(_) => StableErrorCode::RuntimeDeploymentRecordInvalid,
            Self::DeploymentRefused(_) => StableErrorCode::RuntimeDeploymentRefused,
            Self::TypeMismatch => StableErrorCode::RuntimeTypeMismatch,
            Self::NonFiniteValue => StableErrorCode::RuntimeNonFiniteValue,
            Self::InvalidArgumentCount { .. } => StableErrorCode::RuntimeInvalidArgumentCount,
            Self::InvalidArgumentName(_) => StableErrorCode::RuntimeInvalidArgumentName,
            Self::AssertionFailed(_) => StableErrorCode::RuntimeAssertionFailed,
            Self::DivisionByZero => StableErrorCode::RuntimeDivisionByZero,
            Self::ModuloByZero => StableErrorCode::RuntimeModuloByZero,
            Self::Overflow => StableErrorCode::RuntimeOverflow,
            Self::StringCapacityExceeded => StableErrorCode::RuntimeStringCapacityExceeded,
            Self::IndexOutOfBounds { .. } => StableErrorCode::RuntimeIndexOutOfBounds,
            Self::SubrangeViolation { .. } | Self::RetainSubrangeViolation { .. } => {
                StableErrorCode::RuntimeSubrangeViolation
            }
            Self::NullReference => StableErrorCode::RuntimeNullReference,
            Self::ForStepZero => StableErrorCode::RuntimeForStepZero,
            Self::DateTimeRange(_) => StableErrorCode::RuntimeDateTimeRange,
            Self::InvalidFrame(_) => StableErrorCode::RuntimeInvalidFrame,
            Self::ResourceFaulted => StableErrorCode::RuntimeResourceFaulted,
            Self::ResourcePanic(_) => StableErrorCode::RuntimeResourcePanic,
            Self::IoDriver(_) => StableErrorCode::RuntimeIoDriver,
            Self::IoTransport(_) => StableErrorCode::RuntimeIoTransport,
            Self::IoAddress(_) => StableErrorCode::RuntimeIoAddress,
            Self::IoFreshness(_) => StableErrorCode::RuntimeIoFreshness,
            Self::InitFailed { .. } => StableErrorCode::RuntimeInitFailed,
            Self::VmFault(fault) => fault.code,
            Self::ThreadSpawn(_) => StableErrorCode::RuntimeThreadSpawn,
            Self::WatchdogTimeout => StableErrorCode::RuntimeWatchdogTimeout,
            Self::RestartLimitExceeded { .. } => StableErrorCode::RuntimeRestartLimitExceeded,
            Self::SafeStateFailed { .. } => StableErrorCode::RuntimeSafeStateFailed,
            Self::ExecutionTimeout | Self::ExecutionLimit { .. } => {
                StableErrorCode::RuntimeExecutionTimeout
            }
            Self::SimulationFault(_) => StableErrorCode::RuntimeSimulationFault,
            Self::InvalidConfig(_) => StableErrorCode::RuntimeInvalidConfig,
            Self::InvalidBundle(_) => StableErrorCode::RuntimeInvalidBundle,
            Self::RetainStore(_) => StableErrorCode::RuntimeRetainStore,
            Self::RetainCorruption(_) => StableErrorCode::RuntimeRetainCorruption,
            Self::RetainMigration(_) => StableErrorCode::RuntimeRetainMigration,
            Self::ControlError(_) => StableErrorCode::RuntimeControlError,
        }
    }
}

impl From<crate::value::PartialAccessError> for RuntimeError {
    fn from(error: crate::value::PartialAccessError) -> Self {
        match error {
            crate::value::PartialAccessError::IndexOutOfBounds {
                index,
                lower,
                upper,
            } => Self::IndexOutOfBounds {
                index,
                lower,
                upper,
            },
            crate::value::PartialAccessError::TypeMismatch => Self::TypeMismatch,
        }
    }
}

impl From<DateTimeError> for RuntimeError {
    fn from(value: DateTimeError) -> Self {
        Self::DateTimeRange(value)
    }
}

impl From<DateTimeCalcError> for RuntimeError {
    fn from(_: DateTimeCalcError) -> Self {
        Self::Overflow
    }
}

#[cfg(test)]
mod tests {
    use alloc::{format, string::ToString};

    use super::{ExecutionLimitKind, RuntimeError, StableErrorCode};
    use crate::datetime::DateTimeCalcError;
    use crate::value::DateTimeError;

    #[test]
    fn logical_allowance_context_is_distinct_from_physical_expiry_with_stable_timeout_code() {
        let error = RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::AllocationBytes,
            used: 40,
            requested: 24,
            limit: 60,
        };
        assert_eq!(
            error.stable_code(),
            RuntimeError::ExecutionTimeout.stable_code()
        );
        assert_eq!(
            error.to_string(),
            "execution timed out: allocation bytes limit 60, used 40, requested 24"
        );
        assert_ne!(error, RuntimeError::ExecutionTimeout);
    }

    #[test]
    fn retained_subrange_context_preserves_the_range_code_and_operator_details() {
        let error = RuntimeError::RetainSubrangeViolation {
            name: "Plant.limit".into(),
            value: i128::MIN,
            lower: i128::MIN,
            upper: i128::MAX,
        };
        assert_eq!(
            error.stable_code(),
            StableErrorCode::RuntimeSubrangeViolation
        );
        let message = error.to_string();
        assert!(message.contains("Plant.limit"));
        assert_eq!(
            message,
            format!(
                "retain migration error 'invalid retained value for 'Plant.limit': value {} outside declared subrange {}..{}'",
                i128::MIN,
                i128::MIN,
                i128::MAX
            )
        );
        let upper_endpoint = RuntimeError::RetainSubrangeViolation {
            name: "Plant.limit".into(),
            value: i128::MAX,
            lower: i128::MIN,
            upper: i128::MAX,
        };
        assert!(upper_endpoint.to_string().contains(&format!(
            "value {} outside declared subrange {}..{}",
            i128::MAX,
            i128::MIN,
            i128::MAX
        )));
        assert_eq!(
            RuntimeError::NonFiniteValue.stable_code(),
            StableErrorCode::RuntimeNonFiniteValue
        );
    }

    #[test]
    fn read_only_alias_context_preserves_the_shared_fault_code() {
        let contextual = RuntimeError::ReadOnlyAlias("Observed".into());
        assert_eq!(
            contextual.stable_code(),
            StableErrorCode::RuntimeInvalidAlias
        );
        assert_eq!(contextual.to_string(), "VAR_ACCESS 'Observed' is read-only");
    }

    #[test]
    fn partial_access_conversion_preserves_bounds_and_fault_identity() {
        let bounds = RuntimeError::from(crate::value::PartialAccessError::IndexOutOfBounds {
            index: 8,
            lower: 0,
            upper: 7,
        });
        assert_eq!(
            bounds,
            RuntimeError::IndexOutOfBounds {
                index: 8,
                lower: 0,
                upper: 7
            }
        );
        assert_eq!(
            bounds.stable_code(),
            StableErrorCode::RuntimeIndexOutOfBounds
        );
        let mismatch = RuntimeError::from(crate::value::PartialAccessError::TypeMismatch);
        assert_eq!(mismatch, RuntimeError::TypeMismatch);
        assert_eq!(mismatch.stable_code(), StableErrorCode::RuntimeTypeMismatch);
    }

    #[test]
    fn protection_and_profile_faults_have_distinct_stable_codes() {
        let faults = [
            (
                RuntimeError::ReferenceLifetime,
                StableErrorCode::RuntimeReferenceLifetime,
            ),
            (
                RuntimeError::call_depth_exceeded(),
                StableErrorCode::VmCallStackOverflow,
            ),
            (
                RuntimeError::ReadOnlyAlias("Observed".into()),
                StableErrorCode::RuntimeInvalidAlias,
            ),
            (
                RuntimeError::InvalidExecutionState,
                StableErrorCode::RuntimeInvalidExecutionState,
            ),
            (
                RuntimeError::ProfileUnsupported("resource count".into()),
                StableErrorCode::RuntimeProfileUnsupported,
            ),
            (
                RuntimeError::ImportUnsupported {
                    import: "SQRTX".into(),
                    reason: super::ImportRefusal::NoContract,
                },
                StableErrorCode::RuntimeImportUnsupported,
            ),
        ];
        for (fault, expected) in faults {
            assert_eq!(fault.stable_code(), expected);
            assert_ne!(expected, StableErrorCode::RuntimeTypeMismatch);
            assert_ne!(expected, StableErrorCode::RuntimeNullReference);
        }
    }

    #[test]
    fn runtime_error_stable_codes_cover_every_committed_variant() {
        let cases = [
            (
                RuntimeError::UndefinedVariable("x".into()),
                StableErrorCode::RuntimeUndefinedVariable,
            ),
            (
                RuntimeError::UndefinedFunction("f".into()),
                StableErrorCode::RuntimeUndefinedFunction,
            ),
            (
                RuntimeError::UndefinedProgram("p".into()),
                StableErrorCode::RuntimeUndefinedProgram,
            ),
            (
                RuntimeError::UndefinedFunctionBlock("fb".into()),
                StableErrorCode::RuntimeUndefinedFunctionBlock,
            ),
            (
                RuntimeError::UndefinedField("field".into()),
                StableErrorCode::RuntimeUndefinedField,
            ),
            (
                RuntimeError::InvalidIoAddress("%IX0.0".into()),
                StableErrorCode::RuntimeInvalidIoAddress,
            ),
            (
                RuntimeError::TypeMismatch,
                StableErrorCode::RuntimeTypeMismatch,
            ),
            (
                RuntimeError::InvalidArgumentCount {
                    expected: 1,
                    got: 2,
                },
                StableErrorCode::RuntimeInvalidArgumentCount,
            ),
            (
                RuntimeError::InvalidArgumentName("arg".into()),
                StableErrorCode::RuntimeInvalidArgumentName,
            ),
            (
                RuntimeError::AssertionFailed("assertion".into()),
                StableErrorCode::RuntimeAssertionFailed,
            ),
            (
                RuntimeError::DivisionByZero,
                StableErrorCode::RuntimeDivisionByZero,
            ),
            (
                RuntimeError::ModuloByZero,
                StableErrorCode::RuntimeModuloByZero,
            ),
            (RuntimeError::Overflow, StableErrorCode::RuntimeOverflow),
            (
                RuntimeError::StringCapacityExceeded,
                StableErrorCode::RuntimeStringCapacityExceeded,
            ),
            (
                RuntimeError::IndexOutOfBounds {
                    index: 2,
                    lower: 0,
                    upper: 1,
                },
                StableErrorCode::RuntimeIndexOutOfBounds,
            ),
            (
                RuntimeError::SubrangeViolation {
                    value: 2,
                    lower: 0,
                    upper: 1,
                },
                StableErrorCode::RuntimeSubrangeViolation,
            ),
            (
                RuntimeError::NullReference,
                StableErrorCode::RuntimeNullReference,
            ),
            (
                RuntimeError::ForStepZero,
                StableErrorCode::RuntimeForStepZero,
            ),
            (
                RuntimeError::DateTimeRange(DateTimeError::OutOfRange),
                StableErrorCode::RuntimeDateTimeRange,
            ),
            (
                RuntimeError::InvalidFrame(1),
                StableErrorCode::RuntimeInvalidFrame,
            ),
            (
                RuntimeError::ResourceFaulted,
                StableErrorCode::RuntimeResourceFaulted,
            ),
            (
                RuntimeError::ResourcePanic("panic".into()),
                StableErrorCode::RuntimeResourcePanic,
            ),
            (
                RuntimeError::IoDriver("driver".into()),
                StableErrorCode::RuntimeIoDriver,
            ),
            (
                RuntimeError::IoTransport("transport".into()),
                StableErrorCode::RuntimeIoTransport,
            ),
            (
                RuntimeError::IoAddress("address".into()),
                StableErrorCode::RuntimeIoAddress,
            ),
            (
                RuntimeError::IoFreshness("stale".into()),
                StableErrorCode::RuntimeIoFreshness,
            ),
            (
                RuntimeError::InitFailed {
                    owner: "owner".into(),
                    variable: "variable".into(),
                    error: "error".into(),
                },
                StableErrorCode::RuntimeInitFailed,
            ),
            (
                RuntimeError::ThreadSpawn("spawn".into()),
                StableErrorCode::RuntimeThreadSpawn,
            ),
            (
                RuntimeError::WatchdogTimeout,
                StableErrorCode::RuntimeWatchdogTimeout,
            ),
            (
                RuntimeError::RestartLimitExceeded {
                    attempts: 3,
                    reason: "fault".into(),
                },
                StableErrorCode::RuntimeRestartLimitExceeded,
            ),
            (
                RuntimeError::SafeStateFailed {
                    root: "fault".into(),
                    error: "write".into(),
                },
                StableErrorCode::RuntimeSafeStateFailed,
            ),
            (
                RuntimeError::ExecutionTimeout,
                StableErrorCode::RuntimeExecutionTimeout,
            ),
            (
                RuntimeError::SimulationFault("fault".into()),
                StableErrorCode::RuntimeSimulationFault,
            ),
            (
                RuntimeError::InvalidConfig("config".into()),
                StableErrorCode::RuntimeInvalidConfig,
            ),
            (
                RuntimeError::InvalidBundle("bundle".into()),
                StableErrorCode::RuntimeInvalidBundle,
            ),
            (
                RuntimeError::RetainStore("store".into()),
                StableErrorCode::RuntimeRetainStore,
            ),
            (
                RuntimeError::RetainCorruption("corrupt".into()),
                StableErrorCode::RuntimeRetainCorruption,
            ),
            (
                RuntimeError::RetainMigration("migration".into()),
                StableErrorCode::RuntimeRetainMigration,
            ),
            (
                RuntimeError::ControlError("control".into()),
                StableErrorCode::RuntimeControlError,
            ),
            (
                RuntimeError::ImportUnsupported {
                    import: "TON".into(),
                    reason: super::ImportRefusal::ParameterType,
                },
                StableErrorCode::RuntimeImportUnsupported,
            ),
            (
                RuntimeError::DeploymentRecordInvalid(
                    crate::deployment::DeploymentRecordError::Checksum,
                ),
                StableErrorCode::RuntimeDeploymentRecordInvalid,
            ),
            (
                RuntimeError::DeploymentRefused(
                    crate::deployment::AdmissionRefusal::ProgramMismatch,
                ),
                StableErrorCode::RuntimeDeploymentRefused,
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.stable_code(), expected, "{error:?}");
        }
    }

    #[test]
    fn runtime_error_conversions_preserve_committed_boundaries() {
        for source in [
            DateTimeError::OutOfRange,
            DateTimeError::TimezoneNotSupported,
        ] {
            let converted = RuntimeError::from(source);
            assert_eq!(converted, RuntimeError::DateTimeRange(source));
            assert_eq!(
                converted.stable_code(),
                StableErrorCode::RuntimeDateTimeRange
            );
        }

        for source in [
            DateTimeCalcError::InvalidDate,
            DateTimeCalcError::InvalidResolution,
            DateTimeCalcError::Overflow,
        ] {
            let converted = RuntimeError::from(source);
            assert_eq!(converted, RuntimeError::Overflow);
            assert_eq!(converted.stable_code(), StableErrorCode::RuntimeOverflow);
        }
    }
}
