use std::fmt;

use smol_str::SmolStr;

use crate::error::RuntimeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryError {
    UnresolvedName {
        path: SmolStr,
    },
    UnboundProgram {
        program: SmolStr,
    },
    AmbiguousName {
        path: SmolStr,
        candidates: Vec<SmolStr>,
    },
    UnsupportedPathSyntax {
        path: SmolStr,
        reason: String,
    },
    WrongKind {
        path: SmolStr,
        expected: &'static str,
        actual: &'static str,
    },
    UndeclaredBinding {
        path: SmolStr,
    },
    InternalLockFailure {
        context: &'static str,
    },
    InternalFailure {
        context: &'static str,
    },
}

impl BoundaryError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnresolvedName { .. } => "unresolved_name",
            Self::UnboundProgram { .. } => "unbound_program",
            Self::AmbiguousName { .. } => "ambiguous_name",
            Self::UnsupportedPathSyntax { .. } => "unsupported_path_syntax",
            Self::WrongKind { .. } => "wrong_kind",
            Self::UndeclaredBinding { .. } => "undeclared_binding",
            Self::InternalLockFailure { .. } => "internal_lock_failure",
            Self::InternalFailure { .. } => "internal_failure",
        }
    }

    #[must_use]
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::UnresolvedName { path }
            | Self::AmbiguousName { path, .. }
            | Self::UnsupportedPathSyntax { path, .. }
            | Self::WrongKind { path, .. }
            | Self::UndeclaredBinding { path } => Some(path.as_str()),
            Self::UnboundProgram { program } => Some(program.as_str()),
            Self::InternalLockFailure { .. } | Self::InternalFailure { .. } => None,
        }
    }

    #[must_use]
    pub fn candidates(&self) -> &[SmolStr] {
        match self {
            Self::AmbiguousName { candidates, .. } => candidates,
            _ => &[],
        }
    }

    pub(crate) fn from_runtime(path: &str, error: RuntimeError) -> Self {
        match error {
            RuntimeError::UndefinedVariable(name) => Self::UnresolvedName { path: name },
            RuntimeError::UndefinedProgram(program) => Self::UnboundProgram { program },
            RuntimeError::UndefinedField(_) => Self::UnresolvedName { path: path.into() },
            RuntimeError::TypeMismatch => Self::WrongKind {
                path: path.into(),
                expected: "observable value",
                actual: "incompatible runtime value",
            },
            RuntimeError::NullReference => Self::WrongKind {
                path: path.into(),
                expected: "non-null reference",
                actual: "null reference",
            },
            RuntimeError::IndexOutOfBounds { .. } => Self::WrongKind {
                path: path.into(),
                expected: "in-range array index",
                actual: "out-of-bounds array index",
            },
            _ => Self::InternalFailure {
                context: "boundary runtime evaluation",
            },
        }
    }
}

impl fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedName { path } => {
                write!(
                    f,
                    "boundary path '{path}' did not resolve to a declared value"
                )
            }
            Self::UnboundProgram { program } => {
                write!(
                    f,
                    "PROGRAM '{program}' is declared but not bound by the CONFIGURATION"
                )
            }
            Self::AmbiguousName { path, candidates } => write!(
                f,
                "boundary path '{path}' is ambiguous; candidates: {}",
                candidates
                    .iter()
                    .map(SmolStr::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::UnsupportedPathSyntax { path, reason } => {
                write!(
                    f,
                    "boundary path '{path}' uses unsupported syntax: {reason}"
                )
            }
            Self::WrongKind {
                path,
                expected,
                actual,
            } => write!(
                f,
                "boundary path '{path}' expected {expected} but resolved to {actual}"
            ),
            Self::UndeclaredBinding { path } => {
                write!(f, "direct binding target '{path}' is not declared")
            }
            Self::InternalLockFailure { context } => {
                write!(
                    f,
                    "internal lock failure while resolving boundary path: {context}"
                )
            }
            Self::InternalFailure { context } => {
                write!(f, "internal boundary failure: {context}")
            }
        }
    }
}

impl std::error::Error for BoundaryError {}
