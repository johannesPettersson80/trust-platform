type FunctionVarBlocks = (Vec<Param>, Vec<VarDef>, Vec<VarDef>);

fn lower_function_var_blocks(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<FunctionVarBlocks, CompileError> {
    let mut params = Vec::new();
    let mut locals = Vec::new();
    let mut statics = Vec::new();
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
            if qualifiers.constant
                && matches!(kind, VarBlockKind::Var | VarBlockKind::Stat | VarBlockKind::Temp)
            {
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
                VarBlockKind::Input => {
                    for name in parts.names {
                        params.push(Param {
                            name,
                            type_id,
                            direction: ParamDirection::In,
                            address: address_info.clone(),
                            default: init_expr.clone(),
                        });
                    }
                }
                VarBlockKind::Output => {
                    for name in parts.names {
                        params.push(Param {
                            name,
                            type_id,
                            direction: ParamDirection::Out,
                            address: address_info.clone(),
                            default: None,
                        });
                    }
                }
                VarBlockKind::InOut => {
                    for name in parts.names {
                        params.push(Param {
                            name,
                            type_id,
                            direction: ParamDirection::InOut,
                            address: address_info.clone(),
                            default: None,
                        });
                    }
                }
                VarBlockKind::Var | VarBlockKind::Temp => {
                    for name in parts.names {
                        locals.push(VarDef {
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
                VarBlockKind::Stat => {
                    for name in parts.names {
                        statics.push(VarDef {
                            name,
                            type_id,
                            initializer: init_expr.clone(),
                            retain: qualifiers.retain,
                            static_storage: true,
                            external: false,
                            constant: qualifiers.constant,
                            address: address_info.clone(),
                        });
                    }
                }
                VarBlockKind::External => {
                    continue;
                }
                VarBlockKind::Global | VarBlockKind::Unsupported => {
                    return Err(CompileError::new(
                        "unsupported VAR block in function or function block",
                    ));
                }
            }
        }
    }
    Ok((params, locals, statics))
}
