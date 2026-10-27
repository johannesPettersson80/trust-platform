//! Stable machine-readable runtime error identifiers.

#![allow(missing_docs)]

use core::fmt;

/// Closed stable identifier vocabulary exposed at runtime boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StableErrorCode {
    RuntimeReferenceLifetime,
    RuntimeInvalidAlias,
    RuntimeInvalidExecutionState,
    RuntimeProfileUnsupported,
    VmCallStackOverflow,
    RuntimeUndefinedVariable,
    RuntimeUndefinedFunction,
    RuntimeUndefinedProgram,
    RuntimeUndefinedFunctionBlock,
    RuntimeUndefinedField,
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
    RuntimeForStepZero,
    RuntimeDateTimeRange,
    RuntimeInvalidFrame,
    RuntimeResourceFaulted,
    RuntimeResourcePanic,
    RuntimeIoDriver,
    RuntimeIoTransport,
    RuntimeIoAddress,
    RuntimeIoFreshness,
    RuntimeInitFailed,
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
    RuntimeImportUnsupported,
    RuntimeDeploymentRecordInvalid,
    RuntimeDeploymentRefused,
}

impl StableErrorCode {
    /// Return the stable lower-snake-case wire representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeReferenceLifetime => "runtime_reference_lifetime",
            Self::RuntimeInvalidAlias => "runtime_invalid_alias",
            Self::RuntimeInvalidExecutionState => "runtime_invalid_execution_state",
            Self::RuntimeProfileUnsupported => "runtime_profile_unsupported",
            Self::VmCallStackOverflow => "vm_call_stack_overflow",
            Self::RuntimeUndefinedVariable => "runtime_undefined_variable",
            Self::RuntimeUndefinedFunction => "runtime_undefined_function",
            Self::RuntimeUndefinedProgram => "runtime_undefined_program",
            Self::RuntimeUndefinedFunctionBlock => "runtime_undefined_function_block",
            Self::RuntimeUndefinedField => "runtime_undefined_field",
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
            Self::RuntimeForStepZero => "runtime_for_step_zero",
            Self::RuntimeDateTimeRange => "runtime_date_time_range",
            Self::RuntimeInvalidFrame => "runtime_invalid_frame",
            Self::RuntimeResourceFaulted => "runtime_resource_faulted",
            Self::RuntimeResourcePanic => "runtime_resource_panic",
            Self::RuntimeIoDriver => "runtime_io_driver",
            Self::RuntimeIoTransport => "runtime_io_transport",
            Self::RuntimeIoAddress => "runtime_io_address",
            Self::RuntimeIoFreshness => "runtime_io_freshness",
            Self::RuntimeInitFailed => "runtime_init_failed",
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
            Self::RuntimeImportUnsupported => "runtime_import_unsupported",
            Self::RuntimeDeploymentRecordInvalid => "runtime_deployment_record_invalid",
            Self::RuntimeDeploymentRefused => "runtime_deployment_refused",
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
    use alloc::{
        format,
        string::{String, ToString},
    };

    #[test]
    fn representative_stable_error_codes_use_exact_lower_snake_case() {
        for (code, expected) in [
            (
                StableErrorCode::RuntimeTypeMismatch,
                "runtime_type_mismatch",
            ),
            (
                StableErrorCode::RuntimeNonFiniteValue,
                "runtime_non_finite_value",
            ),
        ] {
            let text = code.as_str();
            assert_eq!(text, expected);
            assert!(text
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_'));
        }
    }

    #[test]
    fn every_stable_error_code_has_exact_lower_snake_case_and_display_identity() {
        use StableErrorCode::*;

        let codes = [
            RuntimeReferenceLifetime,
            RuntimeInvalidAlias,
            RuntimeInvalidExecutionState,
            RuntimeProfileUnsupported,
            VmCallStackOverflow,
            RuntimeUndefinedVariable,
            RuntimeUndefinedFunction,
            RuntimeUndefinedProgram,
            RuntimeUndefinedFunctionBlock,
            RuntimeUndefinedField,
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
            RuntimeForStepZero,
            RuntimeDateTimeRange,
            RuntimeInvalidFrame,
            RuntimeResourceFaulted,
            RuntimeResourcePanic,
            RuntimeIoDriver,
            RuntimeIoTransport,
            RuntimeIoAddress,
            RuntimeIoFreshness,
            RuntimeInitFailed,
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
            RuntimeImportUnsupported,
            RuntimeDeploymentRecordInvalid,
            RuntimeDeploymentRefused,
        ];

        assert_eq!(codes.len(), 48);
        for (index, code) in codes.iter().copied().enumerate() {
            assert!(
                !codes[..index].contains(&code),
                "duplicate stable code {code:?}"
            );
            assert_all_variants_are_listed(code);
            let expected = camel_case_to_lower_snake(&format!("{code:?}"));
            assert_eq!(code.as_str(), expected);
            assert_eq!(code.to_string(), expected);
        }
    }

    fn assert_all_variants_are_listed(code: StableErrorCode) {
        use StableErrorCode::*;

        match code {
            RuntimeReferenceLifetime
            | RuntimeInvalidAlias
            | RuntimeInvalidExecutionState
            | RuntimeProfileUnsupported
            | VmCallStackOverflow
            | RuntimeUndefinedVariable
            | RuntimeUndefinedFunction
            | RuntimeUndefinedProgram
            | RuntimeUndefinedFunctionBlock
            | RuntimeUndefinedField
            | RuntimeInvalidIoAddress
            | RuntimeTypeMismatch
            | RuntimeInvalidArgumentCount
            | RuntimeInvalidArgumentName
            | RuntimeAssertionFailed
            | RuntimeDivisionByZero
            | RuntimeModuloByZero
            | RuntimeOverflow
            | RuntimeIndexOutOfBounds
            | RuntimeSubrangeViolation
            | RuntimeNullReference
            | RuntimeForStepZero
            | RuntimeDateTimeRange
            | RuntimeInvalidFrame
            | RuntimeResourceFaulted
            | RuntimeResourcePanic
            | RuntimeIoDriver
            | RuntimeIoTransport
            | RuntimeIoAddress
            | RuntimeIoFreshness
            | RuntimeInitFailed
            | RuntimeThreadSpawn
            | RuntimeWatchdogTimeout
            | RuntimeRestartLimitExceeded
            | RuntimeSafeStateFailed
            | RuntimeExecutionTimeout
            | RuntimeSimulationFault
            | RuntimeInvalidConfig
            | RuntimeInvalidBundle
            | RuntimeRetainStore
            | RuntimeRetainCorruption
            | RuntimeRetainMigration
            | RuntimeControlError
            | RuntimeStringCapacityExceeded
            | RuntimeNonFiniteValue
            | RuntimeImportUnsupported
            | RuntimeDeploymentRecordInvalid
            | RuntimeDeploymentRefused => {}
        }
    }

    fn camel_case_to_lower_snake(name: &str) -> String {
        let mut result = String::new();
        for (index, character) in name.chars().enumerate() {
            if index > 0 && character.is_ascii_uppercase() {
                result.push('_');
            }
            result.push(character.to_ascii_lowercase());
        }
        result
    }
}
