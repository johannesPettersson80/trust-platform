use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::eval::EvalContext;
use crate::value::{ref_indices_from_iter, RefSegment, Value, ValueRef};

use super::access::{
    array_offset, eval_indices, index_to_i64, read_name, resolve_reference, write_field,
    write_indices,
};
use super::ast::LValue;

fn expr_from_lvalue(target: &LValue) -> crate::program_model::Expr {
    match target {
        LValue::Name(name) => crate::program_model::Expr::Name(name.clone()),
        LValue::Index { target, indices } => crate::program_model::Expr::Index {
            target: Box::new(expr_from_lvalue(target)),
            indices: indices.clone(),
        },
        LValue::Field { target, field } => crate::program_model::Expr::Field {
            target: Box::new(expr_from_lvalue(target)),
            field: field.clone(),
        },
        LValue::Deref(expr) => crate::program_model::Expr::Deref(expr.clone()),
    }
}

pub(super) fn resolve_reference_for_lvalue(
    ctx: &mut EvalContext<'_>,
    target: &LValue,
) -> Result<ValueRef, RuntimeError> {
    match target {
        LValue::Name(name) => resolve_reference(ctx, name)
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
        LValue::Index { target, indices } => {
            let base = resolve_reference_for_lvalue(ctx, target)?;
            let array_value = read_lvalue(ctx, target)?;
            let Value::Array(array) = &array_value else {
                return Err(RuntimeError::TypeMismatch);
            };
            let dimensions = array.dimensions();
            let index_values = eval_indices(ctx, indices)?;
            array_offset(dimensions, &index_values)?;
            let mut index_path = Vec::with_capacity(index_values.len());
            for value in index_values {
                index_path.push(index_to_i64(value)?);
            }
            let mut value_ref = base;
            value_ref
                .path
                .push(RefSegment::Index(ref_indices_from_iter(index_path)));
            Ok(value_ref)
        }
        LValue::Field { target, field } => {
            if let Some(qualified) = target
                .qualified_name()
                .map(|prefix| SmolStr::new(format!("{prefix}.{field}")))
            {
                if let Some(reference) = resolve_reference(ctx, &qualified) {
                    return Ok(reference);
                }
            }
            let base_value = read_lvalue(ctx, target)?;
            match base_value {
                Value::Instance(id) => ctx
                    .storage
                    .ref_for_instance_recursive(id, field.as_ref())
                    .ok_or_else(|| RuntimeError::UndefinedField(field.clone())),
                Value::Struct(struct_value) => {
                    if !struct_value.contains_field(field.as_str()) {
                        return Err(RuntimeError::UndefinedField(field.clone()));
                    }
                    let mut value_ref = resolve_reference_for_lvalue(ctx, target)?;
                    value_ref.path.push(RefSegment::Field(field.clone()));
                    Ok(value_ref)
                }
                _ => Err(RuntimeError::TypeMismatch),
            }
        }
        LValue::Deref(expr) => {
            let value = super::eval::eval_expr(ctx, expr)?;
            match value {
                Value::Reference(Some(reference)) => Ok(reference),
                Value::Reference(None) => Err(RuntimeError::NullReference),
                _ => Err(RuntimeError::TypeMismatch),
            }
        }
    }
}

/// Read a value from an assignment target.
pub fn read_lvalue(ctx: &mut EvalContext<'_>, target: &LValue) -> Result<Value, RuntimeError> {
    match target {
        LValue::Name(name) => read_name(ctx, name),
        _ => super::eval::eval_expr(ctx, &expr_from_lvalue(target)),
    }
}

/// Write to an assignment target.
pub fn write_lvalue(
    ctx: &mut EvalContext<'_>,
    target: &LValue,
    value: Value,
) -> Result<(), RuntimeError> {
    match target {
        LValue::Name(name) => write_name(ctx, name, value),
        LValue::Index { target, indices } => {
            let array_value = read_lvalue(ctx, target)?;
            let index_values = eval_indices(ctx, indices)?;
            let updated = write_indices(array_value, &index_values, value)?;
            write_lvalue(ctx, target, updated)
        }
        LValue::Field { target, field } => {
            let base_value = read_lvalue(ctx, target)?;
            let updated = write_field(ctx, base_value, field, value)?;
            write_lvalue(ctx, target, updated)
        }
        LValue::Deref(expr) => {
            let reference_value = super::eval::eval_expr(ctx, expr)?;
            match reference_value {
                Value::Reference(Some(reference)) => {
                    if ctx.storage.write_by_ref(reference, value) {
                        Ok(())
                    } else {
                        Err(RuntimeError::NullReference)
                    }
                }
                Value::Reference(None) => Err(RuntimeError::NullReference),
                _ => Err(RuntimeError::TypeMismatch),
            }
        }
    }
}

pub fn write_name(
    ctx: &mut EvalContext<'_>,
    name: &SmolStr,
    value: Value,
) -> Result<(), RuntimeError> {
    super::access::write_name(ctx, name, value)
}
