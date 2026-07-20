//! Stable machine-readable runtime error identifiers.

#![allow(missing_docs)]

use core::fmt;

/// Closed stable identifier vocabulary exposed at runtime boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StableErrorCode {
    BytecodeInvalidMagic,
    BytecodeUnsupportedVersion,
    BytecodeInvalidHeader,
    BytecodeInvalidChecksum,
    BytecodeInvalidSectionTable,
    BytecodeSectionOutOfBounds,
    BytecodeSectionOverlap,
    BytecodeSectionAlignment,
    BytecodeUnexpectedEof,
    BytecodeInvalidSection,
    BytecodeMissingSection,
    BytecodeInvalidOpcode,
    BytecodeInvalidJumpTarget,
    BytecodeInvalidPouId,
    BytecodeInvalidIndex,
    VmStackUnderflow,
    VmStackOverflow,
    VmCallStackUnderflow,
    VmCallStackOverflow,
    VmUnsupportedOpcode,
    VmUnsupportedReferenceLocation,
    VmInvalidNativeCall,
    VmBytecodeDecode,
    RuntimeUndefinedVariable,
    RuntimeUndefinedFunction,
    RuntimeUndefinedProgram,
    RuntimeUndefinedFunctionBlock,
    RuntimeUndefinedTask,
    RuntimeUndefinedLabel,
    RuntimeUndefinedField,
    RuntimeInvalidTaskSingle,
    RuntimeInvalidIoAddress,
    RuntimeTypeMismatch,
    RuntimeInvalidArgumentCount,
    RuntimeInvalidArgumentName,
    RuntimeAssertionFailed,
    RuntimeDivisionByZero,
    RuntimeModuloByZero,
    RuntimeOverflow,
    RuntimeIndexOutOfBounds,
    RuntimeSubrangeViolation,
    RuntimeNullReference,
    RuntimeInvalidControlFlow,
    RuntimeForStepZero,
    RuntimeConditionNotBool,
    RuntimeCaseSelectorType,
    RuntimeDateTimeRange,
    RuntimeInvalidFrame,
    RuntimeResourceFaulted,
    RuntimeResourcePanic,
    RuntimeIoDriver,
    RuntimeIoTransport,
    RuntimeIoAddress,
    RuntimeIoFreshness,
    RuntimeInitFailed,
    RuntimeUnsupportedBytecodeVersion,
    RuntimeInvalidBytecodeMetadata,
    RuntimeInvalidBytecode,
    RuntimeThreadSpawn,
    RuntimeWatchdogTimeout,
    RuntimeRestartLimitExceeded,
    RuntimeSafeStateFailed,
    RuntimeExecutionTimeout,
    RuntimeSimulationFault,
    RuntimeInvalidConfig,
    RuntimeInvalidBundle,
    RuntimeRetainStore,
    RuntimeRetainCorruption,
    RuntimeRetainMigration,
    RuntimeControlError,
    RuntimeStringCapacityExceeded,
    RuntimeNonFiniteValue,
}

impl StableErrorCode {
    /// Return the stable lower-snake-case wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BytecodeInvalidMagic => "bytecode_invalid_magic",
            Self::BytecodeUnsupportedVersion => "bytecode_unsupported_version",
            Self::BytecodeInvalidHeader => "bytecode_invalid_header",
            Self::BytecodeInvalidChecksum => "bytecode_invalid_checksum",
            Self::BytecodeInvalidSectionTable => "bytecode_invalid_section_table",
            Self::BytecodeSectionOutOfBounds => "bytecode_section_out_of_bounds",
            Self::BytecodeSectionOverlap => "bytecode_section_overlap",
            Self::BytecodeSectionAlignment => "bytecode_section_alignment",
            Self::BytecodeUnexpectedEof => "bytecode_unexpected_eof",
            Self::BytecodeInvalidSection => "bytecode_invalid_section",
            Self::BytecodeMissingSection => "bytecode_missing_section",
            Self::BytecodeInvalidOpcode => "bytecode_invalid_opcode",
            Self::BytecodeInvalidJumpTarget => "bytecode_invalid_jump_target",
            Self::BytecodeInvalidPouId => "bytecode_invalid_pou_id",
            Self::BytecodeInvalidIndex => "bytecode_invalid_index",
            Self::VmStackUnderflow => "vm_stack_underflow",
            Self::VmStackOverflow => "vm_stack_overflow",
            Self::VmCallStackUnderflow => "vm_call_stack_underflow",
            Self::VmCallStackOverflow => "vm_call_stack_overflow",
            Self::VmUnsupportedOpcode => "vm_unsupported_opcode",
            Self::VmUnsupportedReferenceLocation => "vm_unsupported_reference_location",
            Self::VmInvalidNativeCall => "vm_invalid_native_call",
            Self::VmBytecodeDecode => "vm_bytecode_decode",
            Self::RuntimeUndefinedVariable => "runtime_undefined_variable",
            Self::RuntimeUndefinedFunction => "runtime_undefined_function",
            Self::RuntimeUndefinedProgram => "runtime_undefined_program",
            Self::RuntimeUndefinedFunctionBlock => "runtime_undefined_function_block",
            Self::RuntimeUndefinedTask => "runtime_undefined_task",
            Self::RuntimeUndefinedLabel => "runtime_undefined_label",
            Self::RuntimeUndefinedField => "runtime_undefined_field",
            Self::RuntimeInvalidTaskSingle => "runtime_invalid_task_single",
            Self::RuntimeInvalidIoAddress => "runtime_invalid_io_address",
            Self::RuntimeTypeMismatch => "runtime_type_mismatch",
            Self::RuntimeInvalidArgumentCount => "runtime_invalid_argument_count",
            Self::RuntimeInvalidArgumentName => "runtime_invalid_argument_name",
            Self::RuntimeAssertionFailed => "runtime_assertion_failed",
            Self::RuntimeDivisionByZero => "runtime_division_by_zero",
            Self::RuntimeModuloByZero => "runtime_modulo_by_zero",
            Self::RuntimeOverflow => "runtime_overflow",
            Self::RuntimeIndexOutOfBounds => "runtime_index_out_of_bounds",
            Self::RuntimeSubrangeViolation => "runtime_subrange_violation",
            Self::RuntimeNullReference => "runtime_null_reference",
            Self::RuntimeInvalidControlFlow => "runtime_invalid_control_flow",
            Self::RuntimeForStepZero => "runtime_for_step_zero",
            Self::RuntimeConditionNotBool => "runtime_condition_not_bool",
            Self::RuntimeCaseSelectorType => "runtime_case_selector_type",
            Self::RuntimeDateTimeRange => "runtime_date_time_range",
            Self::RuntimeInvalidFrame => "runtime_invalid_frame",
            Self::RuntimeResourceFaulted => "runtime_resource_faulted",
            Self::RuntimeResourcePanic => "runtime_resource_panic",
            Self::RuntimeIoDriver => "runtime_io_driver",
            Self::RuntimeIoTransport => "runtime_io_transport",
            Self::RuntimeIoAddress => "runtime_io_address",
            Self::RuntimeIoFreshness => "runtime_io_freshness",
            Self::RuntimeInitFailed => "runtime_init_failed",
            Self::RuntimeUnsupportedBytecodeVersion => "runtime_unsupported_bytecode_version",
            Self::RuntimeInvalidBytecodeMetadata => "runtime_invalid_bytecode_metadata",
            Self::RuntimeInvalidBytecode => "runtime_invalid_bytecode",
            Self::RuntimeThreadSpawn => "runtime_thread_spawn",
            Self::RuntimeWatchdogTimeout => "runtime_watchdog_timeout",
            Self::RuntimeRestartLimitExceeded => "runtime_restart_limit_exceeded",
            Self::RuntimeSafeStateFailed => "runtime_safe_state_failed",
            Self::RuntimeExecutionTimeout => "runtime_execution_timeout",
            Self::RuntimeSimulationFault => "runtime_simulation_fault",
            Self::RuntimeInvalidConfig => "runtime_invalid_config",
            Self::RuntimeInvalidBundle => "runtime_invalid_bundle",
            Self::RuntimeRetainStore => "runtime_retain_store",
            Self::RuntimeRetainCorruption => "runtime_retain_corruption",
            Self::RuntimeRetainMigration => "runtime_retain_migration",
            Self::RuntimeControlError => "runtime_control_error",
            Self::RuntimeStringCapacityExceeded => "runtime_string_capacity_exceeded",
            Self::RuntimeNonFiniteValue => "runtime_non_finite_value",
        }
    }
}

impl fmt::Display for StableErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::StableErrorCode;

    #[test]
    fn stable_error_codes_use_lower_snake_case() {
        for code in [
            StableErrorCode::BytecodeInvalidMagic,
            StableErrorCode::VmStackUnderflow,
            StableErrorCode::RuntimeTypeMismatch,
            StableErrorCode::RuntimeNonFiniteValue,
        ] {
            let text = code.as_str();
            assert!(!text.is_empty());
            assert!(text
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_'));
        }
    }
}
