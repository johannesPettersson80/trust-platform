use super::super::queries::*;
use super::super::*;
use super::context::{expression_context, pou_context, PouContext};

pub(in crate::db) fn add_unused_symbol_warnings(
    symbols: &SymbolTable,
    file_id: FileId,
    project_used: &FxHashSet<(FileId, SymbolId)>,
    diagnostics: &mut DiagnosticBuilder,
) {
    for symbol in symbols.iter() {
        let Some((code, label)) = unused_warning_kind(symbols, symbol) else {
            continue;
        };

        if project_used.contains(&(file_id, symbol.id)) {
            continue;
        }

        diagnostics.warning(
            code,
            symbol.range,
            format!("unused {label} '{}'", symbol.name),
        );
    }
}

pub(in crate::db) fn collect_used_symbols(
    symbols: &SymbolTable,
    root: &SyntaxNode,
) -> FxHashSet<SymbolId> {
    let mut used = FxHashSet::default();
    for pou in root
        .descendants()
        .filter(|node| matches!(node.kind(), SyntaxKind::Program | SyntaxKind::FunctionBlock))
    {
        let is_test = pou.children_with_tokens().any(|child| {
            matches!(
                child.kind(),
                SyntaxKind::KwTestProgram | SyntaxKind::KwTestFunctionBlock
            )
        });
        if is_test && let Some(symbol_id) = pou_context(symbols, &pou).symbol_id {
            used.insert(symbol_id);
        }
    }
    let program_instances = collect_program_instances(symbols, root);
    for target in program_instances.values() {
        if let Some(symbol_id) = target.symbol_id() {
            used.insert(symbol_id);
        }
    }
    for node in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::NameRef)
    {
        let Some((name, _)) = name_from_node(&node) else {
            continue;
        };
        let context = expression_context(symbols, &node);
        if let Some(symbol_id) = symbols.resolve(name.as_str(), context.scope_id) {
            if is_self_pou_reference(symbols, symbol_id, &context) {
                continue;
            }
            used.insert(symbol_id);
        }
    }
    // Qualified value access chains (GVals.Member, Enum.Member): the parser
    // models them as nested FieldExpr nodes with a plain Name terminal, so
    // the NameRef pass above only ever sees the LEFTMOST segment - the
    // member terminal never resolved in the referencing scope and stayed
    // falsely unused (measured 2026-10-27: GVals.UsedCrossFile fired W001
    // on the constant despite a cross-file expression use; s4's
    // GVLMqtt/GVLEntities constants likewise).
    for node in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::FieldExpr)
    {
        let Some(segments) = field_expr_segments(&node) else {
            continue;
        };
        if segments.len() < 2 {
            continue;
        }
        let context = expression_context(symbols, &node);
        let resolved = symbols.resolve_qualified(&segments).or_else(|| {
            // non-namespace chains: Enum.Member, Struct.Field - resolve the
            // head through the remaining segments, then the terminal as its
            // direct child
            let (terminal, head) = segments.split_last()?;
            let owner = if head.len() == 1 {
                symbols.lookup(&head[0])
            } else {
                symbols.resolve_qualified(head)
            }?;
            symbols.resolve_child_symbol(owner, terminal)
        });
        if let Some(symbol_id) = resolved {
            if is_self_pou_reference(symbols, symbol_id, &context) {
                continue;
            }
            used.insert(symbol_id);
        }
    }
    for type_ref in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::TypeRef)
    {
        let Some((parts, _range)) = type_path_from_type_ref(&type_ref) else {
            continue;
        };
        let type_parts: Vec<SmolStr> = parts.into_iter().map(|(name, _)| name).collect();
        if type_parts.is_empty() {
            continue;
        }
        let scope_id = expression_context(symbols, &type_ref).scope_id;
        let symbol_id = if type_parts.len() == 1 {
            symbols
                .resolve(type_parts[0].as_str(), scope_id)
                .or_else(|| symbols.lookup(type_parts[0].as_str()))
        } else {
            symbols.resolve_qualified(&type_parts)
        };
        let Some(symbol_id) = symbol_id else {
            continue;
        };
        if symbols
            .get(symbol_id)
            .is_some_and(|symbol| symbol.is_type())
        {
            used.insert(symbol_id);
        }
    }
    // Inheritance clauses (EXTENDS/IMPLEMENTS on POUs, classes, and DUTs):
    // the target is an inheritance reference and marks the base used. The
    // parser models the clause target as a plain Name (dotted targets as a
    // QualifiedName), not a TypeRef, so the type-reference pass above never
    // sees it (measured 2026-10-27: fbChild EXTENDS fbBase left fbBase
    // flagged W009; s4's fbLight/fbMqttPayloadBase are pure base classes).
    for node in root.descendants().filter(|n| {
        matches!(
            n.kind(),
            SyntaxKind::ExtendsClause | SyntaxKind::ImplementsClause
        )
    }) {
        let Some((parts, _)) = qualified_name_parts(&node) else {
            continue;
        };
        let segments: Vec<SmolStr> = parts.iter().map(|(name, _)| name.clone()).collect();
        let symbol_id = if segments.len() == 1 {
            symbols
                .resolve(
                    segments[0].as_str(),
                    expression_context(symbols, &node).scope_id,
                )
                .or_else(|| symbols.lookup(segments[0].as_str()))
        } else {
            symbols.resolve_qualified(&segments)
        };
        if let Some(symbol_id) = symbol_id {
            used.insert(symbol_id);
        }
    }
    for config_init in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::ConfigInit)
    {
        let Some(access_path) = config_init
            .children()
            .find(|n| n.kind() == SyntaxKind::AccessPath)
        else {
            continue;
        };
        if let Some(target) = resolve_access_path_target(symbols, &program_instances, &access_path)
        {
            used.insert(target.symbol_id);
        }
    }
    used
}

fn is_self_pou_reference(symbols: &SymbolTable, symbol_id: SymbolId, context: &PouContext) -> bool {
    if context.symbol_id != Some(symbol_id) {
        return false;
    }
    symbols.get(symbol_id).is_some_and(|symbol| {
        matches!(
            symbol.kind,
            SymbolKind::Program | SymbolKind::Function { .. } | SymbolKind::FunctionBlock
        )
    })
}

/// Dotted segments of a qualified access chain: FieldExpr(FieldExpr(NameRef,
/// Name), Name) -> ["GVals", "UsedCrossFile"]. Returns None when any link in
/// the chain is not a plain name (call receivers, index expressions) - those
/// keep the NameRef-only behavior.
fn field_expr_segments(field: &SyntaxNode) -> Option<Vec<SmolStr>> {
    let mut parts = Vec::new();
    let mut current = field.clone();
    loop {
        let terminal = current
            .children()
            .find(|n| n.kind() == SyntaxKind::Name)
            .and_then(|n| name_from_node(&n).map(|(name, _)| name))?;
        parts.push(terminal);
        let lhs = current
            .children()
            .find(|n| matches!(n.kind(), SyntaxKind::FieldExpr | SyntaxKind::NameRef))?;
        match lhs.kind() {
            SyntaxKind::NameRef => {
                parts.push(name_from_node(&lhs)?.0);
                break;
            }
            SyntaxKind::FieldExpr => current = lhs,
            _ => return None,
        }
    }
    parts.reverse();
    Some(parts)
}

pub(in crate::db) fn unused_warning_kind(
    symbols: &SymbolTable,
    symbol: &Symbol,
) -> Option<(DiagnosticCode, &'static str)> {
    if symbol.origin.is_some() {
        return None;
    }
    if symbol.range.is_empty() {
        return None;
    }

    let parent_kind = symbol
        .parent
        .and_then(|id| symbols.get(id))
        .map(|sym| &sym.kind);

    if matches!(
        parent_kind,
        Some(SymbolKind::FunctionBlock | SymbolKind::Class | SymbolKind::Interface)
    ) {
        return None;
    }

    match symbol.kind {
        SymbolKind::Program => Some((DiagnosticCode::UnusedPou, "program")),
        SymbolKind::Function { .. } => Some((DiagnosticCode::UnusedPou, "function")),
        SymbolKind::FunctionBlock => Some((DiagnosticCode::UnusedPou, "function block")),
        SymbolKind::Variable {
            qualifier: VarQualifier::Local | VarQualifier::Temp,
        } => Some((DiagnosticCode::UnusedVariable, "variable")),
        SymbolKind::Variable { .. } => None,
        SymbolKind::Parameter {
            direction: ParamDirection::In,
        } => {
            if is_interface_method_parameter(symbols, symbol) {
                return None;
            }
            Some((DiagnosticCode::UnusedParameter, "parameter"))
        }
        SymbolKind::Parameter { .. } => None,
        SymbolKind::Constant => {
            if symbol.parent.is_some() {
                Some((DiagnosticCode::UnusedVariable, "constant"))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn is_interface_method_parameter(symbols: &SymbolTable, symbol: &Symbol) -> bool {
    let parent = symbol.parent.and_then(|id| symbols.get(id));
    let Some(parent) = parent else {
        return false;
    };
    if !matches!(parent.kind, SymbolKind::Method { .. }) {
        return false;
    }
    let grandparent = parent.parent.and_then(|id| symbols.get(id));
    let Some(grandparent) = grandparent else {
        return false;
    };
    match &grandparent.kind {
        // Method declared inside the INTERFACE POU itself.
        SymbolKind::Interface => true,
        // Method on an FB/CLASS that IMPLEMENTS the interface: the
        // implementing POU must carry the full interface signature even
        // where the body ignores a pin, so such parameters are
        // contract-mandated (measured 2026-10-27: 19 of s4's 20 W002 sites
        // - the fbMqttSub*/fbMqttPayload* families implementing
        // itfMqttSubscribe/itfMqttPayloadBuilder). Methods outside every
        // implemented interface keep the warning.
        SymbolKind::FunctionBlock | SymbolKind::Class => {
            let owner_id = grandparent.id;
            let Some(names) = symbols.implements_names(owner_id) else {
                return false;
            };
            names.iter().any(|name| {
                symbols
                    .resolve_oop_reference_for_owner(owner_id, name.as_str())
                    .filter(|iface_id| {
                        symbols
                            .get(*iface_id)
                            .is_some_and(|iface| matches!(iface.kind, SymbolKind::Interface))
                    })
                    .and_then(|iface_id| symbols.resolve_child_symbol(iface_id, &parent.name))
                    .and_then(|method_id| symbols.get(method_id))
                    .is_some_and(|method| matches!(method.kind, SymbolKind::Method { .. }))
            })
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "unused/contract_tests.rs"]
mod contract_tests;
