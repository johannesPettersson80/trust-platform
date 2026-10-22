pub(in crate::harness) fn parse_subrange(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<(i64, i64), CompileError> {
    let exprs = direct_expr_children(node);
    if exprs.is_empty() {
        return Err(CompileError::new("missing subrange bounds"));
    }
    if exprs.len() == 1 {
        if is_wildcard_expr(&exprs[0]) {
            return Ok((0, i64::MAX));
        }
        let value = const_size_int_from_node(&exprs[0], ctx)?;
        return Ok((value, value));
    }
    if exprs.len() == 2 {
        if is_wildcard_expr(&exprs[0]) || is_wildcard_expr(&exprs[1]) {
            return Ok((0, i64::MAX));
        }
        let lower = const_size_int_from_node(&exprs[0], ctx)?;
        let upper = const_size_int_from_node(&exprs[1], ctx)?;
        return Ok((lower, upper));
    }
    Err(CompileError::new("invalid subrange bounds"))
}

/// Subrange bounds whose untyped literals take `base` as their destination type, so a subrange
/// of LINT accepts bounds beyond the DINT range (IEC 61131-3 section 6.4.4.3: the bounds are values
/// of the base type).
pub(in crate::harness) fn parse_subrange_in(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    base: Option<TypeId>,
) -> Result<(i64, i64), CompileError> {
    let exprs = direct_expr_children(node);
    if exprs.is_empty() {
        return Err(CompileError::new("missing subrange bounds"));
    }
    if exprs.len() == 1 {
        if is_wildcard_expr(&exprs[0]) {
            return Ok((0, i64::MAX));
        }
        let value = const_int_from_node_in(&exprs[0], ctx, base)?;
        return Ok((value, value));
    }
    if exprs.len() == 2 {
        if is_wildcard_expr(&exprs[0]) || is_wildcard_expr(&exprs[1]) {
            return Ok((0, i64::MAX));
        }
        let lower = const_int_from_node_in(&exprs[0], ctx, base)?;
        let upper = const_int_from_node_in(&exprs[1], ctx, base)?;
        return Ok((lower, upper));
    }
    Err(CompileError::new("invalid subrange bounds"))
}

fn is_wildcard_expr(node: &SyntaxNode) -> bool {
    node.text().to_string().trim() == "*"
}

pub(in crate::harness) fn const_value_from_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Value, CompileError> {
    const_value_from_node_in(node, ctx, None)
}

fn const_value_from_node_in(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    expected: Option<TypeId>,
) -> Result<Value, CompileError> {
    let expr = lower_expr_with_context(node, ctx, expected)?;
    ctx.eval_compile_time_const_expr(&expr)
}

pub(in crate::harness) fn const_int_from_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<i64, CompileError> {
    const_int_from_node_in(node, ctx, None)
}

/// Declaration-size evaluation (string capacities, array dimension bounds,
/// IEC_DEVIATIONS 2026-10-21): falls back to the declared initial values of
/// plain variables when the expression is not a true constant. Only
/// declaration-size lowering calls this; every other compile-time-constant
/// context keeps the strict rule.
pub(in crate::harness) fn const_size_int_from_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<i64, CompileError> {
    match const_int_from_node(node, ctx) {
        Ok(value) => Ok(value),
        Err(strict_error) => {
            if ctx.compile_time_consts.declared_sizes_is_empty() {
                return Err(strict_error);
            }
            let size_view = ctx.compile_time_consts.size_evaluation_view();
            let saved = std::mem::replace(&mut ctx.compile_time_consts, size_view);
            let result = const_int_from_node(node, ctx);
            ctx.compile_time_consts = saved;
            match result {
                Ok(value) => Ok(value),
                Err(_) => Err(strict_error),
            }
        }
    }
}

fn const_int_from_node_in(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    expected: Option<TypeId>,
) -> Result<i64, CompileError> {
    let value = const_value_from_node_in(node, ctx, expected)?;
    match value {
        Value::SInt(v) => Ok(v as i64),
        Value::Int(v) => Ok(v as i64),
        Value::DInt(v) => Ok(v as i64),
        Value::LInt(v) => Ok(v),
        Value::USInt(v) => Ok(v as i64),
        Value::UInt(v) => Ok(v as i64),
        Value::UDInt(v) => Ok(v as i64),
        Value::ULInt(v) => {
            Ok(i64::try_from(v).map_err(|_| CompileError::new("integer constant out of range"))?)
        }
        Value::Byte(v) => Ok(v as i64),
        Value::Word(v) => Ok(v as i64),
        Value::DWord(v) => Ok(v as i64),
        Value::LWord(v) => {
            Ok(i64::try_from(v).map_err(|_| CompileError::new("integer constant out of range"))?)
        }
        Value::Enum(enum_value) => Ok(enum_value.numeric_value()),
        _ => Err(CompileError::new("expected integer constant")),
    }
}

pub(in crate::harness) fn const_duration_from_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Duration, CompileError> {
    let value = const_value_from_node(node, ctx)?;
    match value {
        Value::Time(duration) | Value::LTime(duration) => Ok(duration),
        _ => Err(CompileError::new("expected TIME/INTERVAL constant")),
    }
}
