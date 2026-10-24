//! The runtime's execution backend selection, which `status` and the metrics report.

use alloc::format;

use trust_runtime_core::error::RuntimeError;

/// Runtime execution backend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExecutionBackend {
    /// Bytecode VM execution path.
    #[default]
    BytecodeVm,
    /// A compiled program image run by the kernel (specification 44 §8.8).
    Native,
}

impl ExecutionBackend {
    /// Parses backend selection text accepted by configuration and command-line surfaces.
    ///
    /// # Errors
    ///
    /// `RuntimeError::InvalidConfig` for the retired `interpreter` backend and for unknown text.
    pub fn parse(text: &str) -> Result<Self, RuntimeError> {
        match text.trim().to_ascii_lowercase().as_str() {
            "vm" => Ok(Self::BytecodeVm),
            "native" => Ok(Self::Native),
            "interpreter" => Err(RuntimeError::InvalidConfig(
                "runtime.execution_backend='interpreter' is no longer supported for production runtimes; use 'vm'"
                    .into(),
            )),
            _ => Err(RuntimeError::InvalidConfig(
                format!("invalid runtime.execution_backend '{text}'").into(),
            )),
        }
    }

    /// The configuration text of this backend.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BytecodeVm => "vm",
            Self::Native => "native",
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::ExecutionBackend;

    #[test]
    fn parse_accepts_case_insensitive_values() {
        assert_eq!(
            ExecutionBackend::parse("vm").expect("parse vm"),
            ExecutionBackend::BytecodeVm
        );
        assert_eq!(
            ExecutionBackend::parse("VM").expect("parse uppercase vm"),
            ExecutionBackend::BytecodeVm
        );
    }

    #[test]
    fn parse_accepts_trimmed_values() {
        assert_eq!(
            ExecutionBackend::parse(" vm ").expect("parse trimmed vm"),
            ExecutionBackend::BytecodeVm
        );
    }

    #[test]
    fn parse_rejects_empty_and_invalid_values() {
        let empty = ExecutionBackend::parse("").expect_err("empty should fail");
        assert!(empty
            .to_string()
            .contains("invalid runtime.execution_backend ''"));

        let invalid = ExecutionBackend::parse("bytecode").expect_err("invalid should fail");
        assert!(invalid
            .to_string()
            .contains("invalid runtime.execution_backend 'bytecode'"));
    }

    #[test]
    fn parse_rejects_interpreter_values() {
        let err = ExecutionBackend::parse("interpreter")
            .expect_err("interpreter backend should be rejected");
        assert!(err
            .to_string()
            .contains("runtime.execution_backend='interpreter' is no longer supported"));
    }

    #[test]
    fn backend_configuration_text_is_vm() {
        assert_eq!(ExecutionBackend::BytecodeVm.as_str(), "vm");
        assert_eq!(ExecutionBackend::default(), ExecutionBackend::BytecodeVm);
    }
}
