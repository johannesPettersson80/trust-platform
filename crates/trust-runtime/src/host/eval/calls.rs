pub(crate) fn eval_expr(ctx: &mut EvalContext<'_>, expr: &expr::Expr) -> Result<Value, RuntimeError> {
    expr::eval_expr(ctx, expr)
}

/// Execute a list of statements.
pub(crate) fn exec_block(
    ctx: &mut EvalContext<'_>,
    stmts: &[stmt::Stmt],
) -> Result<stmt::StmtResult, RuntimeError> {
    stmt::exec_block(ctx, stmts)
}

/// Call a function definition.
pub(crate) fn call_function<'a>(
    ctx: &mut EvalContext<'a>,
    func: &'a FunctionDef,
    args: &[CallArg],
) -> Result<Value, RuntimeError> {
    let saved_using = ctx.using;
    let saved_return = ctx.return_name.clone();
    let PreparedBindings {
        should_execute,
        param_values,
        out_targets,
    } = match prepare_bindings(ctx, &func.name, &func.params, args, BindingMode::Function) {
        Ok(value) => value,
        Err(err) => {
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            return Err(err);
        }
    };

    let return_default = default_value_for_type_id(func.return_type, ctx.registry, &ctx.profile)
        .map_err(|err| init_failed_debug(&func.name, &func.name, err))?;
    ctx.using = Some(&func.using);
    ctx.storage.push_frame(func.name.clone());
    ctx.return_name = Some(func.name.clone());
    ctx.storage.set_local(func.name.clone(), return_default);
    for (name, value) in param_values {
        ctx.storage.set_local(name, value);
    }

    if !should_execute {
        let output_values = collect_outputs(ctx, &out_targets)?;
        ctx.storage.pop_frame();
        ctx.return_name = saved_return;
        ctx.using = saved_using;
        write_output_values(ctx, output_values)?;
        return default_value_for_type_id(func.return_type, ctx.registry, &ctx.profile)
            .map_err(|err| init_failed_debug(&func.name, &func.name, err));
    }

    let saved_call_depth = ctx.call_depth;
    ctx.call_depth = saved_call_depth.saturating_add(1);

    if let Err(err) = init_locals(ctx, &func.static_locals) {
        ctx.call_depth = saved_call_depth;
        ctx.storage.pop_frame();
        ctx.return_name = saved_return;
        ctx.using = saved_using;
        return Err(err);
    }
    if let Err(err) = init_locals(ctx, &func.locals) {
        ctx.call_depth = saved_call_depth;
        ctx.storage.pop_frame();
        ctx.return_name = saved_return;
        ctx.using = saved_using;
        return Err(err);
    }
    let result = match exec_block(ctx, &func.body) {
        Ok(result) => result,
        Err(err) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            return Err(err);
        }
    };

    let return_value = match result {
        stmt::StmtResult::Return(Some(value)) => value,
        _ => ctx
            .storage
            .current_frame()
            .and_then(|frame| frame.return_value.clone())
            .map_or_else(
                || {
                    default_value_for_type_id(func.return_type, ctx.registry, &ctx.profile)
                        .map_err(|err| init_failed_debug(&func.name, &func.name, err))
                },
                Ok,
            )?,
    };

    let output_values = match collect_outputs(ctx, &out_targets) {
        Ok(values) => values,
        Err(err) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            return Err(err);
        }
    };
    ctx.storage.pop_frame();
    ctx.return_name = saved_return;
    ctx.using = saved_using;
    if let Err(err) = write_output_values(ctx, output_values) {
        ctx.call_depth = saved_call_depth;
        return Err(err);
    }
    ctx.call_depth = saved_call_depth;

    Ok(return_value)
}

/// Call a method definition on a specific instance.
pub(crate) fn call_method(
    ctx: &mut EvalContext<'_>,
    method: &MethodDef,
    instance_id: InstanceId,
    args: &[CallArg],
) -> Result<Value, RuntimeError> {
    let saved_using = ctx.using;
    let saved_instance = ctx.current_instance;
    let saved_return = ctx.return_name.clone();
    let PreparedBindings {
        should_execute,
        param_values,
        out_targets,
    } = match prepare_bindings(ctx, &method.name, &method.params, args, BindingMode::Function) {
        Ok(value) => value,
        Err(err) => {
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            ctx.current_instance = saved_instance;
            return Err(err);
        }
    };
    let return_default = method
        .return_type
        .map(|return_type| {
            default_value_for_type_id(return_type, ctx.registry, &ctx.profile)
                .map_err(|err| init_failed_debug(&method.name, &method.name, err))
        })
        .transpose()?;
    ctx.current_instance = Some(instance_id);
    ctx.storage
        .push_frame_with_instance(method.name.clone(), instance_id);
    ctx.return_name = method.return_type.map(|_| method.name.clone());
    if let Some(return_default) = return_default {
        ctx.storage.set_local(method.name.clone(), return_default);
    }
    for (name, value) in param_values {
        ctx.storage.set_local(name, value);
    }

    if !should_execute {
        let output_values = collect_outputs(ctx, &out_targets)?;
        ctx.storage.pop_frame();
        ctx.return_name = saved_return;
        ctx.using = saved_using;
        ctx.current_instance = saved_instance;
        write_output_values(ctx, output_values)?;
        return match method.return_type {
            Some(return_type) => default_value_for_type_id(return_type, ctx.registry, &ctx.profile)
                .map_err(|err| init_failed_debug(&method.name, &method.name, err)),
            None => Ok(Value::Null),
        };
    }

    let saved_call_depth = ctx.call_depth;
    ctx.call_depth = saved_call_depth.saturating_add(1);

    if let Err(err) = init_locals(ctx, &method.static_locals) {
        ctx.call_depth = saved_call_depth;
        ctx.storage.pop_frame();
        ctx.return_name = saved_return;
        ctx.using = saved_using;
        ctx.current_instance = saved_instance;
        return Err(err);
    }
    if let Err(err) = init_locals(ctx, &method.locals) {
        ctx.call_depth = saved_call_depth;
        ctx.storage.pop_frame();
        ctx.return_name = saved_return;
        ctx.using = saved_using;
        ctx.current_instance = saved_instance;
        return Err(err);
    }
    let result = match exec_block(ctx, &method.body) {
        Ok(result) => result,
        Err(err) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            ctx.current_instance = saved_instance;
            return Err(err);
        }
    };

    let return_value = if let Some(return_type) = method.return_type {
        match result {
            stmt::StmtResult::Return(Some(value)) => value,
            _ => ctx
                .storage
                .current_frame()
                .and_then(|frame| frame.return_value.clone())
                .map_or_else(
                    || {
                        default_value_for_type_id(return_type, ctx.registry, &ctx.profile)
                            .map_err(|err| init_failed_debug(&method.name, &method.name, err))
                    },
                    Ok,
                )?,
        }
    } else {
        Value::Null
    };
    let output_values = match collect_outputs(ctx, &out_targets) {
        Ok(values) => values,
        Err(err) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.return_name = saved_return;
            ctx.using = saved_using;
            ctx.current_instance = saved_instance;
            return Err(err);
        }
    };
    ctx.storage.pop_frame();
    ctx.return_name = saved_return;
    ctx.using = saved_using;
    ctx.current_instance = saved_instance;
    if let Err(err) = write_output_values(ctx, output_values) {
        ctx.call_depth = saved_call_depth;
        return Err(err);
    }
    ctx.call_depth = saved_call_depth;

    Ok(return_value)
}

/// Call a function block definition on a specific instance.
pub(crate) fn call_function_block<'a>(
    ctx: &mut EvalContext<'a>,
    fb: &'a FunctionBlockDef,
    instance_id: InstanceId,
    args: &[CallArg],
) -> Result<(), RuntimeError> {
    let saved_using = ctx.using;
    let saved_instance = ctx.current_instance;
    let PreparedBindings {
        should_execute,
        param_values,
        out_targets,
    } = match prepare_bindings(
        ctx,
        &fb.name,
        &fb.params,
        args,
        BindingMode::FunctionBlock { instance_id },
    ) {
        Ok(value) => value,
        Err(err) => {
            ctx.current_instance = saved_instance;
            ctx.using = saved_using;
            return Err(err);
        }
    };
    ctx.using = Some(&fb.using);
    ctx.current_instance = Some(instance_id);
    ctx.storage
        .push_frame_with_instance(fb.name.clone(), instance_id);
    for (name, value) in param_values {
        ctx.storage.set_instance_var(instance_id, name, value);
    }

    if !should_execute {
        let output_values = collect_outputs(ctx, &out_targets)?;
        ctx.storage.pop_frame();
        ctx.current_instance = saved_instance;
        ctx.using = saved_using;
        write_output_values(ctx, output_values)?;
        return Ok(());
    }
    let saved_call_depth = ctx.call_depth;
    ctx.call_depth = saved_call_depth.saturating_add(1);
    let builtin_kind = fbs::builtin_kind(fb.name.as_ref());
    let result = if let Some(kind) = builtin_kind {
        fbs::execute_builtin_in_storage(ctx.storage, ctx.now, instance_id, kind)
            .map(|_| stmt::StmtResult::Continue)
    } else {
        if let Err(err) = init_locals_in_frame(ctx, &fb.temps) {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.current_instance = saved_instance;
            ctx.using = saved_using;
            return Err(err);
        }
        exec_block(ctx, &fb.body)
    };
    let result = match result {
        Ok(result) => result,
        Err(err) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.current_instance = saved_instance;
            ctx.using = saved_using;
            return Err(err);
        }
    };

    match result {
        stmt::StmtResult::Return(_) | stmt::StmtResult::Continue => {
            let output_values = match collect_outputs(ctx, &out_targets) {
                Ok(values) => values,
                Err(err) => {
                    ctx.call_depth = saved_call_depth;
                    ctx.storage.pop_frame();
                    ctx.current_instance = saved_instance;
                    ctx.using = saved_using;
                    return Err(err);
                }
            };
            ctx.storage.pop_frame();
            ctx.current_instance = saved_instance;
            ctx.using = saved_using;
            if let Err(err) = write_output_values(ctx, output_values) {
                ctx.call_depth = saved_call_depth;
                return Err(err);
            }
            ctx.call_depth = saved_call_depth;
            Ok(())
        }
        stmt::StmtResult::Exit | stmt::StmtResult::LoopContinue | stmt::StmtResult::Jump(_) => {
            ctx.call_depth = saved_call_depth;
            ctx.storage.pop_frame();
            ctx.current_instance = saved_instance;
            ctx.using = saved_using;
            Err(RuntimeError::InvalidControlFlow)
        }
    }
}
