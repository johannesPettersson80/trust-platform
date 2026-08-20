fn lower_program_var_blocks(
    program: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<ProgramVars, CompileError> {
    validate_special_var_sections(program, "PROGRAM", true)?;
    resolve_pou_local_constants(program, ctx)?;
    let mut globals = Vec::new();
    let mut vars = Vec::new();
    let mut temps = Vec::new();
    for var_block in program
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
    {
        let kind = var_block_kind(&var_block)?;
        let qualifiers = var_block_qualifiers(&var_block)?;
        validate_retention_policy(
            "program",
            kind,
            qualifiers,
            &[
                VarBlockKind::Input,
                VarBlockKind::Output,
                VarBlockKind::Var,
                VarBlockKind::Stat,
                VarBlockKind::Global,
            ],
        )?;
        for var_decl in var_block
            .children()
            .filter(|child| child.kind() == SyntaxKind::VarDecl)
        {
            let parts = parse_var_decl(&var_decl)?;
            reject_borrowed_storage_initializer(kind, &parts)?;
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
                && matches!(
                    kind,
                    VarBlockKind::Var | VarBlockKind::Stat | VarBlockKind::Global | VarBlockKind::Temp
                )
            {
                if let Some(expr) = init_expr.as_ref() {
                    let value = ctx.eval_compile_time_const_initializer(expr, type_id)?;
                    for name in &parts.names {
                        if matches!(kind, VarBlockKind::Global) {
                            let qualified = namespace_qualified_name(&var_block, name.as_str());
                            ctx.register_global_compile_time_const(
                                qualified.as_str(),
                                value.clone(),
                            );
                        } else {
                            ctx.register_compile_time_const(name.as_str(), value.clone());
                        }
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
                VarBlockKind::Temp => {
                    for name in parts.names {
                        temps.push(VarDef {
                            name,
                            type_id,
                            initializer: init_expr.clone(),
                            retain: qualifiers.retain,
                            static_storage: false,
                            external: false,
                            in_out: false,
                            constant: qualifiers.constant,
                            address: address_info.clone(),
                        });
                    }
                }
                VarBlockKind::External => {
                    continue;
                }
                VarBlockKind::Global => {
                    for name in parts.names {
                        globals.push(GlobalInit {
                            name: namespace_qualified_name(&var_block, name.as_str()),
                            type_id,
                            initializer: init_expr.clone(),
                            retain: qualifiers.retain,
                            address: parts.address.clone(),
                        });
                    }
                }
                VarBlockKind::Input
                | VarBlockKind::Output
                | VarBlockKind::InOut
                | VarBlockKind::Var
                | VarBlockKind::Stat => {
                    for name in parts.names {
                        vars.push(VarDef {
                            name,
                            type_id,
                            initializer: init_expr.clone(),
                            retain: qualifiers.retain,
                            static_storage: false,
                            external: false,
                            in_out: kind == VarBlockKind::InOut,
                            constant: qualifiers.constant,
                            address: address_info.clone(),
                        });
                    }
                }
                VarBlockKind::Unsupported => {
                    return Err(CompileError::new("unsupported VAR block in PROGRAM"));
                }
            }
        }
    }
    Ok(ProgramVars {
        globals,
        vars,
        temps,
    })
}
