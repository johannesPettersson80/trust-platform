//! Diagnostic types for semantic errors and warnings.
//!
//! This module defines the diagnostic types used to report semantic
//! errors, warnings, and informational messages.

use text_size::TextRange;

/// Severity level of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticSeverity {
    /// Error - prevents compilation.
    Error,
    /// Warning - potential issue.
    Warning,
    /// Information - informational message.
    Info,
    /// Hint - style suggestion.
    Hint,
}

/// A diagnostic code identifying the type of diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    // Syntax errors (E001-E099)
    /// Unexpected token.
    UnexpectedToken,
    /// Missing token.
    MissingToken,
    /// Unclosed block.
    UnclosedBlock,

    // Name resolution errors (E100-E199)
    /// Undefined variable.
    UndefinedVariable,
    /// Undefined type.
    UndefinedType,
    /// Undefined function/method.
    UndefinedFunction,
    /// Duplicate declaration.
    DuplicateDeclaration,
    /// Cannot resolve name.
    CannotResolve,
    /// Invalid identifier.
    InvalidIdentifier,
    /// Undefined aggregate field/member.
    UndefinedField,
    /// Duplicate aggregate field/member.
    DuplicateField,

    // Type errors (E200-E299)
    /// Type mismatch.
    TypeMismatch,
    /// Invalid operation for type.
    InvalidOperation,
    /// Incompatible assignment.
    IncompatibleAssignment,
    /// Wrong number of arguments.
    WrongArgumentCount,
    /// Invalid argument type.
    InvalidArgumentType,
    /// Missing return value.
    MissingReturn,
    /// Invalid return type.
    InvalidReturnType,

    // Semantic errors (E300-E399)
    /// Invalid assignment target.
    InvalidAssignmentTarget,
    /// Constant cannot be modified.
    ConstantModification,
    /// Invalid array index.
    InvalidArrayIndex,
    /// Out of range value.
    OutOfRange,
    /// Cyclic dependency.
    CyclicDependency,
    /// Invalid task configuration (missing/invalid PRIORITY).
    InvalidTaskConfig,
    /// Unknown task reference in program configuration.
    UnknownTask,

    // Warnings (W001-W099)
    /// Unused variable.
    UnusedVariable,
    /// Unused parameter.
    UnusedParameter,
    /// Unreachable code.
    UnreachableCode,
    /// Missing ELSE branch.
    MissingElse,
    /// Implicit type conversion.
    ImplicitConversion,
    /// Shadowed variable.
    ShadowedVariable,
    /// Deprecated feature.
    Deprecated,
    /// Cyclomatic complexity exceeds threshold.
    HighComplexity,
    /// Unused program/function/function block.
    UnusedPou,
    /// Non-deterministic time/date usage.
    NondeterministicTimeDate,
    /// Non-deterministic I/O timing usage.
    NondeterministicIo,
    /// Shared global accessed by multiple tasks with writes.
    SharedGlobalTaskHazard,
    /// Floating-point equality or inequality comparison.
    FloatingPointEquality,
    /// Division or modulo by literal zero.
    LiteralDivisionByZero,

    // Info/Hints (I001-I099)
    /// Suggested simplification.
    Simplification,
    /// Code style suggestion.
    StyleSuggestion,
}

impl DiagnosticCode {
    /// Returns the string code (e.g., "E101").
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            // Syntax
            Self::UnexpectedToken => "E001",
            Self::MissingToken => "E002",
            Self::UnclosedBlock => "E003",
            // Name resolution
            Self::UndefinedVariable => "E101",
            Self::UndefinedType => "E102",
            Self::UndefinedFunction => "E103",
            Self::DuplicateDeclaration => "E104",
            Self::CannotResolve => "E105",
            Self::InvalidIdentifier => "E106",
            Self::UndefinedField => "E107",
            Self::DuplicateField => "E108",
            // Type errors
            Self::TypeMismatch => "E201",
            Self::InvalidOperation => "E202",
            Self::IncompatibleAssignment => "E203",
            Self::WrongArgumentCount => "E204",
            Self::InvalidArgumentType => "E205",
            Self::MissingReturn => "E206",
            Self::InvalidReturnType => "E207",
            // Semantic
            Self::InvalidAssignmentTarget => "E301",
            Self::ConstantModification => "E302",
            Self::InvalidArrayIndex => "E303",
            Self::OutOfRange => "E304",
            Self::CyclicDependency => "E305",
            Self::InvalidTaskConfig => "E306",
            Self::UnknownTask => "E307",
            // Warnings
            Self::UnusedVariable => "W001",
            Self::UnusedParameter => "W002",
            Self::UnreachableCode => "W003",
            Self::MissingElse => "W004",
            Self::ImplicitConversion => "W005",
            Self::ShadowedVariable => "W006",
            Self::Deprecated => "W007",
            Self::HighComplexity => "W008",
            Self::UnusedPou => "W009",
            Self::NondeterministicTimeDate => "W010",
            Self::NondeterministicIo => "W011",
            Self::SharedGlobalTaskHazard => "W012",
            Self::FloatingPointEquality => "W013",
            Self::LiteralDivisionByZero => "W014",
            // Info
            Self::Simplification => "I001",
            Self::StyleSuggestion => "I002",
        }
    }

    /// Returns the default severity for this diagnostic code.
    #[must_use]
    pub fn severity(&self) -> DiagnosticSeverity {
        match self {
            // Errors
            Self::UnexpectedToken
            | Self::MissingToken
            | Self::UnclosedBlock
            | Self::UndefinedVariable
            | Self::UndefinedType
            | Self::UndefinedFunction
            | Self::DuplicateDeclaration
            | Self::CannotResolve
            | Self::InvalidIdentifier
            | Self::UndefinedField
            | Self::DuplicateField
            | Self::TypeMismatch
            | Self::InvalidOperation
            | Self::IncompatibleAssignment
            | Self::WrongArgumentCount
            | Self::InvalidArgumentType
            | Self::MissingReturn
            | Self::InvalidReturnType
            | Self::InvalidAssignmentTarget
            | Self::ConstantModification
            | Self::InvalidArrayIndex
            | Self::OutOfRange
            | Self::CyclicDependency
            | Self::InvalidTaskConfig
            | Self::UnknownTask => DiagnosticSeverity::Error,

            // Warnings
            Self::UnusedVariable
            | Self::UnusedParameter
            | Self::UnreachableCode
            | Self::MissingElse
            | Self::ImplicitConversion
            | Self::ShadowedVariable
            | Self::Deprecated
            | Self::HighComplexity
            | Self::UnusedPou
            | Self::NondeterministicTimeDate
            | Self::NondeterministicIo
            | Self::SharedGlobalTaskHazard
            | Self::FloatingPointEquality
            | Self::LiteralDivisionByZero => DiagnosticSeverity::Warning,

            // Info/Hints
            Self::Simplification | Self::StyleSuggestion => DiagnosticSeverity::Hint,
        }
    }
}

/// Related information for a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedInfo {
    /// The location of the related information.
    pub range: TextRange,
    /// The message.
    pub message: String,
}

/// A diagnostic message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The diagnostic code.
    pub code: DiagnosticCode,
    /// The severity level.
    pub severity: DiagnosticSeverity,
    /// The source range where the diagnostic applies.
    pub range: TextRange,
    /// The diagnostic message.
    pub message: String,
    /// Related information (e.g., "also declared here").
    pub related: Vec<RelatedInfo>,
}

impl Diagnostic {
    /// Creates a new diagnostic.
    pub fn new(code: DiagnosticCode, range: TextRange, message: impl Into<String>) -> Self {
        Self {
            severity: code.severity(),
            code,
            range,
            message: message.into(),
            related: Vec::new(),
        }
    }

    /// Creates an error diagnostic.
    pub fn error(code: DiagnosticCode, range: TextRange, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code,
            range,
            message: message.into(),
            related: Vec::new(),
        }
    }

    /// Creates a warning diagnostic.
    pub fn warning(code: DiagnosticCode, range: TextRange, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code,
            range,
            message: message.into(),
            related: Vec::new(),
        }
    }

    /// Adds related information to the diagnostic.
    #[must_use]
    pub fn with_related(mut self, range: TextRange, message: impl Into<String>) -> Self {
        self.related.push(RelatedInfo {
            range,
            message: message.into(),
        });
        self
    }

    /// Returns true if this is an error.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == DiagnosticSeverity::Error
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let severity = match self.severity {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Info => "info",
            DiagnosticSeverity::Hint => "hint",
        };
        write!(
            f,
            "{severity}[{}]: {} (at {}..{})",
            self.code.code(),
            self.message,
            u32::from(self.range.start()),
            u32::from(self.range.end())
        )
    }
}

/// Builder for collecting diagnostics.
#[derive(Debug, Default)]
pub struct DiagnosticBuilder {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticBuilder {
    /// Creates a new diagnostic builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a diagnostic.
    pub fn add(&mut self, diagnostic: Diagnostic) {
        if self.diagnostics.contains(&diagnostic) {
            return;
        }
        self.diagnostics.push(diagnostic);
    }

    /// Adds an error.
    pub fn error(&mut self, code: DiagnosticCode, range: TextRange, message: impl Into<String>) {
        self.add(Diagnostic::error(code, range, message));
    }

    /// Adds a warning.
    pub fn warning(&mut self, code: DiagnosticCode, range: TextRange, message: impl Into<String>) {
        self.add(Diagnostic::warning(code, range, message));
    }

    /// Returns true if any errors have been recorded.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }

    /// Consumes the builder and returns the diagnostics.
    #[must_use]
    pub fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_creation() {
        let diag = Diagnostic::error(
            DiagnosticCode::UndefinedVariable,
            TextRange::new(10.into(), 15.into()),
            "undefined variable 'foo'",
        );

        assert!(diag.is_error());
        assert_eq!(diag.code.code(), "E101");
    }

    #[test]
    fn test_diagnostic_builder() {
        let mut builder = DiagnosticBuilder::new();

        builder.error(
            DiagnosticCode::TypeMismatch,
            TextRange::new(0.into(), 10.into()),
            "type mismatch",
        );

        builder.warning(
            DiagnosticCode::UnusedVariable,
            TextRange::new(20.into(), 25.into()),
            "unused variable",
        );

        let diagnostics = builder.finish();
        assert_eq!(diagnostics.len(), 2);
    }
}
