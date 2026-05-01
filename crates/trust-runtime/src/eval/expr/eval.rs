use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::eval::ops::{apply_binary, apply_unary, BinaryOp};
use crate::eval::EvalContext;
use crate::stdlib::{conversions, time, StdParams};
use crate::value::{size_of_type, ArrayValue, SizeOfError, StructValue, Value};
use indexmap::IndexMap;

use super::access::{eval_indices, read_field, read_indices, read_name};
use super::ast::{Expr, SizeOfTarget};
use super::call::{
    bind_stdlib_named_args, call_target_name, eval_positional_args, eval_ref_call, eval_split_call,
    resolve_instance_method, resolve_using_function,
};
use super::lvalue::resolve_reference_for_lvalue;

/// Evaluate an expression to a runtime value.
pub fn eval_expr(ctx: &mut EvalContext<'_>, expr: &Expr) -> Result<Value, RuntimeError> {
    match expr {
        Expr::Literal(value) => Ok(value.clone()),
        Expr::ArrayInitializer(elements) => {
            let values = eval_array_initializer_elements(ctx, elements)?;
            ArrayValue::from_untyped_parts(values, vec![(1, elements.len() as i64)])
                .map(|value| Value::Array(Box::new(value)))
                .map_err(|_| RuntimeError::TypeMismatch)
        }
        Expr::StructInitializer(fields) => {
            let mut values = IndexMap::new();
            for (field, expr) in fields {
                if values
                    .keys()
                    .any(|existing: &SmolStr| existing.eq_ignore_ascii_case(field.as_str()))
                {
                    return Err(RuntimeError::TypeMismatch);
                }
                values.insert(field.clone(), eval_expr(ctx, expr)?);
            }
            Ok(Value::Struct(std::sync::Arc::new(
                StructValue::from_untyped_parts("".into(), values),
            )))
        }
        Expr::This => ctx
            .current_instance
            .map(Value::Instance)
            .ok_or(RuntimeError::TypeMismatch),
        Expr::Super => {
            let current = ctx.current_instance.ok_or(RuntimeError::TypeMismatch)?;
            let instance = ctx
                .storage
                .get_instance(current)
                .ok_or(RuntimeError::NullReference)?;
            instance
                .parent
                .map(Value::Instance)
                .ok_or(RuntimeError::TypeMismatch)
        }
        Expr::SizeOf(target) => eval_size_of(ctx, target),
        Expr::Name(name) => read_name(ctx, name),
        Expr::Call { target, args } => {
            if let Some(name) = call_target_name(target) {
                let key = SmolStr::new(name.to_ascii_uppercase());
                if key == "REF" {
                    return eval_ref_call(ctx, args);
                }
                if time::is_runtime_clock_name(key.as_str()) {
                    if !args.is_empty() {
                        return Err(RuntimeError::InvalidArgumentCount {
                            expected: 0,
                            got: args.len(),
                        });
                    }
                    return Ok(Value::Time(ctx.now));
                }
                if time::is_split_name(key.as_str()) {
                    return eval_split_call(ctx, key.as_str(), args);
                }
                if let Some(functions) = ctx.functions {
                    if let Some(func) = functions.get(&key) {
                        return crate::eval::call_function(ctx, func, args);
                    }
                    if !name.contains('.') {
                        if let Some(using) = ctx.using {
                            if let Some(func) =
                                resolve_using_function(functions, name.as_str(), using)
                            {
                                return crate::eval::call_function(ctx, func, args);
                            }
                        }
                    }
                }
                if let Some(stdlib) = ctx.stdlib {
                    let has_named = args.iter().any(|arg| arg.name.is_some());
                    if let Some(entry) = stdlib.get(&key) {
                        let values = if has_named {
                            bind_stdlib_named_args(ctx, &entry.params, args)?
                        } else {
                            eval_positional_args(ctx, args)?
                        };
                        return (entry.func)(&values);
                    }
                    if conversions::is_conversion_name(key.as_str()) {
                        let params = StdParams::Fixed(vec![SmolStr::new("IN")]);
                        let values = if has_named {
                            bind_stdlib_named_args(ctx, &params, args)?
                        } else {
                            eval_positional_args(ctx, args)?
                        };
                        return stdlib.call(&key, &values);
                    }
                }
            }

            if let Expr::Field {
                target: base,
                field,
            } = &**target
            {
                let base_value = eval_expr(ctx, base)?;
                if let Value::Instance(id) = base_value {
                    if let Some(method) = resolve_instance_method(ctx, id, field) {
                        return crate::eval::call_method(ctx, &method, id, args);
                    }
                }
            }
            if let Expr::Name(name) = &**target {
                if let Some(instance_id) = ctx.current_instance {
                    if let Some(method) = resolve_instance_method(ctx, instance_id, name) {
                        return crate::eval::call_method(ctx, &method, instance_id, args);
                    }
                }
            }

            let target_value = eval_expr(ctx, target)?;
            if let Value::Instance(id) = target_value {
                let function_blocks = ctx.function_blocks.ok_or(RuntimeError::TypeMismatch)?;
                let instance = ctx
                    .storage
                    .get_instance(id)
                    .ok_or(RuntimeError::NullReference)?;
                let key = SmolStr::new(instance.type_name.to_ascii_uppercase());
                let fb = function_blocks.get(&key).ok_or_else(|| {
                    RuntimeError::UndefinedFunctionBlock(instance.type_name.clone())
                })?;
                crate::eval::call_function_block(ctx, fb, id, args)?;
                return Ok(Value::Null);
            }

            Err(RuntimeError::TypeMismatch)
        }
        Expr::Unary { op, expr } => {
            let value = eval_expr(ctx, expr)?;
            apply_unary(*op, value)
        }
        Expr::Binary { op, left, right } => {
            if *op == BinaryOp::And {
                let left_value = eval_expr(ctx, left)?;
                if matches!(left_value, Value::Bool(false)) {
                    return Ok(Value::Bool(false));
                }
                let right_value = eval_expr(ctx, right)?;
                return apply_binary(*op, left_value, right_value, &ctx.profile);
            }
            if *op == BinaryOp::Or {
                let left_value = eval_expr(ctx, left)?;
                if matches!(left_value, Value::Bool(true)) {
                    return Ok(Value::Bool(true));
                }
                let right_value = eval_expr(ctx, right)?;
                return apply_binary(*op, left_value, right_value, &ctx.profile);
            }
            let left_value = eval_expr(ctx, left)?;
            let right_value = eval_expr(ctx, right)?;
            apply_binary(*op, left_value, right_value, &ctx.profile)
        }
        Expr::Index { target, indices } => {
            let target_value = eval_expr(ctx, target)?;
            let index_values = eval_indices(ctx, indices)?;
            read_indices(target_value, &index_values)
        }
        Expr::Field { target, field } => {
            if let Some(qualified) = qualified_field_expr_name(expr) {
                if let Ok(value) = read_name(ctx, &qualified) {
                    return Ok(value);
                }
            }
            let target_value = eval_expr(ctx, target)?;
            read_field(ctx, target_value, field)
        }
        Expr::Ref(target) => {
            let value_ref = resolve_reference_for_lvalue(ctx, target)?;
            Ok(Value::Reference(Some(value_ref)))
        }
        Expr::Deref(expr) => {
            let value = eval_expr(ctx, expr)?;
            match value {
                Value::Reference(Some(reference)) => ctx
                    .storage
                    .materialize_by_ref(reference)
                    .ok_or(RuntimeError::NullReference),
                Value::Reference(None) => Err(RuntimeError::NullReference),
                _ => Err(RuntimeError::TypeMismatch),
            }
        }
    }
}

fn eval_array_initializer_elements(
    ctx: &mut EvalContext<'_>,
    elements: &[Expr],
) -> Result<Vec<Value>, RuntimeError> {
    let mut values = Vec::new();
    for expr in elements {
        if let Some((count, repeated_args)) = array_repeat_group(expr)? {
            for _ in 0..count {
                for arg in repeated_args {
                    let crate::program_model::ArgValue::Expr(value_expr) = &arg.value else {
                        return Err(RuntimeError::TypeMismatch);
                    };
                    values.push(eval_expr(ctx, value_expr)?);
                }
            }
            continue;
        }
        values.push(eval_expr(ctx, expr)?);
    }
    Ok(values)
}

fn array_repeat_group(
    expr: &Expr,
) -> Result<Option<(usize, &[crate::program_model::CallArg])>, RuntimeError> {
    let Expr::Call { target, args } = expr else {
        return Ok(None);
    };
    if args.iter().any(|arg| arg.name.is_some()) {
        return Err(RuntimeError::TypeMismatch);
    }
    let count = match target.as_ref() {
        Expr::Literal(Value::SInt(v)) => i64::from(*v),
        Expr::Literal(Value::Int(v)) => i64::from(*v),
        Expr::Literal(Value::DInt(v)) => i64::from(*v),
        Expr::Literal(Value::LInt(v)) => *v,
        Expr::Literal(Value::USInt(v)) => i64::from(*v),
        Expr::Literal(Value::UInt(v)) => i64::from(*v),
        Expr::Literal(Value::UDInt(v)) => i64::from(*v),
        Expr::Literal(Value::ULInt(v)) => {
            i64::try_from(*v).map_err(|_| RuntimeError::TypeMismatch)?
        }
        _ => return Ok(None),
    };
    if count < 0 {
        return Err(RuntimeError::TypeMismatch);
    }
    let count = usize::try_from(count).map_err(|_| RuntimeError::TypeMismatch)?;
    Ok(Some((count, args)))
}

fn qualified_field_expr_name(expr: &Expr) -> Option<SmolStr> {
    match expr {
        Expr::Name(name) => Some(name.clone()),
        Expr::Field { target, field } => {
            let prefix = qualified_field_expr_name(target)?;
            Some(SmolStr::new(format!("{prefix}.{field}")))
        }
        _ => None,
    }
}

fn eval_size_of(ctx: &mut EvalContext<'_>, target: &SizeOfTarget) -> Result<Value, RuntimeError> {
    let size = match target {
        SizeOfTarget::Type(type_id) => {
            size_of_type(*type_id, ctx.registry).map_err(size_error_to_runtime)?
        }
    };
    let size = i32::try_from(size).map_err(|_| RuntimeError::Overflow)?;
    Ok(Value::DInt(size))
}

fn size_error_to_runtime(err: SizeOfError) -> RuntimeError {
    match err {
        SizeOfError::Overflow => RuntimeError::Overflow,
        SizeOfError::UnknownType | SizeOfError::UnsupportedType => RuntimeError::TypeMismatch,
    }
}
