use alloc::format;

use smol_str::SmolStr;

use crate::error::{RuntimeError, StableErrorCode};

#[derive(Debug)]
pub enum VmTrap {
    InvalidOpcode(u8),
    InvalidJumpTarget(i64),
    InvalidRefIndex(u32),
    InvalidConstIndex(u32),
    InvalidLocalRef {
        ref_index: u32,
        start: u32,
        count: u32,
    },
    StackUnderflow,
    StackOverflow,
    CallStackUnderflow,
    CallStackOverflow,
    UnsupportedOpcode(&'static str),
    UnsupportedRefLocation(&'static str),
    ConditionNotBool,
    NullReference,
    DeadlineExceeded,
    BudgetExceeded,
    ForStepZero,
    MissingPou(u32),
    MissingProgram(SmolStr),
    MissingFunctionBlock(SmolStr),
    InvalidNativeCallKind(u32),
    InvalidNativeSymbolIndex(u32),
    InvalidNativeCall(SmolStr),
    BytecodeDecode(SmolStr),
    Runtime(RuntimeError),
}

impl VmTrap {
    /// Return the stable identifier that survives conversion to `RuntimeError`.
    #[must_use]
    pub const fn stable_code(&self) -> StableErrorCode {
        match self {
            Self::InvalidOpcode(_) => StableErrorCode::BytecodeInvalidOpcode,
            Self::InvalidJumpTarget(_) => StableErrorCode::BytecodeInvalidJumpTarget,
            Self::InvalidRefIndex(_)
            | Self::InvalidConstIndex(_)
            | Self::InvalidLocalRef { .. } => StableErrorCode::BytecodeInvalidIndex,
            Self::StackUnderflow => StableErrorCode::VmStackUnderflow,
            Self::StackOverflow => StableErrorCode::VmStackOverflow,
            Self::CallStackUnderflow => StableErrorCode::VmCallStackUnderflow,
            Self::CallStackOverflow => StableErrorCode::VmCallStackOverflow,
            Self::UnsupportedOpcode(_) => StableErrorCode::VmUnsupportedOpcode,
            Self::UnsupportedRefLocation(_) => StableErrorCode::VmUnsupportedReferenceLocation,
            Self::ConditionNotBool => StableErrorCode::RuntimeConditionNotBool,
            Self::NullReference => StableErrorCode::RuntimeNullReference,
            Self::DeadlineExceeded | Self::BudgetExceeded => {
                StableErrorCode::RuntimeExecutionTimeout
            }
            Self::ForStepZero => StableErrorCode::RuntimeForStepZero,
            Self::MissingPou(_) => StableErrorCode::BytecodeInvalidPouId,
            Self::MissingProgram(_) => StableErrorCode::RuntimeUndefinedProgram,
            Self::MissingFunctionBlock(_) => StableErrorCode::RuntimeUndefinedFunctionBlock,
            Self::InvalidNativeCallKind(_)
            | Self::InvalidNativeSymbolIndex(_)
            | Self::InvalidNativeCall(_) => StableErrorCode::VmInvalidNativeCall,
            Self::BytecodeDecode(_) => StableErrorCode::VmBytecodeDecode,
            Self::Runtime(error) => error.stable_code(),
        }
    }

    /// Convert the VM trap into the public runtime error contract.
    pub fn into_runtime_error(self) -> RuntimeError {
        let code = self.stable_code();
        match self {
            Self::ConditionNotBool => RuntimeError::ConditionNotBool,
            Self::NullReference => RuntimeError::NullReference,
            Self::ForStepZero => RuntimeError::ForStepZero,
            Self::MissingPou(pou_id) => {
                RuntimeError::bytecode(code, format!("vm missing pou id {pou_id}"))
            }
            Self::MissingProgram(name) => RuntimeError::UndefinedProgram(name),
            Self::MissingFunctionBlock(name) => RuntimeError::UndefinedFunctionBlock(name),
            Self::DeadlineExceeded | Self::BudgetExceeded => RuntimeError::ExecutionTimeout,
            Self::InvalidNativeCallKind(kind) => {
                RuntimeError::bytecode(code, format!("vm invalid CALL_NATIVE kind {kind}"))
            }
            Self::InvalidNativeSymbolIndex(idx) => {
                RuntimeError::bytecode(code, format!("vm invalid index {idx} for native symbol"))
            }
            Self::InvalidNativeCall(message) => {
                RuntimeError::bytecode(code, format!("vm invalid CALL_NATIVE payload: {message}"))
            }
            Self::Runtime(err) => err,
            Self::InvalidOpcode(opcode) => {
                RuntimeError::bytecode(code, format!("vm invalid opcode 0x{opcode:02X}"))
            }
            Self::InvalidJumpTarget(target) => {
                RuntimeError::bytecode(code, format!("vm invalid jump target {target}"))
            }
            Self::InvalidRefIndex(idx) => {
                RuntimeError::bytecode(code, format!("vm invalid ref index {idx}"))
            }
            Self::InvalidConstIndex(idx) => {
                RuntimeError::bytecode(code, format!("vm invalid const index {idx}"))
            }
            Self::InvalidLocalRef {
                ref_index,
                start,
                count,
            } => RuntimeError::bytecode(
                code,
                format!(
                    "vm invalid local ref {ref_index} (frame local range {start}..{})",
                    start.saturating_add(count)
                ),
            ),
            Self::StackUnderflow => RuntimeError::bytecode(code, "vm operand stack underflow"),
            Self::StackOverflow => RuntimeError::bytecode(code, "vm operand stack overflow"),
            Self::CallStackUnderflow => RuntimeError::bytecode(code, "vm call stack underflow"),
            Self::CallStackOverflow => RuntimeError::bytecode(code, "vm call stack overflow"),
            Self::UnsupportedOpcode(name) => {
                RuntimeError::bytecode(code, format!("vm unsupported opcode {name}"))
            }
            Self::UnsupportedRefLocation(name) => {
                RuntimeError::bytecode(code, format!("vm unsupported ref location {name}"))
            }
            Self::BytecodeDecode(message) => RuntimeError::bytecode(code, message),
        }
    }
}

impl From<RuntimeError> for VmTrap {
    fn from(value: RuntimeError) -> Self {
        Self::Runtime(value)
    }
}
