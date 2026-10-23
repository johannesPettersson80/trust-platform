//! Go to implementation for Structured Text.
//!
//! Finds classes and function blocks that implement a given interface.

use smol_str::SmolStr;
use text_size::{TextRange, TextSize};

use trust_hir::db::FileId;
use trust_hir::{Database, SourceDatabase, Symbol, SymbolKind, Type};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::util::{
    name_range_from_node, qualified_name_parts_from_node, resolve_target_at_position,
    resolve_type_symbol, scope_at_position, ResolvedTarget, SymbolFilter,
};

/// Result of a go-to-implementation request.
#[derive(Debug, Clone)]
pub struct ImplementationResult {
    /// The file containing the implementation.
    pub file_id: FileId,
    /// The range of the implementing symbol.
    pub range: TextRange,
}

/// Finds implementations of the interface at the given position.
pub fn goto_implementation(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Vec<ImplementationResult> {
    let target = resolve_target_at_position(db, file_id, position);
    let Some(ResolvedTarget::Symbol(symbol_id)) = target else {
        return Vec::new();
    };

    let symbols = db.file_symbols_with_project(file_id);
    if let Some(target_parts) = interface_parts_for_symbol(&symbols, symbol_id) {
        return find_interface_implementations(db, &target_parts);
    }

    if let Some(result) = implementation_for_symbol(&symbols, symbol_id, file_id) {
        return vec![result];
    }

    Vec::new()
}

fn find_interface_implementations(
    db: &Database,
    target_parts: &[SmolStr],
) -> Vec<ImplementationResult> {
    let mut results: Vec<ImplementationResult> = Vec::new();

    for candidate_file in db.file_ids() {
        let source = db.source_text(candidate_file);
        let parsed = parse(&source);
        let root = parsed.syntax();
        let symbols = db.file_symbols_with_project(candidate_file);

        for node in root.descendants() {
            if !matches!(node.kind(), SyntaxKind::Class | SyntaxKind::FunctionBlock) {
                continue;
            }

            if !implements_interface(&node, &root, &symbols, target_parts) {
                continue;
            }

            if let Some(range) = symbol_range_for_node(&node, &symbols)
                && !results
                    .iter()
                    .any(|res| res.file_id == candidate_file && res.range == range)
            {
                results.push(ImplementationResult {
                    file_id: candidate_file,
                    range,
                });
            }
        }
    }

    results
}

fn implements_interface(
    node: &SyntaxNode,
    root: &SyntaxNode,
    symbols: &trust_hir::symbols::SymbolTable,
    target_parts: &[SmolStr],
) -> bool {
    for clause in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::ImplementsClause)
    {
        let scope_id = scope_at_position(symbols, root, clause.text_range().start());
        for name_node in clause
            .children()
            .filter(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
        {
            let Some(parts) = qualified_name_parts_from_node(&name_node) else {
                continue;
            };
            let Some(symbol_id) = resolve_type_symbol(symbols, &parts, scope_id) else {
                continue;
            };
            let Some(found_parts) = interface_parts_for_symbol(symbols, symbol_id) else {
                continue;
            };
            if parts_equal_case_insensitive(&found_parts, target_parts) {
                return true;
            }
        }
    }

    false
}

fn interface_parts_for_symbol(
    symbols: &trust_hir::symbols::SymbolTable,
    symbol_id: trust_hir::SymbolId,
) -> Option<Vec<SmolStr>> {
    let symbol = symbols.get(symbol_id)?;
    if matches!(symbol.kind, SymbolKind::Interface) {
        return Some(qualified_symbol_parts(symbols, symbol_id));
    }

    let resolved = symbols.resolve_alias_type(symbol.type_id);
    let Type::Interface { name } = symbols.type_by_id(resolved)? else {
        return None;
    };
    Some(split_qualified_name(name.as_str()))
}

fn implementation_for_symbol(
    symbols: &trust_hir::symbols::SymbolTable,
    symbol_id: trust_hir::SymbolId,
    fallback_file_id: FileId,
) -> Option<ImplementationResult> {
    let filter = SymbolFilter::new(symbols);
    let symbol = symbols.get(symbol_id)?;
    if matches!(symbol.kind, SymbolKind::FunctionBlock | SymbolKind::Class) {
        return Some(implementation_result_for_symbol(symbol, fallback_file_id));
    }

    let resolved = symbols.resolve_alias_type(symbol.type_id);
    let Some(Type::FunctionBlock { .. } | Type::Class { .. }) = symbols.type_by_id(resolved) else {
        return None;
    };

    let def_symbol = filter.symbol_with_type_id(resolved, |sym| {
        matches!(sym.kind, SymbolKind::FunctionBlock | SymbolKind::Class)
    })?;

    Some(implementation_result_for_symbol(
        def_symbol,
        fallback_file_id,
    ))
}

fn implementation_result_for_symbol(
    symbol: &Symbol,
    fallback_file_id: FileId,
) -> ImplementationResult {
    let file_id = symbol
        .origin
        .map(|origin| origin.file_id)
        .unwrap_or(fallback_file_id);
    ImplementationResult {
        file_id,
        range: symbol.range,
    }
}

fn qualified_symbol_parts(
    symbols: &trust_hir::symbols::SymbolTable,
    symbol_id: trust_hir::SymbolId,
) -> Vec<SmolStr> {
    let mut parts = Vec::new();
    let mut current = symbols.get(symbol_id).and_then(|sym| sym.parent);
    while let Some(parent_id) = current {
        let Some(parent) = symbols.get(parent_id) else {
            break;
        };
        if matches!(parent.kind, SymbolKind::Namespace) {
            parts.push(parent.name.clone());
        }
        current = parent.parent;
    }
    parts.reverse();
    if let Some(symbol) = symbols.get(symbol_id) {
        parts.push(symbol.name.clone());
    }
    parts
}

fn symbol_range_for_node(
    node: &SyntaxNode,
    symbols: &trust_hir::symbols::SymbolTable,
) -> Option<TextRange> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let range = name_range_from_node(&name_node)?;
    let filter = SymbolFilter::new(symbols);
    filter
        .symbol_at_range(range)
        .filter(|sym| matches!(sym.kind, SymbolKind::Class | SymbolKind::FunctionBlock))
        .map(|sym| sym.range)
}

fn split_qualified_name(name: &str) -> Vec<SmolStr> {
    name.split('.').map(SmolStr::new).collect()
}

fn parts_equal_case_insensitive(left: &[SmolStr], right: &[SmolStr]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .all(|(a, b)| a.eq_ignore_ascii_case(b.as_str()))
}
