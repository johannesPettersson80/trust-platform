use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::value::Value;

use super::BuiltinExecContext;

pub(super) fn read_bool(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Result<bool, RuntimeError> {
    match ctx.storage.get_instance_var(instance_id, name) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(Value::Null) | None => Ok(false),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn get_or_init_bool(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
    default: bool,
) -> Result<bool, RuntimeError> {
    match ctx.storage.get_instance_var(instance_id, name) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(Value::Null) | None => {
            ctx.storage
                .set_instance_var(instance_id, name, Value::Bool(default));
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
        .set_instance_var(instance_id, name, Value::Bool(value));
}

pub(super) fn read_value(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Result<Value, RuntimeError> {
    ctx.storage
        .get_instance_var(instance_id, name)
        .cloned()
        .ok_or(RuntimeError::TypeMismatch)
}

pub(super) fn read_value_or_null(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Value {
    ctx.storage
        .get_instance_var(instance_id, name)
        .cloned()
        .unwrap_or(Value::Null)
}

pub(super) fn set_instance_value(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
    value: Value,
) {
    ctx.storage.set_instance_var(instance_id, name, value);
}
