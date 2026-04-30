//! Type hierarchy support for Structured Text.

use rustc_hash::FxHashSet;
use smol_str::SmolStr;
use text_size::{TextRange, TextSize};

use trust_hir::db::{FileId, SemanticDatabase, SourceDatabase};
use trust_hir::symbols::{ScopeId, Symbol, SymbolId, SymbolKind, SymbolTable};
use trust_hir::{Database, Type};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::util::{
    resolve_target_at_position_with_context, resolve_type_symbol, resolve_type_symbol_at_node,
};

/// A type hierarchy item for ST types.
#[derive(Debug, Clone)]
pub struct TypeHierarchyItem {
    /// Symbol name.
    pub name: SmolStr,
    /// Symbol kind.
    pub kind: SymbolKind,
    /// File containing the symbol.
    pub file_id: FileId,
    /// Full symbol range.
    pub range: TextRange,
    /// Selection range (name).
    pub selection_range: TextRange,
    /// Symbol ID.
    pub symbol_id: SymbolId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SymbolKey {
    file_id: FileId,
    symbol_id: SymbolId,
}

/// Prepares a type hierarchy item at the given position.
pub fn prepare_type_hierarchy(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<TypeHierarchyItem> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let symbols = db.file_symbols_with_project(file_id);

    let mut symbol_id =
        resolve_target_at_position_with_context(db, file_id, position, &source, &root, &symbols)
            .and_then(|target| match target {
                crate::util::ResolvedTarget::Symbol(symbol_id) => Some(symbol_id),
                crate::util::ResolvedTarget::Field(_) => None,
            })
            .filter(|id| symbols.get(*id).is_some_and(|symbol| symbol.is_type()));

    if symbol_id.is_none() {
        if let Some(name_node) = type_name_node_at_position(&root, position) {
            symbol_id = resolve_type_symbol_at_node(&symbols, &root, &name_node);
        }
    }

    let symbol_id = symbol_id?;
    let symbol = symbols.get(symbol_id)?;
    if !symbol.is_type() {
        return None;
    }

    let key = symbol_key(&symbols, symbol_id, file_id)?;
    type_hierarchy_item_for_key(db, key)
}

/// Resolves supertypes for a type hierarchy item.
pub fn supertypes(db: &Database, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem> {
    let symbols = db.file_symbols_with_project(item.file_id);
    let Some(symbol) = symbols.get(item.symbol_id) else {
        return Vec::new();
    };

    let scope_id = symbols
        .scope_for_owner(symbol.id)
        .unwrap_or(ScopeId::GLOBAL);

    let mut items = Vec::new();
    if let Some(base) = symbols.extends_reference(symbol.id) {
        let base_name = base.name().display();
        if let Some(base_id) = resolve_type_symbol_in_scope(&symbols, base_name.as_str(), scope_id)
        {
            if let Some(key) = symbol_key(&symbols, base_id, item.file_id) {
                if let Some(item) = type_hierarchy_item_for_key(db, key) {
                    items.push(item);
                }
            }
        }
    }

    for interface in symbols.implements_references(symbol.id) {
        let name = interface.name().display();
        if let Some(interface_id) = resolve_type_symbol_in_scope(&symbols, name.as_str(), scope_id)
        {
            if let Some(key) = symbol_key(&symbols, interface_id, item.file_id) {
                if let Some(item) = type_hierarchy_item_for_key(db, key) {
                    items.push(item);
                }
            }
        }
    }

    if matches!(symbol.kind, SymbolKind::Type) {
        if let Some(Type::Alias { target, .. }) = symbols.type_by_id(symbol.type_id) {
            if let Some(target_name) = symbols.type_name(*target) {
                if let Some(target_id) =
                    resolve_type_symbol_in_scope(&symbols, target_name.as_str(), scope_id)
                {
                    if let Some(key) = symbol_key(&symbols, target_id, item.file_id) {
                        if let Some(item) = type_hierarchy_item_for_key(db, key) {
                            items.push(item);
                        }
                    }
                }
            }
        }
    }

    items
}

/// Resolves subtypes for a type hierarchy item.
pub fn subtypes(db: &Database, item: &TypeHierarchyItem) -> Vec<TypeHierarchyItem> {
    let target_key = SymbolKey {
        file_id: item.file_id,
        symbol_id: item.symbol_id,
    };
    let mut seen: FxHashSet<SymbolKey> = FxHashSet::default();
    let mut items = Vec::new();

    for file_id in db.file_ids() {
        let symbols = db.file_symbols_with_project(file_id);
        for symbol in symbols.iter() {
            if !symbol.is_type() {
                continue;
            }
            let Some(key) = symbol_key(&symbols, symbol.id, file_id) else {
                continue;
            };
            if key == target_key || !seen.insert(key) {
                continue;
            }

            if is_subtype_of(&symbols, symbol, target_key, item) {
                if let Some(item) = type_hierarchy_item_for_key(db, key) {
                    items.push(item);
                }
            }
        }
    }

    items
}

fn is_subtype_of(
    symbols: &SymbolTable,
    symbol: &Symbol,
    target_key: SymbolKey,
    target_item: &TypeHierarchyItem,
) -> bool {
    let scope_id = symbols
        .scope_for_owner(symbol.id)
        .unwrap_or(ScopeId::GLOBAL);
    if let Some(base) = symbols.extends_reference(symbol.id) {
        let base_name = base.name().display();
        if let Some(base_id) = resolve_type_symbol_in_scope(symbols, base_name.as_str(), scope_id) {
            if let Some(key) = symbol_key(symbols, base_id, target_item.file_id) {
                if key == target_key {
                    return true;
                }
            }
        }
    }

    for interface in symbols.implements_references(symbol.id) {
        let name = interface.name().display();
        if let Some(interface_id) = resolve_type_symbol_in_scope(symbols, name.as_str(), scope_id) {
            if let Some(key) = symbol_key(symbols, interface_id, target_item.file_id) {
                if key == target_key {
                    return true;
                }
            }
        }
    }

    if matches!(symbol.kind, SymbolKind::Type) {
        if let Some(Type::Alias { target, .. }) = symbols.type_by_id(symbol.type_id) {
            if let Some(target_name) = symbols.type_name(*target) {
                if let Some(target_id) =
                    resolve_type_symbol_in_scope(symbols, target_name.as_str(), scope_id)
                {
                    if let Some(key) = symbol_key(symbols, target_id, target_item.file_id) {
                        return key == target_key;
                    }
                }
            }
        }
    }

    false
}

fn type_hierarchy_item_for_key(db: &Database, key: SymbolKey) -> Option<TypeHierarchyItem> {
    let symbols = db.file_symbols(key.file_id);
    let symbol = symbols.get(key.symbol_id)?;
    if !symbol.is_type() {
        return None;
    }

    Some(TypeHierarchyItem {
        name: symbol.name.clone(),
        kind: symbol.kind.clone(),
        file_id: key.file_id,
        range: symbol.range,
        selection_range: symbol.range,
        symbol_id: key.symbol_id,
    })
}

fn symbol_key(symbols: &SymbolTable, symbol_id: SymbolId, file_id: FileId) -> Option<SymbolKey> {
    let symbol = symbols.get(symbol_id)?;
    if let Some(origin) = symbol.origin {
        Some(SymbolKey {
            file_id: origin.file_id,
            symbol_id: origin.symbol_id,
        })
    } else {
        Some(SymbolKey { file_id, symbol_id })
    }
}

fn resolve_type_symbol_in_scope(
    symbols: &SymbolTable,
    name: &str,
    scope_id: ScopeId,
) -> Option<SymbolId> {
    let parts: Vec<SmolStr> = name.split('.').map(SmolStr::new).collect();
    resolve_type_symbol(symbols, &parts, scope_id)
}

fn type_name_node_at_position(root: &SyntaxNode, position: TextSize) -> Option<SyntaxNode> {
    let token = root.token_at_offset(position).right_biased()?;
    token
        .parent_ancestors()
        .find(|node| matches!(node.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_hir::db::{Database, FileId, SourceDatabase};

    #[test]
    fn type_hierarchy_extends_and_implements() {
        let source = r#"
INTERFACE ITest
END_INTERFACE

CLASS Base
END_CLASS

CLASS Derived EXTENDS Base IMPLEMENTS ITest
END_CLASS
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let derived_pos = TextSize::from(source.find("Derived").unwrap() as u32);
        let derived = prepare_type_hierarchy(&db, file_id, derived_pos).expect("derived");
        let supers = supertypes(&db, &derived);
        assert_eq!(supers.len(), 2);

        let base_pos = TextSize::from(source.find("Base").unwrap() as u32);
        let base = prepare_type_hierarchy(&db, file_id, base_pos).expect("base");
        let subs = subtypes(&db, &base);
        assert_eq!(subs.len(), 1);
        assert!(subs[0].name.eq_ignore_ascii_case("Derived"));
    }

    #[test]
    fn type_hierarchy_resolves_interfaces_in_namespace() {
        let source = r#"
NAMESPACE Repro
INTERFACE IFoo
END_INTERFACE

CLASS Foo IMPLEMENTS IFoo
END_CLASS
END_NAMESPACE
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let foo_offset = source.find("CLASS Foo").unwrap() + "CLASS ".len();
        let foo_pos = TextSize::from(foo_offset as u32);
        let foo = prepare_type_hierarchy(&db, file_id, foo_pos).expect("foo");
        let supers = supertypes(&db, &foo);
        assert_eq!(supers.len(), 1);
        assert!(supers[0].name.eq_ignore_ascii_case("IFoo"));
    }
}
