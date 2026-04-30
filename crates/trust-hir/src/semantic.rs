//! Semantic identity, declaration catalog, and resolver outcome types.
#![allow(dead_code)]

use smol_str::SmolStr;
use text_size::TextRange;

use crate::db::FileId;
use crate::diagnostics::DiagnosticCode;
use crate::symbols::{ScopeId, SymbolId};
use crate::types::TypeId;

/// Legacy compatibility value for call sites that still have to write a `TypeId`.
///
/// New semantic decisions should prefer `SemanticOutcome<TypeId>` and only cross
/// back through this value at old storage/API boundaries.
pub(crate) const LEGACY_UNKNOWN_TYPE_ID: TypeId = TypeId::UNKNOWN;

/// A normalized qualified IEC declaration name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedName {
    parts: Vec<SmolStr>,
}

impl QualifiedName {
    /// Creates a qualified name from non-empty name parts.
    pub fn new(parts: Vec<SmolStr>) -> Option<Self> {
        (!parts.is_empty()).then_some(Self { parts })
    }

    /// Parses a dotted qualified name.
    pub fn from_dotted(name: &str) -> Option<Self> {
        let parts = name
            .split('.')
            .filter(|part| !part.is_empty())
            .map(SmolStr::new)
            .collect::<Vec<_>>();
        Self::new(parts)
    }

    /// Returns the qualified-name parts.
    pub fn parts(&self) -> &[SmolStr] {
        &self.parts
    }

    /// Renders the qualified name in dotted form.
    pub fn display(&self) -> String {
        self.parts
            .iter()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>()
            .join(".")
    }
}

/// A scope path with the concrete scope id and owning symbol chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ScopePath {
    scope_id: ScopeId,
    owners: Vec<SymbolId>,
}

impl ScopePath {
    pub(crate) fn new(scope_id: ScopeId, owners: Vec<SymbolId>) -> Self {
        Self { scope_id, owners }
    }

    pub(crate) fn scope_id(&self) -> ScopeId {
        self.scope_id
    }

    pub(crate) fn owners(&self) -> &[SymbolId] {
        &self.owners
    }
}

/// Source identity for a semantic declaration or reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceIdentity {
    file_id: FileId,
    range: TextRange,
}

impl SourceIdentity {
    /// Creates a source identity from file and source range.
    pub fn new(file_id: FileId, range: TextRange) -> Self {
        Self { file_id, range }
    }

    /// Returns the file containing this source range.
    pub fn file_id(&self) -> FileId {
        self.file_id
    }

    /// Returns the source range.
    pub fn range(&self) -> TextRange {
        self.range
    }
}

/// Semantic role used by resolver outcomes and catalog declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticRole {
    /// Value-like declaration, such as a variable, constant, enum value, or property.
    Value,
    /// Type-like declaration, such as a TYPE, FUNCTION_BLOCK, CLASS, or INTERFACE.
    Type,
    /// Callable declaration, such as a FUNCTION or METHOD.
    Callable,
    /// Namespace declaration.
    Namespace,
    /// Declaration that owns a scope but is not itself a value/type call target.
    ScopeOwner,
}

/// Classified semantic resolver outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticOutcome<T> {
    /// The semantic decision resolved successfully.
    Resolved(T),
    /// The requested name could not be resolved.
    Unknown {
        /// Name that failed to resolve, when known.
        name: Option<QualifiedName>,
        /// Source range of the failed reference, when known.
        range: Option<TextRange>,
    },
    /// The requested name resolved to multiple candidates.
    Ambiguous {
        /// Ambiguous name.
        name: QualifiedName,
        /// Source range of the ambiguous reference, when known.
        range: Option<TextRange>,
    },
    /// The requested name resolved, but to the wrong semantic role.
    WrongKind {
        /// Resolved symbol id.
        symbol_id: SymbolId,
        /// Expected role.
        expected: SemanticRole,
        /// Actual role.
        actual: SemanticRole,
        /// Source range of the wrong-kind reference, when known.
        range: Option<TextRange>,
    },
    /// A cascade was intentionally suppressed because another diagnostic is primary.
    SuppressedCascade {
        /// Primary diagnostic code responsible for the suppression.
        primary: DiagnosticCode,
        /// Source range of the suppressed cascade, when known.
        range: Option<TextRange>,
    },
    /// Internal semantic invariant was violated.
    InvariantViolation {
        /// Human-readable invariant failure.
        message: SmolStr,
        /// Source range tied to the invariant failure, when known.
        range: Option<TextRange>,
    },
}

impl<T> SemanticOutcome<T> {
    /// Maps a resolved value while preserving every non-resolved classification.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> SemanticOutcome<U> {
        match self {
            SemanticOutcome::Resolved(value) => SemanticOutcome::Resolved(f(value)),
            SemanticOutcome::Unknown { name, range } => SemanticOutcome::Unknown { name, range },
            SemanticOutcome::Ambiguous { name, range } => {
                SemanticOutcome::Ambiguous { name, range }
            }
            SemanticOutcome::WrongKind {
                symbol_id,
                expected,
                actual,
                range,
            } => SemanticOutcome::WrongKind {
                symbol_id,
                expected,
                actual,
                range,
            },
            SemanticOutcome::SuppressedCascade { primary, range } => {
                SemanticOutcome::SuppressedCascade { primary, range }
            }
            SemanticOutcome::InvariantViolation { message, range } => {
                SemanticOutcome::InvariantViolation { message, range }
            }
        }
    }

    /// Returns true when the outcome is resolved.
    pub fn is_resolved(&self) -> bool {
        matches!(self, SemanticOutcome::Resolved(_))
    }
}

/// HIR declaration catalog for one analyzed file view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeclarationCatalog {
    entries: Vec<DeclarationRecord>,
    references: Vec<DeclarationReferenceRecord>,
}

impl DeclarationCatalog {
    /// Creates a declaration catalog from declaration and reference records.
    pub fn new(
        mut entries: Vec<DeclarationRecord>,
        mut references: Vec<DeclarationReferenceRecord>,
    ) -> Self {
        entries.sort_by(|left, right| {
            left.source
                .file_id()
                .0
                .cmp(&right.source.file_id().0)
                .then_with(|| {
                    left.source
                        .range()
                        .start()
                        .cmp(&right.source.range().start())
                })
                .then_with(|| {
                    left.qualified_name
                        .display()
                        .cmp(&right.qualified_name.display())
                })
        });
        references.sort_by(|left, right| {
            left.owner_symbol_id
                .0
                .cmp(&right.owner_symbol_id.0)
                .then_with(|| left.name.display().cmp(&right.name.display()))
                .then_with(|| left.kind.cmp(&right.kind))
        });
        Self {
            entries,
            references,
        }
    }

    /// Returns all declaration records.
    pub fn entries(&self) -> &[DeclarationRecord] {
        &self.entries
    }

    /// Returns all retained declaration reference records.
    pub fn references(&self) -> &[DeclarationReferenceRecord] {
        &self.references
    }

    /// Finds a declaration by dotted qualified name.
    pub fn find_qualified(&self, qualified_name: &str) -> Option<&DeclarationRecord> {
        self.entries.iter().find(|entry| {
            entry
                .qualified_name()
                .display()
                .eq_ignore_ascii_case(qualified_name)
        })
    }
}

/// A single source-backed declaration known to HIR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationRecord {
    symbol_id: SymbolId,
    qualified_name: QualifiedName,
    source: SourceIdentity,
    type_id: TypeId,
    kind: DeclarationKind,
    role: SemanticRole,
    owner_symbol_id: Option<SymbolId>,
    owner_scope_id: Option<ScopeId>,
    owned_scope_id: Option<ScopeId>,
    imported: bool,
}

impl DeclarationRecord {
    /// Creates a source-backed declaration record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol_id: SymbolId,
        qualified_name: QualifiedName,
        source: SourceIdentity,
        type_id: TypeId,
        kind: DeclarationKind,
        role: SemanticRole,
        owner_symbol_id: Option<SymbolId>,
        owner_scope_id: Option<ScopeId>,
        owned_scope_id: Option<ScopeId>,
        imported: bool,
    ) -> Self {
        Self {
            symbol_id,
            qualified_name,
            source,
            type_id,
            kind,
            role,
            owner_symbol_id,
            owner_scope_id,
            owned_scope_id,
            imported,
        }
    }

    /// Returns the symbol id for this declaration in the analyzed table.
    pub fn symbol_id(&self) -> SymbolId {
        self.symbol_id
    }

    /// Returns the qualified declaration name.
    pub fn qualified_name(&self) -> &QualifiedName {
        &self.qualified_name
    }

    /// Returns the declaration source identity.
    pub fn source(&self) -> SourceIdentity {
        self.source
    }

    /// Returns the HIR type id attached to this declaration.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the declaration kind.
    pub fn kind(&self) -> DeclarationKind {
        self.kind
    }

    /// Returns the semantic role.
    pub fn role(&self) -> SemanticRole {
        self.role
    }

    /// Returns the owning declaration symbol id, when this declaration is nested.
    pub fn owner_symbol_id(&self) -> Option<SymbolId> {
        self.owner_symbol_id
    }

    /// Returns the scope id that contains this declaration.
    pub fn owner_scope_id(&self) -> Option<ScopeId> {
        self.owner_scope_id
    }

    /// Returns the scope owned by this declaration, when it owns one.
    pub fn owned_scope_id(&self) -> Option<ScopeId> {
        self.owned_scope_id
    }

    /// Returns true when this declaration was imported from another project file.
    pub fn is_imported(&self) -> bool {
        self.imported
    }
}

/// Source declaration kind recorded in the HIR declaration catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeclarationKind {
    /// PROGRAM declaration.
    Program,
    /// CONFIGURATION declaration.
    Configuration,
    /// RESOURCE declaration.
    Resource,
    /// TASK declaration.
    Task,
    /// PROGRAM instance declaration in a CONFIGURATION/RESOURCE.
    ProgramInstance,
    /// NAMESPACE declaration.
    Namespace,
    /// FUNCTION declaration.
    Function,
    /// FUNCTION_BLOCK declaration.
    FunctionBlock,
    /// CLASS declaration.
    Class,
    /// METHOD declaration.
    Method,
    /// PROPERTY declaration.
    Property,
    /// INTERFACE declaration.
    Interface,
    /// Variable declaration.
    Variable,
    /// Constant declaration.
    Constant,
    /// TYPE declaration.
    Type,
    /// Enum value declaration.
    EnumValue,
    /// Formal parameter declaration.
    Parameter,
}

/// Kind of a retained declaration reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeclarationReferenceKind {
    /// EXTENDS reference.
    Extends,
    /// IMPLEMENTS reference.
    Implements,
}

/// A declaration reference retained with its classified resolver outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationReferenceRecord {
    owner_symbol_id: SymbolId,
    kind: DeclarationReferenceKind,
    name: QualifiedName,
    range: Option<TextRange>,
    outcome: SemanticOutcome<SymbolId>,
}

impl DeclarationReferenceRecord {
    /// Creates a declaration reference record.
    pub fn new(
        owner_symbol_id: SymbolId,
        kind: DeclarationReferenceKind,
        name: QualifiedName,
        range: Option<TextRange>,
        outcome: SemanticOutcome<SymbolId>,
    ) -> Self {
        Self {
            owner_symbol_id,
            kind,
            name,
            range,
            outcome,
        }
    }

    /// Returns the owner symbol id for the declaration containing the reference.
    pub fn owner_symbol_id(&self) -> SymbolId {
        self.owner_symbol_id
    }

    /// Returns the retained reference kind.
    pub fn kind(&self) -> DeclarationReferenceKind {
        self.kind
    }

    /// Returns the referenced name.
    pub fn name(&self) -> &QualifiedName {
        &self.name
    }

    /// Returns the source range of this reference, when known.
    pub fn range(&self) -> Option<TextRange> {
        self.range
    }

    /// Returns the classified resolver outcome for this reference.
    pub fn outcome(&self) -> &SemanticOutcome<SymbolId> {
        &self.outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use text_size::TextSize;

    #[test]
    fn qualified_name_rejects_empty_and_splits_dotted_names() {
        assert!(QualifiedName::new(Vec::new()).is_none());

        let name = QualifiedName::from_dotted("Lib.Inner.Type").expect("qualified name");
        assert_eq!(name.display(), "Lib.Inner.Type");
        assert_eq!(name.parts().len(), 3);
    }

    #[test]
    fn semantic_outcome_map_preserves_non_resolved_classification() {
        let range = TextRange::new(TextSize::from(1), TextSize::from(5));
        let outcome: SemanticOutcome<SymbolId> = SemanticOutcome::Ambiguous {
            name: QualifiedName::from_dotted("A.Foo").expect("qualified name"),
            range: Some(range),
        };

        let mapped = outcome.map(|id| id.0);
        assert_eq!(
            mapped,
            SemanticOutcome::Ambiguous {
                name: QualifiedName::from_dotted("A.Foo").expect("qualified name"),
                range: Some(range),
            }
        );
    }

    #[test]
    fn semantic_outcome_is_resolved_only_for_resolved_variant() {
        let range = TextRange::new(TextSize::from(1), TextSize::from(5));
        let name = QualifiedName::from_dotted("A.Foo").expect("qualified name");

        assert!(SemanticOutcome::Resolved(SymbolId(1)).is_resolved());

        for outcome in [
            SemanticOutcome::<SymbolId>::Unknown {
                name: Some(name.clone()),
                range: Some(range),
            },
            SemanticOutcome::Ambiguous {
                name: name.clone(),
                range: Some(range),
            },
            SemanticOutcome::WrongKind {
                symbol_id: SymbolId(1),
                expected: SemanticRole::Type,
                actual: SemanticRole::Value,
                range: Some(range),
            },
            SemanticOutcome::SuppressedCascade {
                primary: DiagnosticCode::CannotResolve,
                range: Some(range),
            },
            SemanticOutcome::InvariantViolation {
                message: SmolStr::new("broken invariant"),
                range: Some(range),
            },
        ] {
            assert!(!outcome.is_resolved(), "{outcome:?}");
        }
    }
}
