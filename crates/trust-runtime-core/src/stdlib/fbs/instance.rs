use crate::error::RuntimeError;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::{InstanceId, VariableStorage};
use crate::value::Value;

use super::BuiltinExecContext;

/// The stored `Value` of one instance variable. The built-in executors hold the `Value` store, so
/// the view always holds its `Value` (C1-9d1); the other built-in modules still read it this way.
pub(super) fn instance_var<'a>(
    storage: &'a VariableStorage,
    instance_id: InstanceId,
    name: &str,
) -> Option<&'a Value> {
    storage
        .read_instance_var(instance_id, name)?
        .borrowed_value()
}

pub(super) fn read_bool(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Result<bool, RuntimeError> {
    let Some(view) = ctx.storage.read_instance_var(instance_id, name) else {
        return Ok(false);
    };
    match view.scalar()? {
        Some(Value::Bool(value)) => Ok(value),
        Some(Value::Null) => Ok(false),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn get_or_init_bool(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
    default: bool,
) -> Result<bool, RuntimeError> {
    let current = match ctx.storage.read_instance_var(instance_id, name) {
        Some(view) => view.scalar()?,
        None => Some(Value::Null),
    };
    match current {
        Some(Value::Bool(value)) => Ok(value),
        Some(Value::Null) => {
            ctx.storage
                .write_instance_var(instance_id, name, Value::Bool(default));
            Ok(default)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn write_bool(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
    value: bool,
) {
    ctx.storage
        .write_instance_var(instance_id, name, Value::Bool(value));
}

pub(super) fn read_value(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Result<Value, RuntimeError> {
    ctx.storage
        .read_instance_var(instance_id, name)
        .ok_or(RuntimeError::TypeMismatch)?
        .materialize()
}

pub(super) fn read_value_or_null(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Value {
    instance_var(ctx.storage, instance_id, name)
        .cloned()
        .unwrap_or(Value::Null)
}

pub(super) fn set_instance_value(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
    value: Value,
) {
    ctx.storage.write_instance_var(instance_id, name, value);
}
