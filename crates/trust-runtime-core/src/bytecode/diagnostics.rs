//! Deferred section diagnostics: reason and context survive without eager formatting.
use super::{BytecodeError, RejectionReason};
use alloc::{
    borrow::Cow,
    string::{String, ToString},
};
use core::fmt;
use smol_str::SmolStr;

/// Structured section rejection details. Rendering is explicit and lazy on no_std.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionDiagnostic {
    /// Compatibility text supplied by callers.
    Text(SmolStr),
    /// Existing fixed validator reason.
    Reason(RejectionReason),
    /// Collection count exceeds the bytes remaining in its section.
    CountBounds(&'static str),
    /// Collection count exceeds its declared hard limit.
    CountLimit(&'static str),
    /// Standard section ID occurs more than once.
    DuplicateSection(u16),
    /// Variable metadata repeats a reference identity.
    DuplicateVariableReference(u32),
    /// Local metadata declares forbidden retain/initializer state.
    LocalInitialization(u32),
    /// Local reference lies outside all POU frame ranges.
    LocalRange(u32),
    /// Constant text is not valid UTF-8.
    StringUtf8(core::str::Utf8Error),
    /// Array constant count disagrees with its type.
    ArrayCount {
        /// Declared type count.
        expected: usize,
        /// Encoded count.
        actual: usize,
    },
    /// Opcode has insufficient operands.
    StackUnderflow(u8),
    /// Named resource exceeds its fixed limit.
    ResourceLimit(&'static str),
    /// Parameter direction is not recognized.
    ParameterDirection(u8),
    /// Output parameter was supplied as a value.
    ParameterTarget(SmolStr),
    /// POU identity occurs more than once.
    DuplicatePou(u32),
    /// Task names a program not present in the module.
    UnknownProgram(SmolStr),
    /// A recognized instruction is not executable by this runtime.
    UnsupportedOpcode {
        /// Existing opcode name.
        name: &'static str,
        /// Wire opcode.
        opcode: u8,
    },
}

impl SectionDiagnostic {
    /// Render the existing message; fixed and caller text are borrowed.
    pub fn message(&self) -> Cow<'_, str> {
        match self {
            Self::Text(value) => Cow::Borrowed(value.as_str()),
            Self::Reason(reason) => Cow::Borrowed(reason.message()),
            _ => Cow::Owned(self.to_string()),
        }
    }
    /// Query rendered text for compatibility with diagnostic consumers.
    pub fn contains(&self, needle: &str) -> bool {
        self.message().contains(needle)
    }
    /// Query rendered text for compatibility with diagnostic consumers.
    pub fn starts_with(&self, prefix: &str) -> bool {
        self.message().starts_with(prefix)
    }
}

impl fmt::Display for SectionDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(value) => f.write_str(value),
            Self::Reason(reason) => reason.fmt(f),
            Self::CountBounds(name) => write!(f, "{name} count exceeds section bounds"),
            Self::CountLimit(name) => write!(f, "{name} count exceeds fixed resource limit"),
            Self::DuplicateSection(id) => write!(f, "duplicate standardized section id 0x{id:04X}"),
            Self::DuplicateVariableReference(id) => write!(f, "duplicate VAR_META ref_idx {id}"),
            Self::LocalInitialization(id) => write!(
                f,
                "local VAR_META ref {id} must use retain=0 and no initializer"
            ),
            Self::LocalRange(id) => write!(
                f,
                "local VAR_META ref {id} is outside every POU local range"
            ),
            Self::StringUtf8(error) => write!(f, "invalid STRING const UTF-8: {error}"),
            Self::ArrayCount { expected, actual } => write!(
                f,
                "array constant count mismatch: expected {expected}, got {actual}"
            ),
            Self::StackUnderflow(opcode) => write!(
                f,
                "operand stack underflow while decoding opcode 0x{opcode:02X}"
            ),
            Self::ResourceLimit(name) => write!(f, "{name} exceed fixed resource limit"),
            Self::ParameterDirection(direction) => {
                write!(f, "invalid parameter direction {direction}")
            }
            Self::ParameterTarget(name) => write!(f, "parameter '{name}' requires target argument"),
            Self::DuplicatePou(id) => write!(f, "duplicate POU id {id}"),
            Self::UnknownProgram(name) => write!(f, "task references unknown program '{name}'"),
            Self::UnsupportedOpcode { name, opcode } => {
                write!(f, "unsupported runtime opcode {name} (0x{opcode:02X})")
            }
        }
    }
}
impl From<SmolStr> for SectionDiagnostic {
    fn from(value: SmolStr) -> Self {
        Self::Text(value)
    }
}
impl From<String> for SectionDiagnostic {
    fn from(value: String) -> Self {
        Self::Text(value.into())
    }
}
impl From<&str> for SectionDiagnostic {
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}
// Text comparisons explicitly request a diagnostic view. Structured equality is
// derived above and never renders/allocates, including runtime fault comparisons.
impl PartialEq<str> for SectionDiagnostic {
    fn eq(&self, other: &str) -> bool {
        self.message().as_ref() == other
    }
}
impl PartialEq<&str> for SectionDiagnostic {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}
impl PartialEq<String> for SectionDiagnostic {
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}
impl PartialEq<SmolStr> for SectionDiagnostic {
    fn eq(&self, other: &SmolStr) -> bool {
        self == other.as_str()
    }
}

impl BytecodeError {
    /// Construct a literal section error without allocating its message.
    #[cold]
    pub fn section_static(message: &'static str) -> Self {
        Self::section_diagnostic(SectionDiagnostic::Text(SmolStr::new_static(message)))
    }
    /// Preserve structured details without formatting during portable admission.
    /// Hosted consumers retain the existing SmolStr payload and exact messages.
    #[cold]
    #[inline(never)]
    pub fn section_diagnostic(detail: SectionDiagnostic) -> Self {
        #[cfg(feature = "std")]
        {
            let text = match detail {
                SectionDiagnostic::Text(text) => text,
                SectionDiagnostic::Reason(reason) => SmolStr::new_static(reason.message()),
                detail => detail.to_string().into(),
            };
            Self::InvalidSection(text)
        }
        #[cfg(not(feature = "std"))]
        {
            Self::InvalidSection(detail)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::RuntimeError;
    use crate::error_code::StableErrorCode;

    #[test]
    fn structured_reasons_preserve_existing_text_category_and_numeric_context() {
        let cases = [
            (
                SectionDiagnostic::CountBounds("TYPE_TABLE"),
                "TYPE_TABLE count exceeds section bounds",
            ),
            (
                SectionDiagnostic::CountLimit("REF_TABLE"),
                "REF_TABLE count exceeds fixed resource limit",
            ),
            (
                SectionDiagnostic::DuplicateSection(0x10),
                "duplicate standardized section id 0x0010",
            ),
            (
                SectionDiagnostic::DuplicateVariableReference(91),
                "duplicate VAR_META ref_idx 91",
            ),
            (
                SectionDiagnostic::LocalInitialization(3),
                "local VAR_META ref 3 must use retain=0 and no initializer",
            ),
            (
                SectionDiagnostic::LocalRange(4),
                "local VAR_META ref 4 is outside every POU local range",
            ),
            (
                SectionDiagnostic::ArrayCount {
                    expected: 9,
                    actual: 7,
                },
                "array constant count mismatch: expected 9, got 7",
            ),
            (
                SectionDiagnostic::StackUnderflow(0xAA),
                "operand stack underflow while decoding opcode 0xAA",
            ),
            (
                SectionDiagnostic::ResourceLimit("operand stack values"),
                "operand stack values exceed fixed resource limit",
            ),
            (
                SectionDiagnostic::ParameterDirection(9),
                "invalid parameter direction 9",
            ),
            (
                SectionDiagnostic::ParameterTarget("OUT".into()),
                "parameter 'OUT' requires target argument",
            ),
            (SectionDiagnostic::DuplicatePou(123), "duplicate POU id 123"),
            (
                SectionDiagnostic::UnknownProgram("long_program_name_not_discarded".into()),
                "task references unknown program 'long_program_name_not_discarded'",
            ),
            (
                SectionDiagnostic::UnsupportedOpcode {
                    name: "reserved",
                    opcode: 0xFF,
                },
                "unsupported runtime opcode reserved (0xFF)",
            ),
        ];
        for (detail, expected) in cases {
            assert_eq!(detail.to_string(), expected);
            let error = BytecodeError::section_diagnostic(detail);
            assert_eq!(error.stable_code(), StableErrorCode::BytecodeInvalidSection);
            assert!(matches!(&error, BytecodeError::InvalidSection(_)));
            let rendered = alloc::format!("invalid section data: {expected}");
            assert_eq!(error.to_string(), rendered);
            let runtime = RuntimeError::from(error);
            assert_eq!(
                runtime.stable_code(),
                StableErrorCode::BytecodeInvalidSection
            );
            assert_eq!(
                runtime.to_string(),
                alloc::format!("invalid bytecode '{rendered}'")
            );
        }
    }
}
