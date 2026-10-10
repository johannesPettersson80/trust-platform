use alloc::format;

use smol_str::SmolStr;

use crate::error::{RuntimeError, StableErrorCode};

/// Failure raised by the shared dispatcher or a hosted execution adapter.
#[derive(Debug)]
pub enum VmTrap {
    /// Unknown instruction byte.
    InvalidOpcode(u8),
    /// Branch destination is outside the current body or not an instruction boundary.
    InvalidJumpTarget(i64),
    /// Reference-table index does not exist.
    InvalidRefIndex(u32),
    /// Constant-pool index does not exist.
    InvalidConstIndex(u32),
    /// Reference is outside the active frame local range.
    InvalidLocalRef {
        /// Requested reference-table index.
        ref_index: u32,
        /// First reference index owned by the frame.
        start: u32,
        /// Number of references owned by the frame.
        count: u32,
    },
    /// Instruction requires more operands than are available.
    StackUnderflow,
    /// Operand stack exceeds its admitted limit.
    StackOverflow,
    /// Execution has no active call frame.
    CallStackUnderflow,
    /// Nested execution exceeds the admitted call depth.
    CallStackOverflow,
    /// Known instruction has no execution implementation.
    UnsupportedOpcode(&'static str),
    /// Reference location is unavailable in this execution composition.
    UnsupportedRefLocation(&'static str),
    /// Branch condition is not a BOOL value.
    ConditionNotBool,
    /// A dereferenced value or storage location does not exist.
    NullReference,
    /// Physical execution deadline has elapsed.
    DeadlineExceeded,
    /// Shared logical execution work allowance is exhausted.
    BudgetExceeded,
    /// FOR loop step is zero.
    ForStepZero,
    /// Requested POU identity is absent.
    MissingPou(u32),
    /// Named program is absent.
    MissingProgram(SmolStr),
    /// Named function block is absent.
    MissingFunctionBlock(SmolStr),
    /// Native call kind is not supported.
    InvalidNativeCallKind(u32),
    /// Native symbol descriptor index is invalid.
    InvalidNativeSymbolIndex(u32),
    /// Native call payload violates its contract.
    InvalidNativeCall(SmolStr),
    /// Execution metadata could not be decoded.
    BytecodeDecode(SmolStr),
    /// Propagate a typed runtime failure without changing its stable identity.
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

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::VmTrap;
    use crate::error::{RuntimeError, StableErrorCode};

    #[test]
    fn vm_traps_preserve_stable_codes_across_runtime_conversion() {
        let cases = vec![
            (
                VmTrap::InvalidOpcode(0xff),
                StableErrorCode::BytecodeInvalidOpcode,
            ),
            (
                VmTrap::InvalidJumpTarget(17),
                StableErrorCode::BytecodeInvalidJumpTarget,
            ),
            (
                VmTrap::InvalidRefIndex(3),
                StableErrorCode::BytecodeInvalidIndex,
            ),
            (
                VmTrap::InvalidConstIndex(4),
                StableErrorCode::BytecodeInvalidIndex,
            ),
            (
                VmTrap::InvalidLocalRef {
                    ref_index: 2,
                    start: 8,
                    count: 3,
                },
                StableErrorCode::BytecodeInvalidIndex,
            ),
            (VmTrap::StackUnderflow, StableErrorCode::VmStackUnderflow),
            (VmTrap::StackOverflow, StableErrorCode::VmStackOverflow),
            (
                VmTrap::CallStackUnderflow,
                StableErrorCode::VmCallStackUnderflow,
            ),
            (
                VmTrap::CallStackOverflow,
                StableErrorCode::VmCallStackOverflow,
            ),
            (
                VmTrap::UnsupportedOpcode("reserved"),
                StableErrorCode::VmUnsupportedOpcode,
            ),
            (
                VmTrap::UnsupportedRefLocation("external"),
                StableErrorCode::VmUnsupportedReferenceLocation,
            ),
            (
                VmTrap::ConditionNotBool,
                StableErrorCode::RuntimeConditionNotBool,
            ),
            (VmTrap::NullReference, StableErrorCode::RuntimeNullReference),
            (
                VmTrap::DeadlineExceeded,
                StableErrorCode::RuntimeExecutionTimeout,
            ),
            (
                VmTrap::BudgetExceeded,
                StableErrorCode::RuntimeExecutionTimeout,
            ),
            (VmTrap::ForStepZero, StableErrorCode::RuntimeForStepZero),
            (VmTrap::MissingPou(9), StableErrorCode::BytecodeInvalidPouId),
            (
                VmTrap::MissingProgram("missing".into()),
                StableErrorCode::RuntimeUndefinedProgram,
            ),
            (
                VmTrap::MissingFunctionBlock("missing".into()),
                StableErrorCode::RuntimeUndefinedFunctionBlock,
            ),
            (
                VmTrap::InvalidNativeCallKind(7),
                StableErrorCode::VmInvalidNativeCall,
            ),
            (
                VmTrap::InvalidNativeSymbolIndex(8),
                StableErrorCode::VmInvalidNativeCall,
            ),
            (
                VmTrap::InvalidNativeCall("invalid".into()),
                StableErrorCode::VmInvalidNativeCall,
            ),
            (
                VmTrap::BytecodeDecode("invalid".into()),
                StableErrorCode::VmBytecodeDecode,
            ),
            (
                VmTrap::Runtime(RuntimeError::TypeMismatch),
                StableErrorCode::RuntimeTypeMismatch,
            ),
        ];

        for (trap, expected) in cases {
            assert_eq!(trap.stable_code(), expected);
            assert_eq!(trap.into_runtime_error().stable_code(), expected);
        }
    }
}
