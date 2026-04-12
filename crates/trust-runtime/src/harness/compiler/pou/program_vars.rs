fn lower_program_var_blocks(
    program: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<ProgramVars, CompileError> {
    let mut globals = Vec::new();
    let mut vars = Vec::new();
    let mut temps = Vec::new();
    for var_block in program
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
    {
        let kind = var_block_kind(&var_block)?;
        let qualifiers = var_block_qualifiers(&var_block);
        for var_decl in var_block
            .children()
            .filter(|child| child.kind() == SyntaxKind::VarDecl)
        {
            let (names, type_ref, initializer, address) = parse_var_decl(&var_decl)?;
            let type_id = lower_type_ref(&type_ref, ctx)?;
            let init_expr = initializer.map(|expr| lower_expr(&expr, ctx)).transpose()?;
            let address_info = address
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
                    for name in names {
                        temps.push(VarDef {
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
                VarBlockKind::Global => {
                    for name in names {
                        globals.push(GlobalInit {
                            name: namespace_qualified_name(&var_block, name.as_str()),
                            type_id,
                            initializer: init_expr.clone(),
                            retain: qualifiers.retain,
                            address: address.clone(),
                            using: ctx.using.clone(),
                        });
                    }
                }
                VarBlockKind::Input
                | VarBlockKind::Output
                | VarBlockKind::InOut
                | VarBlockKind::Var
                | VarBlockKind::Stat => {
                    for name in names {
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
