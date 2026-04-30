fn scope_for_namespace(
    symbols: &SymbolTable,
    root: &SyntaxNode,
    offset: TextSize,
) -> Option<ScopeId> {
    let token = root.token_at_offset(offset).right_biased()?;
    let mut namespaces: Vec<SyntaxNode> = token
        .parent_ancestors()
        .filter(|node| node.kind() == SyntaxKind::Namespace)
        .collect();
    if namespaces.is_empty() {
        return None;
    }

    namespaces.reverse();
    let mut scope_id = ScopeId::GLOBAL;
    for namespace in namespaces {
        let parts = namespace_name_parts(&namespace);
        if parts.is_empty() {
            continue;
        }
        for part in parts {
            let symbol_id = symbols.resolve(part.as_str(), scope_id)?;
            let symbol = symbols.get(symbol_id)?;
            if !matches!(symbol.kind, SymbolKind::Namespace) {
                return None;
            }
            scope_id = symbols.scope_for_owner(symbol_id)?;
        }
    }

    Some(scope_id)
}

fn namespace_name_parts(node: &SyntaxNode) -> Vec<SmolStr> {
    let Some(name_node) = node
        .children()
        .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
    else {
        return Vec::new();
    };

    match name_node.kind() {
        SyntaxKind::Name => name_from_name_node(&name_node).into_iter().collect(),
        SyntaxKind::QualifiedName => name_node
            .children()
            .filter(|child| child.kind() == SyntaxKind::Name)
            .filter_map(|child| name_from_name_node(&child))
            .collect(),
        _ => Vec::new(),
    }
}

fn resolve_field_target(
    db: &Database,
    file_id: FileId,
    symbols: &SymbolTable,
    name_node: &SyntaxNode,
    field_name: &str,
) -> Option<ResolvedTarget> {
    let field_expr = name_node.parent()?;
    if field_expr.kind() != SyntaxKind::FieldExpr {
        return None;
    }

    let base_expr = field_expr.children().next()?;
    let base_type = expression_type_at_node(db, file_id, &base_expr)?;
    let base_type = symbols.resolve_alias_type(base_type);

    if let Some(member_id) = symbols.resolve_member_symbol_in_type(base_type, field_name) {
        return Some(ResolvedTarget::Symbol(member_id));
    }

    if let Some(field_target) = resolve_struct_field(symbols, base_type, field_name) {
        return Some(ResolvedTarget::Field(field_target));
    }

    None
}

fn resolve_field_decl_target(
    symbols: &SymbolTable,
    name_node: &SyntaxNode,
    field_name: &str,
) -> Option<FieldTarget> {
    if name_node.parent()?.kind() != SyntaxKind::VarDecl {
        return None;
    }
    let type_body = name_node
        .ancestors()
        .skip(1)
        .find(|n| matches!(n.kind(), SyntaxKind::StructDef | SyntaxKind::UnionDef))?;
    let type_decl = name_node
        .ancestors()
        .skip(1)
        .find(|n| n.kind() == SyntaxKind::TypeDecl)?;
    let type_name = type_name_for_type_body(&type_decl, &type_body)?;
    let type_id = symbols.lookup_registered_type_name(type_name.as_str())?;
    let type_id = symbols.resolve_alias_type(type_id);

    match symbols.type_by_id(type_id)? {
        Type::Struct { .. } | Type::Union { .. } => Some(FieldTarget {
            type_id,
            name: SmolStr::new(field_name),
            type_name: Some(type_name),
        }),
        _ => None,
    }
}

fn type_name_for_type_body(type_decl: &SyntaxNode, type_body: &SyntaxNode) -> Option<SmolStr> {
    let mut current_type_name: Option<SmolStr> = None;
    for child in type_decl.children() {
        if child.kind() == SyntaxKind::Name {
            current_type_name = name_from_name_node(&child);
            continue;
        }
        if child == *type_body {
            return current_type_name;
        }
    }
    None
}

fn resolve_struct_field(
    symbols: &SymbolTable,
    type_id: TypeId,
    field_name: &str,
) -> Option<FieldTarget> {
    match symbols.type_by_id(type_id)? {
        Type::Struct { name, fields } => fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(field_name))
            .map(|field| FieldTarget {
                type_id,
                name: field.name.clone(),
                type_name: Some(name.clone()),
            }),
        Type::Union { name, variants } => variants
            .iter()
            .find(|variant| variant.name.eq_ignore_ascii_case(field_name))
            .map(|variant| FieldTarget {
                type_id,
                name: variant.name.clone(),
                type_name: Some(name.clone()),
            }),
        _ => None,
    }
}

fn expression_type_at_node(db: &Database, file_id: FileId, node: &SyntaxNode) -> Option<TypeId> {
    let offset = u32::from(node.text_range().start());
    let expr_id = db.expr_id_at_offset(file_id, offset)?;
    Some(db.type_of(file_id, expr_id))
}

pub(crate) fn field_type(symbols: &SymbolTable, target: &FieldTarget) -> Option<TypeId> {
    match symbols.type_by_id(target.type_id)? {
        Type::Struct { fields, .. } => fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(&target.name))
            .map(|field| field.type_id),
        Type::Union { variants, .. } => variants
            .iter()
            .find(|variant| variant.name.eq_ignore_ascii_case(&target.name))
            .map(|variant| variant.type_id),
        _ => None,
    }
}

pub(crate) fn type_detail(symbols: &SymbolTable, type_id: TypeId) -> Option<SmolStr> {
    symbols.type_name(type_id)
}

pub(crate) fn field_declaration_ranges(
    root: &SyntaxNode,
    symbols: &SymbolTable,
    target: &FieldTarget,
) -> Vec<TextRange> {
    let target_type_id = symbols.resolve_alias_type(target.type_id);
    let mut ranges = Vec::new();
    for type_decl in root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::TypeDecl)
    {
        let mut current_type_name: Option<SmolStr> = None;
        for child in type_decl.children() {
            if child.kind() == SyntaxKind::Name {
                current_type_name = name_from_name_node(&child);
                continue;
            }
            if !matches!(child.kind(), SyntaxKind::StructDef | SyntaxKind::UnionDef) {
                continue;
            }

            let Some(type_name) = current_type_name.as_ref() else {
                continue;
            };
            let Some(declared_type_id) = symbols.lookup_registered_type_name(type_name.as_str()) else {
                continue;
            };
            let declared_type_id = symbols.resolve_alias_type(declared_type_id);
            let type_matches = declared_type_id == target_type_id
                || target
                    .type_name
                    .as_ref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(type_name.as_str()));
            if !type_matches {
                continue;
            }

            for var_decl in child.children().filter(|n| n.kind() == SyntaxKind::VarDecl) {
                for name_node in var_decl.children().filter(|n| n.kind() == SyntaxKind::Name) {
                    let Some(ident) = ident_token_in_name(&name_node) else {
                        continue;
                    };
                    if ident.text().eq_ignore_ascii_case(&target.name) {
                        ranges.push(ident.text_range());
                    }
                }
            }
        }
    }

    ranges
}

pub(crate) fn ident_token_in_name(node: &SyntaxNode) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::Ident)
}
