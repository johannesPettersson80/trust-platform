fn prepare_bindings(
    ctx: &mut EvalContext<'_>,
    owner: &SmolStr,
    params: &[Param],
    args: &[CallArg],
    mode: BindingMode,
) -> Result<PreparedBindings, RuntimeError> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    let mut positional_iter = if positional { Some(args.iter()) } else { None };
    if positional {
        let expected = params.iter().filter(|param| !is_en_eno(param)).count();
        if args.len() != expected {
            return Err(RuntimeError::InvalidArgumentCount {
                expected,
                got: args.len(),
            });
        }
    }

    let mut param_values = Vec::new();
    let mut out_targets = Vec::new();

    for param in params {
        if param.name.eq_ignore_ascii_case("EN") && matches!(param.direction, ParamDirection::In) {
            let en_value = if positional {
                Value::Bool(true)
            } else {
                find_arg_value(args, &param.name)
                    .map(|arg| eval_arg_expr(ctx, arg))
                    .transpose()?
                    .unwrap_or(Value::Bool(true))
            };
            param_values.push((param.name.clone(), en_value.clone()));
            if let Value::Bool(false) = en_value {
                let eno_param = params.iter().find(|p| {
                    p.name.eq_ignore_ascii_case("ENO") && matches!(p.direction, ParamDirection::Out)
                });
                if let Some(eno_param) = eno_param {
                    if let Some(arg) = find_arg_target(args, &eno_param.name) {
                        out_targets.push(OutputBinding::Value {
                            target: arg.clone(),
                            value: Value::Bool(false),
                        });
                    }
                }
                return Ok(PreparedBindings {
                    should_execute: false,
                    param_values,
                    out_targets,
                });
            }
            continue;
        }

        if positional
            && param.name.eq_ignore_ascii_case("ENO")
            && matches!(param.direction, ParamDirection::Out)
        {
            let value = default_value_for_type_id(param.type_id, ctx.registry, &ctx.profile)
                .map_err(|err| init_failed_debug(owner, &param.name, err))?;
            param_values.push((param.name.clone(), value));
            continue;
        }

        let arg = if positional {
            positional_iter.as_mut().and_then(|iter| iter.next())
        } else {
            find_arg_value(args, &param.name)
        };
        match param.direction {
            ParamDirection::In => {
                let value = if let Some(arg) = arg {
                    eval_arg_expr(ctx, arg)?
                } else if let BindingMode::FunctionBlock { instance_id } = mode {
                    if let Some(value) = ctx.storage.get_instance_var(instance_id, &param.name) {
                        value.clone()
                    } else if let Some(default) = &param.default {
                        expr::eval_expr(ctx, default)?
                    } else {
                        default_value_for_type_id(param.type_id, ctx.registry, &ctx.profile)
                            .map_err(|err| init_failed_debug(owner, &param.name, err))?
                    }
                } else if let Some(default) = &param.default {
                    expr::eval_expr(ctx, default)?
                } else {
                    default_value_for_type_id(param.type_id, ctx.registry, &ctx.profile)
                        .map_err(|err| init_failed_debug(owner, &param.name, err))?
                };
                let value = coerce_input_value_to_param_type(value, param.type_id)?;
                param_values.push((param.name.clone(), value));
            }
            ParamDirection::Out => {
                if matches!(mode, BindingMode::Function) {
                    let value =
                        default_value_for_type_id(param.type_id, ctx.registry, &ctx.profile)
                            .map_err(|err| init_failed_debug(owner, &param.name, err))?;
                    param_values.push((param.name.clone(), value));
                }
                if let Some(arg) = arg {
                    let ArgValue::Target(target) = &arg.value else {
                        return Err(RuntimeError::TypeMismatch);
                    };
                    out_targets.push(OutputBinding::Param {
                        param: param.name.clone(),
                        target: target.clone(),
                    });
                }
            }
            ParamDirection::InOut => {
                if let Some(arg) = arg {
                    let ArgValue::Target(target) = &arg.value else {
                        return Err(RuntimeError::TypeMismatch);
                    };
                    let value = expr::read_lvalue(ctx, target)?;
                    param_values.push((param.name.clone(), value.clone()));
                    out_targets.push(OutputBinding::Param {
                        param: param.name.clone(),
                        target: target.clone(),
                    });
                }
            }
        }
    }
    Ok(PreparedBindings {
        should_execute: true,
        param_values,
        out_targets,
    })
}

fn coerce_input_value_to_param_type(value: Value, type_id: TypeId) -> Result<Value, RuntimeError> {
    if matches!(
        type_id,
        TypeId::UNKNOWN
            | TypeId::VOID
            | TypeId::ANY
            | TypeId::ANY_DERIVED
            | TypeId::ANY_ELEMENTARY
            | TypeId::ANY_MAGNITUDE
            | TypeId::ANY_INT
            | TypeId::ANY_UNSIGNED
            | TypeId::ANY_SIGNED
            | TypeId::ANY_REAL
            | TypeId::ANY_NUM
            | TypeId::ANY_DURATION
            | TypeId::ANY_BIT
            | TypeId::ANY_CHARS
            | TypeId::ANY_STRING
            | TypeId::ANY_CHAR
            | TypeId::ANY_DATE
            | TypeId::NULL
    ) {
        return Ok(value);
    }

    let Some(type_name) = type_id.builtin_name() else {
        return Ok(value);
    };
    let conversion = format!("TO_{type_name}");
    let Some(result) = crate::stdlib::conversions::call_conversion(
        conversion.as_str(),
        std::slice::from_ref(&value),
    ) else {
        return Ok(value);
    };
    result
}
