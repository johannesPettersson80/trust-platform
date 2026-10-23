fn trim_range_to_non_whitespace(source: &str, range: TextRange) -> Option<TextRange> {
    utilities::trim_range_to_non_whitespace(source, range)
}

fn range_contains(outer: TextRange, inner: TextRange) -> bool {
    utilities::range_contains(outer, inner)
}

fn ranges_overlap(a: TextRange, b: TextRange) -> bool {
    utilities::ranges_overlap(a, b)
}

fn enclosing_stmt_list(root: &SyntaxNode, range: TextRange) -> Option<SyntaxNode> {
    let start_token = root.token_at_offset(range.start()).right_biased()?;
    let end_token = root.token_at_offset(range.end()).left_biased()?;
    let start_list = start_token
        .parent_ancestors()
        .find(|node| node.kind() == SyntaxKind::StmtList)?;
    let end_list = end_token
        .parent_ancestors()
        .find(|node| node.kind() == SyntaxKind::StmtList)?;
    if start_list.text_range() == end_list.text_range() {
        Some(start_list)
    } else {
        None
    }
}

fn statement_range_for_selection(
    source: &str,
    root: &SyntaxNode,
    selection: TextRange,
) -> Option<TextRange> {
    let stmt_list = enclosing_stmt_list(root, selection)?;
    let mut selected = Vec::new();
    for child in stmt_list
        .children()
        .filter(|node| is_statement_kind(node.kind()))
    {
        if ranges_overlap(child.text_range(), selection) {
            selected.push(child);
        }
    }
    if selected.is_empty() {
        return None;
    }
    let start = selected.first()?.text_range().start();
    let end = selected.last()?.text_range().end();
    let covered = TextRange::new(start, end);
    let trimmed = trim_range_to_non_whitespace(source, selection)?;
    let covered_trimmed = trim_range_to_non_whitespace(source, covered)?;
    if trimmed != covered_trimmed {
        return None;
    }
    Some(covered)
}

fn expression_node_for_selection(root: &SyntaxNode, selection: TextRange) -> Option<SyntaxNode> {
    let token = root.token_at_offset(selection.start()).right_biased()?;
    for expr in token
        .parent_ancestors()
        .filter(|node| is_expression_kind(node.kind()))
    {
        let expr_range = node_token_range(&expr);
        if expr_range == selection {
            return Some(expr);
        }
    }
    None
}

fn keyword_token(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == kind)
}

fn function_return_type_range(node: &SyntaxNode) -> Option<TextRange> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let type_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)?;
    let name_end = name_node.text_range().end();
    let type_start = type_node.text_range().start();
    let colon = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| {
            token.kind() == SyntaxKind::Colon
                && token.text_range().start() >= name_end
                && token.text_range().end() <= type_start
        })?;
    Some(TextRange::new(
        colon.text_range().start(),
        type_node.text_range().end(),
    ))
}

fn has_var_output_block(node: &SyntaxNode) -> bool {
    node.children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
        .any(|block| var_block_kind(&block) == Some(SyntaxKind::KwVarOutput))
}

fn var_block_insert_offset(node: &SyntaxNode) -> Option<usize> {
    for child in node.children() {
        if matches!(child.kind(), SyntaxKind::VarBlock | SyntaxKind::StmtList) {
            return Some(usize::from(child.text_range().start()));
        }
    }
    Some(usize::from(node.text_range().end()))
}

fn var_block_kind(node: &SyntaxNode) -> Option<SyntaxKind> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
}

fn replace_name_refs(
    source: &str,
    stmt_list: &SyntaxNode,
    from: &str,
    to: &str,
    edits: &mut RenameResult,
    file_id: FileId,
) {
    for node in stmt_list
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::NameRef)
    {
        if is_call_name(&node) {
            continue;
        }
        let range = node_token_range(&node);
        let text = text_for_range(source, range);
        if text.eq_ignore_ascii_case(from) {
            edits.add_edit(
                file_id,
                TextEdit {
                    range,
                    new_text: to.to_string(),
                },
            );
        }
    }
}

fn owner_symbol_id(symbols: &SymbolTable, owner_node: &SyntaxNode) -> Option<SymbolId> {
    let name_node = owner_node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let ident = ident_token_in_name(&name_node)?;
    symbols
        .iter()
        .find(|symbol| symbol.range == ident.text_range())
        .map(|symbol| symbol.id)
}

fn unique_member_name(symbols: &SymbolTable, owner_id: SymbolId, base: &str) -> SmolStr {
    let mut index = 0;
    loop {
        let candidate = if index == 0 {
            base.to_string()
        } else {
            format!("{base}{index}")
        };
        if is_valid_identifier(&candidate)
            && !is_reserved_keyword(&candidate)
            && !symbols.iter().any(|symbol| {
                symbol.parent == Some(owner_id)
                    && symbol.name.eq_ignore_ascii_case(candidate.as_str())
            })
        {
            return SmolStr::new(candidate);
        }
        index += 1;
    }
}

fn unique_top_level_name(symbols: &SymbolTable, base: &str) -> SmolStr {
    let mut index = 0;
    loop {
        let candidate = if index == 0 {
            base.to_string()
        } else {
            format!("{base}{index}")
        };
        if is_valid_identifier(&candidate)
            && !is_reserved_keyword(&candidate)
            && !symbols
                .iter()
                .any(|symbol| symbol.name.eq_ignore_ascii_case(candidate.as_str()))
        {
            return SmolStr::new(candidate);
        }
        index += 1;
    }
}

fn unique_local_name(symbols: &SymbolTable, owner_node: &SyntaxNode, base: &str) -> String {
    if let Some(owner_id) = owner_symbol_id(symbols, owner_node) {
        unique_member_name(symbols, owner_id, base).to_string()
    } else {
        base.to_string()
    }
}

fn is_call_name(expr: &SyntaxNode) -> bool {
    let Some(parent) = expr.parent() else {
        return false;
    };
    if parent.kind() != SyntaxKind::CallExpr {
        return false;
    }
    parent
        .first_child()
        .is_some_and(|child| child.text_range() == expr.text_range())
}

fn is_write_context(expr: &SyntaxNode) -> bool {
    let mut current = expr.clone();
    while let Some(parent) = current.parent() {
        if parent.kind() == SyntaxKind::AssignStmt {
            if let Some(first_child) = parent.first_child() {
                return first_child.text_range() == current.text_range();
            }
            return false;
        }
        if matches!(
            parent.kind(),
            SyntaxKind::FieldExpr | SyntaxKind::IndexExpr | SyntaxKind::DerefExpr
        ) {
            current = parent;
            continue;
        }
        break;
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct OwnerKey {
    file_id: FileId,
    range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StatementKey {
    file_id: FileId,
    range: TextRange,
}

enum CallContextKind {
    Statement,
    Expression,
}

struct CallContext {
    kind: CallContextKind,
    stmt_range: TextRange,
    insert_offset: usize,
    indent: String,
}

fn has_recursive_call(source: &str, function_node: &SyntaxNode, function_name: &str) -> bool {
    let Some(stmt_list) = function_node
        .children()
        .find(|child| child.kind() == SyntaxKind::StmtList)
    else {
        return false;
    };
    for name_ref in stmt_list
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::NameRef)
    {
        if !is_call_name(&name_ref) {
            continue;
        }
        let range = node_token_range(&name_ref);
        let text = text_for_range(source, range);
        if text.eq_ignore_ascii_case(function_name) {
            return true;
        }
    }
    false
}

fn call_callee_node(call_expr: &SyntaxNode) -> Option<SyntaxNode> {
    call_expr.children().find(|child| {
        matches!(
            child.kind(),
            SyntaxKind::NameRef
                | SyntaxKind::FieldExpr
                | SyntaxKind::QualifiedName
                | SyntaxKind::Name
        )
    })
}

fn call_expr_args_text(source: &str, call_expr: &SyntaxNode) -> String {
    if let Some(arg_list) = call_expr
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)
    {
        let text = text_for_range(source, arg_list.text_range());
        if !text.is_empty() {
            return text;
        }
    }
    "()".to_string()
}

fn call_expr_context(source: &str, call_expr: &SyntaxNode) -> Option<CallContext> {
    let stmt = if let Some(parent) = call_expr.parent() {
        match parent.kind() {
            SyntaxKind::ExprStmt => Some(parent),
            _ => None,
        }
    } else {
        None
    };

    if let Some(stmt) = stmt {
        let stmt_range = stmt.text_range();
        let indent = line_indent_at_offset(source, stmt_range.start());
        return Some(CallContext {
            kind: CallContextKind::Statement,
            stmt_range,
            insert_offset: usize::from(stmt_range.start()),
            indent,
        });
    }

    if let Some(assign_stmt) = call_expr
        .ancestors()
        .find(|node| node.kind() == SyntaxKind::AssignStmt)
    {
        if let Some(rhs_expr) = assign_rhs_expr(&assign_stmt)
            && node_token_range(&rhs_expr) == node_token_range(call_expr)
        {
            let stmt_range = assign_stmt.text_range();
            let indent = line_indent_at_offset(source, stmt_range.start());
            return Some(CallContext {
                kind: CallContextKind::Expression,
                stmt_range,
                insert_offset: usize::from(stmt_range.start()),
                indent,
            });
        }
        return None;
    }

    if let Some(return_stmt) = call_expr
        .ancestors()
        .find(|node| node.kind() == SyntaxKind::ReturnStmt)
    {
        let expr = return_stmt
            .children()
            .find(|node| is_expression_kind(node.kind()))?;
        if node_token_range(&expr) == node_token_range(call_expr) {
            let stmt_range = return_stmt.text_range();
            let indent = line_indent_at_offset(source, stmt_range.start());
            return Some(CallContext {
                kind: CallContextKind::Expression,
                stmt_range,
                insert_offset: usize::from(stmt_range.start()),
                indent,
            });
        }
        return None;
    }

    None
}

fn call_targets_function(
    db: &Database,
    file_id: FileId,
    source: &str,
    root: &SyntaxNode,
    symbols: &SymbolTable,
    call_expr: &SyntaxNode,
    function_id: SymbolId,
) -> Option<TextRange> {
    let callee = call_callee_node(call_expr)?;
    let callee_range = node_token_range(&callee);
    let target = resolve_target_at_position_with_context(
        db,
        file_id,
        callee_range.start(),
        source,
        root,
        symbols,
    );
    if let Some(ResolvedTarget::Symbol(symbol_id)) = target
        && symbol_id == function_id
    {
        return Some(callee_range);
    }

    if let Some(parts) = match callee.kind() {
        SyntaxKind::FieldExpr => qualified_name_from_field_expr(&callee),
        SyntaxKind::QualifiedName | SyntaxKind::Name => qualified_name_parts_from_node(&callee),
        _ => None,
    } && symbols.resolve_qualified(&parts) == Some(function_id)
    {
        return Some(callee_range);
    }

    None
}

fn assign_rhs_expr(assign_stmt: &SyntaxNode) -> Option<SyntaxNode> {
    let mut exprs = assign_stmt
        .children()
        .filter(|node| is_expression_kind(node.kind()));
    let _lhs = exprs.next()?;
    let rhs = exprs.next()?;
    if exprs.next().is_some() {
        return None;
    }
    Some(rhs)
}

fn build_prefix_insert_text(source: &str, insert_offset: usize, line: &str) -> String {
    let mut insert = String::new();
    if insert_offset > 0 {
        let prev = source.as_bytes()[insert_offset - 1];
        if prev != b'\n' && prev != b'\r' {
            insert.push('\n');
        }
    }
    insert.push_str(line);
    if !line.ends_with('\n') {
        insert.push('\n');
    }
    insert
}
