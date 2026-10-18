use crate::error::RuntimeError;
use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::memory::{InstanceId, MemoryLocation, VariableStorage};
use crate::value::Value;

use super::BuiltinExecContext;

/// The view of member `name` of `instance_id`, read by its ordinal among the members the instance
/// holds (C1-9d2a, item (d)): the member's reference names its ordinal, and the slot is read by it.
/// `None` when the instance holds no such member.
pub(super) fn member_view<'a>(
    storage: &'a VariableStorage,
    instance_id: InstanceId,
    name: &str,
) -> Result<Option<ValueView<'a>>, RuntimeError> {
    let Some(member) = storage.ref_for_instance(instance_id, name) else {
        return Ok(None);
    };
    storage
        .read_slot(
            MemoryLocation::Instance(instance_id),
            member.offset as usize,
        )
        .map(Some)
        .map_err(|error| error.into_runtime_error())
}

/// The scalar of member `name` (C1-9d2a, item (d)): `Value::Null` when the instance holds no such
/// member, as a member that holds Null reads; `TypeMismatch` when the member holds no scalar.
pub(super) fn read_scalar_or_null(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Result<Value, RuntimeError> {
    match member_view(ctx.storage, instance_id, name)? {
        Some(view) => view.scalar()?.ok_or(RuntimeError::TypeMismatch),
        None => Ok(Value::Null),
    }
}

pub(super) fn read_bool(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Result<bool, RuntimeError> {
    let Some(view) = member_view(ctx.storage, instance_id, name)? else {
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
    let current = match member_view(ctx.storage, instance_id, name)? {
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
    member_view(ctx.storage, instance_id, name)?
        .ok_or(RuntimeError::TypeMismatch)?
        .materialize()
}

pub(super) fn read_value_or_null(
    ctx: &BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
) -> Result<Value, RuntimeError> {
    match member_view(ctx.storage, instance_id, name)? {
        Some(view) => view.materialize(),
        None => Ok(Value::Null),
    }
}

pub(super) fn set_instance_value(
    ctx: &mut BuiltinExecContext<'_>,
    instance_id: InstanceId,
    name: &str,
    value: Value,
) {
    ctx.storage.write_instance_var(instance_id, name, value);
}
