/// Element type of an array type, following alias chains.
fn array_element_type(registry: &TypeRegistry, type_id: TypeId) -> Option<TypeId> {
    let mut current = type_id;
    for _ in 0..16 {
        match registry.get(current)? {
            Type::Alias { target, .. } => current = *target,
            Type::Array { element, .. } => return Some(*element),
            _ => return None,
        }
    }
    None
}

/// Declared type of a named member of a struct or union type, following alias chains.
fn aggregate_member_type(registry: &TypeRegistry, type_id: TypeId, member: &str) -> Option<TypeId> {
    let mut current = type_id;
    for _ in 0..16 {
        match registry.get(current)? {
            Type::Alias { target, .. } => current = *target,
            Type::Struct { fields, .. } => {
                return fields
                    .iter()
                    .find(|field| field.name.eq_ignore_ascii_case(member))
                    .map(|field| field.type_id);
            }
            Type::Union { variants, .. } => {
                return variants
                    .iter()
                    .find(|variant| variant.name.eq_ignore_ascii_case(member))
                    .map(|variant| variant.type_id);
            }
            _ => return None,
        }
    }
    None
}

/// Array elements are lowered against the array's element type, so an untyped literal takes it.
fn lower_array_initializer(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    expected_type: Option<TypeId>,
) -> Result<Expr, CompileError> {
    let element_type = expected_type.and_then(|type_id| array_element_type(ctx.registry, type_id));
    let mut elements = Vec::new();
    for child in node.children() {
        if is_expression_kind(child.kind()) {
            elements.push(lower_expr_with_context(&child, ctx, element_type)?);
        }
    }
    Ok(Expr::ArrayInitializer(elements))
}

/// Struct and union fields are lowered against their declared types.
fn lower_struct_initializer(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    expected_type: Option<TypeId>,
) -> Result<Expr, CompileError> {
    let mut fields = Vec::new();
    let mut children = node.children().peekable();
    while let Some(child) = children.next() {
        if child.kind() != SyntaxKind::Name {
            continue;
        }
        let field: SmolStr = node_text(&child).into();
        let Some(value_node) = children.find(|candidate| is_expression_kind(candidate.kind()))
        else {
            return Err(CompileError::new("missing aggregate initializer value"));
        };
        let member_type = expected_type
            .and_then(|type_id| aggregate_member_type(ctx.registry, type_id, field.as_str()));
        let value = lower_expr_with_context(&value_node, ctx, member_type)?;
        fields.push((field, value));
    }
    Ok(Expr::StructInitializer(fields))
}

fn lower_aggregate_call_args(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    type_id: TypeId,
) -> Result<Vec<(SmolStr, Expr)>, CompileError> {
    let Some(arg_list) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)
    else {
        return Ok(Vec::new());
    };
    let mut fields = Vec::new();
    for arg in arg_list
        .children()
        .filter(|child| child.kind() == SyntaxKind::Arg)
    {
        let name: SmolStr = arg
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
            .map(|name| node_text(&name).into())
            .ok_or_else(|| {
                CompileError::new(
                    "positional struct initializers are not supported; use named field initializers",
                )
            })?;
        let expr_node =
            first_expr_child(&arg).ok_or_else(|| CompileError::new("missing initializer value"))?;
        let member_type = aggregate_member_type(ctx.registry, type_id, name.as_str());
        let value = lower_expr_with_context(&expr_node, ctx, member_type).and_then(|lowered| {
            resolve_initializer_enum_variant(&expr_node, lowered, type_id, ctx)
        })?;
        fields.push((name, value));
    }
    Ok(fields)
}
