pub(crate) fn eval_positional_args(
    ctx: &mut EvalContext<'_>,
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        let value = read_arg_value(ctx, arg)?;
        values.push(value);
    }
    Ok(values)
}

pub(crate) fn read_arg_value(
    ctx: &mut EvalContext<'_>,
    arg: &CallArg,
) -> Result<Value, RuntimeError> {
    match &arg.value {
        ArgValue::Expr(expr) => super::eval::eval_expr(ctx, expr),
        ArgValue::Target(target) => read_lvalue(ctx, target),
    }
}
