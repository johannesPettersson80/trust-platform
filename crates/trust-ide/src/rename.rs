//! Rename for Structured Text.
//!
//! This module provides safe symbol renaming functionality.

use std::collections::HashMap;
use std::fmt;
use text_size::{TextRange, TextSize};

use crate::refactor::{move_namespace_path, namespace_full_path, parse_namespace_path};
use crate::references::{find_references, find_references_to_field, FindReferencesOptions};
use crate::util::{ident_at_offset, resolve_target_at_position, FieldTarget, ResolvedTarget};
use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::symbols::{ScopeId, SymbolTable};
use trust_hir::{
    is_reserved_keyword, is_valid_identifier, Database, SourceDatabase, SymbolId, Type,
};

/// A text edit representing a change to the source.
#[derive(Debug, Clone)]
pub struct TextEdit {
    /// The range to replace.
    pub range: TextRange,
    /// The new text.
    pub new_text: String,
}

/// Result of a rename operation.
#[derive(Debug, Clone)]
pub struct RenameResult {
    /// Edits grouped by file.
    pub edits: HashMap<FileId, Vec<TextEdit>>,
}

/// Reason a rename request was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameError {
    /// No symbol or field exists at the requested position.
    NoTarget,
    /// The requested name is not a valid IEC identifier.
    InvalidIdentifier {
        /// Requested replacement name.
        new_name: String,
    },
    /// The requested name is a reserved IEC keyword.
    ReservedKeyword {
        /// Requested replacement name.
        new_name: String,
    },
    /// Namespace-path rename was requested at a target that cannot be moved.
    UnsupportedNamespaceRename {
        /// Requested replacement name.
        new_name: String,
    },
    /// The requested name already exists in the target's declaring scope.
    DeclaringScopeConflict {
        /// File containing the conflicting declaring scope.
        file_id: FileId,
        /// Declaration range of the target being renamed.
        range: TextRange,
        /// Requested replacement name.
        new_name: String,
    },
    /// A rename started from an imported use site would conflict in the origin file.
    CrossFileImportedSymbolConflict {
        /// File where the rename was requested.
        use_file_id: FileId,
        /// File containing the original declaration.
        origin_file_id: FileId,
        /// Declaration range of the target being renamed.
        range: TextRange,
        /// Requested replacement name.
        new_name: String,
    },
    /// At least one edited reference would resolve to a different target.
    ReferenceSiteRebind {
        /// File containing the unsafe reference site.
        file_id: FileId,
        /// Range of the edited reference after applying candidate edits.
        range: TextRange,
        /// Requested replacement name.
        new_name: String,
    },
}

impl fmt::Display for RenameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTarget => write!(f, "no symbol or field at the requested position"),
            Self::InvalidIdentifier { new_name } => {
                write!(f, "`{new_name}` is not a valid IEC identifier")
            }
            Self::ReservedKeyword { new_name } => {
                write!(f, "`{new_name}` is a reserved IEC keyword")
            }
            Self::UnsupportedNamespaceRename { new_name } => {
                write!(f, "`{new_name}` is not a supported namespace rename target")
            }
            Self::DeclaringScopeConflict { new_name, .. } => {
                write!(f, "`{new_name}` already exists in the declaring scope")
            }
            Self::CrossFileImportedSymbolConflict { new_name, .. } => write!(
                f,
                "`{new_name}` conflicts with a declaration in the imported symbol's origin file"
            ),
            Self::ReferenceSiteRebind { new_name, .. } => write!(
                f,
                "renaming to `{new_name}` would make at least one reference resolve to a different symbol"
            ),
        }
    }
}

impl std::error::Error for RenameError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SymbolIdentity {
    file_id: FileId,
    symbol_id: SymbolId,
}

#[derive(Debug, Clone)]
struct CandidateReference {
    file_id: FileId,
    new_range: TextRange,
}

impl RenameResult {
    /// Creates an empty rename result.
    #[must_use]
    pub fn new() -> Self {
        Self {
            edits: HashMap::new(),
        }
    }

    /// Adds an edit for a file.
    pub fn add_edit(&mut self, file_id: FileId, edit: TextEdit) {
        self.edits.entry(file_id).or_default().push(edit);
    }

    /// Returns the total number of edits.
    #[must_use]
    pub fn edit_count(&self) -> usize {
        self.edits.values().map(Vec::len).sum()
    }
}

impl Default for RenameResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Prepares a rename operation, checking if rename is valid at the position.
pub fn prepare_rename(db: &Database, file_id: FileId, position: TextSize) -> Option<TextRange> {
    let source = db.source_text(file_id);
    let (_, range) = ident_at_offset(&source, position)?;
    Some(range)
}

/// Performs a rename operation.
pub fn rename(
    db: &Database,
    file_id: FileId,
    position: TextSize,
    new_name: &str,
) -> Option<RenameResult> {
    rename_checked(db, file_id, position, new_name).ok()
}

/// Performs a rename operation and returns the reason when it is refused.
///
/// This is the checked API used by editor protocol frontends. The legacy
/// [`rename`] wrapper preserves the historical `Option` surface for existing
/// callers until they migrate to structured errors.
pub fn rename_checked(
    db: &Database,
    file_id: FileId,
    position: TextSize,
    new_name: &str,
) -> Result<RenameResult, RenameError> {
    let target = resolve_target_at_position(db, file_id, position).ok_or(RenameError::NoTarget)?;

    if new_name.contains('.') {
        let new_path =
            parse_namespace_path(new_name).ok_or_else(|| RenameError::InvalidIdentifier {
                new_name: new_name.to_string(),
            })?;
        if let ResolvedTarget::Symbol(symbol_id) = target {
            let symbols = db.file_symbols(file_id);
            if let Some(path) = namespace_full_path(&symbols, symbol_id) {
                return move_namespace_path(db, &path, &new_path).ok_or_else(|| {
                    RenameError::UnsupportedNamespaceRename {
                        new_name: new_name.to_string(),
                    }
                });
            }
        }
        return Err(RenameError::UnsupportedNamespaceRename {
            new_name: new_name.to_string(),
        });
    }

    // Validate the new name
    if !is_valid_identifier(new_name) {
        return Err(RenameError::InvalidIdentifier {
            new_name: new_name.to_string(),
        });
    }

    // Check if it's a reserved keyword
    if is_reserved_keyword(new_name) {
        return Err(RenameError::ReservedKeyword {
            new_name: new_name.to_string(),
        });
    }

    match target {
        ResolvedTarget::Symbol(symbol_id) => {
            rename_symbol_checked(db, file_id, symbol_id, new_name)
        }
        ResolvedTarget::Field(field) => rename_field_checked(db, file_id, &field, new_name),
    }
}

/// Renames a symbol by ID using semantic reference finding.
pub fn rename_symbol(
    db: &Database,
    file_id: FileId,
    symbol_id: SymbolId,
    new_name: &str,
) -> Option<RenameResult> {
    rename_symbol_checked(db, file_id, symbol_id, new_name).ok()
}

fn rename_symbol_checked(
    db: &Database,
    file_id: FileId,
    symbol_id: SymbolId,
    new_name: &str,
) -> Result<RenameResult, RenameError> {
    if !is_valid_identifier(new_name) {
        return Err(RenameError::InvalidIdentifier {
            new_name: new_name.to_string(),
        });
    }
    if is_reserved_keyword(new_name) {
        return Err(RenameError::ReservedKeyword {
            new_name: new_name.to_string(),
        });
    }

    let symbols = db.file_symbols_with_project(file_id);
    let target_symbol = symbols.get(symbol_id).ok_or(RenameError::NoTarget)?;
    let (target_file_id, target_position) = match target_symbol.origin {
        Some(origin) => {
            let origin_symbols = db.file_symbols_with_project(origin.file_id);
            let position = origin_symbols
                .get(origin.symbol_id)
                .map(|sym| sym.range.start())
                .unwrap_or(target_symbol.range.start());
            (origin.file_id, position)
        }
        None => (file_id, target_symbol.range.start()),
    };
    let (target_identity, target_range) =
        canonical_symbol_identity(db, file_id, symbol_id).ok_or(RenameError::NoTarget)?;

    let origin_symbols = db.file_symbols_with_project(target_identity.file_id);
    if conflict_in_declaring_scope(&origin_symbols, target_identity.symbol_id, new_name).is_some() {
        if target_identity.file_id != file_id {
            return Err(RenameError::CrossFileImportedSymbolConflict {
                use_file_id: file_id,
                origin_file_id: target_identity.file_id,
                range: target_range,
                new_name: new_name.to_string(),
            });
        }
        return Err(RenameError::DeclaringScopeConflict {
            file_id: target_identity.file_id,
            range: target_range,
            new_name: new_name.to_string(),
        });
    }

    let mut result = RenameResult::new();

    // Find all references across project (including declaration)
    let references = find_references(
        db,
        target_file_id,
        target_position,
        FindReferencesOptions {
            include_declaration: true,
        },
    );
    let candidate_references = candidate_reference_ranges(&references, new_name);

    // Create edits for all references
    for reference in references {
        result.add_edit(
            reference.file_id,
            TextEdit {
                range: reference.range,
                new_text: new_name.to_string(),
            },
        );
    }

    reject_rebinding_references(
        db,
        &result,
        &candidate_references,
        target_identity,
        new_name,
    )?;

    Ok(result)
}

fn rename_field_checked(
    db: &Database,
    file_id: FileId,
    field: &FieldTarget,
    new_name: &str,
) -> Result<RenameResult, RenameError> {
    if !is_valid_identifier(new_name) || is_reserved_keyword(new_name) {
        return Err(if is_reserved_keyword(new_name) {
            RenameError::ReservedKeyword {
                new_name: new_name.to_string(),
            }
        } else {
            RenameError::InvalidIdentifier {
                new_name: new_name.to_string(),
            }
        });
    }

    let symbols = db.file_symbols(file_id);
    if field_has_conflict(&symbols, field, new_name) {
        return Err(RenameError::DeclaringScopeConflict {
            file_id,
            range: TextRange::empty(TextSize::from(0)),
            new_name: new_name.to_string(),
        });
    }

    let references = find_references_to_field(
        db,
        file_id,
        field,
        FindReferencesOptions {
            include_declaration: true,
        },
    );

    if references.is_empty() {
        return Err(RenameError::NoTarget);
    }

    let mut result = RenameResult::new();
    for reference in references {
        result.add_edit(
            reference.file_id,
            TextEdit {
                range: reference.range,
                new_text: new_name.to_string(),
            },
        );
    }

    Ok(result)
}

fn conflict_in_declaring_scope(
    symbols: &SymbolTable,
    symbol_id: SymbolId,
    new_name: &str,
) -> Option<SymbolId> {
    // Verify symbol exists.
    symbols.get(symbol_id)?;

    // Find the declaring scope
    let declaring_scope = find_declaring_scope(symbols, symbol_id);

    // Check if new_name already exists in that scope
    if let Some(scope) = symbols.get_scope(declaring_scope) {
        if let Some(existing_id) = scope.lookup_local(new_name) {
            // Conflict if a different symbol has this name
            if existing_id != symbol_id {
                return Some(existing_id);
            }
        }
    }

    None
}

fn canonical_symbol_identity(
    db: &Database,
    local_file_id: FileId,
    symbol_id: SymbolId,
) -> Option<(SymbolIdentity, TextRange)> {
    let symbols = db.file_symbols_with_project(local_file_id);
    let symbol = symbols.get(symbol_id)?;
    if let Some(origin) = symbol.origin {
        let origin_symbols = db.file_symbols_with_project(origin.file_id);
        let origin_symbol = origin_symbols.get(origin.symbol_id)?;
        return Some((
            SymbolIdentity {
                file_id: origin.file_id,
                symbol_id: origin.symbol_id,
            },
            origin_symbol.range,
        ));
    }
    Some((
        SymbolIdentity {
            file_id: local_file_id,
            symbol_id,
        },
        symbol.range,
    ))
}

fn symbol_identity(
    symbols: &SymbolTable,
    symbol_id: SymbolId,
    local_file_id: FileId,
) -> Option<SymbolIdentity> {
    let symbol = symbols.get(symbol_id)?;
    Some(if let Some(origin) = symbol.origin {
        SymbolIdentity {
            file_id: origin.file_id,
            symbol_id: origin.symbol_id,
        }
    } else {
        SymbolIdentity {
            file_id: local_file_id,
            symbol_id,
        }
    })
}

fn candidate_reference_ranges(
    references: &[crate::references::Reference],
    new_name: &str,
) -> Vec<CandidateReference> {
    let mut indexed: Vec<_> = references.iter().enumerate().collect();
    indexed.sort_by_key(|(_, reference)| {
        (
            reference.file_id.0,
            u32::from(reference.range.start()),
            u32::from(reference.range.end()),
        )
    });

    let mut deltas: HashMap<FileId, i64> = HashMap::new();
    let mut candidates = vec![
        CandidateReference {
            file_id: FileId(0),
            new_range: TextRange::empty(TextSize::from(0)),
        };
        references.len()
    ];

    for (index, reference) in indexed {
        let delta = *deltas.entry(reference.file_id).or_insert(0);
        let original_start = i64::from(u32::from(reference.range.start()));
        let original_end = i64::from(u32::from(reference.range.end()));
        let new_start = original_start + delta;
        let new_end = new_start + new_name.len() as i64;
        candidates[index] = CandidateReference {
            file_id: reference.file_id,
            new_range: TextRange::new(
                TextSize::from(new_start as u32),
                TextSize::from(new_end as u32),
            ),
        };
        *deltas.get_mut(&reference.file_id).expect("delta entry") +=
            new_name.len() as i64 - (original_end - original_start);
    }

    candidates
}

fn reject_rebinding_references(
    db: &Database,
    result: &RenameResult,
    candidate_references: &[CandidateReference],
    target_identity: SymbolIdentity,
    new_name: &str,
) -> Result<(), RenameError> {
    let scratch = scratch_database_with_edits(db, result);
    for reference in candidate_references {
        let Some(ResolvedTarget::Symbol(resolved_id)) =
            resolve_target_at_position(&scratch, reference.file_id, reference.new_range.start())
        else {
            return Err(RenameError::ReferenceSiteRebind {
                file_id: reference.file_id,
                range: reference.new_range,
                new_name: new_name.to_string(),
            });
        };
        let symbols = scratch.file_symbols_with_project(reference.file_id);
        if symbol_identity(&symbols, resolved_id, reference.file_id) != Some(target_identity) {
            return Err(RenameError::ReferenceSiteRebind {
                file_id: reference.file_id,
                range: reference.new_range,
                new_name: new_name.to_string(),
            });
        }
    }
    Ok(())
}

fn scratch_database_with_edits(db: &Database, result: &RenameResult) -> Database {
    let mut scratch = Database::new();
    for file_id in db.file_ids() {
        let mut source = db.source_text(file_id).as_ref().clone();
        if let Some(edits) = result.edits.get(&file_id) {
            let mut sorted = edits.clone();
            sorted.sort_by_key(|edit| std::cmp::Reverse(u32::from(edit.range.start())));
            for edit in sorted {
                let start = usize::from(edit.range.start());
                let end = usize::from(edit.range.end());
                source.replace_range(start..end, &edit.new_text);
            }
        }
        scratch.set_source_text(file_id, source);
    }
    scratch
}

fn field_has_conflict(symbols: &SymbolTable, field: &FieldTarget, new_name: &str) -> bool {
    match symbols.type_by_id(field.type_id) {
        Some(Type::Struct { fields, .. }) => fields.iter().any(|field_def| {
            field_def.name.eq_ignore_ascii_case(new_name)
                && !field_def.name.eq_ignore_ascii_case(&field.name)
        }),
        Some(Type::Union { variants, .. }) => variants.iter().any(|variant_def| {
            variant_def.name.eq_ignore_ascii_case(new_name)
                && !variant_def.name.eq_ignore_ascii_case(&field.name)
        }),
        _ => false,
    }
}

/// Finds the scope where a symbol is declared.
fn find_declaring_scope(symbols: &SymbolTable, symbol_id: SymbolId) -> ScopeId {
    for i in 0..symbols.scope_count() {
        let scope_id = ScopeId(i as u32);
        if let Some(scope) = symbols.get_scope(scope_id) {
            if scope.symbols.values().any(|&id| id == symbol_id) {
                return scope_id;
            }
        }
    }
    ScopeId::GLOBAL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_identifier() {
        assert!(is_valid_identifier("foo"));
        assert!(is_valid_identifier("_bar"));
        assert!(is_valid_identifier("FB_Motor"));
        assert!(is_valid_identifier("x1"));

        assert!(!is_valid_identifier(""));
        assert!(!is_valid_identifier("1foo"));
        assert!(!is_valid_identifier("foo-bar"));
        assert!(!is_valid_identifier("foo bar"));
        assert!(!is_valid_identifier("A__B"));
        assert!(!is_valid_identifier("A_"));
        assert!(!is_valid_identifier("__A"));
    }

    #[test]
    fn test_reserved_keywords_rejected() {
        assert!(is_reserved_keyword("EN"));
        assert!(is_reserved_keyword("ENO"));
        assert!(is_reserved_keyword("TOD"));
        assert!(is_reserved_keyword("DT"));
        assert!(is_reserved_keyword("ANY_ELEMENTARY"));
        assert!(is_reserved_keyword("CHAR"));
    }
}
