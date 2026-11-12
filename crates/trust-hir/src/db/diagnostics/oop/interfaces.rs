use super::super::super::queries::*;
use super::super::super::*;
use super::super::context::{
    find_symbol_by_name_range, method_signature_from_table, method_signatures_match_with_table,
    property_signature_from_table, property_signatures_match_with_table,
    resolve_type_symbol_by_name_in_scope_outcome,
};
use crate::diagnostics::Diagnostic;
use crate::semantic::SemanticOutcome;

pub(in crate::db) fn check_interface_conformance(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    for node in root.descendants() {
        if !matches!(node.kind(), SyntaxKind::Class | SyntaxKind::FunctionBlock) {
            continue;
        }

        let Some(clause) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::ImplementsClause)
        else {
            continue;
        };

        let Some((owner_name, owner_range)) = name_from_node(&node) else {
            continue;
        };
        let Some(owner_id) = find_symbol_by_name_range(symbols, owner_name.as_str(), owner_range)
        else {
            continue;
        };

        let (impl_methods, impl_properties) =
            collect_implementation_members_with_table(symbols, owner_id);

        for (iface_parts, iface_range) in implements_clause_names(&clause) {
            let iface_name = qualified_name_string(&iface_parts);
            let Some(owner_scope) = symbols.scope_for_owner(owner_id) else {
                diagnostics.error(
                    DiagnosticCode::CannotResolve,
                    iface_range,
                    format!("cannot resolve owner scope for '{}'", owner_name),
                );
                continue;
            };
            let interface_id = match resolve_type_symbol_by_name_in_scope_outcome(
                symbols,
                iface_name.as_str(),
                owner_scope,
            ) {
                SemanticOutcome::Resolved(interface_id) => interface_id,
                // Missing, ambiguous, and wrong-kind type references are reported by
                // pending type resolution. The conformance pass only validates resolved
                // type symbols that are not interfaces.
                _ => continue,
            };

            let Some(interface_symbol) = symbols.get(interface_id) else {
                continue;
            };

            if !matches!(interface_symbol.kind, SymbolKind::Interface) {
                diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    iface_range,
                    format!("'{}' is not an interface", iface_name),
                );
                continue;
            }

            let members = collect_interface_members_with_table(symbols, interface_id);
            let context = InterfaceCheckContext {
                owner_name: owner_name.as_str(),
                interface_name: iface_name.as_str(),
                interface_range: iface_range,
                allow_missing: false,
            };
            check_interface_methods_with_table(
                symbols,
                &context,
                &members.methods,
                &impl_methods,
                diagnostics,
            );
            check_interface_properties_with_table(
                symbols,
                &context,
                &members.properties,
                &impl_properties,
                diagnostics,
            );
        }
    }
}

fn collect_interface_members_with_table(
    symbols: &SymbolTable,
    interface_id: SymbolId,
) -> InterfaceMembers {
    let mut methods = FxHashMap::default();
    let mut properties = FxHashMap::default();
    let mut visited = FxHashSet::default();
    let mut stack = vec![interface_id];

    while let Some(symbol_id) = stack.pop() {
        if !visited.insert(symbol_id) {
            continue;
        }

        for sym in symbols.iter() {
            if sym.parent != Some(symbol_id) {
                continue;
            }
            match sym.kind {
                SymbolKind::Method { .. } => {
                    let key = normalize_member_name(sym.name.as_str());
                    if methods.contains_key(&key) {
                        continue;
                    }
                    if let Some(sig) = method_signature_from_table(symbols, sym.id) {
                        methods.insert(key, sig);
                    }
                }
                SymbolKind::Property { .. } => {
                    let key = normalize_member_name(sym.name.as_str());
                    if properties.contains_key(&key) {
                        continue;
                    }
                    if let Some(sig) = property_signature_from_table(symbols, sym.id) {
                        properties.insert(key, sig);
                    }
                }
                _ => {}
            }
        }

        // Every base, the first on top: an interface may extend several (specification 04).
        stack.extend(
            super::resolve_extends_interfaces(symbols, symbol_id)
                .into_iter()
                .rev(),
        );
    }

    InterfaceMembers {
        methods,
        properties,
    }
}

/// Two bases of an interface that give prototypes of one name with different signatures make the
/// interface an error (E208; specification 04, "Interface Inheritance"; `docs/IEC_DECISIONS.md`):
/// no method can match both. Prototypes of one signature are one, also when two bases inherit
/// them from a common base. Reported at the later base, which brings the second signature.
pub(super) fn check_base_prototypes(
    symbols: &SymbolTable,
    interface_id: SymbolId,
    clause: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    let Some(interface) = symbols.get(interface_id) else {
        return;
    };
    let Some(scope_id) = symbols.scope_for_owner(interface_id) else {
        return;
    };
    let mut methods: FxHashMap<SmolStr, (SmolStr, MethodSignature)> = FxHashMap::default();
    let mut properties: FxHashMap<SmolStr, (SmolStr, PropertySignature)> = FxHashMap::default();
    for (parts, base_range) in implements_clause_names(clause) {
        let base_name = qualified_name_string(&parts);
        let SemanticOutcome::Resolved(base_id) =
            resolve_type_symbol_by_name_in_scope_outcome(symbols, base_name.as_str(), scope_id)
        else {
            continue;
        };
        if !symbols
            .get(base_id)
            .is_some_and(|base| matches!(base.kind, SymbolKind::Interface))
        {
            continue;
        }
        let members = collect_interface_members_with_table(symbols, base_id);
        let mut base_methods: Vec<_> = members.methods.into_iter().collect();
        base_methods.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (key, signature) in base_methods {
            if let Some((first_base, first)) = methods.get(&key) {
                if !method_signatures_match_with_table(symbols, first, &signature) {
                    diagnostics.error(
                        DiagnosticCode::InterfaceConformance,
                        base_range,
                        format!(
                            "interface '{}' inherits method '{}' from '{}' and from '{}' with different signatures",
                            interface.name, signature.name, first_base, base_name
                        ),
                    );
                }
            } else {
                methods.insert(key, (base_name.clone(), signature));
            }
        }
        let mut base_properties: Vec<_> = members.properties.into_iter().collect();
        base_properties.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (key, signature) in base_properties {
            if let Some((first_base, first)) = properties.get(&key) {
                // The same type and the same accessors: an implementation offering one
                // prototype's accessors may lack the other's.
                if !(property_signatures_match_with_table(symbols, first, &signature)
                    && property_signatures_match_with_table(symbols, &signature, first))
                {
                    diagnostics.error(
                        DiagnosticCode::InterfaceConformance,
                        base_range,
                        format!(
                            "interface '{}' inherits property '{}' from '{}' and from '{}' with different signatures",
                            interface.name, signature.name, first_base, base_name
                        ),
                    );
                }
            } else {
                properties.insert(key, (base_name.clone(), signature));
            }
        }
    }
    check_redeclared_prototypes(symbols, interface, &methods, &properties, diagnostics);
}

/// An interface that declares a prototype of the name of one it inherits is an error (E208; IEC
/// 61131-3 Ed.3 §6.6.6.6.2 error 1), since an interface overrides nothing ("The OVERRIDE feature
/// ... is not applicable for interfaces"). Reported at the interface's own prototype, naming the
/// base it inherits the other from. The CODESYS help does not say what such a declaration gives,
/// so that profile keeps the interface's own prototype (specification 04, "Interface
/// Inheritance").
fn check_redeclared_prototypes(
    symbols: &SymbolTable,
    interface: &Symbol,
    methods: &FxHashMap<SmolStr, (SmolStr, MethodSignature)>,
    properties: &FxHashMap<SmolStr, (SmolStr, PropertySignature)>,
    diagnostics: &mut DiagnosticBuilder,
) {
    if symbols.dialect().is_codesys() {
        return;
    }
    for member in symbols
        .iter()
        .filter(|member| member.parent == Some(interface.id))
    {
        let key = normalize_member_name(member.name.as_str());
        let inherited = match member.kind {
            SymbolKind::Method { .. } => methods.get(&key).map(|(base, _)| ("method", base)),
            SymbolKind::Property { .. } => properties.get(&key).map(|(base, _)| ("property", base)),
            _ => None,
        };
        if let Some((what, base)) = inherited {
            diagnostics.error(
                DiagnosticCode::InterfaceConformance,
                member.range,
                format!(
                    "interface '{}' declares {what} '{}' again, which it inherits from '{base}'",
                    interface.name, member.name
                ),
            );
        }
    }
}

fn collect_implementation_members_with_table(
    symbols: &SymbolTable,
    owner_id: SymbolId,
) -> (
    FxHashMap<SmolStr, MethodSignature>,
    FxHashMap<SmolStr, PropertySignature>,
) {
    let mut methods = FxHashMap::default();
    let mut properties = FxHashMap::default();
    let mut visited = FxHashSet::default();
    let mut current = Some(owner_id);

    while let Some(symbol_id) = current {
        if !visited.insert(symbol_id) {
            break;
        }

        for sym in symbols.iter() {
            if sym.parent != Some(symbol_id) {
                continue;
            }
            match sym.kind {
                SymbolKind::Method { .. } => {
                    let key = normalize_member_name(sym.name.as_str());
                    if methods.contains_key(&key) {
                        continue;
                    }
                    if let Some(sig) = method_signature_from_table(symbols, sym.id) {
                        methods.insert(key, sig);
                    }
                }
                SymbolKind::Property { .. } => {
                    let key = normalize_member_name(sym.name.as_str());
                    if properties.contains_key(&key) {
                        continue;
                    }
                    if let Some(sig) = property_signature_from_table(symbols, sym.id) {
                        properties.insert(key, sig);
                    }
                }
                _ => {}
            }
        }

        current = super::resolve_extends_symbol(symbols, symbol_id);
    }

    (methods, properties)
}

fn check_interface_methods_with_table(
    symbols: &SymbolTable,
    context: &InterfaceCheckContext<'_>,
    expected: &FxHashMap<SmolStr, MethodSignature>,
    provided: &FxHashMap<SmolStr, MethodSignature>,
    diagnostics: &mut DiagnosticBuilder,
) {
    for (key, expected_sig) in expected {
        let Some(actual_sig) = provided.get(key) else {
            if !context.allow_missing {
                diagnostics.error(
                    DiagnosticCode::InterfaceConformance,
                    context.interface_range,
                    format!(
                        "type '{}' must implement method '{}' from interface '{}'",
                        context.owner_name, expected_sig.name, context.interface_name
                    ),
                );
            }
            continue;
        };

        if !method_signatures_match_with_table(symbols, expected_sig, actual_sig) {
            diagnostics.error(
                DiagnosticCode::InterfaceConformance,
                actual_sig.range,
                format!(
                    "method '{}' does not match interface '{}' signature",
                    actual_sig.name, context.interface_name
                ),
            );
            continue;
        }

        if !matches!(
            actual_sig.visibility,
            Visibility::Public | Visibility::Internal
        ) {
            let mut diagnostic = Diagnostic::error(
                DiagnosticCode::InterfaceConformance,
                actual_sig.range,
                format!(
                    "method '{}' implementing interface '{}' must be PUBLIC or INTERNAL",
                    actual_sig.name, context.interface_name
                ),
            );
            diagnostic = diagnostic.with_related(
                actual_sig.range,
                "Hint: update the access specifier to PUBLIC or INTERNAL to satisfy the interface.",
            );
            diagnostics.add(diagnostic);
        }
    }
}

fn check_interface_properties_with_table(
    symbols: &SymbolTable,
    context: &InterfaceCheckContext<'_>,
    expected: &FxHashMap<SmolStr, PropertySignature>,
    provided: &FxHashMap<SmolStr, PropertySignature>,
    diagnostics: &mut DiagnosticBuilder,
) {
    for (key, expected_sig) in expected {
        let Some(actual_sig) = provided.get(key) else {
            if !context.allow_missing {
                diagnostics.error(
                    DiagnosticCode::InterfaceConformance,
                    context.interface_range,
                    format!(
                        "type '{}' must implement property '{}' from interface '{}'",
                        context.owner_name, expected_sig.name, context.interface_name
                    ),
                );
            }
            continue;
        };

        if !property_signatures_match_with_table(symbols, expected_sig, actual_sig) {
            diagnostics.error(
                DiagnosticCode::InterfaceConformance,
                actual_sig.range,
                format!(
                    "property '{}' does not match interface '{}' signature",
                    actual_sig.name, context.interface_name
                ),
            );
            continue;
        }

        if !matches!(
            actual_sig.visibility,
            Visibility::Public | Visibility::Internal
        ) {
            let mut diagnostic = Diagnostic::error(
                DiagnosticCode::InterfaceConformance,
                actual_sig.range,
                format!(
                    "property '{}' implementing interface '{}' must be PUBLIC or INTERNAL",
                    actual_sig.name, context.interface_name
                ),
            );
            diagnostic = diagnostic.with_related(
                actual_sig.range,
                "Hint: update the access specifier to PUBLIC or INTERNAL to satisfy the interface.",
            );
            diagnostics.add(diagnostic);
        }
    }
}
