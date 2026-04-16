use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::eval::EvalContext;
use crate::value::{
    parse_partial_access, read_partial_access, write_partial_access, PartialAccessError, Value,
    ValueRef,
};

use super::ast::Expr;

pub(super) fn write_name(
    ctx: &mut EvalContext<'_>,
    name: &SmolStr,
    value: Value,
) -> Result<(), RuntimeError> {
    if ctx.storage.get_local(name.as_ref()).is_some() {
        ctx.storage.set_local(name.clone(), value.clone());
        return Ok(());
    }
    if let Some(reference) = crate::eval::static_storage_value_ref(ctx, name) {
        if ctx.storage.write_by_ref(reference, value.clone()) {
            return Ok(());
        }
        return Err(RuntimeError::NullReference);
    }
    if let Some(instance_id) = ctx.current_instance {
        if let Some(reference) = ctx
            .storage
            .ref_for_instance_recursive(instance_id, name.as_ref())
        {
            if ctx.storage.write_by_ref(reference, value.clone()) {
                return Ok(());
            }
            return Err(RuntimeError::NullReference);
        }
    }
    if let Some(access) = ctx.access {
        if let Some(binding) = access.get(name.as_ref()) {
            if let Some(partial) = binding.partial {
                let current = ctx
                    .storage
                    .read_by_ref(binding.reference.clone())
                    .cloned()
                    .ok_or(RuntimeError::NullReference)?;
                let updated = write_partial_access(current, partial, value)
                    .map_err(partial_access_error_to_runtime)?;
                if ctx.storage.write_by_ref(binding.reference.clone(), updated) {
                    return Ok(());
                }
                return Err(RuntimeError::NullReference);
            }
            if ctx.storage.write_by_ref(binding.reference.clone(), value) {
                return Ok(());
            }
            return Err(RuntimeError::NullReference);
        }
    }
    ctx.storage.set_global(name.clone(), value);
    Ok(())
}

pub(super) fn read_name(ctx: &EvalContext<'_>, name: &SmolStr) -> Result<Value, RuntimeError> {
    if let Some(value) = ctx.storage.get_local(name.as_ref()) {
        return Ok(value.clone());
    }
    if let Some(reference) = crate::eval::static_storage_value_ref(ctx, name) {
        return ctx
            .storage
            .read_by_ref(reference)
            .cloned()
            .ok_or(RuntimeError::NullReference);
    }
    if let Some(instance_id) = ctx.current_instance {
        if let Some(value) = ctx
            .storage
            .get_instance_var_recursive(instance_id, name.as_ref())
        {
            return Ok(value.clone());
        }
    }
    if let Some(access) = ctx.access {
        if let Some(binding) = access.get(name.as_ref()) {
            let value = ctx
                .storage
                .read_by_ref(binding.reference.clone())
                .cloned()
                .ok_or(RuntimeError::NullReference)?;
            if let Some(partial) = binding.partial {
                return read_partial_access(&value, partial)
                    .map_err(partial_access_error_to_runtime);
            }
            return Ok(value);
        }
    }
    if let Some(value) = ctx.storage.get_global(name.as_ref()) {
        return Ok(value.clone());
    }
    if let Some(value) = ctx.storage.get_retain(name.as_ref()) {
        return Ok(value.clone());
    }
    Err(RuntimeError::UndefinedVariable(name.clone()))
}

pub(super) fn resolve_reference(ctx: &EvalContext<'_>, name: &SmolStr) -> Option<ValueRef> {
    if let Some(value_ref) = ctx.storage.ref_for_local(name.as_ref()) {
        return Some(value_ref);
    }
    if let Some(value_ref) = crate::eval::static_storage_value_ref(ctx, name) {
        return Some(value_ref);
    }
    if let Some(instance_id) = ctx.current_instance {
        if let Some(value_ref) = ctx
            .storage
            .ref_for_instance_recursive(instance_id, name.as_ref())
        {
            return Some(value_ref);
        }
    }
    if let Some(access) = ctx.access {
        if let Some(binding) = access.get(name.as_ref()) {
            return Some(binding.reference.clone());
        }
    }
    ctx.storage.ref_for_global(name.as_ref())
}

pub(super) fn read_indices(target: Value, indices: &[Value]) -> Result<Value, RuntimeError> {
    match target {
        Value::Array(array) => {
            let offset = array_offset(&array.dimensions, indices)?;
            array
                .elements
                .get(offset)
                .cloned()
                .ok_or(RuntimeError::TypeMismatch)
        }
        Value::String(text) => read_string_index(text.as_str(), indices, false),
        Value::WString(text) => read_string_index(text.as_str(), indices, true),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn write_indices(
    target: Value,
    indices: &[Value],
    value: Value,
) -> Result<Value, RuntimeError> {
    match target {
        Value::Array(mut array) => {
            let offset = array_offset(&array.dimensions, indices)?;
            if let Some(slot) = array.elements.get_mut(offset) {
                *slot = value;
                Ok(Value::Array(array))
            } else {
                Err(RuntimeError::TypeMismatch)
            }
        }
        Value::String(text) => write_string_index(text.as_str(), indices, value, false),
        Value::WString(text) => write_string_index(text.as_str(), indices, value, true),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn read_field(
    ctx: &EvalContext<'_>,
    target: Value,
    field: &SmolStr,
) -> Result<Value, RuntimeError> {
    if let Some(access) = parse_partial_access(field.as_str()) {
        return read_partial_access(&target, access).map_err(partial_access_error_to_runtime);
    }
    match target {
        Value::Struct(struct_value) => struct_value
            .fields
            .get(field)
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedField(field.clone())),
        Value::Instance(id) => ctx
            .storage
            .get_instance_var_recursive(id, field.as_ref())
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedField(field.clone())),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn write_field(
    ctx: &mut EvalContext<'_>,
    target: Value,
    field: &SmolStr,
    value: Value,
) -> Result<Value, RuntimeError> {
    if let Some(access) = parse_partial_access(field.as_str()) {
        return write_partial_access(target, access, value)
            .map_err(partial_access_error_to_runtime);
    }
    match target {
        Value::Struct(mut struct_value) => {
            let struct_value_mut = std::sync::Arc::make_mut(&mut struct_value);
            if struct_value_mut.fields.contains_key(field) {
                struct_value_mut.fields.insert(field.clone(), value);
                Ok(Value::Struct(struct_value))
            } else {
                Err(RuntimeError::UndefinedField(field.clone()))
            }
        }
        Value::Instance(id) => {
            let Some(reference) = ctx.storage.ref_for_instance_recursive(id, field.as_ref()) else {
                return Err(RuntimeError::UndefinedField(field.clone()));
            };
            if ctx.storage.write_by_ref(reference, value) {
                Ok(Value::Instance(id))
            } else {
                Err(RuntimeError::NullReference)
            }
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn index_to_i64(value: Value) -> Result<i64, RuntimeError> {
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

pub(super) fn array_offset(
    dimensions: &[(i64, i64)],
    indices: &[Value],
) -> Result<usize, RuntimeError> {
    if dimensions.len() != indices.len() {
        return Err(RuntimeError::TypeMismatch);
    }
    let mut offset: i128 = 0;
    let mut stride: i128 = 1;
    for ((lower, upper), index_value) in dimensions.iter().zip(indices).rev() {
        let idx = index_to_i64(index_value.clone())?;
        if idx < *lower || idx > *upper {
            return Err(RuntimeError::IndexOutOfBounds {
                index: idx,
                lower: *lower,
                upper: *upper,
            });
        }
        let len = (*upper - *lower + 1) as i128;
        offset += (idx - *lower) as i128 * stride;
        stride *= len;
    }
    usize::try_from(offset).map_err(|_| RuntimeError::TypeMismatch)
}

fn read_string_index(text: &str, indices: &[Value], wide: bool) -> Result<Value, RuntimeError> {
    let Some(position) = single_string_index(indices)? else {
        return Err(RuntimeError::TypeMismatch);
    };
    let Some(ch) = text.chars().nth(position) else {
        return Err(RuntimeError::IndexOutOfBounds {
            index: i64::try_from(position + 1).map_err(|_| RuntimeError::Overflow)?,
            lower: 1,
            upper: i64::try_from(text.chars().count()).map_err(|_| RuntimeError::Overflow)?,
        });
    };
    if wide {
        let code = u16::try_from(ch as u32).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::WChar(code))
    } else {
        let code = u8::try_from(ch as u32).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::Char(code))
    }
}

fn write_string_index(
    text: &str,
    indices: &[Value],
    value: Value,
    wide: bool,
) -> Result<Value, RuntimeError> {
    let Some(position) = single_string_index(indices)? else {
        return Err(RuntimeError::TypeMismatch);
    };
    let mut chars: Vec<char> = text.chars().collect();
    if position >= chars.len() {
        return Err(RuntimeError::IndexOutOfBounds {
            index: i64::try_from(position + 1).map_err(|_| RuntimeError::Overflow)?,
            lower: 1,
            upper: i64::try_from(chars.len()).map_err(|_| RuntimeError::Overflow)?,
        });
    }
    chars[position] = value_to_char(value, wide)?;
    let updated: String = chars.into_iter().collect();
    if wide {
        Ok(Value::WString(updated))
    } else {
        Ok(Value::String(updated.into()))
    }
}

fn single_string_index(indices: &[Value]) -> Result<Option<usize>, RuntimeError> {
    if indices.len() != 1 {
        return Ok(None);
    }
    let index = index_to_i64(indices[0].clone())?;
    if index < 1 {
        return Err(RuntimeError::IndexOutOfBounds {
            index,
            lower: 1,
            upper: i64::MAX,
        });
    }
    usize::try_from(index - 1)
        .map(Some)
        .map_err(|_| RuntimeError::Overflow)
}

fn value_to_char(value: Value, wide: bool) -> Result<char, RuntimeError> {
    let code = match value {
        Value::Char(code) => u32::from(code),
        Value::WChar(code) => u32::from(code),
        Value::String(text) => {
            let mut chars = text.chars();
            let ch = chars.next().ok_or(RuntimeError::TypeMismatch)?;
            if chars.next().is_some() {
                return Err(RuntimeError::TypeMismatch);
            }
            return Ok(ch);
        }
        Value::WString(text) => {
            let mut chars = text.chars();
            let ch = chars.next().ok_or(RuntimeError::TypeMismatch)?;
            if chars.next().is_some() {
                return Err(RuntimeError::TypeMismatch);
            }
            return Ok(ch);
        }
        _ => return Err(RuntimeError::TypeMismatch),
    };
    if !wide && code > u32::from(u8::MAX) {
        return Err(RuntimeError::Overflow);
    }
    std::char::from_u32(code).ok_or(RuntimeError::TypeMismatch)
}

fn partial_access_error_to_runtime(err: PartialAccessError) -> RuntimeError {
    match err {
        PartialAccessError::IndexOutOfBounds {
            index,
            lower,
            upper,
        } => RuntimeError::IndexOutOfBounds {
            index,
            lower,
            upper,
        },
        PartialAccessError::TypeMismatch => RuntimeError::TypeMismatch,
    }
}

pub(super) fn eval_indices(
    ctx: &mut EvalContext<'_>,
    indices: &[Expr],
) -> Result<Vec<Value>, RuntimeError> {
    let mut values = Vec::with_capacity(indices.len());
    for expr in indices {
        values.push(super::eval::eval_expr(ctx, expr)?);
    }
    Ok(values)
}
