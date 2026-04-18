//! Find references for Structured Text.
//!
//! This module provides functionality to find all references to a symbol.

use text_size::{TextRange, TextSize};

use smol_str::SmolStr;
use trust_hir::db::{FileId, SemanticDatabase};
use trust_hir::{Database, SourceDatabase, SymbolId, Type, TypeId};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::util::{
    field_declaration_ranges, ident_token_in_name, is_type_name_node, is_type_symbol_kind,
    qualified_name_from_field_expr, resolve_target_at_position_with_context,
    resolve_type_symbol_at_node, scope_at_position, FieldTarget, IdeContext, ResolvedTarget,
};

/// A reference to a symbol.
#[derive(Debug, Clone)]
pub struct Reference {
    /// The file containing the reference.
    pub file_id: trust_hir::db::FileId,
    /// The range of the reference.
    pub range: TextRange,
    /// Whether this reference is a write (assignment).
    pub is_write: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SymbolIdentity {
    file_id: FileId,
    symbol_id: SymbolId,
}

/// Options for finding references.
#[derive(Debug, Clone, Copy, Default)]
pub struct FindReferencesOptions {
    /// Include the declaration in the results.
    pub include_declaration: bool,
}

/// Finds all references to the symbol at the given position.
pub fn find_references(
    db: &Database,
    file_id: FileId,
    position: TextSize,
    options: FindReferencesOptions,
) -> Vec<Reference> {
    let context = IdeContext::new(db, file_id);

    if let Some(target) = resolve_target_at_position_with_context(
        db,
        file_id,
        position,
        &context.source,
        &context.root,
        &context.symbols,
    ) {
        return match target {
            ResolvedTarget::Symbol(symbol_id) => {
                let symbols = &context.symbols;
                let Some(target_symbol) = symbols.get(symbol_id) else {
                    return Vec::new();
                };
                let Some(identity) = symbol_identity(symbols, symbol_id, file_id) else {
                    return Vec::new();
                };
                if is_type_symbol_kind(&target_symbol.kind) {
                    find_type_references_across_project(db, identity, options)
                } else {
                    find_references_to_symbol_across_project(
                        db,
                        identity,
                        target_symbol.name.clone(),
                        options,
                    )
                }
            }
            ResolvedTarget::Field(field) => find_references_to_field(db, file_id, &field, options),
        };
    }

    Vec::new()
}

fn find_references_to_symbol_across_project(
    db: &Database,
    identity: SymbolIdentity,
    target_name: SmolStr,
    options: FindReferencesOptions,
) -> Vec<Reference> {
    let mut references = Vec::new();
    if options.include_declaration {
        if let Some(reference) = declaration_reference(db, identity) {
            references.push(reference);
        }
    }

    for other_file_id in db.file_ids() {
        references.extend(find_references_to_symbol_in_file_by_identity(
            db,
            other_file_id,
            identity,
            &target_name,
        ));
    }

    references
}

fn find_references_to_symbol_in_file_by_identity(
    db: &Database,
    file_id: FileId,
    identity: SymbolIdentity,
    target_name: &SmolStr,
) -> Vec<Reference> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let symbols = db.file_symbols_with_project(file_id);

    let mut references = Vec::new();

    for node in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::NameRef)
    {
        let Some(ident_token) = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == SyntaxKind::Ident)
        else {
            continue;
        };

        let ref_name = ident_token.text();
        let ref_range = ident_token.text_range();

        if !ref_name.eq_ignore_ascii_case(target_name) {
            continue;
        }

        if let Some(ResolvedTarget::Symbol(resolved_id)) = resolve_target_at_position_with_context(
            db,
            file_id,
            ref_range.start(),
            &source,
            &root,
            &symbols,
        ) {
            if symbol_identity(&symbols, resolved_id, file_id) == Some(identity) {
                let is_write = is_write_context(&node);
                references.push(Reference {
                    file_id,
                    range: ref_range,
                    is_write,
                });
            }
        }
    }

    for node in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::FieldExpr)
    {
        if let Some((member_id, range)) =
            resolve_field_expr_member(db, file_id, &symbols, &root, &node)
        {
            if symbol_identity(&symbols, member_id, file_id) == Some(identity) {
                let is_write = is_write_context(&node);
                references.push(Reference {
                    file_id,
                    range,
                    is_write,
                });
            }
        }
    }

    references
}

fn find_type_references_across_project(
    db: &Database,
    identity: SymbolIdentity,
    options: FindReferencesOptions,
) -> Vec<Reference> {
    let mut references = Vec::new();
    if options.include_declaration {
        if let Some(reference) = declaration_reference(db, identity) {
            references.push(reference);
        }
    }

    for other_file_id in db.file_ids() {
        references.extend(find_type_references_in_file_by_identity(
            db,
            other_file_id,
            identity,
        ));
    }

    references
}

fn find_type_references_in_file_by_identity(
    db: &Database,
    file_id: FileId,
    identity: SymbolIdentity,
) -> Vec<Reference> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let symbols = db.file_symbols_with_project(file_id);

    let mut references = Vec::new();

    for name_node in root.descendants().filter(|n| n.kind() == SyntaxKind::Name) {
        if !is_type_name_node(&name_node) {
            continue;
        }
        if !is_terminal_type_name(&name_node) {
            continue;
        }

        let Some(ident_token) = ident_token_in_name(&name_node) else {
            continue;
        };

        let Some(symbol_id) = resolve_type_symbol_at_node(&symbols, &root, &name_node) else {
            continue;
        };

        if symbol_identity(&symbols, symbol_id, file_id) == Some(identity) {
            references.push(Reference {
                file_id,
                range: ident_token.text_range(),
                is_write: false,
            });
        }
    }

    references
}

fn declaration_reference(db: &Database, identity: SymbolIdentity) -> Option<Reference> {
    let symbols = db.file_symbols_with_project(identity.file_id);
    let symbol = symbols.get(identity.symbol_id)?;
    Some(Reference {
        file_id: identity.file_id,
        range: symbol.range,
        is_write: false,
    })
}

/// Finds all references to a symbol by ID.
pub fn find_references_to_symbol(
    db: &Database,
    file_id: FileId,
    symbol_id: SymbolId,
    options: FindReferencesOptions,
) -> Vec<Reference> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let symbols = db.file_symbols(file_id);

    find_references_to_symbol_in_context(db, file_id, symbol_id, options, &root, &symbols)
}

fn find_references_to_symbol_in_context(
    db: &Database,
    file_id: FileId,
    symbol_id: SymbolId,
    options: FindReferencesOptions,
    root: &SyntaxNode,
    symbols: &trust_hir::symbols::SymbolTable,
) -> Vec<Reference> {
    let mut references = Vec::new();

    // Get the target symbol info
    let Some(target_symbol) = symbols.get(symbol_id) else {
        return references;
    };
    let target_name = target_symbol.name.clone();

    if is_type_symbol_kind(&target_symbol.kind) {
        return find_type_references_to_symbol(file_id, symbol_id, options, root, symbols);
    }

    // Include declaration if requested
    if options.include_declaration {
        references.push(Reference {
            file_id,
            range: target_symbol.range,
            is_write: false,
        });
    }

    // Find all NameRef nodes that reference our symbol
    for node in root.descendants() {
        if node.kind() != SyntaxKind::NameRef {
            continue;
        }

        // Extract the identifier from NameRef
        let Some(ident_token) = node
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == SyntaxKind::Ident)
        else {
            continue;
        };

        let ref_name = ident_token.text();
        let ref_range = ident_token.text_range();

        // Case-insensitive name check
        if !ref_name.eq_ignore_ascii_case(&target_name) {
            continue;
        }

        // Skip declaration site (we already added it if requested)
        if ref_range == target_symbol.range {
            continue;
        }

        // Resolve this reference in its scope
        let ref_scope = scope_at_position(symbols, root, ref_range.start());
        if let Some(resolved_id) = symbols.resolve(ref_name, ref_scope) {
            if resolved_id == symbol_id {
                let is_write = is_write_context(&node);
                references.push(Reference {
                    file_id,
                    range: ref_range,
                    is_write,
                });
            }
        }
    }

    for node in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::FieldExpr)
    {
        if let Some((member_id, range)) =
            resolve_field_expr_member(db, file_id, symbols, root, &node)
        {
            if member_id == symbol_id {
                let is_write = is_write_context(&node);
                references.push(Reference {
                    file_id,
                    range,
                    is_write,
                });
            }
        } else if let Some((qualified_id, range)) =
            resolve_field_expr_qualified_symbol(symbols, &node)
        {
            if qualified_id == symbol_id {
                let is_write = is_write_context(&node);
                references.push(Reference {
                    file_id,
                    range,
                    is_write,
                });
            }
        }
    }

    references
}

pub(crate) fn find_references_to_field(
    db: &Database,
    file_id: FileId,
    target: &FieldTarget,
    options: FindReferencesOptions,
) -> Vec<Reference> {
    let _ = file_id;
    let mut references = Vec::new();

    for other_file_id in db.file_ids() {
        let source = db.source_text(other_file_id);
        let parsed = parse(&source);
        let root = parsed.syntax();
        let symbols = db.file_symbols_with_project(other_file_id);

        references.extend(find_references_to_field_in_context(
            db,
            other_file_id,
            target,
            options,
            &source,
            &root,
            &symbols,
        ));
    }

    references
}

fn find_references_to_field_in_context(
    db: &Database,
    file_id: FileId,
    target: &FieldTarget,
    options: FindReferencesOptions,
    _source: &str,
    root: &SyntaxNode,
    symbols: &trust_hir::symbols::SymbolTable,
) -> Vec<Reference> {
    let mut references = Vec::new();

    if options.include_declaration {
        for range in field_declaration_ranges(root, symbols, target) {
            references.push(Reference {
                file_id,
                range,
                is_write: false,
            });
        }
    }

    for node in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::FieldExpr)
    {
        let Some(field_name) = field_name_from_field_expr(&node) else {
            continue;
        };
        if !field_name.text().eq_ignore_ascii_case(&target.name) {
            continue;
        }
        let Some(base_type) = field_expr_base_type(db, file_id, symbols, root, &node) else {
            continue;
        };
        let type_matches = field_access_type_matches_target(symbols, base_type, target);
        if !type_matches {
            continue;
        }
        references.push(Reference {
            file_id,
            range: field_name.text_range(),
            is_write: is_write_context(&node),
        });
    }

    references
}

fn find_type_references_to_symbol(
    file_id: FileId,
    target_id: SymbolId,
    options: FindReferencesOptions,
    root: &SyntaxNode,
    symbols: &trust_hir::symbols::SymbolTable,
) -> Vec<Reference> {
    let mut references = Vec::new();

    if options.include_declaration {
        if let Some(symbol) = symbols.get(target_id) {
            references.push(Reference {
                file_id,
                range: symbol.range,
                is_write: false,
            });
        }
    }

    for name_node in root.descendants().filter(|n| n.kind() == SyntaxKind::Name) {
        if !is_type_name_node(&name_node) {
            continue;
        }
        if !is_terminal_type_name(&name_node) {
            continue;
        }
        let Some(ident_token) = ident_token_in_name(&name_node) else {
            continue;
        };
        let Some(symbol_id) = resolve_type_symbol_at_node(symbols, root, &name_node) else {
            continue;
        };
        if symbol_id == target_id {
            references.push(Reference {
                file_id,
                range: ident_token.text_range(),
                is_write: false,
            });
        }
    }

    references
}

// Text-based fallback intentionally omitted to keep references strictly symbol-aware.

fn resolve_field_expr_member(
    db: &Database,
    file_id: FileId,
    symbols: &trust_hir::symbols::SymbolTable,
    root: &SyntaxNode,
    node: &SyntaxNode,
) -> Option<(SymbolId, TextRange)> {
    let name_token = field_name_from_field_expr(node)?;
    let base_type = normalized_field_access_type(
        symbols,
        field_expr_base_type(db, file_id, symbols, root, node)?,
    );
    let member_id = symbols.resolve_member_symbol_in_type(base_type, name_token.text())?;
    Some((member_id, name_token.text_range()))
}

fn field_expr_base_type(
    db: &Database,
    file_id: FileId,
    symbols: &trust_hir::symbols::SymbolTable,
    root: &SyntaxNode,
    node: &SyntaxNode,
) -> Option<TypeId> {
    let base_expr = node.children().next()?;
    if base_expr.kind() == SyntaxKind::NameRef {
        let ident = base_expr
            .descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.kind() == SyntaxKind::Ident)?;
        let scope_id = scope_at_position(symbols, root, base_expr.text_range().start());
        if let Some(symbol_id) = symbols.resolve(ident.text(), scope_id) {
            if let Some(symbol) = symbols.get(symbol_id) {
                let same_file = symbol
                    .origin
                    .map(|origin| origin.file_id == file_id)
                    .unwrap_or(true);
                if same_file
                    && matches!(
                        symbol.kind,
                        trust_hir::symbols::SymbolKind::Variable { .. }
                            | trust_hir::symbols::SymbolKind::Constant
                            | trust_hir::symbols::SymbolKind::Parameter { .. }
                    )
                {
                    return Some(symbols.resolve_alias_type(symbol.type_id));
                }
            }
        }

        // Some declarations (notably FB VAR_INPUT/VAR_OUTPUT members) can be missed
        // by scope-only lookup in recovery-heavy AST states. Fall back to the nearest
        // same-file variable/constant symbol with the same name.
        if let Some(symbol) = symbols
            .iter()
            .filter(|symbol| {
                matches!(
                    symbol.kind,
                    trust_hir::symbols::SymbolKind::Variable { .. }
                        | trust_hir::symbols::SymbolKind::Constant
                        | trust_hir::symbols::SymbolKind::Parameter { .. }
                )
            })
            .filter(|symbol| symbol.name.eq_ignore_ascii_case(ident.text()))
            .filter(|symbol| match symbol.origin {
                Some(origin) => origin.file_id == file_id,
                None => true,
            })
            .filter(|symbol| symbol.range.start() <= base_expr.text_range().start())
            .max_by_key(|symbol| u32::from(symbol.range.start()))
        {
            return Some(symbols.resolve_alias_type(symbol.type_id));
        }
    }

    let offset = u32::from(base_expr.text_range().start());
    let expr_id = db.expr_id_at_offset(file_id, offset)?;
    let base_type = db.type_of(file_id, expr_id);
    Some(symbols.resolve_alias_type(base_type))
}

fn field_name_from_field_expr(node: &SyntaxNode) -> Option<trust_syntax::syntax::SyntaxToken> {
    let name_node = node.children().find(|n| n.kind() == SyntaxKind::Name)?;
    ident_token_in_name(&name_node)
}

fn field_access_type_matches_target(
    symbols: &trust_hir::symbols::SymbolTable,
    base_type: TypeId,
    target: &FieldTarget,
) -> bool {
    let base_type = normalized_field_access_type(symbols, base_type);
    let target_type = normalized_field_access_type(symbols, target.type_id);
    if base_type == target_type {
        return true;
    }

    let base_name = named_type_identity(symbols, base_type);
    let target_name = target
        .type_name
        .clone()
        .or_else(|| named_type_identity(symbols, target_type));
    match (base_name, target_name) {
        (Some(base_name), Some(target_name)) => base_name.eq_ignore_ascii_case(&target_name),
        _ => false,
    }
}

fn normalized_field_access_type(
    symbols: &trust_hir::symbols::SymbolTable,
    type_id: TypeId,
) -> TypeId {
    let mut current = symbols.resolve_alias_type(type_id);
    while let Some(next) = match symbols.type_by_id(current) {
        Some(Type::Reference { target } | Type::Pointer { target }) => {
            Some(symbols.resolve_alias_type(*target))
        }
        _ => None,
    } {
        if next == current {
            break;
        }
        current = next;
    }
    current
}

fn named_type_identity(
    symbols: &trust_hir::symbols::SymbolTable,
    type_id: TypeId,
) -> Option<SmolStr> {
    if let Some(name) = symbols.type_name(type_id) {
        return Some(name);
    }

    match symbols.type_by_id(type_id)? {
        Type::Struct { name, .. }
        | Type::Union { name, .. }
        | Type::Enum { name, .. }
        | Type::FunctionBlock { name }
        | Type::Class { name }
        | Type::Interface { name }
        | Type::Alias { name, .. } => Some(name.clone()),
        _ => None,
    }
}

fn resolve_field_expr_qualified_symbol(
    symbols: &trust_hir::symbols::SymbolTable,
    node: &SyntaxNode,
) -> Option<(SymbolId, TextRange)> {
    let parts = qualified_name_from_field_expr(node)?;
    let symbol_id = symbols.resolve_qualified(&parts)?;
    let range = field_name_from_field_expr(node)?.text_range();
    Some((symbol_id, range))
}

fn is_terminal_type_name(name_node: &SyntaxNode) -> bool {
    let Some(parent) = name_node.parent() else {
        return true;
    };
    if parent.kind() != SyntaxKind::QualifiedName {
        return true;
    }
    let mut last = None;
    for child in parent.children().filter(|n| n.kind() == SyntaxKind::Name) {
        last = Some(child);
    }
    last.is_none_or(|node| node == *name_node)
}

fn symbol_identity(
    symbols: &trust_hir::symbols::SymbolTable,
    symbol_id: SymbolId,
    local_file_id: FileId,
) -> Option<SymbolIdentity> {
    let symbol = symbols.get(symbol_id)?;
    if let Some(origin) = symbol.origin {
        Some(SymbolIdentity {
            file_id: origin.file_id,
            symbol_id: origin.symbol_id,
        })
    } else {
        Some(SymbolIdentity {
            file_id: local_file_id,
            symbol_id,
        })
    }
}

/// Checks if a node is in a write context (LHS of assignment).
fn is_write_context(expr: &SyntaxNode) -> bool {
    let mut current = expr.clone();
    while let Some(parent) = current.parent() {
        if parent.kind() == SyntaxKind::AssignStmt {
            if let Some(first_child) = parent.first_child() {
                return first_child.text_range() == current.text_range();
            }
            return false;
        }
        if matches!(
            parent.kind(),
            SyntaxKind::FieldExpr | SyntaxKind::IndexExpr | SyntaxKind::DerefExpr
        ) {
            current = parent;
            continue;
        }
        break;
    }
    false
}
