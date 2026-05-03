pub(crate) fn bind_stdlib_named_args(
    ctx: &mut EvalContext<'_>,
    params: &StdParams,
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    if args.iter().any(|arg| arg.name.is_none()) {
        return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
    }
    match params {
        StdParams::Fixed(params) => bind_stdlib_named_args_fixed(ctx, params, args),
        StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } => bind_stdlib_named_args_variadic(ctx, fixed, prefix, *start, *min, args),
    }
}

fn bind_stdlib_named_args_fixed(
    ctx: &mut EvalContext<'_>,
    params: &[SmolStr],
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    if args.len() != params.len() {
        return Err(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        });
    }

    let mut values: Vec<Option<Value>> = vec![None; params.len()];
    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
        };
        let key = name.to_ascii_uppercase();
        let position = params
            .iter()
            .position(|param| param.as_str() == key)
            .ok_or_else(|| RuntimeError::InvalidArgumentName(name.clone()))?;
        if values[position].is_some() {
            return Err(RuntimeError::InvalidArgumentName(name.clone()));
        }
        let value = read_arg_value(ctx, arg)?;
        values[position] = Some(value);
    }

    let mut resolved = Vec::with_capacity(values.len());
    for value in values {
        let Some(value) = value else {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            });
        };
        resolved.push(value);
    }
    Ok(resolved)
}

fn bind_stdlib_named_args_variadic(
    ctx: &mut EvalContext<'_>,
    fixed: &[SmolStr],
    prefix: &SmolStr,
    start: usize,
    min: usize,
    args: &[CallArg],
) -> Result<Vec<Value>, RuntimeError> {
    let mut fixed_values: Vec<Option<Value>> = vec![None; fixed.len()];
    let mut variadic_values: Vec<Option<Value>> = Vec::new();
    let mut max_index: Option<usize> = None;

    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(RuntimeError::InvalidArgumentName("<unnamed>".into()));
        };
        let key = name.to_ascii_uppercase();
        if let Some(position) = fixed.iter().position(|param| param.as_str() == key) {
            if fixed_values[position].is_some() {
                return Err(RuntimeError::InvalidArgumentName(name.clone()));
            }
            let value = read_arg_value(ctx, arg)?;
            fixed_values[position] = Some(value);
            continue;
        }

        let prefix_str = prefix.as_str();
        if let Some(suffix) = key.strip_prefix(prefix_str) {
            if suffix.is_empty() {
                return Err(RuntimeError::InvalidArgumentName(name.clone()));
            }
            let index = suffix
                .parse::<usize>()
                .map_err(|_| RuntimeError::InvalidArgumentName(name.clone()))?;
            if index < start {
                return Err(RuntimeError::InvalidArgumentName(name.clone()));
            }
            let offset = index - start;
            if variadic_values.len() <= offset {
                variadic_values.resize(offset + 1, None);
            }
            if variadic_values[offset].is_some() {
                return Err(RuntimeError::InvalidArgumentName(name.clone()));
            }
            let value = read_arg_value(ctx, arg)?;
            variadic_values[offset] = Some(value);
            max_index = Some(max_index.map_or(offset, |max| max.max(offset)));
            continue;
        }

        return Err(RuntimeError::InvalidArgumentName(name.clone()));
    }

    for value in &fixed_values {
        if value.is_none() {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + min,
                got: args.len(),
            });
        }
    }

    let count = max_index.map(|idx| idx + 1).unwrap_or(0);
    if count < min {
        return Err(RuntimeError::InvalidArgumentCount {
            expected: fixed.len() + min,
            got: args.len(),
        });
    }

    for idx in 0..count {
        if variadic_values
            .get(idx)
            .and_then(|value| value.as_ref())
            .is_none()
        {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + count,
                got: args.len(),
            });
        }
    }

    let mut resolved = Vec::with_capacity(fixed.len() + count);
    for value in fixed_values {
        let Some(value) = value else {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + count,
                got: args.len(),
            });
        };
        resolved.push(value);
    }
    for value in variadic_values.into_iter().take(count) {
        let Some(value) = value else {
            return Err(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + count,
                got: args.len(),
            });
        };
        resolved.push(value);
    }
    Ok(resolved)
}
