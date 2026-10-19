//! The types whose values the flat store copies as bytes (specification 34 MEM-12; C1-10c, C1-12,
//! C1-13): which they are, and when two of them have the same bytes for the same value. The engine's
//! places and the in-place initializers ask these questions, and preparation asks them when it
//! reserves the activation stack, so the answers come from this one place.

use crate::bytecode::{TypeData, TypeEntry, TypeKind, BYTECODE_MAX_TYPE_NESTING};
use crate::vm::PreparedModule;

/// The `TYPE_TABLE` primitive ids of `STRING` and `WSTRING`.
pub(crate) const TEXT_PRIMITIVES: [u16; 2] = [24, 25];

/// The entry `type_id` names, with its aliases resolved.
pub(crate) fn resolve(prepared: &PreparedModule, type_id: u32) -> Option<&TypeEntry> {
    let mut current = type_id;
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        let entry = prepared.type_table().entries.get(current as usize)?;
        match &entry.data {
            TypeData::Alias { target_type_id } => current = *target_type_id,
            _ => return Some(entry),
        }
    }
    None
}

/// Whether a value of `type_id` is an array or a structure: a `Value` that the lifetime walk enters.
pub(crate) fn is_compound(prepared: &PreparedModule, type_id: u32) -> bool {
    resolve(prepared, type_id).is_some_and(|entry| {
        matches!(
            entry.data,
            TypeData::Array { .. } | TypeData::Struct { .. } | TypeData::Union { .. }
        )
    })
}

/// Whether a store may copy a value of `type_id` as its bytes: a text, or an array or a structure
/// whose leaves are numbers, bits, characters, texts, times, dates and enumerations. A text of the
/// destination's type fits its capacity, so its bytes, the tail zeroed, are the bytes the
/// destination would hold (C1-12). A reference and an instance are checked and placed by the value
/// they hold, and an overlapping or located structure is written through its physical layout
/// (C1-15), so a value that holds any of them is stored as a value.
pub(crate) fn copies_as_bytes(prepared: &PreparedModule, type_id: u32) -> bool {
    resolve(prepared, type_id).is_some_and(|entry| {
        matches!(entry.data, TypeData::Array { .. } | TypeData::Struct { .. })
            && plain(prepared, type_id, 0)
            || text_primitive(prepared, type_id).is_some()
    })
}

/// Whether a value of `type_id` holds a text.
pub(crate) fn holds_text(prepared: &PreparedModule, type_id: u32) -> bool {
    prepared
        .type_layout(type_id)
        .is_some_and(|layout| !layout.text_leaves.is_empty())
}

/// The primitive id and declared length of the text type `type_id`, or `None` for another type.
pub(crate) fn text_primitive(prepared: &PreparedModule, type_id: u32) -> Option<(u16, u16)> {
    match resolve(prepared, type_id)?.data {
        TypeData::Primitive {
            prim_id,
            max_length,
        } if TEXT_PRIMITIVES.contains(&prim_id) => Some((prim_id, max_length)),
        _ => None,
    }
}

/// Whether every leaf of a value of `type_id` is a scalar or a text its bytes hold whole.
fn plain(prepared: &PreparedModule, type_id: u32, depth: usize) -> bool {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return false;
    }
    let Some(entry) = resolve(prepared, type_id) else {
        return false;
    };
    match &entry.data {
        TypeData::Primitive { .. } => true,
        TypeData::Enum { .. } => true,
        TypeData::Subrange { base_type_id, .. } => plain(prepared, *base_type_id, depth + 1),
        TypeData::Array { elem_type_id, .. } => plain(prepared, *elem_type_id, depth + 1),
        TypeData::Struct { fields } => {
            !matches!(
                entry.kind,
                TypeKind::OverlappingStruct | TypeKind::LocatedStruct
            ) && fields
                .iter()
                .all(|field| plain(prepared, field.type_id, depth + 1))
        }
        _ => false,
    }
}

/// Whether values of `left` and `right` have the same bytes for the same value: the same type, or
/// arrays of the same bounds whose elements are.
pub(crate) fn same_type(prepared: &PreparedModule, left: u32, right: u32, depth: usize) -> bool {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return false;
    }
    let (Some(left_entry), Some(right_entry)) = (resolve(prepared, left), resolve(prepared, right))
    else {
        return false;
    };
    if core::ptr::eq(left_entry, right_entry) {
        return true;
    }
    match (&left_entry.data, &right_entry.data) {
        (
            TypeData::Array {
                elem_type_id: left_element,
                dims: left_dims,
            },
            TypeData::Array {
                elem_type_id: right_element,
                dims: right_dims,
            },
        ) => {
            left_dims == right_dims && same_type(prepared, *left_element, *right_element, depth + 1)
        }
        _ => false,
    }
}

/// The number of elements of an array of `dims`.
pub(crate) fn element_count(dims: &[(i64, i64)]) -> Option<usize> {
    dims.iter().try_fold(1usize, |count, (lower, upper)| {
        let length = upper.checked_sub(*lower)?.checked_add(1)?;
        count.checked_mul(usize::try_from(length).ok()?)
    })
}
