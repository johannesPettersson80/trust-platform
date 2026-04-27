struct FunctionCallUpdateContext<'a> {
    function_file_id: FileId,
    function_node: &'a SyntaxNode,
    function_name: &'a SmolStr,
    function_id: SymbolId,
    function_type: &'a str,
    output_name: Option<&'a str>,
}

fn update_function_call_sites(
    db: &Database,
    context: FunctionCallUpdateContext<'_>,
    edits: &mut RenameResult,
) -> Option<()> {
    let function_range = context.function_node.text_range();
    let mut owner_instances: FxHashMap<OwnerKey, SmolStr> = FxHashMap::default();
    let mut inserted_calls: FxHashSet<StatementKey> = FxHashSet::default();

    for ref_file_id in db.file_ids() {
        let source = db.source_text(ref_file_id);
        let root = parse(&source).syntax();
        let symbols = db.file_symbols_with_project(ref_file_id);

        for call_expr in root
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::CallExpr)
        {
            if ref_file_id == context.function_file_id
                && range_contains(function_range, call_expr.text_range())
            {
                continue;
            }

            let Some(callee_range) = call_targets_function(
                db,
                ref_file_id,
                &source,
                &root,
                &symbols,
                &call_expr,
                context.function_id,
            ) else {
                continue;
            };

            let call_context = call_expr_context(&source, &call_expr)?;
            if matches!(call_context.kind, CallContextKind::Expression)
                && context.output_name.is_none()
            {
                return None;
            }

            let owner = call_expr.ancestors().find(|node| {
                matches!(
                    node.kind(),
                    SyntaxKind::Program
                        | SyntaxKind::Function
                        | SyntaxKind::FunctionBlock
                        | SyntaxKind::Method
                        | SyntaxKind::Action
                )
            })?;
            let owner_key = OwnerKey {
                file_id: ref_file_id,
                range: owner.text_range(),
            };
            let instance_name = if let Some(name) = owner_instances.get(&owner_key) {
                name.clone()
            } else {
                let base = format!("{}Instance", context.function_name);
                let name = SmolStr::new(unique_local_name(&symbols, &owner, &base));
                let insert_offset = var_block_insert_offset(&owner)?;
                let indent = line_indent_at_offset(&source, TextSize::from(insert_offset as u32));
                let indent_unit = indent_unit_for(&indent);
                let var_block =
                    build_var_block(&indent, indent_unit, name.as_str(), context.function_type);
                let insert_text = build_insert_text(&source, insert_offset, &var_block);
                edits.add_edit(
                    ref_file_id,
                    TextEdit {
                        range: TextRange::new(
                            TextSize::from(insert_offset as u32),
                            TextSize::from(insert_offset as u32),
                        ),
                        new_text: insert_text,
                    },
                );
                owner_instances.insert(owner_key, name.clone());
                name
            };

            match call_context.kind {
                CallContextKind::Statement => {
                    edits.add_edit(
                        ref_file_id,
                        TextEdit {
                            range: callee_range,
                            new_text: instance_name.to_string(),
                        },
                    );
                }
                CallContextKind::Expression => {
                    let output_name = context.output_name?;
                    let args_text = call_expr_args_text(&source, &call_expr);
                    let call_line =
                        format!("{}{}{};", call_context.indent, instance_name, args_text);
                    let statement_key = StatementKey {
                        file_id: ref_file_id,
                        range: call_context.stmt_range,
                    };
                    if inserted_calls.insert(statement_key) {
                        edits.add_edit(
                            ref_file_id,
                            TextEdit {
                                range: TextRange::new(
                                    TextSize::from(call_context.insert_offset as u32),
                                    TextSize::from(call_context.insert_offset as u32),
                                ),
                                new_text: build_prefix_insert_text(
                                    &source,
                                    call_context.insert_offset,
                                    &call_line,
                                ),
                            },
                        );
                    }
                    edits.add_edit(
                        ref_file_id,
                        TextEdit {
                            range: call_expr.text_range(),
                            new_text: format!("{instance_name}.{output_name}"),
                        },
                    );
                }
            }
        }
    }

    Some(())
}

fn function_block_has_type_references(db: &Database, owner_name: &str) -> bool {
    for file_id in db.file_ids() {
        let source = db.source_text(file_id);
        let parsed = parse(&source);
        let root = parsed.syntax();
        let symbols = db.file_symbols_with_project(file_id);
        for name_node in root
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::Name)
        {
            if !is_type_name_node(&name_node) {
                continue;
            }
            let Some(symbol_id) = resolve_type_symbol_at_node(&symbols, &root, &name_node) else {
                continue;
            };
            let Some(candidate) = symbol_qualified_name(&symbols, symbol_id) else {
                continue;
            };
            if candidate.eq_ignore_ascii_case(owner_name) {
                return true;
            }
        }
    }
    false
}

struct OutputVarInfo {
    name: SmolStr,
    type_name: String,
    removal_range: TextRange,
}

fn output_var_info(source: &str, _root: &SyntaxNode, node: &SyntaxNode) -> Option<OutputVarInfo> {
    let block = node
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
        .find(|block| var_block_kind(block) == Some(SyntaxKind::KwVarOutput))?;
    let decls: Vec<_> = block
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarDecl)
        .collect();
    if decls.len() != 1 {
        return None;
    }
    let decl = decls.first()?;
    let names: Vec<_> = decl
        .children()
        .filter(|child| child.kind() == SyntaxKind::Name)
        .filter_map(|node| ident_token_in_name(&node))
        .collect();
    if names.len() != 1 {
        return None;
    }
    let ident = names.first()?;
    let type_node = decl
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)?;
    let type_name = text_for_range(source, type_node.text_range());
    if type_name.is_empty() {
        return None;
    }
    let removal_range = crate::text_range::extend_range_to_line_end(source, block.text_range());
    Some(OutputVarInfo {
        name: SmolStr::new(ident.text()),
        type_name,
        removal_range,
    })
}

fn is_statement_kind(kind: SyntaxKind) -> bool {
    kind.is_statement_node()
}

fn find_enclosing_owner_node(
    root: &SyntaxNode,
    position: TextSize,
    kinds: &[SyntaxKind],
) -> Option<SyntaxNode> {
    let token = root.token_at_offset(position).right_biased()?;
    token
        .parent_ancestors()
        .find(|node| kinds.contains(&node.kind()))
}

fn initializer_expr_in_var_decl(var_decl: &SyntaxNode) -> Option<SyntaxNode> {
    var_decl
        .children()
        .find(|node| is_expression_kind(node.kind()))
}

fn inline_expr_info(
    db: &Database,
    file_id: FileId,
    source: &str,
    root: &SyntaxNode,
    expr: &SyntaxNode,
) -> Option<InlineExprInfo> {
    let text = text_for_range(source, expr.text_range());
    if text.is_empty() {
        return None;
    }
    let symbols = db.file_symbols_with_project(file_id);
    let context = ConstExprContext {
        db,
        file_id,
        source,
        root,
        symbols: &symbols,
    };
    let const_info = expression_const_info(&context, expr);
    Some(InlineExprInfo {
        text,
        kind: expr.kind(),
        is_const_expr: const_info.is_const,
        is_path_like: expression_is_path_like(expr),
        requires_local_scope: const_info.requires_local_scope,
    })
}

struct ConstExprContext<'a> {
    db: &'a Database,
    file_id: FileId,
    source: &'a str,
    root: &'a SyntaxNode,
    symbols: &'a SymbolTable,
}

struct ConstExprInfo {
    is_const: bool,
    requires_local_scope: bool,
}

fn expression_const_info(context: &ConstExprContext<'_>, expr: &SyntaxNode) -> ConstExprInfo {
    match expr.kind() {
        SyntaxKind::Literal => ConstExprInfo {
            is_const: true,
            requires_local_scope: false,
        },
        SyntaxKind::NameRef => name_ref_const_info(context, expr),
        SyntaxKind::FieldExpr => field_expr_const_info(context, expr),
        SyntaxKind::ParenExpr | SyntaxKind::UnaryExpr | SyntaxKind::BinaryExpr => {
            let mut requires_local_scope = false;
            for child in expr
                .children()
                .filter(|child| is_expression_kind(child.kind()))
            {
                let info = expression_const_info(context, &child);
                if !info.is_const {
                    return ConstExprInfo {
                        is_const: false,
                        requires_local_scope: false,
                    };
                }
                if info.requires_local_scope {
                    requires_local_scope = true;
                }
            }
            ConstExprInfo {
                is_const: true,
                requires_local_scope,
            }
        }
        _ => ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        },
    }
}

fn name_ref_const_info(context: &ConstExprContext<'_>, node: &SyntaxNode) -> ConstExprInfo {
    let Some(offset) = node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| token.text_range().start())
    else {
        return ConstExprInfo {
            is_const: false,
            requires_local_scope: false,
        };
    };
    let target = resolve_target_at_position_with_context(
        context.db,
        context.file_id,
        offset,
        context.source,
        context.root,
        context.symbols,
    );
    let Some(ResolvedTarget::Symbol(symbol_id)) = target else {
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
        requires_local_scope: symbol.parent.is_some(),
    }
}
