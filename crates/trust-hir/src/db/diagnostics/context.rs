use super::super::queries::*;
use super::super::*;
use crate::semantic::{QualifiedName, SemanticOutcome, SemanticRole};

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
    kind.is_pou_declaration()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::db) enum PouContextResolution {
    Resolved,
    NoPouAncestor,
    MissingName,
    MissingOwnerSymbol,
    MissingOwnerScope,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::db) struct PouContext {
    pub(in crate::db) scope_id: ScopeId,
    pub(in crate::db) return_type: Option<TypeId>,
    pub(in crate::db) this_type: Option<TypeId>,
    pub(in crate::db) super_type: Option<TypeId>,
    pub(in crate::db) symbol_id: Option<SymbolId>,
    pub(in crate::db) resolution: PouContextResolution,
}

impl PouContext {
    fn global() -> Self {
        Self {
            scope_id: ScopeId::GLOBAL,
            return_type: None,
            this_type: None,
            super_type: None,
            symbol_id: None,
            resolution: PouContextResolution::NoPouAncestor,
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
        None => {
            return PouContext {
                resolution: PouContextResolution::MissingName,
                ..PouContext::global()
            };
        }
    };

    let pou_symbol_id = find_symbol_by_name_range(symbols, pou_name.as_str(), pou_range);
    let scope_id = pou_symbol_id.and_then(|id| find_scope_for_symbol(symbols, id));
    let resolution = match (pou_symbol_id, scope_id) {
        (None, _) => PouContextResolution::MissingOwnerSymbol,
        (Some(_), None) => PouContextResolution::MissingOwnerScope,
        (Some(_), Some(_)) => PouContextResolution::Resolved,
    };

    let return_type = pou_symbol_id.and_then(|id| {
        symbols.get(id).and_then(|sym| match &sym.kind {
            SymbolKind::Function { return_type, .. } => Some(*return_type),
            SymbolKind::Method { return_type, .. } => *return_type,
            _ => None,
        })
    });

    let (this_type, super_type) = receiver_types_for_pou(symbols, pou_symbol_id, pou_node);

    PouContext {
        scope_id: scope_id.unwrap_or(ScopeId::GLOBAL),
        return_type,
        this_type,
        super_type,
        symbol_id: pou_symbol_id,
        resolution,
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
    let scope_id = symbols.scope_for_owner(owner)?;
    match resolve_type_by_name_in_scope_outcome(symbols, name, scope_id) {
        SemanticOutcome::Resolved(type_id) => Some(type_id),
        _ => None,
    }
}

pub(in crate::db) fn resolve_type_by_name_in_scope_outcome(
    symbols: &SymbolTable,
    name: &str,
    scope_id: ScopeId,
) -> SemanticOutcome<TypeId> {
    if let Some(id) = TypeId::from_builtin_name(name) {
        return SemanticOutcome::Resolved(id);
    }

    match resolve_type_symbol_by_name_in_scope_outcome(symbols, name, scope_id) {
        SemanticOutcome::Resolved(symbol_id) => {
            let Some(symbol) = symbols.get(symbol_id) else {
                return SemanticOutcome::InvariantViolation {
                    message: SmolStr::new("resolved type symbol is missing from table"),
                    range: None,
                };
            };
            SemanticOutcome::Resolved(symbol.type_id)
        }
        SemanticOutcome::Unknown { name, range } => SemanticOutcome::Unknown { name, range },
        SemanticOutcome::Ambiguous { name, range } => SemanticOutcome::Ambiguous { name, range },
        SemanticOutcome::WrongKind {
            symbol_id,
            expected,
            actual,
            range,
        } => SemanticOutcome::WrongKind {
            symbol_id,
            expected,
            actual,
            range,
        },
        SemanticOutcome::SuppressedCascade { primary, range } => {
            SemanticOutcome::SuppressedCascade { primary, range }
        }
        SemanticOutcome::InvariantViolation { message, range } => {
            SemanticOutcome::InvariantViolation { message, range }
        }
    }
}

pub(in crate::db) fn resolve_type_symbol_by_name_in_scope_outcome(
    symbols: &SymbolTable,
    name: &str,
    scope_id: ScopeId,
) -> SemanticOutcome<SymbolId> {
    let qualified = QualifiedName::from_dotted(name);
    let symbol_id =
        if let Some(qualified) = qualified.as_ref().filter(|name| name.parts().len() > 1) {
            symbols.resolve_qualified(qualified.parts())
        } else {
            symbols
                .resolve(name, scope_id)
                .or_else(|| symbols.lookup(name))
        };

    let Some(symbol_id) = symbol_id else {
        return SemanticOutcome::Unknown {
            name: qualified,
            range: None,
        };
    };
    let Some(symbol) = symbols.get(symbol_id) else {
        return SemanticOutcome::InvariantViolation {
            message: SmolStr::new("resolved symbol is missing from table"),
            range: None,
        };
    };
    if symbol.is_type() {
        return SemanticOutcome::Resolved(symbol_id);
    }
    SemanticOutcome::WrongKind {
        symbol_id,
        expected: SemanticRole::Type,
        actual: semantic_role_for_symbol(symbol),
        range: None,
    }
}

fn semantic_role_for_symbol(symbol: &Symbol) -> SemanticRole {
    match symbol.kind {
        SymbolKind::Namespace => SemanticRole::Namespace,
        SymbolKind::Function { .. } | SymbolKind::Method { .. } => SemanticRole::Callable,
        SymbolKind::Type
        | SymbolKind::FunctionBlock
        | SymbolKind::Class
        | SymbolKind::Interface => SemanticRole::Type,
        SymbolKind::Program
        | SymbolKind::Configuration
        | SymbolKind::Resource
        | SymbolKind::Task
        | SymbolKind::ProgramInstance => SemanticRole::ScopeOwner,
        _ => SemanticRole::Value,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use trust_syntax::parser::parse;

    fn root(source: &str) -> SyntaxNode {
        parse(source).syntax()
    }

    #[test]
    fn pou_context_classifies_missing_owner_symbol() {
        let root = root("PROGRAM Main\nEND_PROGRAM\n");
        let program = root
            .descendants()
            .find(|node| node.kind() == SyntaxKind::Program)
            .expect("program node");
        let symbols = SymbolTable::new();

        let context = pou_context(&symbols, &program);

        assert_eq!(context.resolution, PouContextResolution::MissingOwnerSymbol);
        assert_eq!(context.scope_id, ScopeId::GLOBAL);
    }

    #[test]
    fn pou_context_classifies_missing_name() {
        let root = root("PROGRAM\nEND_PROGRAM\n");
        let program = root
            .descendants()
            .find(|node| node.kind() == SyntaxKind::Program)
            .expect("program node");
        let symbols = SymbolTable::new();

        let context = pou_context(&symbols, &program);

        assert_eq!(context.resolution, PouContextResolution::MissingName);
        assert_eq!(context.scope_id, ScopeId::GLOBAL);
    }

    #[test]
    fn pou_context_classifies_missing_owner_scope() {
        let root = root("PROGRAM Main\nEND_PROGRAM\n");
        let program = root
            .descendants()
            .find(|node| node.kind() == SyntaxKind::Program)
            .expect("program node");
        let (name, range) = name_from_node(&program).expect("program name");
        let mut symbols = SymbolTable::new();
        symbols.add_symbol_raw(Symbol::new(
            SymbolId::UNKNOWN,
            name,
            SymbolKind::Program,
            TypeId::VOID,
            range,
        ));

        let context = pou_context(&symbols, &program);

        assert_eq!(context.resolution, PouContextResolution::MissingOwnerScope);
        assert_eq!(context.scope_id, ScopeId::GLOBAL);
    }

    #[test]
    fn pou_context_resolves_function_return_type() {
        let root = root("FUNCTION Fn : INT\nEND_FUNCTION\n");
        let function = root
            .descendants()
            .find(|node| node.kind() == SyntaxKind::Function)
            .expect("function node");
        let (name, range) = name_from_node(&function).expect("function name");
        let mut symbols = SymbolTable::new();
        let function_id = symbols.add_symbol_raw(Symbol::new(
            SymbolId::UNKNOWN,
            name,
            SymbolKind::Function {
                return_type: TypeId::INT,
                parameters: Vec::new(),
            },
            TypeId::INT,
            range,
        ));
        let scope_id = symbols.push_scope(ScopeKind::Function, Some(function_id));

        let context = pou_context(&symbols, &function);

        assert_eq!(context.resolution, PouContextResolution::Resolved);
        assert_eq!(context.scope_id, scope_id);
        assert_eq!(context.symbol_id, Some(function_id));
        assert_eq!(context.return_type, Some(TypeId::INT));
    }

    #[test]
    fn pou_context_resolves_method_return_type() {
        let root = root(
            r#"
CLASS C
METHOD M : DINT
END_METHOD
END_CLASS
"#,
        );
        let class = root
            .descendants()
            .find(|node| node.kind() == SyntaxKind::Class)
            .expect("class node");
        let method = root
            .descendants()
            .find(|node| node.kind() == SyntaxKind::Method)
            .expect("method node");
        let (class_name, class_range) = name_from_node(&class).expect("class name");
        let (method_name, method_range) = name_from_node(&method).expect("method name");
        let mut symbols = SymbolTable::new();
        let class_id = symbols.add_symbol_raw(Symbol::new(
            SymbolId::UNKNOWN,
            class_name,
            SymbolKind::Class,
            TypeId::UNKNOWN,
            class_range,
        ));
        let mut method_symbol = Symbol::new(
            SymbolId::UNKNOWN,
            method_name,
            SymbolKind::Method {
                return_type: Some(TypeId::DINT),
                parameters: Vec::new(),
            },
            TypeId::DINT,
            method_range,
        );
        method_symbol.parent = Some(class_id);
        let method_id = symbols.add_symbol_raw(method_symbol);
        let scope_id = symbols.push_scope(ScopeKind::Method, Some(method_id));

        let context = pou_context(&symbols, &method);

        assert_eq!(context.resolution, PouContextResolution::Resolved);
        assert_eq!(context.scope_id, scope_id);
        assert_eq!(context.symbol_id, Some(method_id));
        assert_eq!(context.return_type, Some(TypeId::DINT));
    }

    #[test]
    fn expression_context_classifies_missing_pou_owner() {
        let root = root("PROGRAM Main\nVAR x : INT; END_VAR\nx := 1;\nEND_PROGRAM\n");
        let expr = root
            .descendants()
            .find(|node| node.kind() == SyntaxKind::Literal)
            .expect("literal expression");
        let symbols = SymbolTable::new();

        let context = expression_context(&symbols, &expr);

        assert_eq!(context.resolution, PouContextResolution::MissingOwnerSymbol);
    }

    #[test]
    fn action_context_classifies_missing_owner() {
        let root = root(
            r#"
FUNCTION_BLOCK FB
ACTION Step
END_ACTION
END_FUNCTION_BLOCK
"#,
        );
        let action = root
            .descendants()
            .find(|node| node.kind() == SyntaxKind::Action)
            .expect("action node");
        let symbols = SymbolTable::new();

        let context = action_context(&symbols, &action);

        assert_eq!(context.resolution, PouContextResolution::MissingOwnerSymbol);
    }

    #[test]
    fn type_resolution_outcome_classifies_wrong_kind() {
        let mut symbols = SymbolTable::new();
        let program_id = symbols.add_symbol(Symbol::new(
            SymbolId::UNKNOWN,
            "Main",
            SymbolKind::Program,
            TypeId::VOID,
            TextRange::empty(0.into()),
        ));

        let outcome =
            resolve_type_symbol_by_name_in_scope_outcome(&symbols, "Main", ScopeId::GLOBAL);

        assert_eq!(
            outcome,
            SemanticOutcome::WrongKind {
                symbol_id: program_id,
                expected: SemanticRole::Type,
                actual: SemanticRole::ScopeOwner,
                range: None,
            }
        );
    }
}
