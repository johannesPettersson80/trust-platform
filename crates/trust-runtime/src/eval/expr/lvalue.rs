use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::eval::EvalContext;
use crate::value::{RefSegment, Value, ValueRef};

use super::access::{
    array_offset, eval_indices, index_to_i64, read_field, read_indices, read_name,
    resolve_reference, write_field, write_indices,
};
use super::ast::LValue;

fn qualified_field_name(name: &SmolStr, field: &SmolStr) -> SmolStr {
    SmolStr::new(format!("{name}.{field}"))
}

pub(super) fn resolve_reference_for_lvalue(
    ctx: &mut EvalContext<'_>,
    target: &LValue,
) -> Result<ValueRef, RuntimeError> {
    match target {
        LValue::Name(name) => resolve_reference(ctx, name)
            .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone())),
        LValue::Index { name, indices } => {
            let base = resolve_reference(ctx, name)
                .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;
            let array_value = read_name(ctx, name)?;
            let Value::Array(array) = &array_value else {
                return Err(RuntimeError::TypeMismatch);
            };
            let dimensions = &array.dimensions;
            let index_values = eval_indices(ctx, indices)?;
            array_offset(dimensions, &index_values)?;
            let mut index_path = Vec::with_capacity(index_values.len());
            for value in index_values {
                index_path.push(index_to_i64(value)?);
            }
            let mut value_ref = base;
            value_ref.path.push(RefSegment::Index(index_path));
            Ok(value_ref)
        }
        LValue::Field { name, field } => {
            let qualified = qualified_field_name(name, field);
            if let Some(reference) = resolve_reference(ctx, &qualified) {
                return Ok(reference);
            }
            let base_value = read_name(ctx, name)?;
            match base_value {
                Value::Instance(id) => ctx
                    .storage
                    .ref_for_instance_recursive(id, field.as_ref())
                    .ok_or_else(|| RuntimeError::UndefinedField(field.clone())),
                Value::Struct(struct_value) => {
                    let fields = &struct_value.fields;
                    if !fields.contains_key(field) {
                        return Err(RuntimeError::UndefinedField(field.clone()));
                    }
                    let mut value_ref = resolve_reference(ctx, name)
                        .ok_or_else(|| RuntimeError::UndefinedVariable(name.clone()))?;
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
        LValue::Index { name, indices } => {
            let array_value = read_name(ctx, name)?;
            let index_values = eval_indices(ctx, indices)?;
            read_indices(array_value, &index_values)
        }
        LValue::Field { name, field } => {
            let qualified = qualified_field_name(name, field);
            if let Ok(value) = read_name(ctx, &qualified) {
                return Ok(value);
            }
            let struct_value = read_name(ctx, name)?;
            read_field(ctx, struct_value, field)
        }
        LValue::Deref(expr) => {
            let value = super::eval::eval_expr(ctx, expr)?;
            match value {
                Value::Reference(Some(reference)) => ctx
                    .storage
                    .read_by_ref(reference)
                    .cloned()
                    .ok_or(RuntimeError::NullReference),
                Value::Reference(None) => Err(RuntimeError::NullReference),
                _ => Err(RuntimeError::TypeMismatch),
            }
        }
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
        LValue::Index { name, indices } => {
            let array_value = read_name(ctx, name)?;
            let index_values = eval_indices(ctx, indices)?;
            let updated = write_indices(array_value, &index_values, value)?;
            write_name(ctx, name, updated)
        }
        LValue::Field { name, field } => {
            let qualified = qualified_field_name(name, field);
            if resolve_reference(ctx, &qualified).is_some() {
                return write_name(ctx, &qualified, value);
            }
            let struct_value = read_name(ctx, name)?;
            if let Value::Instance(id) = struct_value {
                let Some(reference) = ctx.storage.ref_for_instance_recursive(id, field.as_ref())
                else {
                    return Err(RuntimeError::UndefinedField(field.clone()));
                };
                if ctx.storage.write_by_ref(reference, value) {
                    Ok(())
                } else {
                    Err(RuntimeError::NullReference)
                }
            } else {
                let updated = write_field(ctx, struct_value, field, value)?;
                write_name(ctx, name, updated)
            }
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
