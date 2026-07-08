//! Go to definition for Structured Text.
//!
//! This module provides navigation to symbol declarations.

use text_size::{TextRange, TextSize};

use trust_hir::db::SemanticDatabase;
use trust_hir::{Database, SourceDatabase, SymbolId};
use trust_syntax::parser::parse;

use crate::util::{
    contains_ascii_case_insensitive, field_declaration_ranges, field_type,
    resolve_target_at_position, FieldTarget, ResolvedTarget,
};

/// Result of a go-to-definition request.
#[derive(Debug, Clone)]
pub struct DefinitionResult {
    /// The file containing the definition.
    pub file_id: trust_hir::db::FileId,
    /// The range of the definition.
    pub range: TextRange,
}

/// Finds the definition of the symbol at the given position.
pub fn goto_definition(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
) -> Option<DefinitionResult> {
    let target = resolve_target_at_position(db, file_id, position)?;
    match target {
        ResolvedTarget::Symbol(symbol_id) => definition_of_symbol(db, file_id, symbol_id),
        ResolvedTarget::Field(field) => definition_of_field(db, &field),
    }
}

/// Finds the declaration of the symbol at the given position.
///
/// For Structured Text, declaration and definition are the same for most symbols.
pub fn goto_declaration(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
) -> Option<DefinitionResult> {
    goto_definition(db, file_id, position)
}

/// Finds the type definition for the symbol at the given position.
pub fn goto_type_definition(
    db: &Database,
    file_id: trust_hir::db::FileId,
    position: TextSize,
) -> Option<DefinitionResult> {
    let target = resolve_target_at_position(db, file_id, position)?;
    match target {
        ResolvedTarget::Symbol(symbol_id) => type_definition_for_symbol(db, file_id, symbol_id),
        ResolvedTarget::Field(field) => {
            let symbols = db.file_symbols_with_project(file_id);
            let type_id = field_type(&symbols, &field)?;
            type_definition_for_type_id(db, file_id, type_id)
        }
    }
}

/// Finds the definition of a symbol by ID.
pub fn definition_of_symbol(
    db: &Database,
    file_id: trust_hir::db::FileId,
    symbol_id: SymbolId,
) -> Option<DefinitionResult> {
    let symbols = db.file_symbols_with_project(file_id);
    let symbol = symbols.get(symbol_id)?;
    let (origin_file, origin_range) = if let Some(origin) = symbol.origin {
        let origin_symbols = db.file_symbols(origin.file_id);
        let origin_range = origin_symbols
            .get(origin.symbol_id)
            .map(|sym| sym.range)
            .unwrap_or(symbol.range);
        (origin.file_id, origin_range)
    } else {
        (file_id, symbol.range)
    };

    Some(DefinitionResult {
        file_id: origin_file,
        range: origin_range,
    })
}

fn type_definition_for_symbol(
    db: &Database,
    file_id: trust_hir::db::FileId,
    symbol_id: SymbolId,
) -> Option<DefinitionResult> {
    let symbols = db.file_symbols_with_project(file_id);
    let symbol = symbols.get(symbol_id)?;

    if is_type_symbol(symbol) {
        return definition_of_symbol(db, file_id, symbol_id);
    }

    type_definition_for_type_id(db, file_id, symbol.type_id)
}

fn type_definition_for_type_id(
    db: &Database,
    file_id: trust_hir::db::FileId,
    type_id: trust_hir::TypeId,
) -> Option<DefinitionResult> {
    let symbols = db.file_symbols_with_project(file_id);
    let symbol = symbols
        .iter()
        .find(|sym| sym.type_id == type_id && is_type_symbol(sym))?;
    definition_of_symbol(db, file_id, symbol.id)
}

fn definition_of_field(db: &Database, field: &FieldTarget) -> Option<DefinitionResult> {
    for candidate_file_id in db.file_ids() {
        let source = db.source_text(candidate_file_id);
        if !contains_ascii_case_insensitive(&source, &field.name) {
            continue;
        }
        let parsed = parse(&source);
        let root = parsed.syntax();
        let symbols = db.file_symbols_with_project(candidate_file_id);
        if let Some(range) = field_declaration_ranges(&root, &symbols, field)
            .into_iter()
            .next()
        {
            return Some(DefinitionResult {
                file_id: candidate_file_id,
                range,
            });
        }
    }
    None
}

fn is_type_symbol(symbol: &trust_hir::symbols::Symbol) -> bool {
    matches!(
        symbol.kind,
        trust_hir::symbols::SymbolKind::Type
            | trust_hir::symbols::SymbolKind::FunctionBlock
            | trust_hir::symbols::SymbolKind::Class
            | trust_hir::symbols::SymbolKind::Interface
    )
}
