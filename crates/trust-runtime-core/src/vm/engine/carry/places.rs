//! Where a carried value lies (specification 34 §11.3): the declarations of the running program that
//! the candidate's continue, by name, and the cells of instances, members, structure fields and array
//! elements in each program's flat store.

use super::super::EngineState;
use super::shape;
use crate::bytecode::{StorageDeclaration, StorageOwner, StorageRole, TypeData};
use crate::error::RuntimeError;
use crate::memory::layout::ByteRange;
use crate::memory::InstanceId;
use crate::vm::PreparedModule;

/// Whether `declaration` holds a value an online change carries: a variable, an input or output of
/// an instance (role `Parameter` in its template), a static, an edge's last input or a native-state
/// slot that owns typed storage, and is neither a constant nor an in-out member.
pub(super) fn carried(declaration: &StorageDeclaration) -> bool {
    const CONSTANT: u8 = 1;
    const IN_OUT: u8 = 1 << 3;
    matches!(
        declaration.role,
        StorageRole::Variable
            | StorageRole::Parameter
            | StorageRole::Static
            | StorageRole::EdgePhase
            | StorageRole::NativeState
    ) && declaration.type_id.is_some()
        && declaration.owns_storage()
        && declaration.flags & (CONSTANT | IN_OUT) == 0
}

/// Whether a member of role `old` and one of role `new` hold the same kind of value: a variable, an
/// input and an output are one kind (a member moved between `VAR`, `VAR_INPUT` and `VAR_OUTPUT`
/// keeps its value), and a static, an edge's last input and a native-state slot each its own.
pub(super) fn same_kind(old: StorageRole, new: StorageRole) -> bool {
    let value = |role| matches!(role, StorageRole::Variable | StorageRole::Parameter);
    old == new || (value(old) && value(new))
}

/// The running program's global declaration that `declaration`, a global or a function static of the
/// candidate, continues: a global of the same name, or a static of the same name in a function of the
/// same name.
pub(super) fn matching_global(
    before: &PreparedModule,
    after: &PreparedModule,
    declaration: &StorageDeclaration,
) -> Option<u32> {
    let name = declared_name(after, declaration).ok()?;
    let owner = match declaration.owner_pou_id {
        Some(function) => Some(pou_name(after, function).ok()?),
        None => None,
    };
    before
        .layout
        .entries
        .iter()
        .enumerate()
        .find(|(_, old)| {
            old.owner == StorageOwner::Global
                && old.role == declaration.role
                && carried(old)
                && declared_name(before, old)
                    .is_ok_and(|old_name| old_name.eq_ignore_ascii_case(name))
                && match (owner, old.owner_pou_id) {
                    (None, None) => true,
                    (Some(owner), Some(old_owner)) => pou_name(before, old_owner)
                        .is_ok_and(|old_owner| old_owner.eq_ignore_ascii_case(owner)),
                    _ => false,
                }
        })
        .and_then(|(index, _)| u32::try_from(index).ok())
}

/// The declared name of `declaration`: its source name when it has one (a static), else its storage
/// name.
pub(super) fn declared_name<'n>(
    prepared: &'n PreparedModule,
    declaration: &StorageDeclaration,
) -> Result<&'n str, RuntimeError> {
    let index = declaration.source_name_idx.unwrap_or(declaration.name_idx);
    prepared
        .vm
        .strings
        .get(index as usize)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The name of POU `pou` of `prepared`.
pub(super) fn pou_name(prepared: &PreparedModule, pou: u32) -> Result<&str, RuntimeError> {
    prepared
        .vm
        .pou_name(pou)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The template, the cells and the base of the persistent instance `instance` of `state`.
pub(super) fn persistent(
    state: &EngineState<'_>,
    instance: InstanceId,
) -> Result<(u32, ByteRange, Option<InstanceId>), RuntimeError> {
    let persistent = state
        .data
        .store
        .instance(instance)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    Ok((persistent.pou, persistent.range, persistent.parent))
}

/// The cells of member declaration `id` of the instance whose own cells are `instance`.
pub(super) fn member_range(
    prepared: &PreparedModule,
    instance: ByteRange,
    id: u32,
) -> Result<ByteRange, RuntimeError> {
    let declaration = prepared
        .layout
        .entries
        .get(id as usize)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    let type_id = declaration
        .type_id
        .ok_or(RuntimeError::InvalidExecutionState)?;
    Ok(ByteRange {
        start: instance
            .start
            .checked_add(
                prepared
                    .declaration_offset(id)
                    .ok_or(RuntimeError::InvalidExecutionState)?,
            )
            .ok_or(RuntimeError::Overflow)?,
        bytes: prepared
            .type_layout(type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .shape
            .bytes,
        ..instance
    })
}

/// The cells of field `ordinal` of the structure of `type_id` at `range`.
pub(super) fn field_range(
    prepared: &PreparedModule,
    type_id: u32,
    range: ByteRange,
    ordinal: usize,
) -> Result<ByteRange, RuntimeError> {
    let structure = shape::resolved(prepared, type_id).ok_or(RuntimeError::TypeMismatch)?;
    let member = prepared
        .type_layout(structure)
        .and_then(|layout| layout.members.get(ordinal))
        .ok_or(RuntimeError::InvalidExecutionState)?;
    let field = match shape::data(prepared, type_id) {
        Some(TypeData::Struct { fields }) => fields.get(ordinal),
        _ => None,
    }
    .ok_or(RuntimeError::InvalidExecutionState)?;
    Ok(ByteRange {
        start: range
            .start
            .checked_add(member.byte_offset)
            .ok_or(RuntimeError::Overflow)?,
        bytes: prepared
            .type_layout(field.type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .shape
            .bytes,
        ..range
    })
}

/// The byte stride of the elements of the array of `type_id`.
pub(super) fn stride(prepared: &PreparedModule, type_id: u32) -> Result<u32, RuntimeError> {
    let array = shape::resolved(prepared, type_id).ok_or(RuntimeError::TypeMismatch)?;
    prepared
        .type_layout(array)
        .and_then(|layout| layout.array.as_ref())
        .map(|array| array.byte_stride)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The cells of the element at flat `offset` of an array whose elements are `element` and whose
/// stride is `stride`, at `range`.
pub(super) fn element_range(
    prepared: &PreparedModule,
    element: u32,
    range: ByteRange,
    offset: usize,
    stride: u32,
) -> Result<ByteRange, RuntimeError> {
    let offset = u32::try_from(offset).map_err(|_| RuntimeError::Overflow)?;
    Ok(ByteRange {
        start: range
            .start
            .checked_add(offset.checked_mul(stride).ok_or(RuntimeError::Overflow)?)
            .ok_or(RuntimeError::Overflow)?,
        bytes: prepared
            .type_layout(element)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .shape
            .bytes,
        ..range
    })
}

/// The number of elements from `lower` to `upper`, inclusive.
pub(super) fn element_count(lower: i64, upper: i64) -> Result<u32, RuntimeError> {
    upper
        .checked_sub(lower)
        .and_then(|span| span.checked_add(1))
        .and_then(|count| u32::try_from(count).ok())
        .ok_or(RuntimeError::Overflow)
}

/// The number of elements of an array of `dims`.
pub(super) fn element_total(dims: &[(i64, i64)]) -> Option<i64> {
    dims.iter().try_fold(1i64, |total, (lower, upper)| {
        total.checked_mul(upper.checked_sub(*lower)?.checked_add(1)?)
    })
}

/// Step `indices` to the next element of an array of `dims` in flat order (the last index fastest),
/// or `false` after the last one.
pub(super) fn next_index(indices: &mut [i64], dims: &[(i64, i64)]) -> bool {
    for (index, (lower, upper)) in indices.iter_mut().zip(dims).rev() {
        if *index < *upper {
            *index += 1;
            return true;
        }
        *index = *lower;
    }
    false
}

/// Whether a value of `type_id` holds an instance or a reference leaf.
pub(super) fn holds_leaves(prepared: &PreparedModule, type_id: u32) -> bool {
    prepared
        .type_layout(type_id)
        .is_none_or(|layout| layout.holds_reference_or_instance)
}

/// Copy the bytes `source` of the running program's store into the bytes `at` of the candidate's.
pub(super) fn copy(
    from: &EngineState<'_>,
    source: ByteRange,
    target: &mut EngineState<'_>,
    at: ByteRange,
) -> Result<(), RuntimeError> {
    let bytes = from
        .data
        .reader(from.prepared)
        .view_cells()
        .bytes(source)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    if target.data.store.write_bytes(at, bytes) {
        Ok(())
    } else {
        Err(RuntimeError::InvalidExecutionState)
    }
}
