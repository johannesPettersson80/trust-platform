//! Typed reasons for refusing a built-in import at admission (spec 34 §5.2.1).

use core::fmt;

/// The part of a built-in import contract that an artifact's import fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportRefusal {
    /// No built-in import has this name.
    NoContract,
    /// The number of arguments or declared parameters differs from the contract.
    ParameterCount,
    /// An argument or parameter name differs from the contract's parameters.
    ParameterName,
    /// An output argument is not a writable target.
    OutputTarget,
    /// A declared parameter's type or direction differs from the contract.
    ParameterType,
    /// The hidden native state of a function block differs from the contract.
    StateLayout,
}

impl fmt::Display for ImportRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NoContract => "no built-in contract has this name",
            Self::ParameterCount => {
                "the number of arguments or parameters differs from the contract"
            }
            Self::ParameterName => "an argument or parameter name differs from the contract",
            Self::OutputTarget => "an output argument is not a writable target",
            Self::ParameterType => "a parameter's type or direction differs from the contract",
            Self::StateLayout => "the hidden native state differs from the contract",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ImportRefusal::*;
    use crate::error::{RuntimeError, StableErrorCode};
    use alloc::{format, string::ToString};

    #[test]
    fn a_refusal_names_the_import_and_the_failed_part_of_its_contract() {
        for (reason, text) in [
            (NoContract, "no built-in contract has this name"),
            (
                ParameterCount,
                "the number of arguments or parameters differs from the contract",
            ),
            (
                ParameterName,
                "an argument or parameter name differs from the contract",
            ),
            (OutputTarget, "an output argument is not a writable target"),
            (
                ParameterType,
                "a parameter's type or direction differs from the contract",
            ),
            (
                StateLayout,
                "the hidden native state differs from the contract",
            ),
        ] {
            let error = RuntimeError::ImportUnsupported {
                import: "TON".into(),
                reason,
            };
            assert_eq!(
                error.to_string(),
                format!("unsupported import 'TON': {text}")
            );
            assert_eq!(
                error.stable_code(),
                StableErrorCode::RuntimeImportUnsupported
            );
            assert_eq!(error.stable_code().as_str(), "runtime_import_unsupported");
        }
    }
}
