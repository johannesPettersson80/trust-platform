use crate::error::RuntimeError;
use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::memory::InstanceId;
use crate::value::Value;

use super::members::Member;
use super::BuiltinExecContext;

/// The view of `member` of `instance_id`, the variable the instance declares itself under the
/// member's name. With the places preparation fixed for the block's POU, the member is read where it
/// lies in the instance's bytes, and a member the POU does not declare is none (checklist P1-4);
/// otherwise it is found by its name. `None` when the instance holds no such member. The store is
/// whichever one holds the instance: the `Value` storage, or the engine's routed store, which finds a
/// persistent instance in its cells (C1-9d3a).
pub(super) fn member_view<'a, S: StoreAccess>(
    ctx: &'a BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
) -> Result<Option<ValueView<'a>>, RuntimeError> {
    Ok(match ctx.places {
        Some(places) => places
            .member(member)
            .and_then(|(range, type_id, presence)| {
                ctx.storage.read_place(range, type_id, presence)
            }),
        None => ctx.storage.read_instance_var(instance_id, member.name()),
    })
}

/// The scalar of `member` (C1-9d2a, item (d)): `Value::Null` when the instance holds no such
/// member, as a member that holds Null reads; `TypeMismatch` when the member holds no scalar.
pub(super) fn read_scalar_or_null<S: StoreAccess>(
    ctx: &BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
) -> Result<Value, RuntimeError> {
    match member_view(ctx, instance_id, member)? {
        Some(view) => view.scalar()?.ok_or(RuntimeError::TypeMismatch),
        None => Ok(Value::Null),
    }
}

pub(super) fn read_bool<S: StoreAccess>(
    ctx: &BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
) -> Result<bool, RuntimeError> {
    let Some(view) = member_view(ctx, instance_id, member)? else {
        return Ok(false);
    };
    match view.scalar()? {
        Some(Value::Bool(value)) => Ok(value),
        Some(Value::Null) => Ok(false),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn get_or_init_bool<S: StoreAccess + StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
    default: bool,
) -> Result<bool, RuntimeError> {
    let current = match member_view(ctx, instance_id, member)? {
        Some(view) => view.scalar()?,
        None => Some(Value::Null),
    };
    match current {
        Some(Value::Bool(value)) => Ok(value),
        Some(Value::Null) => {
            write_member(ctx, instance_id, member, Value::Bool(default));
            Ok(default)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn write_bool<S: StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
    value: bool,
) {
    write_member(ctx, instance_id, member, Value::Bool(value));
}

pub(super) fn read_value<S: StoreAccess>(
    ctx: &BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
) -> Result<Value, RuntimeError> {
    member_view(ctx, instance_id, member)?
        .ok_or(RuntimeError::TypeMismatch)?
        .materialize()
}

pub(super) fn read_value_or_null<S: StoreAccess>(
    ctx: &BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
) -> Result<Value, RuntimeError> {
    match member_view(ctx, instance_id, member)? {
        Some(view) => view.materialize(),
        None => Ok(Value::Null),
    }
}

pub(super) fn set_instance_value<S: StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
    value: Value,
) {
    write_member(ctx, instance_id, member, value);
}

/// Write `value` to `member`, at the place preparation fixed for the block's POU when there is one,
/// as [`member_view`] finds it (checklist P1-4). Places are fixed only for a persistent instance,
/// whose cells hold no place for a member its POU does not declare, so such a member is written
/// nowhere, as the name lookup writes it nowhere.
fn write_member<S: StoreMutate>(
    ctx: &mut BuiltinExecContext<'_, S>,
    instance_id: InstanceId,
    member: Member,
    value: Value,
) {
    match ctx.places {
        Some(places) => {
            if let Some((range, type_id, presence)) = places.member(member) {
                ctx.storage.write_place(range, type_id, presence, value);
            }
        }
        None => {
            ctx.storage
                .write_instance_var(instance_id, member.name(), value);
        }
    }
}
