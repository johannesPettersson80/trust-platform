//! Runtime errors and configuration.

#![allow(missing_docs)]

#[cfg(feature = "std")]
use alloc::string::ToString;
use smol_str::SmolStr;
use thiserror::Error;

use crate::datetime::DateTimeCalcError;
use crate::value::DateTimeError;

pub use crate::error_code::StableErrorCode;

mod preparation;
pub use preparation::PreparationDiagnostic;
mod execution;
pub use execution::ExecutionLimitKind;

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

    /// Undefined task by name.
    #[error("undefined task '{0}'")]
    UndefinedTask(SmolStr),

    /// Undefined label target.
    #[error("undefined label '{0}'")]
    UndefinedLabel(SmolStr),

    /// Undefined field name.
    #[error("undefined field '{0}'")]
    UndefinedField(SmolStr),

    /// Invalid SINGLE input for a task.
    #[error("invalid task SINGLE input '{0}'")]
    InvalidTaskSingle(SmolStr),

    /// Invalid I/O address syntax.
    #[error("invalid I/O address '{0}'")]
    InvalidIoAddress(SmolStr),

    /// Write to a constant declaration.
    #[error("write to a constant declaration")]
    ConstantWrite,

    /// Write outside initializer staging storage.
    #[error("write outside initializer staging storage")]
    StagingViolation,

    /// Read outside the initializer visibility frontier.
    #[error("read outside the initializer visibility frontier")]
    VisibilityViolation,

    /// Reference outlived its activation or instance.
    #[error("reference outlived its activation or instance")]
    ReferenceLifetime,

    /// Program-root replacement is forbidden.
    #[error("program-root replacement is forbidden")]
    ProgramRootReplacement,

    /// Invalid storage alias use.
    #[error("invalid storage alias use")]
    InvalidAlias,

    /// Hosted context for a rejected read-only alias, sharing the alias fault code.
    #[error("VAR_ACCESS '{0}' is read-only")]
    ReadOnlyAlias(SmolStr),

    /// Preparation resource limit exceeded.
    #[error("preparation resource limit exceeded")]
    PreparationLimit,

    /// Admitted execution metadata is inconsistent.
    #[error("admitted execution metadata is inconsistent")]
    InvalidExecutionState,

    /// A valid artifact requires a capability absent from the selected profile.
    #[error("unsupported execution profile: {0}")]
    ProfileUnsupported(SmolStr),

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

    /// Invalid control flow (EXIT/CONTINUE outside loop).
    #[error("invalid control flow")]
    InvalidControlFlow,

    /// FOR loop step cannot be zero.
    #[error("FOR loop step cannot be zero")]
    ForStepZero,

    /// Condition is not BOOL.
    #[error("condition is not BOOL")]
    ConditionNotBool,

    /// CASE selector type not supported.
    #[error("case selector type not supported")]
    CaseSelectorType,

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

    /// Unsupported bytecode version.
    #[error("unsupported bytecode version {major}.{minor}")]
    UnsupportedBytecodeVersion { major: u16, minor: u16 },

    /// Invalid or incomplete bytecode metadata.
    #[error("invalid bytecode metadata '{0}'")]
    InvalidBytecodeMetadata(SmolStr),

    /// Invalid bytecode container.
    #[error("invalid bytecode '{0}'")]
    InvalidBytecode(SmolStr),

    /// Bytecode or VM structural failure with a preserved stable identifier.
    #[error("invalid bytecode '{detail}'")]
    Bytecode {
        /// Stable machine-readable failure identifier.
        code: StableErrorCode,
        /// Human-readable diagnostic detail.
        detail: SmolStr,
    },

    /// Structured portable decode/validation error, rendered only on request.
    #[error("invalid bytecode '{0}'")]
    BytecodeCause(crate::bytecode::BytecodeError),

    /// Structured portable execution-metadata rejection.
    #[error("invalid bytecode '{0}'")]
    PreparationDiagnostic(PreparationDiagnostic),

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
    /// Construct a bytecode/VM structural error without losing its stable code.
    pub fn bytecode(code: StableErrorCode, detail: impl Into<SmolStr>) -> Self {
        Self::Bytecode {
            code,
            detail: detail.into(),
        }
    }

    /// Return the stable machine identifier for this runtime failure.
    #[must_use]
    pub const fn stable_code(&self) -> StableErrorCode {
        match self {
            Self::UndefinedVariable(_) => StableErrorCode::RuntimeUndefinedVariable,
            Self::UndefinedFunction(_) => StableErrorCode::RuntimeUndefinedFunction,
            Self::UndefinedProgram(_) => StableErrorCode::RuntimeUndefinedProgram,
            Self::UndefinedFunctionBlock(_) => StableErrorCode::RuntimeUndefinedFunctionBlock,
            Self::UndefinedTask(_) => StableErrorCode::RuntimeUndefinedTask,
            Self::UndefinedLabel(_) => StableErrorCode::RuntimeUndefinedLabel,
            Self::UndefinedField(_) => StableErrorCode::RuntimeUndefinedField,
            Self::InvalidTaskSingle(_) => StableErrorCode::RuntimeInvalidTaskSingle,
            Self::InvalidIoAddress(_) => StableErrorCode::RuntimeInvalidIoAddress,
            Self::ConstantWrite => StableErrorCode::RuntimeConstantWrite,
            Self::StagingViolation => StableErrorCode::RuntimeStagingViolation,
            Self::VisibilityViolation => StableErrorCode::RuntimeVisibilityViolation,
            Self::ReferenceLifetime => StableErrorCode::RuntimeReferenceLifetime,
            Self::ProgramRootReplacement => StableErrorCode::RuntimeProgramRootReplacement,
            Self::InvalidAlias | Self::ReadOnlyAlias(_) => StableErrorCode::RuntimeInvalidAlias,
            Self::PreparationLimit => StableErrorCode::RuntimePreparationLimit,
            Self::InvalidExecutionState => StableErrorCode::RuntimeInvalidExecutionState,
            Self::ProfileUnsupported(_) => StableErrorCode::RuntimeProfileUnsupported,
            Self::TypeMismatch => StableErrorCode::RuntimeTypeMismatch,
            Self::NonFiniteValue => StableErrorCode::RuntimeNonFiniteValue,
            Self::InvalidArgumentCount { .. } => StableErrorCode::RuntimeInvalidArgumentCount,
            Self::InvalidArgumentName(_) => StableErrorCode::RuntimeInvalidArgumentName,
            Self::AssertionFailed(_) => StableErrorCode::RuntimeAssertionFailed,
            Self::DivisionByZero => StableErrorCode::RuntimeDivisionByZero,
            Self::ModuloByZero => StableErrorCode::RuntimeModuloByZero,
            Self::Overflow => StableErrorCode::RuntimeOverflow,
            Self::IndexOutOfBounds { .. } => StableErrorCode::RuntimeIndexOutOfBounds,
            Self::SubrangeViolation { .. } | Self::RetainSubrangeViolation { .. } => {
                StableErrorCode::RuntimeSubrangeViolation
            }
            Self::NullReference => StableErrorCode::RuntimeNullReference,
            Self::InvalidControlFlow => StableErrorCode::RuntimeInvalidControlFlow,
            Self::ForStepZero => StableErrorCode::RuntimeForStepZero,
            Self::ConditionNotBool => StableErrorCode::RuntimeConditionNotBool,
            Self::CaseSelectorType => StableErrorCode::RuntimeCaseSelectorType,
            Self::DateTimeRange(_) => StableErrorCode::RuntimeDateTimeRange,
            Self::InvalidFrame(_) => StableErrorCode::RuntimeInvalidFrame,
            Self::ResourceFaulted => StableErrorCode::RuntimeResourceFaulted,
            Self::ResourcePanic(_) => StableErrorCode::RuntimeResourcePanic,
            Self::IoDriver(_) => StableErrorCode::RuntimeIoDriver,
            Self::IoTransport(_) => StableErrorCode::RuntimeIoTransport,
            Self::IoAddress(_) => StableErrorCode::RuntimeIoAddress,
            Self::IoFreshness(_) => StableErrorCode::RuntimeIoFreshness,
            Self::InitFailed { .. } => StableErrorCode::RuntimeInitFailed,
            Self::UnsupportedBytecodeVersion { .. } => {
                StableErrorCode::RuntimeUnsupportedBytecodeVersion
            }
            Self::InvalidBytecodeMetadata(_) => StableErrorCode::RuntimeInvalidBytecodeMetadata,
            Self::InvalidBytecode(_) => StableErrorCode::RuntimeInvalidBytecode,
            Self::Bytecode { code, .. } => *code,
            Self::BytecodeCause(cause) => cause.stable_code(),
            Self::PreparationDiagnostic(_) => StableErrorCode::VmBytecodeDecode,
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

impl From<crate::bytecode::BytecodeError> for RuntimeError {
    #[cold]
    #[inline(never)]
    fn from(error: crate::bytecode::BytecodeError) -> Self {
        #[cfg(feature = "std")]
        {
            Self::bytecode(error.stable_code(), error.to_string())
        }
        #[cfg(not(feature = "std"))]
        {
            Self::BytecodeCause(error)
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
    use crate::bytecode::BytecodeError;
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
            RuntimeError::InvalidAlias.stable_code()
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
                RuntimeError::ConstantWrite,
                StableErrorCode::RuntimeConstantWrite,
            ),
            (
                RuntimeError::StagingViolation,
                StableErrorCode::RuntimeStagingViolation,
            ),
            (
                RuntimeError::VisibilityViolation,
                StableErrorCode::RuntimeVisibilityViolation,
            ),
            (
                RuntimeError::ReferenceLifetime,
                StableErrorCode::RuntimeReferenceLifetime,
            ),
            (
                RuntimeError::ProgramRootReplacement,
                StableErrorCode::RuntimeProgramRootReplacement,
            ),
            (
                RuntimeError::InvalidAlias,
                StableErrorCode::RuntimeInvalidAlias,
            ),
            (
                RuntimeError::ReadOnlyAlias("Observed".into()),
                StableErrorCode::RuntimeInvalidAlias,
            ),
            (
                RuntimeError::PreparationLimit,
                StableErrorCode::RuntimePreparationLimit,
            ),
            (
                RuntimeError::InvalidExecutionState,
                StableErrorCode::RuntimeInvalidExecutionState,
            ),
            (
                RuntimeError::ProfileUnsupported("resource count".into()),
                StableErrorCode::RuntimeProfileUnsupported,
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
                RuntimeError::UndefinedTask("task".into()),
                StableErrorCode::RuntimeUndefinedTask,
            ),
            (
                RuntimeError::UndefinedLabel("label".into()),
                StableErrorCode::RuntimeUndefinedLabel,
            ),
            (
                RuntimeError::UndefinedField("field".into()),
                StableErrorCode::RuntimeUndefinedField,
            ),
            (
                RuntimeError::InvalidTaskSingle("single".into()),
                StableErrorCode::RuntimeInvalidTaskSingle,
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
                RuntimeError::InvalidControlFlow,
                StableErrorCode::RuntimeInvalidControlFlow,
            ),
            (
                RuntimeError::ForStepZero,
                StableErrorCode::RuntimeForStepZero,
            ),
            (
                RuntimeError::ConditionNotBool,
                StableErrorCode::RuntimeConditionNotBool,
            ),
            (
                RuntimeError::CaseSelectorType,
                StableErrorCode::RuntimeCaseSelectorType,
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
                RuntimeError::UnsupportedBytecodeVersion { major: 2, minor: 0 },
                StableErrorCode::RuntimeUnsupportedBytecodeVersion,
            ),
            (
                RuntimeError::InvalidBytecodeMetadata("metadata".into()),
                StableErrorCode::RuntimeInvalidBytecodeMetadata,
            ),
            (
                RuntimeError::InvalidBytecode("container".into()),
                StableErrorCode::RuntimeInvalidBytecode,
            ),
            (
                RuntimeError::bytecode(
                    StableErrorCode::BytecodeInvalidMagic,
                    "invalid bytecode magic",
                ),
                StableErrorCode::BytecodeInvalidMagic,
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
        ];

        for (error, expected) in cases {
            assert_eq!(error.stable_code(), expected, "{error:?}");
        }
    }

    #[test]
    fn runtime_error_conversions_preserve_committed_boundaries() {
        let source = BytecodeError::InvalidHeader("section count".into());
        let source_detail = source.to_string();
        let converted = RuntimeError::from(source.clone());
        #[cfg(feature = "std")]
        assert_eq!(
            converted,
            RuntimeError::Bytecode {
                code: StableErrorCode::BytecodeInvalidHeader,
                detail: source_detail.clone().into(),
            }
        );
        #[cfg(not(feature = "std"))]
        assert_eq!(converted, RuntimeError::BytecodeCause(source));
        assert_eq!(
            converted.to_string(),
            alloc::format!("invalid bytecode '{source_detail}'")
        );
        assert_eq!(
            converted.stable_code(),
            StableErrorCode::BytecodeInvalidHeader
        );

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
