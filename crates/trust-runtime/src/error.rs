//! Runtime errors and configuration.

#![allow(missing_docs)]

use smol_str::SmolStr;
use thiserror::Error;

use crate::datetime::DateTimeCalcError;
use crate::value::DateTimeError;

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

    /// I/O driver error.
    #[error("i/o driver error '{0}'")]
    IoDriver(SmolStr),

    /// Unsupported bytecode version.
    #[error("unsupported bytecode version {major}.{minor}")]
    UnsupportedBytecodeVersion { major: u16, minor: u16 },

    /// Invalid or incomplete bytecode metadata.
    #[error("invalid bytecode metadata '{0}'")]
    InvalidBytecodeMetadata(SmolStr),

    /// Invalid bytecode container.
    #[error("invalid bytecode '{0}'")]
    InvalidBytecode(SmolStr),

    /// Thread spawn error.
    #[error("thread spawn error '{0}'")]
    ThreadSpawn(SmolStr),

    /// Watchdog timeout.
    #[error("watchdog timeout")]
    WatchdogTimeout,

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

    /// Control protocol error.
    #[error("control error '{0}'")]
    ControlError(SmolStr),
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
