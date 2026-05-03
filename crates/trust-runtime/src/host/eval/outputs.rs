fn collect_outputs(
    ctx: &mut EvalContext<'_>,
    out_targets: &[OutputBinding],
) -> Result<Vec<(expr::LValue, Value)>, RuntimeError> {
    let mut values = Vec::new();
    for binding in out_targets {
        match binding {
            OutputBinding::Param { param, target } => {
                let value = expr::read_lvalue(ctx, &expr::LValue::Name(param.clone()))?;
                values.push((target.clone(), value));
            }
            OutputBinding::Value { target, value } => {
                values.push((target.clone(), value.clone()));
            }
        }
    }
    Ok(values)
}

fn write_output_values(
    ctx: &mut EvalContext<'_>,
    values: Vec<(expr::LValue, Value)>,
) -> Result<(), RuntimeError> {
    for (target, value) in values {
        expr::write_lvalue(ctx, &target, value)?;
    }
    Ok(())
}

fn eval_arg_expr(ctx: &mut EvalContext<'_>, arg: &CallArg) -> Result<Value, RuntimeError> {
    expr::read_arg_value(ctx, arg)
}

fn find_arg_value<'a>(args: &'a [CallArg], name: &SmolStr) -> Option<&'a CallArg> {
    args.iter().find(|arg| arg.name.as_ref() == Some(name))
}

fn find_arg_target<'a>(args: &'a [CallArg], name: &SmolStr) -> Option<&'a expr::LValue> {
    args.iter().find_map(|arg| match &arg.value {
        ArgValue::Target(target) if arg.name.as_ref() == Some(name) => Some(target),
        _ => None,
    })
}

fn is_en_eno(param: &Param) -> bool {
    matches!(param.direction, ParamDirection::In | ParamDirection::Out)
        && (param.name.eq_ignore_ascii_case("EN") || param.name.eq_ignore_ascii_case("ENO"))
}
