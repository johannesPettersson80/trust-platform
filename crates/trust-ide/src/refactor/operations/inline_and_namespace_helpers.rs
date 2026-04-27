fn field_expr_const_info(context: &ConstExprContext<'_>, node: &SyntaxNode) -> ConstExprInfo {
    let Some(parts) = qualified_name_from_field_expr(node) else {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    };
    let Some(symbol_id) = context.symbols.resolve_qualified(&parts) else {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    };
    let Some(symbol) = context.symbols.get(symbol_id) else {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    };
    if !matches!(symbol.kind, SymbolKind::EnumValue { .. }) && !symbol_is_constant(symbol) {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    }
    ConstExprInfo {
        is_const: true,
        requires_local_scope: false,
    }
}

fn expression_is_path_like(expr: &SyntaxNode) -> bool {
    match expr.kind() {
        SyntaxKind::NameRef | SyntaxKind::FieldExpr | SyntaxKind::IndexExpr => true,
        SyntaxKind::ParenExpr => expr
            .children()
            .filter(|child| is_expression_kind(child.kind()))
            .any(|child| expression_is_path_like(&child)),
        _ => false,
    }
}

fn wrap_expression_for_inline(kind: SyntaxKind, expr_text: &str) -> String {
    match kind {
        SyntaxKind::Literal
        | SyntaxKind::NameRef
        | SyntaxKind::ParenExpr
        | SyntaxKind::FieldExpr
        | SyntaxKind::IndexExpr => expr_text.to_string(),
        _ => format!("({expr_text})"),
    }
}

fn reference_has_disallowed_context(
    db: &Database,
    file_id: FileId,
    range: TextRange,
    is_path_like: bool,
) -> bool {
    let source = db.source_text(file_id);
    let root = parse(&source).syntax();
    let Some(token) = root.token_at_offset(range.start()).right_biased() else {
        return false;
    };
    if token.text_range() != range {
        return false;
    }
    let name_ref = token
        .parent_ancestors()
        .find(|node| node.kind() == SyntaxKind::NameRef);
    let Some(name_ref) = name_ref else {
        return false;
    };
    let Some(parent) = name_ref.parent() else {
        return false;
    };
    let is_base = parent
        .children()
        .next()
        .map(|child| child.text_range() == name_ref.text_range())
        .unwrap_or(false);
    match parent.kind() {
        SyntaxKind::CallExpr => is_base,
        SyntaxKind::AddrExpr | SyntaxKind::DerefExpr => is_base,
        SyntaxKind::FieldExpr | SyntaxKind::IndexExpr => is_base && !is_path_like,
        _ => false,
    }
}

fn var_decl_removal_range(
    source: &str,
    root: &SyntaxNode,
    symbol_range: TextRange,
) -> Option<TextRange> {
    let var_decl = crate::var_decl::find_var_decl_for_range(root, symbol_range)?;
    let names: Vec<SyntaxToken> = var_decl
        .children()
        .filter(|node| node.kind() == SyntaxKind::Name)
        .filter_map(|node| ident_token_in_name(&node))
        .collect();
    if names.is_empty() {
        return None;
    }
    let index = names
        .iter()
        .position(|token| token.text_range() == symbol_range)?;

    if names.len() == 1 {
        return Some(crate::text_range::extend_range_to_line_end(
            source,
            var_decl.text_range(),
        ));
    }

    if index + 1 < names.len() {
        let end = names[index + 1].text_range().start();
        return Some(TextRange::new(symbol_range.start(), end));
    }

    let tokens: Vec<SyntaxToken> = var_decl
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .collect();
    let target_index = tokens
        .iter()
        .position(|token| token.text_range() == symbol_range)?;
    let comma = tokens[..target_index]
        .iter()
        .rev()
        .find(|token| token.kind() == SyntaxKind::Comma)?;

    let mut end = var_decl.text_range().end();
    for token in tokens.iter().skip(target_index + 1) {
        if token.kind().is_trivia() {
            end = token.text_range().end();
            continue;
        }
        end = token.text_range().start();
        break;
    }

    Some(TextRange::new(comma.text_range().start(), end))
}

fn text_for_range(source: &str, range: TextRange) -> String {
    utilities::text_for_range(source, range)
}

fn normalize_member_name(name: &str) -> SmolStr {
    SmolStr::new(name.to_ascii_lowercase())
}

fn is_expression_kind(kind: SyntaxKind) -> bool {
    kind.is_initializer_expression_node()
}

fn qualified_name_parts(node: &SyntaxNode) -> Vec<SmolStr> {
    utilities::qualified_name_parts(node)
}

fn path_eq_ignore_ascii_case(a: &[SmolStr], b: &[SmolStr]) -> bool {
    utilities::path_eq_ignore_ascii_case(a, b)
}

fn path_starts_with_ignore_ascii_case(path: &[SmolStr], prefix: &[SmolStr]) -> bool {
    utilities::path_starts_with_ignore_ascii_case(path, prefix)
}

fn join_namespace_path(parts: &[SmolStr]) -> String {
    utilities::join_namespace_path(parts)
}

fn node_token_range(node: &SyntaxNode) -> text_size::TextRange {
    utilities::node_token_range(node)
}
