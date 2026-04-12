fn lower_program_node(
    program_node: &SyntaxNode,
    registry: &mut trust_hir::types::TypeRegistry,
    profile: DateTimeProfile,
    file_id: u32,
    statement_locations: &mut Vec<crate::debug::SourceLocation>,
) -> Result<LoweredProgram, CompileError> {
    let name = qualified_pou_name(program_node)?;
    let using = collect_using_directives(program_node);
    let mut ctx = LoweringContext {
        registry,
        profile,
        using,
        file_id,
        statement_locations,
    };
    let vars = lower_program_var_blocks(program_node, &mut ctx)?;
    let body = lower_stmt_list(program_node, &mut ctx)?;
    Ok(LoweredProgram {
        program: ProgramDef {
            name,
            vars: vars.vars,
            temps: vars.temps,
            using: ctx.using.clone(),
            body,
        },
        globals: vars.globals,
    })
}

fn lower_function_block_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<FunctionBlockDef, CompileError> {
    let name = qualified_pou_name(node)?;
    let mut base = None;
    if let Some(extends_clause) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ExtendsClause)
    {
        if let Some(base_name) = extends_clause
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
        {
            let raw = node_text(&base_name);
            let resolved = resolve_named_type(ctx.registry, &raw, &ctx.using)?;
            let type_id = ctx
                .registry
                .lookup(resolved.as_ref())
                .ok_or_else(|| CompileError::new("unknown base type"))?;
            let base_type = ctx
                .registry
                .get(type_id)
                .ok_or_else(|| CompileError::new("unknown base type"))?;
            base = Some(match base_type {
                trust_hir::Type::FunctionBlock { .. } => FunctionBlockBase::FunctionBlock(resolved),
                trust_hir::Type::Class { .. } => FunctionBlockBase::Class(resolved),
                _ => {
                    return Err(CompileError::new(
                        "function block EXTENDS must reference a FUNCTION_BLOCK or CLASS",
                    ))
                }
            });
        }
    }
    let (params, vars, temps) = lower_function_block_var_blocks(node, ctx)?;
    let mut methods = Vec::new();
    for method_node in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::Method)
    {
        methods.push(lower_method_node(&method_node, ctx)?);
    }
    let body = lower_stmt_list(node, ctx)?;
    Ok(FunctionBlockDef {
        name,
        base,
        params,
        vars,
        temps,
        using: ctx.using.clone(),
        methods,
        body,
    })
}

fn lower_class_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<ClassDef, CompileError> {
    let name = qualified_pou_name(node)?;
    let mut base = None;
    if let Some(extends_clause) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ExtendsClause)
    {
        if let Some(base_name) = extends_clause
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
        {
            let raw = node_text(&base_name);
            base = Some(resolve_named_type(ctx.registry, &raw, &ctx.using)?);
        }
    }

    let vars = lower_class_var_blocks(node, ctx)?;
    let mut methods = Vec::new();
    for method_node in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::Method)
    {
        methods.push(lower_method_node(&method_node, ctx)?);
    }

    Ok(ClassDef {
        name,
        base,
        vars,
        using: ctx.using.clone(),
        methods,
    })
}

fn lower_interface_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<InterfaceDef, CompileError> {
    let name = qualified_pou_name(node)?;
    let mut base = None;
    if let Some(extends_clause) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ExtendsClause)
    {
        if let Some(base_name) = extends_clause
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
        {
            let raw = node_text(&base_name);
            base = Some(resolve_named_type(ctx.registry, &raw, &ctx.using)?);
        }
    }

    let mut methods = Vec::new();
    for method_node in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::Method)
    {
        methods.push(lower_method_node(&method_node, ctx)?);
    }

    Ok(InterfaceDef {
        name,
        base,
        using: ctx.using.clone(),
        methods,
    })
}

fn lower_function_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<FunctionDef, CompileError> {
    let name = qualified_pou_name(node)?;
    let return_type = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .ok_or_else(|| CompileError::new("missing function return type"))?;
    let return_type = lower_type_ref(&return_type, ctx)?;

    let (params, locals, static_locals) = lower_function_var_blocks(node, ctx)?;
    let body = lower_stmt_list(node, ctx)?;

    Ok(FunctionDef {
        name,
        return_type,
        params,
        locals,
        static_locals,
        using: ctx.using.clone(),
        body,
    })
}

fn lower_method_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<MethodDef, CompileError> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .ok_or_else(|| CompileError::new("missing method name"))?;
    let raw = node_text(&name_node);
    let name = qualify_with_namespaces(node, &raw);

    let using = collect_using_directives(node);
    let mut method_ctx = LoweringContext {
        registry: ctx.registry,
        profile: ctx.profile,
        using,
        file_id: ctx.file_id,
        statement_locations: ctx.statement_locations,
    };

    let return_type = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .map(|type_ref| lower_type_ref(&type_ref, &mut method_ctx))
        .transpose()?;

    let (params, locals, static_locals) = lower_function_var_blocks(node, &mut method_ctx)?;
    let body = lower_stmt_list(node, &mut method_ctx)?;

    Ok(MethodDef {
        name,
        return_type,
        params,
        locals,
        static_locals,
        using: method_ctx.using.clone(),
        body,
    })
}
