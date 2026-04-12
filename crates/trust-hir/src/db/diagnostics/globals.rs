use super::super::queries::*;
use super::super::*;
use super::context::{
    expression_context, find_symbol_by_name_range, is_global_symbol, namespace_path_for_symbol,
    normalized_name,
};
use super::expression::is_expression_kind;

pub(in crate::db) fn resolve_pending_types_with_table(
    symbols: &SymbolTable,
    pending: Vec<PendingType>,
    diagnostics: &mut DiagnosticBuilder,
) {
    for entry in pending {
        if !is_type_defined_in_scope_with_table(symbols, entry.name.as_str(), entry.scope_id) {
            diagnostics.error(
                DiagnosticCode::UndefinedType,
                entry.range,
                format!("cannot resolve type '{}'", entry.name),
            );
        }
    }
}

pub(in crate::db) fn resolve_declared_var_types_with_project(
    symbols: &mut SymbolTable,
    root: &SyntaxNode,
) {
    for var_decl in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
    {
        let Some(type_ref) = var_decl
            .children()
            .find(|child| child.kind() == SyntaxKind::TypeRef)
        else {
            continue;
        };

        if !is_simple_type_ref(&type_ref) {
            continue;
        }

        let Some((parts, _range)) = type_path_from_type_ref(&type_ref) else {
            continue;
        };
        let type_parts: Vec<SmolStr> = parts.iter().map(|(name, _)| name.clone()).collect();
        if type_parts.is_empty() {
            continue;
        }

        let scope_id = expression_context(symbols, &var_decl).scope_id;
        let type_id = resolve_type_path_with_table(symbols, &type_parts, scope_id);
        if type_id == TypeId::UNKNOWN {
            continue;
        }

        for name_node in var_decl
            .children()
            .filter(|child| child.kind() == SyntaxKind::Name)
        {
            let Some((name, range)) = name_from_node(&name_node) else {
                continue;
            };
            let Some(symbol_id) = find_symbol_by_name_range(symbols, name.as_str(), range) else {
                continue;
            };
            let needs_update = symbols
                .get(symbol_id)
                .is_some_and(|symbol| symbol.type_id == TypeId::UNKNOWN);
            if !needs_update {
                continue;
            }
            if let Some(symbol) = symbols.get_mut(symbol_id) {
                symbol.type_id = type_id;
            }
        }
    }
}

fn is_simple_type_ref(node: &SyntaxNode) -> bool {
    !node.descendants().any(|child| {
        matches!(
            child.kind(),
            SyntaxKind::ArrayType
                | SyntaxKind::PointerType
                | SyntaxKind::ReferenceType
                | SyntaxKind::StringType
                | SyntaxKind::Subrange
        )
    })
}

fn resolve_type_path_with_table(
    symbols: &SymbolTable,
    parts: &[SmolStr],
    scope_id: ScopeId,
) -> TypeId {
    if parts.is_empty() {
        return TypeId::UNKNOWN;
    }

    if parts.len() == 1 {
        let name = parts[0].as_str();
        if let Some(id) = TypeId::from_builtin_name(name) {
            return id;
        }
        if let Some(symbol_id) = symbols.resolve(name, scope_id) {
            if let Some(symbol) = symbols.get(symbol_id) {
                if symbol.is_type() {
                    return symbol.type_id;
                }
            }
        }
        if let Some(id) = symbols.lookup_type(name) {
            return id;
        }
        return TypeId::UNKNOWN;
    }

    let Some(symbol_id) = symbols.resolve_qualified(parts) else {
        return TypeId::UNKNOWN;
    };
    let Some(symbol) = symbols.get(symbol_id) else {
        return TypeId::UNKNOWN;
    };
    if symbol.is_type() {
        symbol.type_id
    } else {
        TypeId::UNKNOWN
    }
}

pub(in crate::db) fn is_type_defined_in_scope_with_table(
    symbols: &SymbolTable,
    name: &str,
    scope_id: ScopeId,
) -> bool {
    if name.contains('.') {
        let parts: Vec<SmolStr> = name.split('.').map(SmolStr::new).collect();
        let Some(symbol_id) = symbols.resolve_qualified(&parts) else {
            return false;
        };
        return symbols.get(symbol_id).is_some_and(|sym| sym.is_type());
    }

    if TypeId::from_builtin_name(name).is_some() {
        return true;
    }

    if let Some(symbol_id) = symbols.resolve(name, scope_id) {
        if let Some(symbol) = symbols.get(symbol_id) {
            if symbol.is_type() {
                return true;
            }
        }
    }

    symbols.lookup_type(name).is_some()
}

#[derive(Hash, PartialEq, Eq)]
struct GlobalKey {
    namespace: Vec<SmolStr>,
    name: SmolStr,
}

struct GlobalInfo {
    type_id: TypeId,
    is_constant: bool,
    origin: SymbolOrigin,
    range: TextRange,
}

pub(in crate::db) fn check_global_external_links_with_project(
    symbols: &mut SymbolTable,
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
    file_id: FileId,
) {
    let mut globals: FxHashMap<GlobalKey, GlobalInfo> = FxHashMap::default();

    for symbol in symbols.iter() {
        if !is_global_symbol(symbols, symbol) {
            continue;
        }
        let namespace = namespace_path_for_symbol(symbols, symbol.id);
        let key = GlobalKey {
            namespace,
            name: normalized_name(symbol.name.as_str()),
        };
        let is_constant = matches!(symbol.kind, SymbolKind::Constant);
        let origin = symbol.origin.unwrap_or(SymbolOrigin {
            file_id,
            symbol_id: symbol.id,
        });
        let info = GlobalInfo {
            type_id: symbol.type_id,
            is_constant,
            origin,
            range: symbol.range,
        };
        if let Some(existing) = globals.insert(key, info) {
            let diagnostic = Diagnostic::error(
                DiagnosticCode::DuplicateDeclaration,
                symbol.range,
                format!("duplicate global declaration of '{}'", symbol.name),
            )
            .with_related(existing.range, "previously declared here");
            diagnostics.add(diagnostic);
        }
    }

    for block in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::VarBlock)
    {
        let qualifier = var_qualifier_from_block(&block);
        if qualifier != VarQualifier::External {
            continue;
        }
        let is_constant = var_block_is_constant(&block);
        for var_decl in block.children().filter(|n| n.kind() == SyntaxKind::VarDecl) {
            let has_initializer = var_decl.children().any(|n| is_expression_kind(n.kind()));
            for name_node in var_decl.children().filter(|n| n.kind() == SyntaxKind::Name) {
                let Some((name, range)) = name_from_node(&name_node) else {
                    continue;
                };
                let symbol_id = find_symbol_by_name_range(symbols, name.as_str(), range);
                let type_id = symbol_id
                    .and_then(|id| symbols.get(id))
                    .map(|sym| sym.type_id)
                    .unwrap_or(TypeId::UNKNOWN);
                let namespace = symbol_id
                    .map(|id| namespace_path_for_symbol(symbols, id))
                    .unwrap_or_default();
                let key = GlobalKey {
                    namespace,
                    name: normalized_name(name.as_str()),
                };
                let Some(global) = globals.get(&key) else {
                    diagnostics.error(
                        DiagnosticCode::UndefinedVariable,
                        range,
                        format!("VAR_EXTERNAL '{}' has no matching VAR_GLOBAL", name),
                    );
                    continue;
                };

                let mut link_ok = true;
                let target_type = symbols.resolve_alias_type(global.type_id);
                let source_type = symbols.resolve_alias_type(type_id);
                if target_type != TypeId::UNKNOWN
                    && source_type != TypeId::UNKNOWN
                    && target_type != source_type
                {
                    diagnostics.error(
                        DiagnosticCode::TypeMismatch,
                        range,
                        format!(
                            "VAR_EXTERNAL '{}' type '{}' does not match VAR_GLOBAL type '{}'",
                            name,
                            symbols.type_name(source_type).unwrap_or_else(|| "?".into()),
                            symbols.type_name(target_type).unwrap_or_else(|| "?".into())
                        ),
                    );
                    link_ok = false;
                }

                if global.is_constant && !is_constant {
                    diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        range,
                        format!(
                            "VAR_EXTERNAL '{}' must be CONSTANT to match VAR_GLOBAL CONSTANT",
                            name
                        ),
                    );
                    link_ok = false;
                }

                if has_initializer {
                    diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        range,
                        format!("VAR_EXTERNAL '{}' cannot declare an initial value", name),
                    );
                    link_ok = false;
                }

                if link_ok {
                    if let Some(symbol_id) = symbol_id {
                        if let Some(symbol) = symbols.get_mut(symbol_id) {
                            symbol.origin = Some(global.origin);
                        }
                    }
                }
            }
        }
    }
}
