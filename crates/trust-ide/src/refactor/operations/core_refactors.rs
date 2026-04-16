/// Parses a dotted namespace path into parts, validating identifiers.
pub fn parse_namespace_path(path: &str) -> Option<Vec<SmolStr>> {
    utilities::parse_namespace_path(path)
}

/// Returns the full namespace path (including the namespace itself).
pub(crate) fn namespace_full_path(
    symbols: &SymbolTable,
    symbol_id: SymbolId,
) -> Option<Vec<SmolStr>> {
    utilities::namespace_full_path(symbols, symbol_id)
}

fn symbol_qualified_name(symbols: &SymbolTable, symbol_id: SymbolId) -> Option<String> {
    utilities::symbol_qualified_name(symbols, symbol_id)
}

/// Moves a namespace path by rewriting `USING` directives and qualified names.
///
/// The namespace leaf name must remain unchanged; this does not relocate declarations.
pub fn move_namespace_path(
    db: &Database,
    old_path: &[SmolStr],
    new_path: &[SmolStr],
) -> Option<RenameResult> {
    if old_path.is_empty() || new_path.is_empty() {
        return None;
    }
    if !old_path
        .last()
        .zip(new_path.last())
        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b.as_str()))
    {
        return None;
    }

    let mut result = RenameResult::new();

    for file_id in db.file_ids() {
        apply_move_in_file(db, file_id, old_path, new_path, &mut result);
    }

    if result.edit_count() == 0 {
        None
    } else {
        Some(result)
    }
}

/// Generates stub implementations for missing interface members on a class/function block.
///
/// Returns edits that insert method/property stubs before END_CLASS/END_FUNCTION_BLOCK.
pub fn generate_interface_stubs(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<RenameResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let symbols = db.file_symbols_with_project(file_id);

    let owner_node = find_enclosing_owner_node(
        &root,
        position,
        &[SyntaxKind::Class, SyntaxKind::FunctionBlock],
    )?;
    let implements_clause = owner_node
        .children()
        .find(|child| child.kind() == SyntaxKind::ImplementsClause)?;
    let owner_name_node = owner_node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let owner_ident = ident_token_in_name(&owner_name_node)?;
    let owner_symbol = symbols.iter().find(|symbol| {
        symbol.range == owner_ident.text_range()
            && matches!(symbol.kind, SymbolKind::Class | SymbolKind::FunctionBlock)
    })?;
    let owner_id = owner_symbol.id;

    let implemented = collect_implementation_members(&symbols, owner_id);
    let interface_names = implements_clause_names(&implements_clause);
    let stubs =
        collect_missing_interface_stubs(db, &symbols, &interface_names, &implemented, file_id);

    if stubs.is_empty() {
        return None;
    }

    let member_indent = member_indent_for_owner(&source, &owner_node);
    let insert_offset = owner_end_token_offset(&owner_node)?;
    let insert_text = build_stub_insert_text(&source, insert_offset, &stubs, &member_indent);

    let mut result = RenameResult::new();
    result.add_edit(
        file_id,
        TextEdit {
            range: TextRange::new(
                TextSize::from(insert_offset as u32),
                TextSize::from(insert_offset as u32),
            ),
            new_text: insert_text,
        },
    );

    Some(result)
}

/// Inlines a variable/constant at the given position with safety checks.
pub fn inline_symbol(db: &Database, file_id: FileId, position: TextSize) -> Option<InlineResult> {
    let symbols = db.file_symbols_with_project(file_id);

    let target = resolve_target_at_position(db, file_id, position)?;
    let ResolvedTarget::Symbol(symbol_id) = target else {
        return None;
    };
    let symbol = symbols.get(symbol_id)?;

    let (kind, allow_inline) = if symbol_is_constant(symbol) {
        (InlineTargetKind::Constant, true)
    } else {
        match symbol.kind {
        SymbolKind::Variable { qualifier } => {
            let allowed = matches!(
                qualifier,
                trust_hir::symbols::VarQualifier::Local
                    | trust_hir::symbols::VarQualifier::Temp
                    | trust_hir::symbols::VarQualifier::Static
            );
            (InlineTargetKind::Variable, allowed)
        }
        _ => (InlineTargetKind::Variable, false),
        }
    };

    if !allow_inline {
        return None;
    }

    let (decl_file_id, decl_range) = if let Some(origin) = symbol.origin {
        let origin_symbols = db.file_symbols(origin.file_id);
        let origin_range = origin_symbols
            .get(origin.symbol_id)
            .map(|sym| sym.range)
            .unwrap_or(symbol.range);
        (origin.file_id, origin_range)
    } else {
        (file_id, symbol.range)
    };
    let decl_source = db.source_text(decl_file_id);
    let decl_root = parse(&decl_source).syntax();
    let var_decl = crate::var_decl::find_var_decl_for_range(&decl_root, decl_range)?;
    let expr = initializer_expr_in_var_decl(&var_decl)?;
    let expr_info = inline_expr_info(db, decl_file_id, &decl_source, &decl_root, &expr)?;

    if !expr_info.is_const_expr {
        return None;
    }

    let references = find_references(
        db,
        file_id,
        position,
        FindReferencesOptions {
            include_declaration: false,
        },
    );
    if references.is_empty() {
        return None;
    }

    if references.iter().any(|reference| reference.is_write) {
        return None;
    }

    if references.iter().any(|reference| {
        reference_has_disallowed_context(
            db,
            reference.file_id,
            reference.range,
            expr_info.is_path_like,
        )
    }) {
        return None;
    }

    if references
        .iter()
        .any(|reference| reference.file_id != decl_file_id)
        && expr_info.requires_local_scope
    {
        return None;
    }

    let replacement = wrap_expression_for_inline(expr_info.kind, &expr_info.text);

    let removal_range = var_decl_removal_range(&decl_source, &decl_root, decl_range)?;

    let mut edits = RenameResult::new();
    for reference in references {
        edits.add_edit(
            reference.file_id,
            TextEdit {
                range: reference.range,
                new_text: replacement.clone(),
            },
        );
    }
    edits.add_edit(
        decl_file_id,
        TextEdit {
            range: removal_range,
            new_text: String::new(),
        },
    );

    Some(InlineResult {
        edits,
        name: symbol.name.clone(),
        kind,
    })
}

/// Extracts selected statements into a new METHOD on the enclosing CLASS/FUNCTION_BLOCK.
pub fn extract_method(db: &Database, file_id: FileId, range: TextRange) -> Option<ExtractResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let selection = trim_range_to_non_whitespace(&source, range)?;

    let stmt_range = statement_range_for_selection(&source, &root, selection)?;
    let method_node = find_enclosing_owner_node(&root, selection.start(), &[SyntaxKind::Method])?;
    let owner_node = find_enclosing_owner_node(
        &root,
        selection.start(),
        &[SyntaxKind::Class, SyntaxKind::FunctionBlock],
    )?;
    if !range_contains(owner_node.text_range(), method_node.text_range()) {
        return None;
    }

    let statements = text_for_range(&source, stmt_range);
    if statements.is_empty() {
        return None;
    }

    let symbols = db.file_symbols_with_project(file_id);
    let owner_id = owner_symbol_id(&symbols, &owner_node)?;
    let method_id = owner_symbol_id(&symbols, &method_node)?;
    let name = unique_member_name(&symbols, owner_id, "ExtractedMethod");

    let params = collect_extract_params(db, file_id, &source, &root, selection, |symbol| {
        symbol.parent == Some(method_id)
    });
    let member_indent = member_indent_for_owner(&source, &owner_node);
    let indent_unit = indent_unit_for(&member_indent);
    let param_blocks = build_param_blocks(&params, &member_indent, indent_unit);
    let call_args = build_formal_args(&params);
    let body_indent = format!("{member_indent}{indent_unit}");
    let body_text = reindent_block(&statements, &body_indent);
    let method_text = build_method_extract_text(&name, &member_indent, &param_blocks, &body_text);

    let insert_offset = owner_end_token_offset(&owner_node)?;
    let insert_text = build_insert_text(&source, insert_offset, &method_text);

    let call_indent = line_indent_at_offset(&source, stmt_range.start());
    let call_text = call_replace_text(&source, stmt_range, &call_indent, &name, &call_args);

    let mut edits = RenameResult::new();
    edits.add_edit(
        file_id,
        TextEdit {
            range: stmt_range,
            new_text: call_text,
        },
    );
    edits.add_edit(
        file_id,
        TextEdit {
            range: TextRange::new(
                TextSize::from(insert_offset as u32),
                TextSize::from(insert_offset as u32),
            ),
            new_text: insert_text,
        },
    );

    Some(ExtractResult {
        edits,
        name,
        kind: ExtractTargetKind::Method,
    })
}

/// Extracts a selected expression into a GET-only PROPERTY on the enclosing CLASS.
pub fn extract_property(db: &Database, file_id: FileId, range: TextRange) -> Option<ExtractResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let selection = trim_range_to_non_whitespace(&source, range)?;
    let expr_node = expression_node_for_selection(&root, selection)?;

    if is_write_context(&expr_node) || is_call_name(&expr_node) {
        return None;
    }

    let owner_node = find_enclosing_owner_node(&root, selection.start(), &[SyntaxKind::Class])?;
    let symbols = db.file_symbols_with_project(file_id);
    let owner_id = owner_symbol_id(&symbols, &owner_node)?;

    let target = resolve_target_at_position_with_context(
        db,
        file_id,
        selection.start(),
        &source,
        &root,
        &symbols,
    )?;
    let type_name = match target {
        ResolvedTarget::Symbol(symbol_id) => symbols.type_name(symbols.get(symbol_id)?.type_id),
        ResolvedTarget::Field(field) => symbols.type_name(field.type_id),
    }?;

    let name = unique_member_name(&symbols, owner_id, "ExtractedProperty");
    let expr_text = text_for_range(&source, expr_node.text_range());
    if expr_text.is_empty() {
        return None;
    }

    let member_indent = member_indent_for_owner(&source, &owner_node);
    let indent_unit = indent_unit_for(&member_indent);
    let body_indent = format!("{member_indent}{indent_unit}");
    let property_text = build_property_extract_text(
        &name,
        type_name.as_str(),
        &expr_text,
        &member_indent,
        &body_indent,
    );

    let insert_offset = owner_end_token_offset(&owner_node)?;
    let insert_text = build_insert_text(&source, insert_offset, &property_text);

    let mut edits = RenameResult::new();
    edits.add_edit(
        file_id,
        TextEdit {
            range: selection,
            new_text: name.to_string(),
        },
    );
    edits.add_edit(
        file_id,
        TextEdit {
            range: TextRange::new(
                TextSize::from(insert_offset as u32),
                TextSize::from(insert_offset as u32),
            ),
            new_text: insert_text,
        },
    );

    Some(ExtractResult {
        edits,
        name,
        kind: ExtractTargetKind::Property,
    })
}

/// Extracts selected statements into a new FUNCTION POU.
pub fn extract_pou(db: &Database, file_id: FileId, range: TextRange) -> Option<ExtractResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let selection = trim_range_to_non_whitespace(&source, range)?;

    let owner_node = find_enclosing_owner_node(
        &root,
        selection.start(),
        &[
            SyntaxKind::Program,
            SyntaxKind::Function,
            SyntaxKind::FunctionBlock,
        ],
    )?;
    let symbols = db.file_symbols_with_project(file_id);
    let owner_id = owner_symbol_id(&symbols, &owner_node)?;
    let name = unique_top_level_name(&symbols, "ExtractedFunction");

    if let Some(expr_node) = expression_node_for_selection(&root, selection) {
        if is_write_context(&expr_node) || is_call_name(&expr_node) {
            return None;
        }
        let expr_id = db.expr_id_at_offset(file_id, u32::from(expr_node.text_range().start()))?;
        let type_name = symbols.type_name(db.type_of(file_id, expr_id))?;
        let expr_text = text_for_range(&source, expr_node.text_range());
        if expr_text.is_empty() {
            return None;
        }
        let params = collect_extract_params(db, file_id, &source, &root, selection, |symbol| {
            symbol.parent == Some(owner_id)
        });
        let indent_unit = indent_unit_for("");
        let param_blocks = build_param_blocks(&params, "", indent_unit);
        let call_args = build_formal_args(&params);
        let function_text = build_function_extract_text(
            &name,
            type_name.as_str(),
            &param_blocks,
            "",
            indent_unit,
            Some(&expr_text),
        );

        let insert_offset = usize::from(owner_node.text_range().end());
        let insert_text = build_insert_text(&source, insert_offset, &function_text);

        let call_text = build_call_expression(&name, &call_args);

        let mut edits = RenameResult::new();
        edits.add_edit(
            file_id,
            TextEdit {
                range: selection,
                new_text: call_text,
            },
        );
        edits.add_edit(
            file_id,
            TextEdit {
                range: TextRange::new(
                    TextSize::from(insert_offset as u32),
                    TextSize::from(insert_offset as u32),
                ),
                new_text: insert_text,
            },
        );

        return Some(ExtractResult {
            edits,
            name,
            kind: ExtractTargetKind::Function,
        });
    }

    let stmt_range = statement_range_for_selection(&source, &root, selection)?;
    let statements = text_for_range(&source, stmt_range);
    if statements.is_empty() {
        return None;
    }

    let params = collect_extract_params(db, file_id, &source, &root, selection, |symbol| {
        symbol.parent == Some(owner_id)
    });
    let indent_unit = indent_unit_for("");
    let param_blocks = build_param_blocks(&params, "", indent_unit);
    let call_args = build_formal_args(&params);
    let body_text = reindent_block(&statements, indent_unit);
    let function_text =
        build_function_extract_text(&name, "BOOL", &param_blocks, &body_text, indent_unit, None);

    let insert_offset = usize::from(owner_node.text_range().end());
    let insert_text = build_insert_text(&source, insert_offset, &function_text);

    let call_indent = line_indent_at_offset(&source, stmt_range.start());
    let call_text = call_replace_text(&source, stmt_range, &call_indent, &name, &call_args);

    let mut edits = RenameResult::new();
    edits.add_edit(
        file_id,
        TextEdit {
            range: stmt_range,
            new_text: call_text,
        },
    );
    edits.add_edit(
        file_id,
        TextEdit {
            range: TextRange::new(
                TextSize::from(insert_offset as u32),
                TextSize::from(insert_offset as u32),
            ),
            new_text: insert_text,
        },
    );

    Some(ExtractResult {
        edits,
        name,
        kind: ExtractTargetKind::Function,
    })
}

/// Converts a FUNCTION to a FUNCTION_BLOCK.
pub fn convert_function_to_function_block(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<RenameResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let node = find_enclosing_owner_node(&root, position, &[SyntaxKind::Function])?;

    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let function_name = name_from_name_node(&name_node)?;
    let symbols = db.file_symbols_with_project(file_id);
    let function_id = owner_symbol_id(&symbols, &node)?;
    let function_type =
        symbol_qualified_name(&symbols, function_id).unwrap_or_else(|| function_name.to_string());
    let mut output_name = None::<String>;

    if has_recursive_call(&source, &node, function_name.as_str()) {
        return None;
    }

    let mut edits = RenameResult::new();
    if let Some(token) = keyword_token(&node, SyntaxKind::KwFunction) {
        edits.add_edit(
            file_id,
            TextEdit {
                range: token.text_range(),
                new_text: "FUNCTION_BLOCK".to_string(),
            },
        );
    }
    if let Some(token) = keyword_token(&node, SyntaxKind::KwEndFunction) {
        edits.add_edit(
            file_id,
            TextEdit {
                range: token.text_range(),
                new_text: "END_FUNCTION_BLOCK".to_string(),
            },
        );
    }

    let return_type = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .map(|child| text_for_range(&source, child.text_range()));
    if let Some(range) = function_return_type_range(&node) {
        edits.add_edit(
            file_id,
            TextEdit {
                range,
                new_text: String::new(),
            },
        );
    }

    if let Some(return_type) = return_type {
        if has_var_output_block(&node) {
            return None;
        }
        let output_name_local = unique_local_name(&symbols, &node, "result");
        output_name = Some(output_name_local.clone());
        let insert_offset = var_block_insert_offset(&node)?;
        let indent = line_indent_at_offset(&source, TextSize::from(insert_offset as u32));
        let indent_unit = indent_unit_for(&indent);
        let var_block_text =
            build_var_output_block(&indent, indent_unit, &output_name_local, &return_type);
        edits.add_edit(
            file_id,
            TextEdit {
                range: TextRange::new(
                    TextSize::from(insert_offset as u32),
                    TextSize::from(insert_offset as u32),
                ),
                new_text: var_block_text,
            },
        );

        if let Some(stmt_list) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::StmtList)
        {
            replace_name_refs(
                &source,
                &stmt_list,
                function_name.as_str(),
                output_name_local.as_str(),
                &mut edits,
                file_id,
            );
        }
    }

    let call_context = FunctionCallUpdateContext {
        function_file_id: file_id,
        function_node: &node,
        function_name: &function_name,
        function_id,
        function_type: function_type.as_str(),
        output_name: output_name.as_deref(),
    };
    update_function_call_sites(db, call_context, &mut edits)?;

    (edits.edit_count() > 0).then_some(edits)
}

/// Converts a FUNCTION_BLOCK to a FUNCTION when it has a single VAR_OUTPUT variable.
pub fn convert_function_block_to_function(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<RenameResult> {
    let source = db.source_text(file_id);
    let parsed = parse(&source);
    let root = parsed.syntax();
    let node = find_enclosing_owner_node(&root, position, &[SyntaxKind::FunctionBlock])?;
    let symbols = db.file_symbols_with_project(file_id);
    let owner_id = owner_symbol_id(&symbols, &node)?;
    let owner_name = symbol_qualified_name(&symbols, owner_id)?;
    if function_block_has_type_references(db, owner_name.as_str()) {
        return None;
    }
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    let output = output_var_info(&source, &root, &node)?;
    let function_name = name_from_name_node(&name_node)?;

    let mut edits = RenameResult::new();
    if let Some(token) = keyword_token(&node, SyntaxKind::KwFunctionBlock) {
        edits.add_edit(
            file_id,
            TextEdit {
                range: token.text_range(),
                new_text: "FUNCTION".to_string(),
            },
        );
    }
    if let Some(token) = keyword_token(&node, SyntaxKind::KwEndFunctionBlock) {
        edits.add_edit(
            file_id,
            TextEdit {
                range: token.text_range(),
                new_text: "END_FUNCTION".to_string(),
            },
        );
    }

    let name_end = name_node.text_range().end();
    edits.add_edit(
        file_id,
        TextEdit {
            range: TextRange::new(name_end, name_end),
            new_text: format!(" : {}", output.type_name),
        },
    );

    edits.add_edit(
        file_id,
        TextEdit {
            range: output.removal_range,
            new_text: String::new(),
        },
    );

    if let Some(stmt_list) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::StmtList)
    {
        replace_name_refs(
            &source,
            &stmt_list,
            output.name.as_str(),
            function_name.as_str(),
            &mut edits,
            file_id,
        );
    }

    (edits.edit_count() > 0).then_some(edits)
}
