//! Structured materialization failures. Hosted rendering retains the original text.
use super::RuntimeError;
use core::fmt;
use smol_str::SmolStr;

/// Exact context of a module or constant-pool materialization failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparationDiagnostic {
    /// A typed metadata index does not identify an existing entry.
    InvalidIndex {
        /// Static label identifying the indexed table or field.
        kind: &'static str,
        /// Rejected wire index.
        index: u32,
    },
    /// Repeated POU identity.
    DuplicatePou(u32),
    /// POU identity missing from the decoded name index.
    MissingDecodedPou(u32),
    /// Named POU whose executable range exceeds the code section.
    PouCodeRange(SmolStr),
    /// A case-normalized name occurs twice in one POU category.
    DuplicateName {
        /// POU category such as PROGRAM or FUNCTION_BLOCK.
        kind: &'static str,
        /// Conflicting normalized application name.
        name: SmolStr,
    },
    /// A method name is repeated within its owner POU.
    DuplicateMethod {
        /// Conflicting normalized method name.
        name: SmolStr,
        /// Enclosing POU identity.
        owner: u32,
    },
    /// A raw I/O reference names an unsupported image area.
    InvalidIoArea(u32),
    /// Compound constant payload count differs from its declared shape.
    ConstantCount {
        /// Static label identifying the counted elements or fields.
        kind: &'static str,
        /// Count required by the type table.
        expected: usize,
        /// Count encoded in the payload.
        got: usize,
    },
    /// Type-table index whose kind cannot be decoded as a constant.
    UnsupportedConstantType(u32),
    /// Required constant metadata has no string index.
    MissingString {
        /// Static label identifying the missing name.
        kind: &'static str,
    },
    /// Constant metadata names a string outside the string table.
    StringIndexOutOfBounds {
        /// Static label identifying the referring name field.
        kind: &'static str,
        /// Rejected string index, retained even though legacy text omits it.
        index: u32,
    },
    /// A compound constant lacks bytes for its length prefix or body.
    ConstantTruncated {
        /// Static payload label, without an allocated length suffix.
        kind: &'static str,
        /// True when the missing bytes belong to the child length prefix.
        length: bool,
        /// Required bytes at this cursor.
        need: usize,
        /// Available bytes at this cursor.
        have: usize,
    },
    /// Bytes remain after the complete constant was decoded.
    ConstantTrailing {
        /// Static payload label.
        kind: &'static str,
        /// Number of unconsumed bytes.
        remaining: usize,
    },
    /// A primitive constant does not have its required fixed width.
    ConstantLength {
        /// Static primitive payload label.
        kind: &'static str,
        /// Encoded payload width.
        actual: usize,
        /// Width required by the primitive type.
        expected: usize,
    },
    /// Invalid STRING encoding, including the original offset and error length.
    ConstantUtf8(core::str::Utf8Error),
    /// Invalid WSTRING encoding containing an unpaired UTF-16 surrogate.
    ConstantUtf16,
    /// Primitive type identity not supported by constant materialization.
    UnsupportedPrimitive(u16),
}

impl PreparationDiagnostic {
    /// Render only in hosted builds; MCU errors retain their typed context.
    #[cold]
    #[inline(never)]
    pub fn into_runtime_error(self) -> RuntimeError {
        #[cfg(feature = "std")]
        {
            use alloc::string::ToString;
            RuntimeError::bytecode(super::StableErrorCode::VmBytecodeDecode, self.to_string())
        }
        #[cfg(not(feature = "std"))]
        {
            RuntimeError::PreparationDiagnostic(self)
        }
    }
}

impl fmt::Display for PreparationDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIndex { kind, index } => write!(f, "invalid {kind} index {index}"),
            Self::DuplicatePou(id) => write!(f, "duplicate POU id {id}"),
            Self::MissingDecodedPou(id) => write!(f, "missing decoded POU name for id {id}"),
            Self::PouCodeRange(name) => write!(f, "POU '{name}' code range out of bounds"),
            Self::DuplicateName { kind, name } => write!(f, "duplicate {kind} name '{name}'"),
            Self::DuplicateMethod { name, owner } => {
                write!(f, "duplicate METHOD name '{name}' for owner POU {owner}")
            }
            Self::InvalidIoArea(area) => write!(f, "invalid VM IO owner area {area}"),
            Self::ConstantCount {
                kind,
                expected,
                got,
            } => write!(f, "{kind} mismatch: expected {expected}, got {got}"),
            Self::UnsupportedConstantType(index) => {
                write!(f, "unsupported const type kind at index {index}")
            }
            Self::MissingString { kind } => write!(f, "{kind} missing"),
            Self::StringIndexOutOfBounds { kind, .. } => write!(f, "{kind} index out of bounds"),
            Self::ConstantTruncated {
                kind,
                length,
                need,
                have,
            } => write!(
                f,
                "truncated {kind}{}: need {need} bytes, have {have}",
                if *length { " length" } else { "" }
            ),
            Self::ConstantTrailing { kind, remaining } => {
                write!(f, "invalid {kind} length: {remaining} trailing bytes")
            }
            Self::ConstantLength {
                kind,
                actual,
                expected,
            } => write!(f, "invalid {kind} length {actual}, expected {expected}"),
            Self::ConstantUtf8(error) => write!(f, "invalid STRING const UTF-8: {error}"),
            Self::ConstantUtf16 => {
                f.write_str("invalid WSTRING const UTF-16: invalid utf-16: lone surrogate found")
            }
            Self::UnsupportedPrimitive(id) => write!(f, "unsupported const primitive id {id}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn preparation_diagnostics_preserve_messages_and_context() {
        let cases = [
            (
                PreparationDiagnostic::InvalidIndex {
                    kind: "POU name string",
                    index: 7,
                },
                "invalid POU name string index 7",
            ),
            (PreparationDiagnostic::DuplicatePou(3), "duplicate POU id 3"),
            (
                PreparationDiagnostic::MissingDecodedPou(5),
                "missing decoded POU name for id 5",
            ),
            (
                PreparationDiagnostic::PouCodeRange("Plant".into()),
                "POU 'Plant' code range out of bounds",
            ),
            (
                PreparationDiagnostic::DuplicateName {
                    kind: "FUNCTION_BLOCK",
                    name: "TIMER".into(),
                },
                "duplicate FUNCTION_BLOCK name 'TIMER'",
            ),
            (
                PreparationDiagnostic::DuplicateMethod {
                    name: "RUN".into(),
                    owner: 11,
                },
                "duplicate METHOD name 'RUN' for owner POU 11",
            ),
            (
                PreparationDiagnostic::InvalidIoArea(4),
                "invalid VM IO owner area 4",
            ),
            (
                PreparationDiagnostic::ConstantCount {
                    kind: "ARRAY const element count",
                    expected: 2,
                    got: 1,
                },
                "ARRAY const element count mismatch: expected 2, got 1",
            ),
            (
                PreparationDiagnostic::UnsupportedConstantType(9),
                "unsupported const type kind at index 9",
            ),
            (
                PreparationDiagnostic::MissingString {
                    kind: "enum const type name",
                },
                "enum const type name missing",
            ),
            (
                PreparationDiagnostic::StringIndexOutOfBounds {
                    kind: "const field name",
                    index: 17,
                },
                "const field name index out of bounds",
            ),
            (
                PreparationDiagnostic::ConstantTruncated {
                    kind: "ARRAY const element",
                    length: true,
                    need: 4,
                    have: 2,
                },
                "truncated ARRAY const element length: need 4 bytes, have 2",
            ),
            (
                PreparationDiagnostic::ConstantTruncated {
                    kind: "ARRAY const element",
                    length: false,
                    need: 8,
                    have: 2,
                },
                "truncated ARRAY const element: need 8 bytes, have 2",
            ),
            (
                PreparationDiagnostic::ConstantTrailing {
                    kind: "ARRAY const payload",
                    remaining: 3,
                },
                "invalid ARRAY const payload length: 3 trailing bytes",
            ),
            (
                PreparationDiagnostic::ConstantLength {
                    kind: "REAL const payload",
                    actual: 3,
                    expected: 4,
                },
                "invalid REAL const payload length 3, expected 4",
            ),
            (
                PreparationDiagnostic::ConstantUtf16,
                "invalid WSTRING const UTF-16: invalid utf-16: lone surrogate found",
            ),
            (
                PreparationDiagnostic::UnsupportedPrimitive(99),
                "unsupported const primitive id 99",
            ),
        ];
        for (diagnostic, expected) in cases {
            assert_eq!(diagnostic.to_string(), expected);
            let error = diagnostic.clone().into_runtime_error();
            assert_eq!(
                error.stable_code(),
                super::super::StableErrorCode::VmBytecodeDecode
            );
            #[cfg(not(feature = "std"))]
            assert_eq!(error, RuntimeError::PreparationDiagnostic(diagnostic));
            #[cfg(feature = "std")]
            assert_eq!(
                error,
                RuntimeError::bytecode(super::super::StableErrorCode::VmBytecodeDecode, expected)
            );
        }
    }
}
