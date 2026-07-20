pub(in crate::harness) fn lower_enclosing_storage_type(
    node: &SyntaxNode,
    name: &str,
    ctx: &mut LoweringContext<'_>,
) -> Result<Option<TypeId>, CompileError> {
    let (semantic_db, semantic_file_id) = match (ctx.semantic_db, ctx.semantic_file_id) {
        (Some(db), Some(file_id)) => (db, file_id),
        _ => return Ok(None),
    };
    let Some(owner) = node.ancestors().find(|ancestor| {
        matches!(
            ancestor.kind(),
            SyntaxKind::Program
                | SyntaxKind::Function
                | SyntaxKind::FunctionBlock
                | SyntaxKind::Class
                | SyntaxKind::Method
                | SyntaxKind::Property
        )
    }) else {
        return Ok(None);
    };
    let Some(owner_name) = owner
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
    else {
        return Ok(None);
    };

    let analysis = semantic_db.analyze(semantic_file_id);
    let symbols = analysis.symbols.as_ref();
    let owner_token_range = owner_name
        .descendants_with_tokens()
        .find_map(|element| {
            element
                .into_token()
                .filter(|token| token.kind() == SyntaxKind::Ident)
                .map(|token| token.text_range())
        })
        .unwrap_or_else(|| owner_name.text_range());
    let Some(owner_id) =
        symbols.lookup_by_name_range(&node_text(&owner_name), owner_token_range)
    else {
        return Ok(None);
    };
    let Some(scope_id) = symbols.scope_for_owner(owner_id) else {
        return Ok(None);
    };
    let Some(symbol_id) = symbols.resolve(name, scope_id) else {
        return Ok(None);
    };
    let Some(symbol) = symbols.get(symbol_id) else {
        return Ok(None);
    };
    if !matches!(
        symbol.kind,
        SymbolKind::Variable { .. } | SymbolKind::Parameter { .. }
    ) {
        return Ok(None);
    }
    if symbol.type_id == TypeId::UNKNOWN {
        return Ok(None);
    }
    import_hir_type_to_runtime(ctx.registry, symbols, symbol.type_id).map(Some)
}
