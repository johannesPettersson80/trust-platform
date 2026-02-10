//! Statement execution.

#![allow(missing_docs)]

use smol_str::SmolStr;

use crate::debug::SourceLocation;
use crate::error::RuntimeError;
use crate::eval::expr::{eval_expr, read_lvalue, write_lvalue, Expr, LValue};
use crate::eval::EvalContext;
use crate::value::Value;

/// Statement execution result.
#[derive(Debug, Clone, PartialEq)]
pub enum StmtResult {
    Continue,
    Return(Option<Value>),
    Exit,
    LoopContinue,
    Jump(SmolStr),
}

/// CASE label.
#[derive(Debug, Clone)]
pub enum CaseLabel {
    Single(i64),
    Range(i64, i64),
}

/// Statement node.
#[derive(Debug, Clone)]
pub enum Stmt {
    Assign {
        target: LValue,
        value: Expr,
        location: Option<SourceLocation>,
    },
    AssignAttempt {
        target: LValue,
        value: Expr,
        location: Option<SourceLocation>,
    },
    Expr {
        expr: Expr,
        location: Option<SourceLocation>,
    },
    If {
        condition: Expr,
        then_block: Vec<Stmt>,
        else_if: Vec<(Expr, Vec<Stmt>)>,
        else_block: Vec<Stmt>,
        location: Option<SourceLocation>,
    },
    Case {
        selector: Expr,
        branches: Vec<(Vec<CaseLabel>, Vec<Stmt>)>,
        else_block: Vec<Stmt>,
        location: Option<SourceLocation>,
    },
    For {
        control: SmolStr,
        start: Expr,
        end: Expr,
        step: Expr,
        body: Vec<Stmt>,
        location: Option<SourceLocation>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
        location: Option<SourceLocation>,
    },
    Repeat {
        body: Vec<Stmt>,
        until: Expr,
        location: Option<SourceLocation>,
    },
    Label {
        name: SmolStr,
        stmt: Option<Box<Stmt>>,
        location: Option<SourceLocation>,
    },
    Jmp {
        target: SmolStr,
        location: Option<SourceLocation>,
    },
    Return {
        expr: Option<Expr>,
        location: Option<SourceLocation>,
    },
    Exit {
        location: Option<SourceLocation>,
    },
    Continue {
        location: Option<SourceLocation>,
    },
}

impl Stmt {
    #[must_use]
    pub fn location(&self) -> Option<&SourceLocation> {
        match self {
            Stmt::Assign { location, .. }
            | Stmt::AssignAttempt { location, .. }
            | Stmt::Expr { location, .. }
            | Stmt::If { location, .. }
            | Stmt::Case { location, .. }
            | Stmt::For { location, .. }
            | Stmt::While { location, .. }
            | Stmt::Repeat { location, .. }
            | Stmt::Label { location, .. }
            | Stmt::Jmp { location, .. }
            | Stmt::Return { location, .. }
            | Stmt::Exit { location, .. }
            | Stmt::Continue { location, .. } => location.as_ref(),
        }
    }
}

/// Execute a statement.
pub fn exec_stmt(ctx: &mut EvalContext<'_>, stmt: &Stmt) -> Result<StmtResult, RuntimeError> {
    check_execution_budget(ctx)?;
    #[cfg(feature = "debug")]
    if let Some(hook) = ctx.debug.take() {
        hook.on_statement_with_context(ctx, stmt.location(), ctx.call_depth);
        ctx.debug = Some(hook);
    }
    match stmt {
        Stmt::Assign { target, value, .. } => {
            let value = eval_expr(ctx, value)?;
            write_lvalue(ctx, target, value)?;
            if let Some(return_name) = &ctx.return_name {
                if target.name() == return_name {
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
            let selector_int = match selector_value {
                Value::SInt(v) => v as i64,
                Value::Int(v) => v as i64,
                Value::DInt(v) => v as i64,
                Value::LInt(v) => v,
                _ => return Err(RuntimeError::CaseSelectorType),
            };
            for (labels, block) in branches {
                for label in labels {
                    let matches = match label {
                        CaseLabel::Single(value) => *value == selector_int,
                        CaseLabel::Range(lower, upper) => {
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

fn check_execution_budget(ctx: &EvalContext<'_>) -> Result<(), RuntimeError> {
    if let Some(deadline) = ctx.execution_deadline {
        if std::time::Instant::now() >= deadline {
            return Err(RuntimeError::ExecutionTimeout);
        }
    }
    Ok(())
}

/// Execute a list of statements.
pub fn exec_block(ctx: &mut EvalContext<'_>, stmts: &[Stmt]) -> Result<StmtResult, RuntimeError> {
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

fn eval_bool(ctx: &mut EvalContext<'_>, expr: &Expr) -> Result<bool, RuntimeError> {
    match eval_expr(ctx, expr)? {
        Value::Bool(value) => Ok(value),
        _ => Err(RuntimeError::ConditionNotBool),
    }
}

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

fn is_unsigned_int(value: &Value) -> bool {
    matches!(
        value,
        Value::USInt(_) | Value::UInt(_) | Value::UDInt(_) | Value::ULInt(_)
    )
}

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
