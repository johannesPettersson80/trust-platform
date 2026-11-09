fn lower_program_node(
    program_node: &SyntaxNode,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<LoweredProgram, CompileError> {
    let name = qualified_pou_name(program_node)?;
    let using = collect_using_directives(program_node);
    let mut ctx = inputs.context(registry, using);
    let vars = lower_program_var_blocks(program_node, &mut ctx)?;
    let mut globals = vars.globals;
    let mut access = Vec::new();
    for block in program_node
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarAccessBlock)
    {
        let lowered = super::config::lower_var_access_block(&block, &mut ctx)?;
        globals.extend(lowered.globals);
        for mut declaration in lowered.access {
            if let AccessPath::Parts(parts) = &mut declaration.path {
                parts.insert(0, AccessPart::Name(name.clone()));
            }
            access.push(declaration);
        }
    }
    let body = lower_stmt_list(program_node, &mut ctx)?;
    // A program's METHODs and textual ACTIONs stand in its statement list, which runs only its
    // statements (specification 04 §4, §8).
    let mut methods = Vec::new();
    for member in program_node
        .children()
        .filter(|child| child.kind() == SyntaxKind::StmtList)
        .flat_map(|list| list.children())
    {
        match member.kind() {
            SyntaxKind::Method => methods.push(lower_method_node(&member, &mut ctx)?),
            SyntaxKind::Action => methods.push(lower_action_node(&member, &mut ctx)?),
            _ => {}
        }
    }
    Ok(LoweredProgram {
        program: ProgramDef {
            name,
            vars: vars.vars,
            temps: vars.temps,
            using: ctx.using.clone(),
            body,
            methods,
        },
        globals,
        access,
    })
}

fn lower_function_block_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<FunctionBlockDef, CompileError> {
    let name = qualified_pou_name(node)?;
    let interfaces = lower_implemented_interfaces(node, ctx)?;
    let mut base = None;
    if let Some(extends_clause) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ExtendsClause)
        && let Some(base_name) = extends_clause
            .children()
            .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
    {
        // A base in a namespace is written qualified, for example `Lib.Base`.
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
    let (params, vars, temps) = lower_function_block_var_blocks(node, ctx)?;
    let mut methods = Vec::new();
    for method_node in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::Method)
    {
        methods.push(lower_method_node(&method_node, ctx)?);
    }
    let mut property_aliases = Vec::new();
    // A property can alias any variable of the block but a VAR_IN_OUT, which holds a reference.
    let variables: Vec<&str> = params
        .iter()
        .filter(|param| param.direction != ParamDirection::InOut)
        .map(|param| param.name.as_str())
        .chain(vars.iter().map(|var| var.name.as_str()))
        .collect();
    for property_node in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::Property)
    {
        methods.extend(lower_property_methods(&property_node, ctx)?);
        property_aliases.extend(property_alias(&property_node, &variables));
    }
    for action_node in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::Action)
    {
        methods.push(lower_action_node(&action_node, ctx)?);
    }
    let body = lower_stmt_list(node, ctx)?;
    Ok(FunctionBlockDef {
        name,
        base,
        interfaces,
        params,
        vars,
        temps,
        using: ctx.using.clone(),
        methods,
        body,
        property_aliases,
        call_after_init: trust_hir::attributes::declares_attribute(node, "call_after_init"),
    })
}

fn lower_class_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<ClassDef, CompileError> {
    let name = qualified_pou_name(node)?;
    let interfaces = lower_implemented_interfaces(node, ctx)?;
    let mut base = None;
    if let Some(extends_clause) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ExtendsClause)
        && let Some(base_name) = extends_clause
            .children()
            .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
    {
        // A base in a namespace is written qualified, for example `Lib.Base`.
        let raw = node_text(&base_name);
        base = Some(resolve_named_type(ctx.registry, &raw, &ctx.using)?);
    }

    let vars = lower_class_var_blocks(node, ctx)?;
    let mut methods = Vec::new();
    for method_node in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::Method)
    {
        methods.push(lower_method_node(&method_node, ctx)?);
    }
    let variables: Vec<&str> = vars.iter().map(|var| var.name.as_str()).collect();
    let mut property_aliases = Vec::new();
    for property_node in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::Property)
    {
        methods.extend(lower_property_methods(&property_node, ctx)?);
        property_aliases.extend(property_alias(&property_node, &variables));
    }

    Ok(ClassDef {
        name,
        base,
        interfaces,
        vars,
        using: ctx.using.clone(),
        methods,
        property_aliases,
    })
}

fn lower_implemented_interfaces(
    node: &SyntaxNode,
    ctx: &LoweringContext<'_>,
) -> Result<Vec<SmolStr>, CompileError> {
    let Some(clause) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ImplementsClause)
    else {
        return Ok(Vec::new());
    };
    clause
        .children()
        .filter(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
        .map(|name| resolve_named_type(ctx.registry, &node_text(&name), &ctx.using))
        .collect()
}

fn lower_interface_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<InterfaceDef, CompileError> {
    if node.children().any(|child| {
        matches!(
            child.kind(),
            SyntaxKind::VarBlock | SyntaxKind::VarAccessBlock | SyntaxKind::VarConfigBlock
        )
    }) {
        return Err(CompileError::new(
            "direct variable sections are not supported in INTERFACE",
        ));
    }
    let name = qualified_pou_name(node)?;
    let mut base = None;
    if let Some(extends_clause) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ExtendsClause)
        && let Some(base_name) = extends_clause
            .children()
            .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
    {
        // A base in a namespace is written qualified, for example `Lib.Base`.
        let raw = node_text(&base_name);
        base = Some(resolve_named_type(ctx.registry, &raw, &ctx.using)?);
    }

    let mut methods = Vec::new();
    for method_node in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::Method)
    {
        methods.push(lower_method_node(&method_node, ctx)?);
    }
    for property_node in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::Property)
    {
        methods.extend(lower_property_methods(&property_node, ctx)?);
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
    let name: SmolStr = node_text(&name_node).into();

    let using = collect_using_directives(node);
    let mut method_ctx = LoweringContext {
        registry: ctx.registry,
        profile: ctx.profile,
        using,
        file_id: ctx.file_id,
        semantic_db: ctx.semantic_db,
        semantic_file_id: ctx.semantic_file_id,
        statement_locations: ctx.statement_locations,
        compile_time_consts: ctx.compile_time_consts.clone(),
        lower_bodies: ctx.lower_bodies,
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
        call_after_init: trust_hir::attributes::declares_attribute(node, "call_after_init"),
    })
}

/// A textual ACTION: a method of its owner without parameters, result or variables of its own,
/// whose body runs on the owner's variables when it is called (specification 04 §8).
fn lower_action_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<MethodDef, CompileError> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .ok_or_else(|| CompileError::new("missing action name"))?;
    let name: SmolStr = node_text(&name_node).into();
    let body = lower_stmt_list(node, ctx)?;
    Ok(MethodDef {
        name,
        return_type: None,
        params: Vec::new(),
        locals: Vec::new(),
        static_locals: Vec::new(),
        using: ctx.using.clone(),
        body,
        call_after_init: false,
    })
}

/// The alias of a property whose GET only returns one of `variables` (`Prop := Var;`), writable when its
/// SET only assigns that variable (`Var := Prop;`) (specification 44 §5.8); `None` for a property whose
/// GET does anything else, which a name read does not find.
fn property_alias(property: &SyntaxNode, variables: &[&str]) -> Option<PropertyAlias> {
    let name: SmolStr = node_text(
        &property
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)?,
    )
    .into();
    // The one assignment of an accessor, as its target's and its value's names.
    let only_assignment = |accessor: SyntaxKind| -> Option<(String, String)> {
        let node = property.children().find(|child| child.kind() == accessor)?;
        let mut statements = node
            .descendants()
            .filter(|statement| statement.kind().is_statement_node());
        let assignment = statements.next()?;
        if statements.next().is_some() || assignment.kind() != SyntaxKind::AssignStmt {
            return None;
        }
        let names: Vec<String> = assignment
            .children()
            .filter(|side| side.kind() == SyntaxKind::NameRef)
            .map(|side| node_text(&side))
            .collect();
        match names.as_slice() {
            [target, value] if assignment.children().count() == 2 => {
                Some((target.clone(), value.clone()))
            }
            _ => None,
        }
    };
    let (target, value) = only_assignment(SyntaxKind::PropertyGet)?;
    if !target.eq_ignore_ascii_case(&name) {
        return None;
    }
    let member = variables
        .iter()
        .find(|variable| variable.eq_ignore_ascii_case(&value))?;
    let writable = only_assignment(SyntaxKind::PropertySet).is_some_and(|(target, value)| {
        target.eq_ignore_ascii_case(member) && value.eq_ignore_ascii_case(&name)
    });
    Some(PropertyAlias {
        name,
        member: SmolStr::new(member),
        writable,
    })
}

fn lower_property_methods(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<MethodDef>, CompileError> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .ok_or_else(|| CompileError::new("missing property name"))?;
    let name: SmolStr = node_text(&name_node).into();
    let type_ref = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .ok_or_else(|| CompileError::new("missing property type"))?;
    let prop_type = lower_type_ref(&type_ref, ctx)?;
    let using = collect_using_directives(node);
    let mut property_ctx = LoweringContext {
        registry: ctx.registry,
        profile: ctx.profile,
        using,
        file_id: ctx.file_id,
        semantic_db: ctx.semantic_db,
        semantic_file_id: ctx.semantic_file_id,
        statement_locations: ctx.statement_locations,
        compile_time_consts: ctx.compile_time_consts.clone(),
        lower_bodies: ctx.lower_bodies,
    };

    let mut methods = Vec::new();
    if let Some(get_node) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::PropertyGet)
    {
        methods.push(MethodDef {
            name: name.clone(),
            return_type: Some(prop_type),
            params: Vec::new(),
            locals: Vec::new(),
            static_locals: Vec::new(),
            using: property_ctx.using.clone(),
            body: lower_stmt_list(&get_node, &mut property_ctx)?,
            call_after_init: false,
        });
    }

    if let Some(set_node) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::PropertySet)
    {
        methods.push(MethodDef {
            name: crate::program_model::property_setter_method_name(&name),
            return_type: None,
            params: vec![Param {
                name: name.clone(),
                type_id: prop_type,
                direction: ParamDirection::In,
                address: None,
                default: None,
                retain: crate::RetainPolicy::Unspecified,
                noinit: false,
                hidden: false,
            }],
            locals: Vec::new(),
            static_locals: Vec::new(),
            using: property_ctx.using.clone(),
            body: lower_stmt_list(&set_node, &mut property_ctx)?,
            call_after_init: false,
        });
    }

    Ok(methods)
}
