pub(in crate::harness) fn lower_lvalue(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<LValue, CompileError> {
    match node.kind() {
        SyntaxKind::NameRef => Ok(LValue::Name(node_text(node).into())),
        SyntaxKind::IndexExpr => {
            let exprs = direct_expr_children(node);
            if exprs.len() < 2 {
                return Err(CompileError::new("invalid index expression"));
            }
            let target = lower_lvalue(&exprs[0], ctx)?;
            let mut indices = Vec::new();
            for expr in exprs.iter().skip(1) {
                indices.push(lower_expr(expr, ctx)?);
            }
            if indices.is_empty() {
                return Err(CompileError::new("missing index expression"));
            }
            Ok(LValue::Index {
                target: Box::new(target),
                indices,
            })
        }
        SyntaxKind::FieldExpr => {
            let exprs = direct_expr_children(node);
            if exprs.is_empty() {
                return Err(CompileError::new("invalid field expression"));
            }
            let target = lower_lvalue(&exprs[0], ctx)?;
            let field = node
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::Literal))
                .ok_or_else(|| CompileError::new("missing field name"))?;
            Ok(LValue::Field {
                target: Box::new(target),
                field: node_text(&field).into(),
            })
        }
        SyntaxKind::DerefExpr => {
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing deref target"))?;
            Ok(LValue::Deref(Box::new(lower_expr(&expr, ctx)?)))
        }
        _ => Err(CompileError::new("unsupported assignment target")),
    }
}

pub(in crate::harness) fn lower_expr(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Expr, CompileError> {
    match node.kind() {
        SyntaxKind::Literal => lower_literal(node, ctx),
        SyntaxKind::NameRef => Ok(Expr::Name(node_text(node).into())),
        SyntaxKind::ThisExpr => Ok(Expr::This),
        SyntaxKind::SuperExpr => Ok(Expr::Super),
        SyntaxKind::UnaryExpr => {
            let op = unary_op_from_node(node)?;
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing unary operand"))?;
            Ok(Expr::Unary {
                op,
                expr: Box::new(lower_expr(&expr, ctx)?),
            })
        }
        SyntaxKind::BinaryExpr => {
            let op = binary_op_from_node(node)?;
            let exprs = direct_expr_children(node);
            if exprs.len() != 2 {
                return Err(CompileError::new("invalid binary expression"));
            }
            Ok(Expr::Binary {
                op,
                left: Box::new(lower_expr(&exprs[0], ctx)?),
                right: Box::new(lower_expr(&exprs[1], ctx)?),
            })
        }
        SyntaxKind::ParenExpr => {
            let expr = first_expr_child(node)
                .ok_or_else(|| CompileError::new("missing parenthesized expression"))?;
            lower_expr(&expr, ctx)
        }
        SyntaxKind::IndexExpr => {
            let exprs = direct_expr_children(node);
            if exprs.len() < 2 {
                return Err(CompileError::new("invalid index expression"));
            }
            let mut indices = Vec::new();
            for expr in exprs.iter().skip(1) {
                indices.push(lower_expr(expr, ctx)?);
            }
            Ok(Expr::Index {
                target: Box::new(lower_expr(&exprs[0], ctx)?),
                indices,
            })
        }
        SyntaxKind::FieldExpr => {
            let exprs = direct_expr_children(node);
            if exprs.is_empty() {
                return Err(CompileError::new("invalid field expression"));
            }
            let field = node
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::Literal))
                .ok_or_else(|| CompileError::new("missing field name"))?;
            Ok(Expr::Field {
                target: Box::new(lower_expr(&exprs[0], ctx)?),
                field: node_text(&field).into(),
            })
        }
        SyntaxKind::DerefExpr => {
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing deref target"))?;
            Ok(Expr::Deref(Box::new(lower_expr(&expr, ctx)?)))
        }
        SyntaxKind::AddrExpr => {
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing ADR operand"))?;
            let lvalue = lower_lvalue(&expr, ctx)?;
            Ok(Expr::Ref(lvalue))
        }
        SyntaxKind::CallExpr => lower_call_expr(node, ctx),
        SyntaxKind::SizeOfExpr => lower_sizeof_expr(node, ctx),
        SyntaxKind::ArrayInitializer | SyntaxKind::InitializerList => {
            Err(CompileError::new("initializer lists are not supported yet"))
        }
        _ => Err(CompileError::new("unsupported expression")),
    }
}

fn lower_sizeof_expr(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Expr, CompileError> {
    if let Some(type_ref) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
    {
        let type_id = lower_type_ref(&type_ref, ctx)?;
        return Ok(Expr::SizeOf(crate::program_model::SizeOfTarget::Type(type_id)));
    }
    if let Some(expr_node) = node
        .children()
        .find(|child| is_expression_kind(child.kind()))
    {
        let expr = lower_expr(&expr_node, ctx)?;
        return Ok(Expr::SizeOf(crate::program_model::SizeOfTarget::Expr(
            Box::new(expr),
        )));
    }
    Err(CompileError::new("SIZEOF expects a type or expression"))
}

fn lower_call_expr(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Expr, CompileError> {
    let target = first_expr_child(node).ok_or_else(|| CompileError::new("missing call target"))?;
    let target = lower_expr(&target, ctx)?;
    let args = lower_call_args(node, ctx)?;
    Ok(Expr::Call {
        target: Box::new(target),
        args,
    })
}

fn lower_call_args(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<CallArg>, CompileError> {
    let arg_list = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList);
    let Some(arg_list) = arg_list else {
        return Ok(Vec::new());
    };
    let mut args = Vec::new();
    for arg in arg_list
        .children()
        .filter(|child| child.kind() == SyntaxKind::Arg)
    {
        args.push(lower_call_arg(&arg, ctx)?);
    }
    Ok(args)
}

fn lower_call_arg(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<CallArg, CompileError> {
    let name = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map(|name| node_text(&name).into());

    let mut has_arrow = false;
    for token in node
        .children_with_tokens()
        .filter_map(|child| child.into_token())
    {
        if token.kind() == SyntaxKind::Arrow {
            has_arrow = true;
        }
    }

    let expr_node =
        first_expr_child(node).ok_or_else(|| CompileError::new("missing call argument"))?;
    let value = if has_arrow {
        ArgValue::Target(lower_lvalue(&expr_node, ctx)?)
    } else {
        match lower_lvalue(&expr_node, ctx) {
            Ok(target) => ArgValue::Target(target),
            Err(_) => ArgValue::Expr(lower_expr(&expr_node, ctx)?),
        }
    };

    Ok(CallArg { name, value })
}
