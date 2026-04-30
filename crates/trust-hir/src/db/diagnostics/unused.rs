use super::super::queries::*;
use super::super::*;
use super::context::{expression_context, PouContext};

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
    matches!(
        grandparent.map(|sym| &sym.kind),
        Some(SymbolKind::Interface)
    )
}
