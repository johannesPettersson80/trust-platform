fn lower_class_var_blocks(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<VarDef>, CompileError> {
    let mut vars = Vec::new();
    for var_block in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
    {
        let kind = var_block_kind(&var_block)?;
        let qualifiers = var_block_qualifiers(&var_block);
        for var_decl in var_block
            .children()
            .filter(|child| child.kind() == SyntaxKind::VarDecl)
        {
            let parts = parse_var_decl(&var_decl)?;
            let type_id = lower_type_ref(&parts.type_ref, ctx)?;
            let init_expr = parts
                .initializer
                .as_ref()
                .map(|expr| {
                    lower_expr(expr, ctx).and_then(|lowered| {
                        resolve_initializer_enum_variant(expr, lowered, type_id, ctx)
                    })
                })
                .transpose()?;
            if qualifiers.constant && matches!(kind, VarBlockKind::Var | VarBlockKind::Stat) {
                if let Some(expr) = init_expr.as_ref() {
                    let value = ctx.eval_compile_time_const_initializer(expr, type_id)?;
                    for name in &parts.names {
                        ctx.register_compile_time_const(name.as_str(), value.clone());
                    }
                }
            }
            let address_info = parts
                .address
                .as_ref()
                .map(|text| IoAddress::parse(text))
                .transpose()
                .map_err(|err| CompileError::new(format!("invalid I/O address: {err}")))?;
            if matches!(kind, VarBlockKind::Input | VarBlockKind::InOut)
                && address_info
                    .as_ref()
                    .map(|addr| addr.wildcard)
                    .unwrap_or(false)
            {
                return Err(CompileError::new(
                    "wildcard address not allowed in VAR_INPUT/VAR_IN_OUT",
                ));
            }
            match kind {
                VarBlockKind::Var
                | VarBlockKind::Stat
                | VarBlockKind::Input
                | VarBlockKind::Output
                | VarBlockKind::InOut => {
                    for name in parts.names {
                        vars.push(VarDef {
                            name,
                            type_id,
                            initializer: init_expr.clone(),
                            retain: qualifiers.retain,
                            static_storage: false,
                            external: false,
                            constant: qualifiers.constant,
                            address: address_info.clone(),
                        });
                    }
                }
                VarBlockKind::External => {
                    continue;
                }
                _ => {
                    return Err(CompileError::new("unsupported VAR block in CLASS"));
                }
            }
        }
    }
    Ok(vars)
}
