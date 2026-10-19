//! The bytes of one value, classified for the retained encoder (specification 11 §6.7, specification
//! 34 MEM-12; C1-9d2b-2).
//!
//! `leaf_in` answers what `read_value_in` builds from the same bytes, as the parts of the value
//! instead of the value: a scalar decoded from its leaf, a text as its characters in place, the
//! names of an enumeration value, and the shape of a structure or an array, whose members and
//! elements the caller reaches by the same walk. Nothing is allocated, and the bytes are refused
//! where `read_value_in` refuses them, so an encoder that follows this classification writes the
//! bytes of the value it would have read.

use alloc::format;

use super::read::read_scalar;
use super::{
    check_slice_length, element_count, enumeration_kind, layout_of, refusal, shape, string,
    text_geometry, type_entry, Shape, KIND_REASON, UNION_LEAF, UNION_REASON,
};
use crate::bytecode::TypeData;
use crate::error::RuntimeError;
use crate::memory::cells::Cell;
use crate::memory::flat::{self, LeafKind, TextBytes};
use crate::value::Value;
use crate::vm::PreparedModule;

/// What the bytes of one value hold, as the retained encoder reads them without building a `Value`.
#[derive(Debug, PartialEq)]
pub(crate) enum CellLeaf<'a> {
    /// The null value: an instance leaf that names no instance, or an unbound native counter slot.
    Null,
    /// A scalar other than a text, decoded from its leaf.
    Scalar(Value),
    /// A text: its characters in place, UTF-8 for a `STRING` and little-endian UTF-16 code units for
    /// a `WSTRING`.
    Text {
        /// Whether the leaf holds a `WSTRING`.
        wide: bool,
        /// The bytes in use.
        bytes: &'a [u8],
    },
    /// An enumeration value, with the names its type declares for it.
    Enumeration {
        /// The enumeration type's name.
        type_name: &'a str,
        /// The declared name of the variant that has the value.
        variant: &'a str,
        /// The numeric value.
        value: i64,
    },
    /// A structure of `members` members, in declaration order.
    Structure {
        /// The structure type's name.
        type_name: &'a str,
        /// The number of members.
        members: usize,
    },
    /// An array of `elements` elements, in flat order.
    Array {
        /// The declared bounds of each dimension.
        dimensions: &'a [(i64, i64)],
        /// The number of elements.
        elements: usize,
    },
    /// A reference leaf, null or not.
    Reference,
    /// An instance leaf.
    Instance,
    /// A structure with a physical layout, its backing in the bytes: its members are bit fields of it
    /// or lie beside it (C1-15), so the caller reads its value whole.
    Overlay,
}

/// Classify the bytes of one value of the `TYPE_TABLE` type `type_id`.
///
/// # Errors
///
/// The refusal `read_value_in` gives for the same bytes: a leaf the type does not hold, an
/// enumeration leaf that names no variant of its type, a slice that is not the layout's length,
/// and a union (C1-15).
pub(crate) fn leaf_in<'a>(
    bytes: &'a [u8],
    type_id: u32,
    prepared: &'a PreparedModule,
) -> Result<CellLeaf<'a>, RuntimeError> {
    check_slice_length(bytes.len(), type_id, prepared)?;
    match shape(prepared, type_id)? {
        Shape::Scalar(prim_id) => Ok(match read_scalar(type_id, prim_id, bytes, prepared)? {
            Value::Null => CellLeaf::Null,
            value => CellLeaf::Scalar(value),
        }),
        Shape::Enumeration(enum_type) => enumeration_leaf(type_id, enum_type, bytes, prepared),
        Shape::Structure(struct_type) => structure_leaf(type_id, struct_type, prepared),
        Shape::Array(_) => array_leaf(type_id, prepared),
        Shape::Reference => Ok(CellLeaf::Reference),
        Shape::Instance => Ok(match flat::decode(LeafKind::Instance, bytes)? {
            Cell::Instance(_) => CellLeaf::Instance,
            _ => CellLeaf::Null,
        }),
        Shape::Text(_) => {
            let (length, _, wide) =
                text_geometry(prepared, type_id).ok_or(RuntimeError::InvalidExecutionState)?;
            let bytes = match flat::read_text(bytes, length, wide)? {
                TextBytes::Narrow(text) => text.as_bytes(),
                TextBytes::Wide(units) => units,
            };
            Ok(CellLeaf::Text { wide, bytes })
        }
        Shape::Overlay(_) => Ok(CellLeaf::Overlay),
        Shape::Union => Err(refusal(prepared, type_id, UNION_LEAF, UNION_REASON)),
    }
}

/// The declared name of member `ordinal`, in declaration order, of the structure type `type_id`.
///
/// # Errors
///
/// [`RuntimeError::InvalidExecutionState`] when the type has no such member.
pub(crate) fn structure_member_name(
    prepared: &PreparedModule,
    type_id: u32,
    ordinal: usize,
) -> Result<&str, RuntimeError> {
    let member = layout_of(prepared, type_id)?
        .members
        .get(ordinal)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    string(prepared, member.name_idx)
}

/// The name a value of the type carries: its `STRING_TABLE` name, or the empty name of an anonymous type.
fn type_name_of(prepared: &PreparedModule, type_id: u32) -> &str {
    prepared
        .type_table()
        .entries
        .get(type_id as usize)
        .and_then(|entry| entry.name_idx)
        .and_then(|name_idx| prepared.strings().get(name_idx as usize))
        .unwrap_or("")
}

/// The enumeration value one leaf holds, with the names its type declares for it.
fn enumeration_leaf<'a>(
    type_id: u32,
    enum_type: u32,
    bytes: &[u8],
    prepared: &'a PreparedModule,
) -> Result<CellLeaf<'a>, RuntimeError> {
    let TypeData::Enum { variants, .. } = &type_entry(prepared, enum_type)?.data else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let Cell::Enum { value, .. } = flat::decode(enumeration_kind(prepared, enum_type)?, bytes)?
    else {
        return Err(refusal(prepared, type_id, "leaf", KIND_REASON));
    };
    let Some(variant) = variants.iter().find(|variant| variant.value == value) else {
        return Err(refusal(
            prepared,
            enum_type,
            "value",
            &format!("the enumeration has no variant with the value {value}"),
        ));
    };
    Ok(CellLeaf::Enumeration {
        type_name: type_name_of(prepared, enum_type),
        variant: string(prepared, variant.name_idx)?,
        value,
    })
}

/// The structure of one type: its name and its number of members, which its layout and its declaration
/// must agree on.
fn structure_leaf(
    type_id: u32,
    struct_type: u32,
    prepared: &PreparedModule,
) -> Result<CellLeaf<'_>, RuntimeError> {
    let layout = layout_of(prepared, type_id)?;
    let TypeData::Struct { fields } = &type_entry(prepared, struct_type)?.data else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    if layout.members.len() != fields.len() {
        return Err(RuntimeError::InvalidExecutionState);
    }
    Ok(CellLeaf::Structure {
        type_name: type_name_of(prepared, struct_type),
        members: fields.len(),
    })
}

/// The array of one type: its bounds and its number of elements.
fn array_leaf(type_id: u32, prepared: &PreparedModule) -> Result<CellLeaf<'_>, RuntimeError> {
    let Some(array) = layout_of(prepared, type_id)?.array.as_ref() else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let elements = element_count(&array.dims).ok_or(RuntimeError::InvalidExecutionState)?;
    Ok(CellLeaf::Array {
        dimensions: &array.dims,
        elements,
    })
}
