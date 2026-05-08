//! Call hierarchy support for Structured Text.

use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use std::sync::Arc;
use text_size::{TextRange, TextSize};

use rustc_hash::FxHashSet;
use trust_hir::db::{FileId, SemanticDatabase, SourceDatabase};
use trust_hir::symbols::{Symbol, SymbolId, SymbolKind, SymbolTable};
use trust_hir::types::Type;
use trust_hir::Database;
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::util::{
    find_enclosing_pou, is_pou_symbol_kind, resolve_target_at_position_with_context,
    scope_at_position,
};

/// A call hierarchy item for ST symbols.
#[derive(Debug, Clone)]
pub struct CallHierarchyItem {
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

/// Incoming call information.
#[derive(Debug, Clone)]
pub struct CallHierarchyIncomingCall {
    /// Caller item.
    pub from: CallHierarchyItem,
    /// Call site ranges in the caller.
    pub from_ranges: Vec<TextRange>,
}

/// Outgoing call information.
#[derive(Debug, Clone)]
pub struct CallHierarchyOutgoingCall {
    /// Callee item.
    pub to: CallHierarchyItem,
    /// Call site ranges in the caller.
    pub from_ranges: Vec<TextRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SymbolKey {
    file_id: FileId,
    symbol_id: SymbolId,
}

#[derive(Debug, Clone)]
struct CallEdge {
    caller: SymbolKey,
    callee: SymbolKey,
    range: TextRange,
}

/// Prepares a call hierarchy item at the given position.
pub fn prepare_call_hierarchy(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<CallHierarchyItem> {
    prepare_call_hierarchy_in_files(db, file_id, position, None)
}

/// Prepares a call hierarchy item at the given position, scoped to a file set.
pub fn prepare_call_hierarchy_in_files(
    db: &Database,
    file_id: FileId,
    position: TextSize,
    allowed_files: Option<&FxHashSet<FileId>>,
) -> Option<CallHierarchyItem> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let symbols = symbols_for_call_hierarchy(db, file_id, allowed_files);

    let target =
        resolve_target_at_position_with_context(db, file_id, position, &source, &root, &symbols);
    let target = match target {
        Some(target) => target,
        None => {
            return fallback_prepare_call_hierarchy(db, file_id, position, &root);
        }
    };
    let symbol_id = match target {
        crate::util::ResolvedTarget::Symbol(symbol_id) => symbol_id,
        crate::util::ResolvedTarget::Field(_) => return None,
    };
    let symbol = symbols.get(symbol_id)?;
    if !is_pou_symbol_kind(&symbol.kind) {
        return fallback_prepare_call_hierarchy(db, file_id, position, &root);
    }

    let key = symbol_key(&symbols, symbol_id, file_id)?;
    call_hierarchy_item_for_key(db, key)
}

fn fallback_prepare_call_hierarchy(
    db: &Database,
    file_id: FileId,
    position: TextSize,
    root: &SyntaxNode,
) -> Option<CallHierarchyItem> {
    let pou_node = find_enclosing_pou(root, position)?;
    let name = pou_name_from_node(&pou_node)?;
    let local_symbols = db.file_symbols(file_id);
    let symbol = local_symbols
        .iter()
        .find(|sym| sym.name.eq_ignore_ascii_case(&name) && is_pou_symbol_kind(&sym.kind))?;
    let key = SymbolKey {
        file_id,
        symbol_id: symbol.id,
    };
    call_hierarchy_item_for_key(db, key)
}

fn pou_name_from_node(pou_node: &SyntaxNode) -> Option<SmolStr> {
    let name_node = pou_node.children().find(|n| n.kind() == SyntaxKind::Name)?;
    let ident = name_node
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == SyntaxKind::Ident)?;
    Some(SmolStr::new(ident.text()))
}

/// Resolves incoming calls for the given call hierarchy item.
pub fn incoming_calls(db: &Database, item: &CallHierarchyItem) -> Vec<CallHierarchyIncomingCall> {
    incoming_calls_in_files(db, item, None)
}

/// Resolves incoming calls for the given call hierarchy item, scoped to a file set.
pub fn incoming_calls_in_files(
    db: &Database,
    item: &CallHierarchyItem,
    allowed_files: Option<&FxHashSet<FileId>>,
) -> Vec<CallHierarchyIncomingCall> {
    let edges = collect_call_edges_in_files(db, allowed_files);
    let key = SymbolKey {
        file_id: item.file_id,
        symbol_id: item.symbol_id,
    };
    let mut grouped: FxHashMap<SymbolKey, Vec<TextRange>> = FxHashMap::default();
    for edge in edges.into_iter().filter(|edge| edge.callee == key) {
        grouped.entry(edge.caller).or_default().push(edge.range);
    }

    grouped
        .into_iter()
        .filter_map(|(caller, ranges)| {
            let from = call_hierarchy_item_for_key(db, caller)?;
            Some(CallHierarchyIncomingCall {
                from,
                from_ranges: ranges,
            })
        })
        .collect()
}

/// Resolves outgoing calls for the given call hierarchy item.
pub fn outgoing_calls(db: &Database, item: &CallHierarchyItem) -> Vec<CallHierarchyOutgoingCall> {
    outgoing_calls_in_files(db, item, None)
}

/// Resolves outgoing calls for the given call hierarchy item, scoped to a file set.
pub fn outgoing_calls_in_files(
    db: &Database,
    item: &CallHierarchyItem,
    allowed_files: Option<&FxHashSet<FileId>>,
) -> Vec<CallHierarchyOutgoingCall> {
    let edges = collect_call_edges_in_files(db, allowed_files);
    let key = SymbolKey {
        file_id: item.file_id,
        symbol_id: item.symbol_id,
    };
    let mut grouped: FxHashMap<SymbolKey, Vec<TextRange>> = FxHashMap::default();
    for edge in edges.into_iter().filter(|edge| edge.caller == key) {
        grouped.entry(edge.callee).or_default().push(edge.range);
    }

    grouped
        .into_iter()
        .filter_map(|(callee, ranges)| {
            let to = call_hierarchy_item_for_key(db, callee)?;
            Some(CallHierarchyOutgoingCall {
                to,
                from_ranges: ranges,
            })
        })
        .collect()
}

fn call_hierarchy_item_for_key(db: &Database, key: SymbolKey) -> Option<CallHierarchyItem> {
    let symbols = db.file_symbols(key.file_id);
    let symbol = symbols.get(key.symbol_id)?;
    if !is_pou_symbol_kind(&symbol.kind) {
        return None;
    }

    Some(CallHierarchyItem {
        name: symbol.name.clone(),
        kind: symbol.kind.clone(),
        file_id: key.file_id,
        range: symbol.range,
        selection_range: symbol.range,
        symbol_id: key.symbol_id,
    })
}

fn fb_callee_for_instance_variable(
    symbols: &SymbolTable,
    var_symbol: &Symbol,
    file_id: FileId,
) -> Option<SymbolKey> {
    let resolved_type = symbols.resolve_alias_type(var_symbol.type_id);
    let type_name = match symbols.type_by_id(resolved_type)? {
        Type::FunctionBlock { name } | Type::Class { name } | Type::Interface { name } => {
            name.clone()
        }
        _ => return None,
    };
    let owner_id = symbols.resolve_global_or_qualified_name(type_name.as_str())?;
    symbol_key(symbols, owner_id, file_id)
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

fn symbols_for_call_hierarchy(
    db: &Database,
    file_id: FileId,
    allowed_files: Option<&FxHashSet<FileId>>,
) -> Arc<SymbolTable> {
    if let Some(allowed) = allowed_files {
        db.file_symbols_with_project_filtered(file_id, allowed)
    } else {
        db.file_symbols_with_project(file_id)
    }
}

fn collect_call_edges_in_files(
    db: &Database,
    allowed_files: Option<&FxHashSet<FileId>>,
) -> Vec<CallEdge> {
    let mut edges = Vec::new();
    let file_ids: Vec<FileId> = match allowed_files {
        Some(files) => {
            let mut ids: Vec<_> = files.iter().copied().collect();
            ids.sort_by_key(|id| id.0);
            ids
        }
        None => db.file_ids(),
    };
    let unique_pou = build_unique_pou_map(db, &file_ids);

    for file_id in file_ids {
        let source = db.source_text(file_id);
        let parsed = parse(&source);
        let root = parsed.syntax();
        let symbols = symbols_for_call_hierarchy(db, file_id, allowed_files);

        for call_expr in root
            .descendants()
            .filter(|n| n.kind() == SyntaxKind::CallExpr)
        {
            let Some(callee_offset) = callee_name_offset(&call_expr) else {
                continue;
            };

            let target = resolve_target_at_position_with_context(
                db,
                file_id,
                callee_offset,
                &source,
                &root,
                &symbols,
            );

            let mut callee_key = None;
            if let Some(crate::util::ResolvedTarget::Symbol(symbol_id)) = target {
                if let Some(symbol) = symbols.get(symbol_id) {
                    if is_pou_symbol_kind(&symbol.kind) {
                        callee_key = symbol_key(&symbols, symbol_id, file_id);
                    } else {
                        // FB-instance call (`var(...)` where `var : SomeFB`).
                        // The call expression resolves to a Variable, not a
                        // callable — but the call target is the underlying
                        // FB/Class/Interface type. This is the dominant
                        // invocation pattern in IEC 61131-3.
                        callee_key = fb_callee_for_instance_variable(&symbols, symbol, file_id);
                    }
                }
            }

            if callee_key.is_none() {
                if let Some(name) = callee_name_from_call_expr(&call_expr) {
                    let key = SmolStr::new(name.to_ascii_uppercase());
                    if let Some(Some(unique_key)) = unique_pou.get(&key) {
                        callee_key = Some(*unique_key);
                    }
                }
            }

            let Some(callee_key) = callee_key else {
                continue;
            };

            let caller_key =
                caller_symbol_key(&symbols, &root, call_expr.text_range().start(), file_id);
            let Some(caller_key) = caller_key else {
                continue;
            };
            edges.push(CallEdge {
                caller: caller_key,
                callee: callee_key,
                range: call_expr.text_range(),
            });
        }
    }
    edges
}

fn build_unique_pou_map(
    db: &Database,
    file_ids: &[FileId],
) -> FxHashMap<SmolStr, Option<SymbolKey>> {
    let mut map: FxHashMap<SmolStr, Option<SymbolKey>> = FxHashMap::default();
    for &file_id in file_ids {
        let symbols = db.file_symbols(file_id);
        for symbol in symbols.iter() {
            if symbol.parent.is_some() {
                continue;
            }
            if !is_pou_symbol_kind(&symbol.kind) {
                continue;
            }
            let key = SmolStr::new(symbol.name.to_ascii_uppercase());
            let entry = map.entry(key).or_insert_with(|| {
                Some(SymbolKey {
                    file_id,
                    symbol_id: symbol.id,
                })
            });
            if entry.is_some() {
                *entry = None;
            }
        }
    }
    map
}

fn caller_symbol_key(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    position: TextSize,
    file_id: FileId,
) -> Option<SymbolKey> {
    let scope_id = scope_at_position(symbols, root, position);
    let scope = symbols.get_scope(scope_id)?;
    let owner = scope.owner?;
    symbol_key(symbols, owner, file_id)
}

fn callee_name_offset(call_expr: &SyntaxNode) -> Option<TextSize> {
    let callee = call_expr
        .children()
        .find(|child| child.kind() != SyntaxKind::ArgList)?;

    match callee.kind() {
        SyntaxKind::NameRef => callee
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| token.kind() == SyntaxKind::Ident)
            .map(|token| token.text_range().start()),
        SyntaxKind::FieldExpr => callee
            .descendants()
            .filter(|child| child.kind() == SyntaxKind::NameRef)
            .last()
            .and_then(|child| {
                child
                    .descendants_with_tokens()
                    .filter_map(|element| element.into_token())
                    .find(|token| token.kind() == SyntaxKind::Ident)
                    .map(|token| token.text_range().start())
            }),
        _ => None,
    }
}

fn callee_name_from_call_expr(call_expr: &SyntaxNode) -> Option<String> {
    let callee = call_expr
        .children()
        .find(|child| child.kind() != SyntaxKind::ArgList)?;

    if callee.kind() != SyntaxKind::NameRef {
        return None;
    }

    callee
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| token.text().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_hir::db::{Database, FileId, SourceDatabase};

    #[test]
    fn call_hierarchy_outgoing_collects_calls() {
        let source = r#"
FUNCTION Add : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    Add := A + B;
END_FUNCTION

PROGRAM Main
VAR
    result : INT;
END_VAR
    result := Add(1, 2);
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let position = TextSize::from(source.find("Main").expect("main") as u32);
        let item = prepare_call_hierarchy(&db, file_id, position).expect("prepare");
        let outgoing = outgoing_calls(&db, &item);
        assert_eq!(outgoing.len(), 1);
        assert!(outgoing[0].to.name.eq_ignore_ascii_case("Add"));
    }

    #[test]
    fn call_hierarchy_tracks_fb_instance_calls() {
        // FB-instance calls (`var(...)` where `var : SomeFB`) are the dominant
        // invocation pattern in IEC 61131-3. They must surface as call edges
        // pointing to the FB itself, not get dropped because the call-expr
        // resolves to a Variable rather than a callable. Without this, call
        // hierarchy on an FB returns empty incoming calls in any real ST
        // project that uses FB instances.
        let source = r#"
FUNCTION_BLOCK LevelController
VAR_INPUT
    Speed : INT;
END_VAR
    ;
END_FUNCTION_BLOCK

PROGRAM MainProgram
VAR
    Ctrl : LevelController;
END_VAR
    Ctrl(Speed := 5);
END_PROGRAM
"#;
        let mut db = Database::new();
        let file_id = FileId(0);
        db.set_source_text(file_id, source.to_string());

        let position = TextSize::from(source.find("LevelController").expect("FB name") as u32);
        let item = prepare_call_hierarchy(&db, file_id, position).expect("prepare");

        let incoming = incoming_calls(&db, &item);
        assert_eq!(
            incoming.len(),
            1,
            "expected one incoming call from MainProgram via Ctrl(...) but got {} ({:?})",
            incoming.len(),
            incoming
                .iter()
                .map(|c| c.from.name.as_str())
                .collect::<Vec<_>>()
        );
        assert!(
            incoming[0].from.name.eq_ignore_ascii_case("MainProgram"),
            "expected caller to be MainProgram, got {}",
            incoming[0].from.name
        );
    }

    #[test]
    fn call_hierarchy_respects_allowed_files() {
        let source_main = r#"
PROGRAM Main
VAR
    result : DINT;
END_VAR
    result := AddTwo(1, 2);
END_PROGRAM
"#;
        let source_primary = r#"
FUNCTION AddTwo : DINT
VAR_INPUT
    A : DINT;
    B : DINT;
END_VAR
    AddTwo := A + B;
END_FUNCTION
"#;
        let source_shadow = r#"
FUNCTION AddTwo : DINT
VAR_INPUT
    A : DINT;
    B : DINT;
END_VAR
    AddTwo := A - B;
END_FUNCTION
"#;

        let mut db = Database::new();
        let file_main = FileId(0);
        let file_primary = FileId(1);
        let file_shadow = FileId(2);
        db.set_source_text(file_main, source_main.to_string());
        db.set_source_text(file_primary, source_primary.to_string());
        db.set_source_text(file_shadow, source_shadow.to_string());

        let position = TextSize::from(source_primary.find("AddTwo").expect("name") as u32);
        let mut allowed = FxHashSet::default();
        allowed.insert(file_main);
        allowed.insert(file_primary);

        let item = prepare_call_hierarchy_in_files(&db, file_primary, position, Some(&allowed))
            .expect("prepare");
        let incoming = incoming_calls_in_files(&db, &item, Some(&allowed));
        assert_eq!(incoming.len(), 1);
        assert!(incoming[0].from.name.eq_ignore_ascii_case("Main"));
    }
}
