use super::super::queries::*;
use super::super::*;
use super::context::{
    find_symbol_by_name_range, namespace_path_for_symbol,
    resolve_type_symbol_by_name_in_scope_outcome,
};
use crate::semantic::SemanticOutcome;

mod interfaces;
mod modifiers;
mod overrides;
mod shadowing;

pub(in crate::db) use interfaces::check_interface_conformance;

pub(super) fn visibility_label(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Public => "PUBLIC",
        Visibility::Protected => "PROTECTED",
        Visibility::Private => "PRIVATE",
        Visibility::Internal => "INTERNAL",
    }
}

pub(in crate::db) fn check_class_semantics(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    for node in root.descendants().filter(|n| n.kind() == SyntaxKind::Class) {
        let Some((class_name, class_range)) = name_from_node(&node) else {
            continue;
        };
        let Some(class_id) = find_symbol_by_name_range(symbols, class_name.as_str(), class_range)
        else {
            continue;
        };
        let Some(class_symbol) = symbols.get(class_id) else {
            continue;
        };

        let declared_methods: Vec<SymbolId> = symbols
            .iter()
            .filter(|sym| sym.parent == Some(class_id))
            .filter(|sym| matches!(sym.kind, SymbolKind::Method { .. }))
            .map(|sym| sym.id)
            .collect();

        let declared_vars: Vec<SymbolId> = symbols
            .iter()
            .filter(|sym| sym.parent == Some(class_id))
            .filter(|sym| matches!(sym.kind, SymbolKind::Variable { .. } | SymbolKind::Constant))
            .map(|sym| sym.id)
            .collect();

        modifiers::check_class_modifiers(
            symbols,
            class_symbol,
            class_range,
            &declared_methods,
            diagnostics,
        );

        let extends_clause = node
            .children()
            .find(|child| child.kind() == SyntaxKind::ExtendsClause);
        let extends_range = if let Some(clause) = extends_clause.as_ref() {
            qualified_name_parts(clause)
                .map(|(_, range)| range)
                .unwrap_or_else(|| clause.text_range())
        } else {
            class_range
        };

        if extends_clause.is_some() && class_inheritance_cycle(symbols, class_id) {
            diagnostics.error(
                DiagnosticCode::CyclicDependency,
                extends_range,
                "class inheritance cycle detected",
            );
        }

        if let Some(base_id) = resolve_extends_symbol(symbols, class_id) {
            if let Some(base_symbol) = symbols.get(base_id) {
                if !matches!(base_symbol.kind, SymbolKind::Class) {
                    diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        extends_range,
                        "CLASS can only EXTENDS another CLASS",
                    );
                } else if base_symbol.modifiers.is_final {
                    diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        extends_range,
                        format!("cannot extend FINAL class '{}'", base_symbol.name),
                    );
                }
            }
        }

        let inherited_vars = collect_inherited_variables(symbols, class_id);
        let inherited_methods = collect_inherited_methods(symbols, class_id);

        shadowing::check_member_shadowing(
            symbols,
            &declared_vars,
            &declared_methods,
            &inherited_vars,
            diagnostics,
        );

        overrides::check_method_overrides(
            symbols,
            class_symbol,
            class_range,
            &declared_methods,
            &inherited_methods,
            diagnostics,
        );
    }
}

pub(in crate::db) fn check_abstract_instantiations(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    for var_decl in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::VarDecl)
    {
        for name_node in var_decl.children().filter(|n| n.kind() == SyntaxKind::Name) {
            let Some((name, range)) = name_from_node(&name_node) else {
                continue;
            };
            let Some(symbol_id) = find_symbol_by_name_range(symbols, name.as_str(), range) else {
                continue;
            };
            let Some(symbol) = symbols.get(symbol_id) else {
                continue;
            };
            let symbol_type = symbols.resolve_alias_type(symbol.type_id);
            let resolved_type = symbol_type;

            let Some(Type::Class { name: class_name }) = symbols.type_by_id(resolved_type) else {
                continue;
            };
            let Some(class_id) = symbols.resolve_global_or_qualified_name(class_name.as_str())
            else {
                continue;
            };
            let Some(class_symbol) = symbols.get(class_id) else {
                continue;
            };
            if !class_symbol.modifiers.is_abstract {
                continue;
            }

            if let SymbolKind::Parameter {
                direction: ParamDirection::In | ParamDirection::InOut,
            } = symbol.kind
            {
                continue;
            }

            diagnostics.error(
                DiagnosticCode::InvalidOperation,
                range,
                format!(
                    "cannot instantiate ABSTRACT class '{}' here",
                    class_symbol.name
                ),
            );
        }
    }
}

pub(in crate::db) fn check_property_accessors(
    symbols: &SymbolTable,
    diagnostics: &mut DiagnosticBuilder,
) {
    for sym in symbols.iter() {
        let SymbolKind::Property {
            has_get, has_set, ..
        } = sym.kind
        else {
            continue;
        };
        if !has_get && !has_set {
            diagnostics.error(
                DiagnosticCode::InvalidOperation,
                sym.range,
                format!("property '{}' must declare GET or SET", sym.name),
            );
        }
    }
}

pub(in crate::db) fn check_extends_implements_semantics(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::Interface => {
                let extends_clause = node
                    .children()
                    .find(|child| child.kind() == SyntaxKind::ExtendsClause);
                let Some(clause) = extends_clause.as_ref() else {
                    continue;
                };
                let extends_range = qualified_name_parts(clause)
                    .map(|(_, range)| range)
                    .unwrap_or_else(|| clause.text_range());
                let Some((name, range)) = name_from_node(&node) else {
                    continue;
                };
                let Some(interface_id) = find_symbol_by_name_range(symbols, name.as_str(), range)
                else {
                    continue;
                };
                if let SemanticOutcome::Resolved(base_id) =
                    resolve_extends_symbol_outcome(symbols, interface_id, Some(extends_range))
                {
                    if let Some(base_symbol) = symbols.get(base_id) {
                        if !matches!(base_symbol.kind, SymbolKind::Interface) {
                            diagnostics.error(
                                DiagnosticCode::InvalidOperation,
                                extends_range,
                                "INTERFACE can only EXTENDS another INTERFACE",
                            );
                        }
                    }
                }
                if interface_inheritance_cycle(symbols, interface_id) {
                    diagnostics.error(
                        DiagnosticCode::CyclicDependency,
                        extends_range,
                        "interface inheritance cycle detected",
                    );
                }
            }
            SyntaxKind::FunctionBlock => {
                let extends_clause = node
                    .children()
                    .find(|child| child.kind() == SyntaxKind::ExtendsClause);
                let Some(clause) = extends_clause.as_ref() else {
                    continue;
                };
                let extends_range = qualified_name_parts(clause)
                    .map(|(_, range)| range)
                    .unwrap_or_else(|| clause.text_range());
                let Some((name, range)) = name_from_node(&node) else {
                    continue;
                };
                let Some(fb_id) = find_symbol_by_name_range(symbols, name.as_str(), range) else {
                    continue;
                };
                if let Some(base_id) = resolve_extends_symbol(symbols, fb_id) {
                    if let Some(base_symbol) = symbols.get(base_id) {
                        if matches!(base_symbol.kind, SymbolKind::Interface) {
                            diagnostics.error(
                                DiagnosticCode::InvalidOperation,
                                extends_range,
                                "FUNCTION_BLOCK cannot EXTENDS an INTERFACE",
                            );
                        } else if !matches!(
                            base_symbol.kind,
                            SymbolKind::FunctionBlock | SymbolKind::Class
                        ) {
                            diagnostics.error(
                                DiagnosticCode::InvalidOperation,
                                extends_range,
                                "FUNCTION_BLOCK EXTENDS must reference a FUNCTION_BLOCK or CLASS",
                            );
                        } else if matches!(base_symbol.kind, SymbolKind::Class)
                            && base_symbol.modifiers.is_final
                        {
                            diagnostics.error(
                                DiagnosticCode::InvalidOperation,
                                extends_range,
                                format!("cannot extend FINAL class '{}'", base_symbol.name),
                            );
                        }
                    }
                }
                if function_block_inheritance_cycle(symbols, fb_id) {
                    diagnostics.error(
                        DiagnosticCode::CyclicDependency,
                        extends_range,
                        "function block inheritance cycle detected",
                    );
                }
            }
            _ => {}
        }
    }
}

pub(in crate::db) fn resolve_extends_symbol(
    symbols: &SymbolTable,
    owner: SymbolId,
) -> Option<SymbolId> {
    match resolve_extends_symbol_outcome(symbols, owner, None) {
        SemanticOutcome::Resolved(symbol_id) => Some(symbol_id),
        _ => None,
    }
}

pub(in crate::db) fn resolve_extends_symbol_outcome(
    symbols: &SymbolTable,
    owner: SymbolId,
    range: Option<TextRange>,
) -> SemanticOutcome<SymbolId> {
    let Some(name) = symbols.extends_name(owner) else {
        return SemanticOutcome::Unknown { name: None, range };
    };
    let Some(scope_id) = symbols.scope_for_owner(owner) else {
        return SemanticOutcome::InvariantViolation {
            message: SmolStr::new("cannot resolve owner scope for EXTENDS declaration"),
            range,
        };
    };
    match resolve_type_symbol_by_name_in_scope_outcome(symbols, name.as_str(), scope_id) {
        SemanticOutcome::Resolved(symbol_id) => SemanticOutcome::Resolved(symbol_id),
        SemanticOutcome::Unknown { name, .. } => SemanticOutcome::Unknown { name, range },
        SemanticOutcome::Ambiguous { name, .. } => SemanticOutcome::Ambiguous { name, range },
        SemanticOutcome::WrongKind {
            symbol_id,
            expected,
            actual,
            ..
        } => SemanticOutcome::WrongKind {
            symbol_id,
            expected,
            actual,
            range,
        },
        SemanticOutcome::SuppressedCascade { primary, .. } => {
            SemanticOutcome::SuppressedCascade { primary, range }
        }
        SemanticOutcome::InvariantViolation { message, .. } => {
            SemanticOutcome::InvariantViolation { message, range }
        }
    }
}

pub(in crate::db) fn class_inheritance_cycle(symbols: &SymbolTable, class_id: SymbolId) -> bool {
    extends_chain_cycle(symbols, class_id, |_| true)
}

pub(in crate::db) fn interface_inheritance_cycle(
    symbols: &SymbolTable,
    interface_id: SymbolId,
) -> bool {
    extends_chain_cycle(symbols, interface_id, |kind| {
        matches!(kind, SymbolKind::Interface)
    })
}

pub(in crate::db) fn function_block_inheritance_cycle(
    symbols: &SymbolTable,
    fb_id: SymbolId,
) -> bool {
    extends_chain_cycle(symbols, fb_id, |kind| {
        matches!(kind, SymbolKind::FunctionBlock | SymbolKind::Class)
    })
}

fn extends_chain_cycle(
    symbols: &SymbolTable,
    start_id: SymbolId,
    mut should_follow: impl FnMut(&SymbolKind) -> bool,
) -> bool {
    let mut visited = FxHashSet::default();
    let mut current = Some(start_id);
    while let Some(symbol_id) = current {
        if !visited.insert(symbol_id) {
            return true;
        }
        let base_id = resolve_extends_symbol(symbols, symbol_id);
        let Some(base_id) = base_id else {
            break;
        };
        let Some(base_symbol) = symbols.get(base_id) else {
            break;
        };
        if !should_follow(&base_symbol.kind) {
            break;
        }
        current = Some(base_id);
    }
    false
}

pub(in crate::db) fn collect_inherited_variables(
    symbols: &SymbolTable,
    class_id: SymbolId,
) -> FxHashMap<SmolStr, SymbolId> {
    let mut vars = FxHashMap::default();
    let mut visited = FxHashSet::default();
    let mut current = resolve_extends_symbol(symbols, class_id);
    while let Some(base_id) = current {
        if !visited.insert(base_id) {
            break;
        }
        for sym in symbols.iter() {
            if sym.parent != Some(base_id) {
                continue;
            }
            if !matches!(sym.kind, SymbolKind::Variable { .. } | SymbolKind::Constant) {
                continue;
            }
            if !member_is_inherited(symbols, sym, class_id) {
                continue;
            }
            let key = normalize_member_name(sym.name.as_str());
            vars.entry(key).or_insert(sym.id);
        }
        current = resolve_extends_symbol(symbols, base_id);
    }
    vars
}

pub(in crate::db) fn collect_inherited_methods(
    symbols: &SymbolTable,
    class_id: SymbolId,
) -> FxHashMap<SmolStr, SymbolId> {
    let mut methods = FxHashMap::default();
    let mut visited = FxHashSet::default();
    let mut current = resolve_extends_symbol(symbols, class_id);
    while let Some(base_id) = current {
        if !visited.insert(base_id) {
            break;
        }
        for sym in symbols.iter() {
            if sym.parent != Some(base_id) {
                continue;
            }
            if !matches!(sym.kind, SymbolKind::Method { .. }) {
                continue;
            }
            if !member_is_inherited(symbols, sym, class_id) {
                continue;
            }
            let key = normalize_member_name(sym.name.as_str());
            methods.entry(key).or_insert(sym.id);
        }
        current = resolve_extends_symbol(symbols, base_id);
    }
    methods
}

pub(in crate::db) fn member_is_inherited(
    symbols: &SymbolTable,
    member: &Symbol,
    derived_id: SymbolId,
) -> bool {
    let Some(owner_id) = member.parent else {
        return false;
    };
    match member.visibility {
        Visibility::Private => false,
        Visibility::Internal => {
            namespace_path_for_symbol(symbols, owner_id)
                == namespace_path_for_symbol(symbols, derived_id)
        }
        Visibility::Public | Visibility::Protected => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_block_cycle_detection_walks_mixed_class_links() {
        let mut symbols = SymbolTable::new();
        let fb = symbols.add_symbol(Symbol::new(
            SymbolId::UNKNOWN,
            "Fb",
            SymbolKind::FunctionBlock,
            TypeId(1000),
            TextRange::empty(0.into()),
        ));
        let class = symbols.add_symbol(Symbol::new(
            SymbolId::UNKNOWN,
            "Base",
            SymbolKind::Class,
            TypeId(1001),
            TextRange::empty(0.into()),
        ));
        let root_scope = symbols.current_scope();
        symbols.push_scope(ScopeKind::FunctionBlock, Some(fb));
        symbols.set_current_scope(root_scope);
        symbols.push_scope(ScopeKind::Class, Some(class));
        symbols.set_current_scope(root_scope);
        symbols.set_extends(fb, "Base".into());
        symbols.set_extends(class, "Fb".into());

        assert!(function_block_inheritance_cycle(&symbols, fb));
    }

    #[test]
    fn extends_resolution_does_not_fallback_to_global_when_owner_scope_is_missing() {
        let mut symbols = SymbolTable::new();
        let derived = symbols.add_symbol(Symbol::new(
            SymbolId::UNKNOWN,
            "Derived",
            SymbolKind::FunctionBlock,
            TypeId(1000),
            TextRange::empty(0.into()),
        ));
        symbols.add_symbol(Symbol::new(
            SymbolId::UNKNOWN,
            "Base",
            SymbolKind::Class,
            TypeId(1001),
            TextRange::empty(0.into()),
        ));
        symbols.set_extends(derived, "Base".into());

        assert_eq!(resolve_extends_symbol(&symbols, derived), None);
    }
}
