//! Statement execution.

#![allow(missing_docs)]

pub use crate::program_model::stmt::{CaseLabel, Stmt, StmtResult};

#[cfg(test)]
use crate::error::RuntimeError;
#[cfg(test)]
use crate::eval::expr::{eval_expr, read_lvalue, write_lvalue, Expr, LValue};
#[cfg(test)]
use crate::eval::EvalContext;
#[cfg(test)]
use crate::program_model::BinaryOp;
#[cfg(test)]
use crate::value::Value;
#[cfg(test)]
use smol_str::SmolStr;

/// Execute a statement.
#[cfg(test)]
pub(crate) fn exec_stmt(
    ctx: &mut EvalContext<'_>,
    stmt: &Stmt,
) -> Result<StmtResult, RuntimeError> {
    check_execution_budget(ctx)?;
    #[cfg(feature = "debug")]
    if let Some(hook) = ctx.debug.take() {
        {
            let mut debug_ctx = crate::debug::DebugRuntimeContext {
                storage: &mut *ctx.storage,
                registry: ctx.registry,
                stdlib: ctx.stdlib,
                profile: ctx.profile,
                current_instance: ctx.current_instance,
                now: ctx.now,
            };
            hook.on_statement_with_context(&mut debug_ctx, stmt.location(), ctx.call_depth);
        }
        ctx.debug = Some(hook);
    }
    match stmt {
        Stmt::Assign { target, value, .. } => {
            let value = eval_expr(ctx, value)?;
            write_lvalue(ctx, target, value)?;
            if let Some(return_name) = &ctx.return_name {
                if matches!(target, LValue::Name(name) if name == return_name) {
                    let value = read_lvalue(ctx, target)?;
                    if let Some(frame) = ctx.storage.current_frame_mut() {
                        frame.return_value = Some(value);
                    }
                }
            }
            Ok(StmtResult::Continue)
        }
        Stmt::AssignAttempt { target, value, .. } => {
            let value = eval_expr(ctx, value)?;
            let target_value = read_lvalue(ctx, target)?;
            if !matches!(target_value, Value::Reference(_)) {
                return Err(RuntimeError::TypeMismatch);
            }
            let value = match value {
                Value::Reference(_) => value,
                Value::Null => Value::Reference(None),
                _ => Value::Reference(None),
            };
            write_lvalue(ctx, target, value)?;
            Ok(StmtResult::Continue)
        }
        Stmt::Expr { expr, .. } => {
            let _ = eval_expr(ctx, expr)?;
            Ok(StmtResult::Continue)
        }
        Stmt::If {
            condition,
            then_block,
            else_if,
            else_block,
            ..
        } => {
            if eval_bool(ctx, condition)? {
                return exec_block(ctx, then_block);
            }
            for (elsif_cond, elsif_block) in else_if {
                if eval_bool(ctx, elsif_cond)? {
                    return exec_block(ctx, elsif_block);
                }
            }
            exec_block(ctx, else_block)
        }
        Stmt::Case {
            selector,
            branches,
            else_block,
            ..
        } => {
            let selector_value = eval_expr(ctx, selector)?;
            for (labels, block) in branches {
                for label in labels {
                    let matches = match label {
                        CaseLabel::Single(value) => match crate::program_model::apply_binary(
                            BinaryOp::Eq,
                            selector_value.clone(),
                            value.clone(),
                            &ctx.profile,
                        )? {
                            Value::Bool(matches) => matches,
                            _ => return Err(RuntimeError::CaseSelectorType),
                        },
                        CaseLabel::Range(lower, upper) => {
                            let selector_int = match &selector_value {
                                Value::SInt(v) => *v as i64,
                                Value::Int(v) => *v as i64,
                                Value::DInt(v) => *v as i64,
                                Value::LInt(v) => *v,
                                _ => return Err(RuntimeError::CaseSelectorType),
                            };
                            selector_int >= *lower && selector_int <= *upper
                        }
                    };
                    if matches {
                        return exec_block(ctx, block);
                    }
                }
            }
            exec_block(ctx, else_block)
        }
        Stmt::For {
            control,
            start,
            end,
            step,
            body,
            ..
        } => {
            let start_value = eval_expr(ctx, start)?;
            let end_value = eval_expr(ctx, end)?;
            let step_value = eval_expr(ctx, step)?;
            let start_i = int_value(start_value)?;
            let end_i = int_value(end_value)?;
            let step_i = int_value(step_value)?;
            if step_i == 0 {
                return Err(RuntimeError::ForStepZero);
            }
            let control_template = read_lvalue(ctx, &LValue::Name(control.clone()))?;
            if is_unsigned_int(&control_template) && step_i < 0 {
                return Err(RuntimeError::TypeMismatch);
            }
            let mut current = start_i;
            write_lvalue(
                ctx,
                &LValue::Name(control.clone()),
                coerce_loop_value(&control_template, current)?,
            )?;
            loop {
                check_execution_budget(ctx)?;
                if (step_i > 0 && current > end_i) || (step_i < 0 && current < end_i) {
                    break;
                }
                ctx.loop_depth += 1;
                let result = exec_block(ctx, body)?;
                ctx.loop_depth -= 1;
                match result {
                    StmtResult::Continue => {}
                    StmtResult::LoopContinue => {}
                    StmtResult::Exit => break,
                    StmtResult::Return(_) => return Ok(result),
                    StmtResult::Jump(_) => return Err(RuntimeError::InvalidControlFlow),
                }
                current += step_i;
                write_lvalue(
                    ctx,
                    &LValue::Name(control.clone()),
                    coerce_loop_value(&control_template, current)?,
                )?;
            }
            Ok(StmtResult::Continue)
        }
        Stmt::While {
            condition, body, ..
        } => {
            loop {
                check_execution_budget(ctx)?;
                if !eval_bool(ctx, condition)? {
                    break;
                }
                ctx.loop_depth += 1;
                let result = exec_block(ctx, body)?;
                ctx.loop_depth -= 1;
                match result {
                    StmtResult::Continue => {}
                    StmtResult::LoopContinue => continue,
                    StmtResult::Exit => break,
                    StmtResult::Return(_) => return Ok(result),
                    StmtResult::Jump(_) => return Err(RuntimeError::InvalidControlFlow),
                }
            }
            Ok(StmtResult::Continue)
        }
        Stmt::Repeat { body, until, .. } => loop {
            check_execution_budget(ctx)?;
            ctx.loop_depth += 1;
            let result = exec_block(ctx, body)?;
            ctx.loop_depth -= 1;
            match result {
                StmtResult::Continue => {}
                StmtResult::LoopContinue => {}
                StmtResult::Exit => return Ok(StmtResult::Continue),
                StmtResult::Return(_) => return Ok(result),
                StmtResult::Jump(_) => return Err(RuntimeError::InvalidControlFlow),
            }
            if eval_bool(ctx, until)? {
                return Ok(StmtResult::Continue);
            }
        },
        Stmt::Label { stmt, .. } => {
            if let Some(inner) = stmt {
                exec_stmt(ctx, inner)
            } else {
                Ok(StmtResult::Continue)
            }
        }
        Stmt::Jmp { target, .. } => Ok(StmtResult::Jump(target.clone())),
        Stmt::Return { expr, .. } => {
            let value = expr.as_ref().map(|expr| eval_expr(ctx, expr)).transpose()?;
            Ok(StmtResult::Return(value))
        }
        Stmt::Exit { .. } => {
            if ctx.loop_depth == 0 {
                Err(RuntimeError::InvalidControlFlow)
            } else {
                Ok(StmtResult::Exit)
            }
        }
        Stmt::Continue { .. } => {
            if ctx.loop_depth == 0 {
                Err(RuntimeError::InvalidControlFlow)
            } else {
                Ok(StmtResult::LoopContinue)
            }
        }
    }
}

#[cfg(test)]
fn check_execution_budget(ctx: &EvalContext<'_>) -> Result<(), RuntimeError> {
    if let Some(deadline) = ctx.execution_deadline {
        if std::time::Instant::now() >= deadline {
            return Err(RuntimeError::ExecutionTimeout);
        }
    }
    Ok(())
}

/// Execute a list of statements.
#[cfg(test)]
pub(crate) fn exec_block(
    ctx: &mut EvalContext<'_>,
    stmts: &[Stmt],
) -> Result<StmtResult, RuntimeError> {
    let mut labels = rustc_hash::FxHashMap::default();
    for (idx, stmt) in stmts.iter().enumerate() {
        if let Stmt::Label { name, .. } = stmt {
            let key = SmolStr::new(name.to_ascii_uppercase());
            labels.entry(key).or_insert(idx);
        }
    }

    let mut idx = 0;
    while idx < stmts.len() {
        let result = exec_stmt(ctx, &stmts[idx])?;
        match result {
            StmtResult::Continue => idx += 1,
            StmtResult::Jump(target) => {
                let key = SmolStr::new(target.to_ascii_uppercase());
                if let Some(next) = labels.get(&key) {
                    idx = *next;
                } else {
                    return Err(RuntimeError::UndefinedLabel(target));
                }
            }
            _ => return Ok(result),
        }
    }
    Ok(StmtResult::Continue)
}

#[cfg(test)]
fn eval_bool(ctx: &mut EvalContext<'_>, expr: &Expr) -> Result<bool, RuntimeError> {
    match eval_expr(ctx, expr)? {
        Value::Bool(value) => Ok(value),
        _ => Err(RuntimeError::ConditionNotBool),
    }
}

#[cfg(test)]
fn int_value(value: Value) -> Result<i64, RuntimeError> {
    match value {
        Value::SInt(v) => Ok(v as i64),
        Value::Int(v) => Ok(v as i64),
        Value::DInt(v) => Ok(v as i64),
        Value::LInt(v) => Ok(v),
        Value::USInt(v) => Ok(v as i64),
        Value::UInt(v) => Ok(v as i64),
        Value::UDInt(v) => Ok(v as i64),
        Value::ULInt(v) => Ok(v as i64),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

#[cfg(test)]
fn is_unsigned_int(value: &Value) -> bool {
    matches!(
        value,
        Value::USInt(_) | Value::UInt(_) | Value::UDInt(_) | Value::ULInt(_)
    )
}

#[cfg(test)]
fn coerce_loop_value(template: &Value, value: i64) -> Result<Value, RuntimeError> {
    match template {
        Value::SInt(_) => i8::try_from(value)
            .map(Value::SInt)
            .map_err(|_| RuntimeError::Overflow),
        Value::Int(_) => i16::try_from(value)
            .map(Value::Int)
            .map_err(|_| RuntimeError::Overflow),
        Value::DInt(_) => i32::try_from(value)
            .map(Value::DInt)
            .map_err(|_| RuntimeError::Overflow),
        Value::LInt(_) => Ok(Value::LInt(value)),
        Value::USInt(_) => {
            let unsigned = u64::try_from(value).map_err(|_| RuntimeError::TypeMismatch)?;
            u8::try_from(unsigned)
                .map(Value::USInt)
                .map_err(|_| RuntimeError::Overflow)
        }
        Value::UInt(_) => {
            let unsigned = u64::try_from(value).map_err(|_| RuntimeError::TypeMismatch)?;
            u16::try_from(unsigned)
                .map(Value::UInt)
                .map_err(|_| RuntimeError::Overflow)
        }
        Value::UDInt(_) => {
            let unsigned = u64::try_from(value).map_err(|_| RuntimeError::TypeMismatch)?;
            u32::try_from(unsigned)
                .map(Value::UDInt)
                .map_err(|_| RuntimeError::Overflow)
        }
        Value::ULInt(_) => {
            let unsigned = u64::try_from(value).map_err(|_| RuntimeError::TypeMismatch)?;
            Ok(Value::ULInt(unsigned))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}
