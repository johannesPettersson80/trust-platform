//! Runtime errors and configuration.

#![allow(missing_docs)]

use alloc::string::ToString;
use smol_str::SmolStr;
use thiserror::Error;

use crate::datetime::DateTimeCalcError;
use crate::value::DateTimeError;

pub use crate::error_code::StableErrorCode;

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

    /// Type mismatch between values.
    #[error("type mismatch")]
    TypeMismatch,

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
            Self::TypeMismatch => StableErrorCode::RuntimeTypeMismatch,
            Self::InvalidArgumentCount { .. } => StableErrorCode::RuntimeInvalidArgumentCount,
            Self::InvalidArgumentName(_) => StableErrorCode::RuntimeInvalidArgumentName,
            Self::AssertionFailed(_) => StableErrorCode::RuntimeAssertionFailed,
            Self::DivisionByZero => StableErrorCode::RuntimeDivisionByZero,
            Self::ModuloByZero => StableErrorCode::RuntimeModuloByZero,
            Self::Overflow => StableErrorCode::RuntimeOverflow,
            Self::IndexOutOfBounds { .. } => StableErrorCode::RuntimeIndexOutOfBounds,
            Self::SubrangeViolation { .. } => StableErrorCode::RuntimeSubrangeViolation,
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
            Self::ThreadSpawn(_) => StableErrorCode::RuntimeThreadSpawn,
            Self::WatchdogTimeout => StableErrorCode::RuntimeWatchdogTimeout,
            Self::RestartLimitExceeded { .. } => StableErrorCode::RuntimeRestartLimitExceeded,
            Self::SafeStateFailed { .. } => StableErrorCode::RuntimeSafeStateFailed,
            Self::ExecutionTimeout => StableErrorCode::RuntimeExecutionTimeout,
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
    fn from(error: crate::bytecode::BytecodeError) -> Self {
        Self::bytecode(error.stable_code(), error.to_string())
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
