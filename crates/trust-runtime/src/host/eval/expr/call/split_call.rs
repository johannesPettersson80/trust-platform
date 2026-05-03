pub(crate) fn eval_split_call(
    ctx: &mut EvalContext<'_>,
    name: &str,
    args: &[CallArg],
) -> Result<Value, RuntimeError> {
    let params: &[&str] = match name {
        "SPLIT_DATE" => &["IN", "YEAR", "MONTH", "DAY"],
        "SPLIT_TOD" | "SPLIT_LTOD" => &["IN", "HOUR", "MINUTE", "SECOND", "MILLISECOND"],
        "SPLIT_DT" | "SPLIT_LDT" => &[
            "IN",
            "YEAR",
            "MONTH",
            "DAY",
            "HOUR",
            "MINUTE",
            "SECOND",
            "MILLISECOND",
        ],
        _ => return Err(RuntimeError::UndefinedFunction(name.into())),
    };

    let (input, outputs) = bind_split_args(ctx, params, args)?;

    match name {
        "SPLIT_DATE" => {
            let (year, month, day) = time::split_date(&input, ctx.profile)?;
            write_output_int(ctx, &outputs[0], year)?;
            write_output_int(ctx, &outputs[1], month)?;
            write_output_int(ctx, &outputs[2], day)?;
        }
        "SPLIT_TOD" => {
            let (hour, minute, second, millis) = time::split_tod(&input, ctx.profile)?;
            write_output_int(ctx, &outputs[0], hour)?;
            write_output_int(ctx, &outputs[1], minute)?;
            write_output_int(ctx, &outputs[2], second)?;
            write_output_int(ctx, &outputs[3], millis)?;
        }
        "SPLIT_LTOD" => {
            let (hour, minute, second, millis) = time::split_ltod(&input)?;
            write_output_int(ctx, &outputs[0], hour)?;
            write_output_int(ctx, &outputs[1], minute)?;
            write_output_int(ctx, &outputs[2], second)?;
            write_output_int(ctx, &outputs[3], millis)?;
        }
        "SPLIT_DT" => {
            let (year, month, day, hour, minute, second, millis) =
                time::split_dt(&input, ctx.profile)?;
            write_output_int(ctx, &outputs[0], year)?;
            write_output_int(ctx, &outputs[1], month)?;
            write_output_int(ctx, &outputs[2], day)?;
            write_output_int(ctx, &outputs[3], hour)?;
            write_output_int(ctx, &outputs[4], minute)?;
            write_output_int(ctx, &outputs[5], second)?;
            write_output_int(ctx, &outputs[6], millis)?;
        }
        "SPLIT_LDT" => {
            let (year, month, day, hour, minute, second, millis) = time::split_ldt(&input)?;
            write_output_int(ctx, &outputs[0], year)?;
            write_output_int(ctx, &outputs[1], month)?;
            write_output_int(ctx, &outputs[2], day)?;
            write_output_int(ctx, &outputs[3], hour)?;
            write_output_int(ctx, &outputs[4], minute)?;
            write_output_int(ctx, &outputs[5], second)?;
            write_output_int(ctx, &outputs[6], millis)?;
        }
        _ => {}
    }

    Ok(Value::Null)
}

fn bind_split_args(
    ctx: &mut EvalContext<'_>,
    params: &[&str],
    args: &[CallArg],
) -> Result<(Value, Vec<LValue>), RuntimeError> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    if positional {
        if args.len() != params.len() {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            });
        }
        let mut input = None;
        let mut outputs = Vec::with_capacity(params.len().saturating_sub(1));
        for (idx, arg) in args.iter().enumerate() {
            if idx == 0 {
                input = Some(read_arg_value(ctx, arg)?);
            } else {
                outputs.push(require_output_target(arg)?);
            }
        }
        let input = input.ok_or(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        })?;
        return Ok((input, outputs));
    }

    if args.iter().any(|arg| arg.name.is_none()) {
        return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
    }
    if args.len() != params.len() {
        return Err(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        });
    }

    let mut assigned: Vec<Option<&CallArg>> = vec![None; params.len()];
    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
        };
        let key = name.to_ascii_uppercase();
        let position = params
            .iter()
            .position(|param| param.eq_ignore_ascii_case(&key))
            .ok_or_else(|| RuntimeError::InvalidArgumentName(name.clone()))?;
        if assigned[position].is_some() {
            return Err(RuntimeError::InvalidArgumentName(name.clone()));
        }
        assigned[position] = Some(arg);
    }

    let mut input = None;
    let mut outputs = Vec::with_capacity(params.len().saturating_sub(1));
    for (idx, arg) in assigned.iter().enumerate() {
        let Some(arg) = arg else {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            });
        };
        if idx == 0 {
            input = Some(read_arg_value(ctx, arg)?);
        } else {
            outputs.push(require_output_target(arg)?);
        }
    }
    let input = input.ok_or(RuntimeError::InvalidArgumentCount {
        expected: params.len(),
        got: args.len(),
    })?;
    Ok((input, outputs))
}

fn require_output_target(arg: &CallArg) -> Result<LValue, RuntimeError> {
    match &arg.value {
        ArgValue::Target(target) => Ok(target.clone()),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn write_output_int(
    ctx: &mut EvalContext<'_>,
    target: &LValue,
    value: i64,
) -> Result<(), RuntimeError> {
    let current = read_lvalue(ctx, target)?;
    let converted = match current {
        Value::SInt(_) => Value::SInt(i8::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::Int(_) => Value::Int(i16::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::DInt(_) => Value::DInt(i32::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::LInt(_) => Value::LInt(value),
        Value::USInt(_) => {
            if value < 0 {
                return Err(RuntimeError::Overflow);
            }
            Value::USInt(u8::try_from(value).map_err(|_| RuntimeError::Overflow)?)
        }
        Value::UInt(_) => {
            if value < 0 {
                return Err(RuntimeError::Overflow);
            }
            Value::UInt(u16::try_from(value).map_err(|_| RuntimeError::Overflow)?)
        }
        Value::UDInt(_) => {
            if value < 0 {
                return Err(RuntimeError::Overflow);
            }
            Value::UDInt(u32::try_from(value).map_err(|_| RuntimeError::Overflow)?)
        }
        Value::ULInt(_) => {
            if value < 0 {
                return Err(RuntimeError::Overflow);
            }
            Value::ULInt(value as u64)
        }
        _ => return Err(RuntimeError::TypeMismatch),
    };
    write_lvalue(ctx, target, converted)
}
