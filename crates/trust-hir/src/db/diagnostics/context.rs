use super::super::queries::*;
use super::super::*;

pub(in crate::db) fn is_global_symbol(symbols: &SymbolTable, symbol: &Symbol) -> bool {
    let parent_ok = match symbol.parent {
        None => true,
        Some(parent_id) => symbols
            .get(parent_id)
            .map(|parent| {
                matches!(
                    parent.kind,
                    SymbolKind::Namespace
                        | SymbolKind::Configuration
                        | SymbolKind::Resource
                        | SymbolKind::Program
                )
            })
            .unwrap_or(false),
    };
    match symbol.kind {
        SymbolKind::Variable {
            qualifier: VarQualifier::Global,
        } => parent_ok,
        SymbolKind::Constant => parent_ok,
        _ => false,
    }
}

pub(in crate::db) fn namespace_path_for_symbol(
    symbols: &SymbolTable,
    symbol_id: SymbolId,
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
    parts
}

pub(in crate::db) fn normalized_name(name: &str) -> SmolStr {
    SmolStr::new(name.to_ascii_uppercase())
}

pub(in crate::db) fn find_scope_for_symbol(
    symbols: &SymbolTable,
    symbol_id: SymbolId,
) -> Option<ScopeId> {
    symbols.scope_for_owner(symbol_id)
}

pub(in crate::db) fn find_symbol_by_name_range(
    symbols: &SymbolTable,
    name: &str,
    range: TextRange,
) -> Option<SymbolId> {
    symbols.lookup_by_name_range(name, range)
}

pub(in crate::db) fn property_type_for_node(
    symbols: &SymbolTable,
    node: &SyntaxNode,
) -> Option<TypeId> {
    let (name, range) = name_from_node(node)?;
    let symbol_id = find_symbol_by_name_range(symbols, name.as_str(), range)?;
    symbols.get(symbol_id).and_then(|sym| match sym.kind {
        SymbolKind::Property { prop_type, .. } => Some(prop_type),
        _ => None,
    })
}

pub(in crate::db) fn is_top_level_stmt_list(stmt_list: &SyntaxNode, pou: &SyntaxNode) -> bool {
    if !stmt_list_belongs_to_pou(stmt_list, pou) {
        return false;
    }

    !stmt_list
        .ancestors()
        .skip(1)
        .take_while(|node| node != pou)
        .any(|node| node.kind() == SyntaxKind::StmtList)
}

pub(in crate::db) fn stmt_list_belongs_to_pou(stmt_list: &SyntaxNode, pou: &SyntaxNode) -> bool {
    stmt_list
        .ancestors()
        .find(|node| is_pou_kind(node.kind()))
        .map(|node| &node == pou)
        .unwrap_or(false)
}

pub(in crate::db) fn is_pou_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Program
            | SyntaxKind::Function
            | SyntaxKind::FunctionBlock
            | SyntaxKind::Class
            | SyntaxKind::Method
            | SyntaxKind::Property
            | SyntaxKind::Interface
    )
}

#[derive(Debug, Clone, Copy)]
pub(in crate::db) struct PouContext {
    pub(in crate::db) scope_id: ScopeId,
    pub(in crate::db) return_type: Option<TypeId>,
    pub(in crate::db) this_type: Option<TypeId>,
    pub(in crate::db) super_type: Option<TypeId>,
    pub(in crate::db) symbol_id: Option<SymbolId>,
}

impl PouContext {
    fn global() -> Self {
        Self {
            scope_id: ScopeId::GLOBAL,
            return_type: None,
            this_type: None,
            super_type: None,
            symbol_id: None,
        }
    }
}

pub(in crate::db) fn expression_context(symbols: &SymbolTable, node: &SyntaxNode) -> PouContext {
    node.ancestors()
        .find(|ancestor| is_pou_kind(ancestor.kind()))
        .map(|pou| pou_context(symbols, &pou))
        .unwrap_or_else(PouContext::global)
}

pub(in crate::db) fn action_context(symbols: &SymbolTable, node: &SyntaxNode) -> PouContext {
    node.ancestors()
        .find(|ancestor| {
            matches!(
                ancestor.kind(),
                SyntaxKind::Program | SyntaxKind::FunctionBlock
            )
        })
        .map(|pou| pou_context(symbols, &pou))
        .unwrap_or_else(PouContext::global)
}

pub(in crate::db) fn pou_context(symbols: &SymbolTable, pou_node: &SyntaxNode) -> PouContext {
    let (pou_name, pou_range) = match name_from_node(pou_node) {
        Some((name, range)) => (name, range),
        None => return PouContext::global(),
    };

    let pou_symbol_id = find_symbol_by_name_range(symbols, pou_name.as_str(), pou_range);
    let scope_id = pou_symbol_id
        .and_then(|id| find_scope_for_symbol(symbols, id))
        .unwrap_or(ScopeId::GLOBAL);

    let return_type = pou_symbol_id.and_then(|id| {
        symbols.get(id).and_then(|sym| match &sym.kind {
            SymbolKind::Function { return_type, .. } => Some(*return_type),
            SymbolKind::Method { return_type, .. } => *return_type,
            _ => None,
        })
    });

    let (this_type, super_type) = receiver_types_for_pou(symbols, pou_symbol_id, pou_node);

    PouContext {
        scope_id,
        return_type,
        this_type,
        super_type,
        symbol_id: pou_symbol_id,
    }
}

pub(in crate::db) fn receiver_types_for_pou(
    symbols: &SymbolTable,
    pou_symbol_id: Option<SymbolId>,
    pou_node: &SyntaxNode,
) -> (Option<TypeId>, Option<TypeId>) {
    let this_type = match pou_node.kind() {
        SyntaxKind::FunctionBlock | SyntaxKind::Class | SyntaxKind::Interface => pou_symbol_id
            .and_then(|id| symbols.get(id))
            .map(|sym| sym.type_id),
        SyntaxKind::Method | SyntaxKind::Property => pou_symbol_id
            .and_then(|id| symbols.get(id))
            .and_then(|sym| sym.parent)
            .and_then(|parent| symbols.get(parent))
            .map(|sym| sym.type_id),
        _ => None,
    };

    let owner_symbol_id = match pou_node.kind() {
        SyntaxKind::FunctionBlock | SyntaxKind::Class | SyntaxKind::Interface => pou_symbol_id,
        SyntaxKind::Method | SyntaxKind::Property => pou_symbol_id
            .and_then(|id| symbols.get(id))
            .and_then(|sym| sym.parent),
        _ => None,
    };

    let super_type = owner_symbol_id.and_then(|id| extends_type_for_symbol(symbols, id));

    (this_type, super_type)
}

pub(in crate::db) fn extends_type_for_symbol(
    symbols: &SymbolTable,
    owner: SymbolId,
) -> Option<TypeId> {
    let name = symbols.extends_name(owner)?;
    let scope_id = symbols.scope_for_owner(owner).unwrap_or(ScopeId::GLOBAL);
    resolve_type_by_name_in_scope(symbols, name.as_str(), scope_id)
}

pub(in crate::db) fn resolve_type_by_name_in_scope(
    symbols: &SymbolTable,
    name: &str,
    scope_id: ScopeId,
) -> Option<TypeId> {
    if let Some(id) = TypeId::from_builtin_name(name) {
        return Some(id);
    }
    if name.contains('.') {
        let parts: Vec<SmolStr> = name.split('.').map(SmolStr::new).collect();
        let symbol_id = symbols.resolve_qualified(&parts)?;
        let symbol = symbols.get(symbol_id)?;
        return symbol.is_type().then_some(symbol.type_id);
    }
    if let Some(symbol_id) = symbols.resolve(name, scope_id) {
        if let Some(symbol) = symbols.get(symbol_id) {
            if symbol.is_type() {
                return Some(symbol.type_id);
            }
        }
    }
    symbols.lookup_type(name)
}

pub(in crate::db) fn resolve_type_symbol_by_name_in_scope(
    symbols: &SymbolTable,
    name: &str,
    scope_id: ScopeId,
) -> Option<SymbolId> {
    if name.contains('.') {
        let parts: Vec<SmolStr> = name.split('.').map(SmolStr::new).collect();
        let symbol_id = symbols.resolve_qualified(&parts)?;
        return symbols
            .get(symbol_id)
            .and_then(|sym| sym.is_type().then_some(symbol_id));
    }
    if let Some(symbol_id) = symbols.resolve(name, scope_id) {
        if symbols
            .get(symbol_id)
            .map(|sym| sym.is_type())
            .unwrap_or(false)
        {
            return Some(symbol_id);
        }
    }
    if let Some(symbol_id) = symbols.lookup(name) {
        if symbols
            .get(symbol_id)
            .map(|sym| sym.is_type())
            .unwrap_or(false)
        {
            return Some(symbol_id);
        }
    }
    None
}

pub(in crate::db) fn method_signature_from_table(
    symbols: &SymbolTable,
    symbol_id: SymbolId,
) -> Option<MethodSignature> {
    let symbol = symbols.get(symbol_id)?;
    let (return_type, parameters) = match &symbol.kind {
        SymbolKind::Method {
            return_type,
            parameters,
        } => (*return_type, parameters),
        _ => return None,
    };

    let mut params = Vec::new();
    for param_id in parameters {
        let param_symbol = symbols.get(*param_id)?;
        let SymbolKind::Parameter { direction } = param_symbol.kind else {
            continue;
        };
        params.push(ParamSignature {
            name: param_symbol.name.clone(),
            direction,
            type_id: symbols.resolve_alias_type(param_symbol.type_id),
        });
    }

    Some(MethodSignature {
        name: symbol.name.clone(),
        return_type: return_type.map(|ty| symbols.resolve_alias_type(ty)),
        parameters: params,
        visibility: symbol.visibility,
        range: symbol.range,
    })
}

pub(in crate::db) fn method_signatures_match_with_table(
    symbols: &SymbolTable,
    expected: &MethodSignature,
    actual: &MethodSignature,
) -> bool {
    let expected_return = symbols.resolve_alias_type(expected.return_type.unwrap_or(TypeId::VOID));
    let actual_return = symbols.resolve_alias_type(actual.return_type.unwrap_or(TypeId::VOID));
    if expected_return != actual_return {
        return false;
    }

    if expected.parameters.len() != actual.parameters.len() {
        return false;
    }

    for (expected_param, actual_param) in expected.parameters.iter().zip(actual.parameters.iter()) {
        if expected_param.direction != actual_param.direction {
            return false;
        }
        if symbols.resolve_alias_type(expected_param.type_id)
            != symbols.resolve_alias_type(actual_param.type_id)
        {
            return false;
        }
        if !expected_param
            .name
            .eq_ignore_ascii_case(actual_param.name.as_str())
        {
            return false;
        }
    }

    true
}

pub(in crate::db) fn property_signature_from_table(
    symbols: &SymbolTable,
    symbol_id: SymbolId,
) -> Option<PropertySignature> {
    let symbol = symbols.get(symbol_id)?;
    let (prop_type, has_get, has_set) = match symbol.kind {
        SymbolKind::Property {
            prop_type,
            has_get,
            has_set,
        } => (prop_type, has_get, has_set),
        _ => return None,
    };

    Some(PropertySignature {
        name: symbol.name.clone(),
        prop_type: symbols.resolve_alias_type(prop_type),
        has_get,
        has_set,
        visibility: symbol.visibility,
        range: symbol.range,
    })
}

pub(in crate::db) fn property_signatures_match_with_table(
    symbols: &SymbolTable,
    expected: &PropertySignature,
    actual: &PropertySignature,
) -> bool {
    if symbols.resolve_alias_type(expected.prop_type)
        != symbols.resolve_alias_type(actual.prop_type)
    {
        return false;
    }
    if expected.has_get && !actual.has_get {
        return false;
    }
    if expected.has_set && !actual.has_set {
        return false;
    }
    true
}
