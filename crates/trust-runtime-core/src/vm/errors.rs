use alloc::format;

use smol_str::SmolStr;

use crate::error::RuntimeError;

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
    /// Convert the VM trap into the public runtime error contract.
    pub fn into_runtime_error(self) -> RuntimeError {
        match self {
            Self::ConditionNotBool => RuntimeError::ConditionNotBool,
            Self::NullReference => RuntimeError::NullReference,
            Self::ForStepZero => RuntimeError::ForStepZero,
            Self::MissingPou(pou_id) => {
                RuntimeError::InvalidBytecode(format!("vm missing pou id {pou_id}").into())
            }
            Self::MissingProgram(name) => RuntimeError::UndefinedProgram(name),
            Self::MissingFunctionBlock(name) => RuntimeError::UndefinedFunctionBlock(name),
            Self::DeadlineExceeded | Self::BudgetExceeded => RuntimeError::ExecutionTimeout,
            Self::InvalidNativeCallKind(kind) => {
                RuntimeError::InvalidBytecode(format!("vm invalid CALL_NATIVE kind {kind}").into())
            }
            Self::InvalidNativeSymbolIndex(idx) => RuntimeError::InvalidBytecode(
                format!("vm invalid index {idx} for native symbol").into(),
            ),
            Self::InvalidNativeCall(message) => RuntimeError::InvalidBytecode(
                format!("vm invalid CALL_NATIVE payload: {message}").into(),
            ),
            Self::Runtime(err) => err,
            Self::InvalidOpcode(opcode) => {
                RuntimeError::InvalidBytecode(format!("vm invalid opcode 0x{opcode:02X}").into())
            }
            Self::InvalidJumpTarget(target) => {
                RuntimeError::InvalidBytecode(format!("vm invalid jump target {target}").into())
            }
            Self::InvalidRefIndex(idx) => {
                RuntimeError::InvalidBytecode(format!("vm invalid ref index {idx}").into())
            }
            Self::InvalidConstIndex(idx) => {
                RuntimeError::InvalidBytecode(format!("vm invalid const index {idx}").into())
            }
            Self::InvalidLocalRef {
                ref_index,
                start,
                count,
            } => RuntimeError::InvalidBytecode(
                format!(
                    "vm invalid local ref {ref_index} (frame local range {start}..{})",
                    start.saturating_add(count)
                )
                .into(),
            ),
            Self::StackUnderflow => {
                RuntimeError::InvalidBytecode("vm operand stack underflow".into())
            }
            Self::StackOverflow => {
                RuntimeError::InvalidBytecode("vm operand stack overflow".into())
            }
            Self::CallStackUnderflow => {
                RuntimeError::InvalidBytecode("vm call stack underflow".into())
            }
            Self::CallStackOverflow => {
                RuntimeError::InvalidBytecode("vm call stack overflow".into())
            }
            Self::UnsupportedOpcode(name) => {
                RuntimeError::InvalidBytecode(format!("vm unsupported opcode {name}").into())
            }
            Self::UnsupportedRefLocation(name) => {
                RuntimeError::InvalidBytecode(format!("vm unsupported ref location {name}").into())
            }
            Self::BytecodeDecode(message) => RuntimeError::InvalidBytecode(message),
        }
    }
}

impl From<RuntimeError> for VmTrap {
    fn from(value: RuntimeError) -> Self {
        Self::Runtime(value)
    }
}
