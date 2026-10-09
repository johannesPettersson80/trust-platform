/// Lowers the initializer of a variable, constant, member or type of `type_id`. A literal
/// (also a signed one) takes the declared type, as an assigned literal takes its target's,
/// so `x : BOOL := 1` and `d : DWORD := 16#FFFF_FFFF` initialize like `x := 1` and
/// `d := 16#FFFF_FFFF` assign (IEC 61131-3 Ed.3 6.3.2), also inside array and structure
/// initializers. Other expressions are lowered without a context, as before.
pub(in crate::harness) fn lower_initializer_expr(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    type_id: TypeId,
) -> Result<Expr, CompileError> {
    match node.kind() {
        SyntaxKind::Literal => lower_expr_with_context(node, ctx, Some(type_id)),
        SyntaxKind::UnaryExpr
            if first_expr_child(node).is_some_and(|operand| operand.kind() == SyntaxKind::Literal) =>
        {
            lower_expr_with_context(node, ctx, Some(type_id))
        }
        SyntaxKind::ArrayInitializer => {
            let Some(element) = initializer_array_element(type_id, ctx.registry) else {
                return lower_expr(node, ctx);
            };
            let mut elements = Vec::new();
            for child in node.children() {
                if is_expression_kind(child.kind()) {
                    elements.push(lower_initializer_expr(&child, ctx, element)?);
                }
            }
            Ok(Expr::ArrayInitializer(elements))
        }
        SyntaxKind::InitializerList => {
            let Some(members) = initializer_struct_fields(type_id, ctx.registry) else {
                return lower_expr(node, ctx);
            };
            let mut fields = Vec::new();
            let mut children = node.children().peekable();
            while let Some(child) = children.next() {
                if child.kind() != SyntaxKind::Name {
                    continue;
                }
                let field: SmolStr = node_text(&child).into();
                let Some(value_node) =
                    children.find(|candidate| is_expression_kind(candidate.kind()))
                else {
                    return Err(CompileError::new("missing aggregate initializer value"));
                };
                let member_type = members
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(field.as_str()))
                    .map(|(_, member_type)| *member_type);
                let value = match member_type {
                    Some(member_type) => lower_initializer_expr(&value_node, ctx, member_type)?,
                    None => lower_expr(&value_node, ctx)?,
                };
                fields.push((field, value));
            }
            Ok(Expr::StructInitializer(fields))
        }
        _ => lower_expr(node, ctx),
    }
}

/// The element type of an array type (through aliases).
fn initializer_array_element(type_id: TypeId, registry: &TypeRegistry) -> Option<TypeId> {
    let mut current = type_id;
    for _ in 0..32 {
        match registry.get(current)? {
            Type::Alias { target, .. } => current = *target,
            Type::Array { element, .. } => return Some(*element),
            _ => return None,
        }
    }
    None
}

/// The member names and types of a structure type (through aliases).
fn initializer_struct_fields(
    type_id: TypeId,
    registry: &TypeRegistry,
) -> Option<Vec<(SmolStr, TypeId)>> {
    let mut current = type_id;
    for _ in 0..32 {
        match registry.get(current)? {
            Type::Alias { target, .. } => current = *target,
            Type::Struct { fields, .. } => {
                return Some(
                    fields
                        .iter()
                        .map(|field| (field.name.clone(), field.type_id))
                        .collect(),
                )
            }
            _ => return None,
        }
    }
    None
}
