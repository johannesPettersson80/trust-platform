//! The cells of one value, classified for the retained encoder (specification 11 §6.7, specification
//! 34 §6.2, MEM-10; C1-9d2b-2).
//!
//! `leaf_in` answers what `read_value_in` builds from the same cells, as the parts of the value instead
//! of the value: a scalar copied out of its cell, a text as the bytes of its buffer, the names of an
//! enumeration value, and the shape of a structure or an array, whose members and elements the caller
//! reaches by the same walk. Nothing is allocated, and the cells are refused where `read_value_in`
//! refuses them, so an encoder that follows this classification writes the bytes of the value it would
//! have read.

use alloc::format;

use smol_str::SmolStr;

use super::read::read_scalar;
use super::{
    check_slice_length, element_count, layout_of, refusal, shape, string, type_entry, Shape,
    KIND_REASON, OVERLAP_LEAF, OVERLAP_REASON, REFERENCE_LEAF, REFERENCE_TARGET_REASON, UNION_LEAF,
    UNION_REASON, WSTRING_PRIMITIVE,
};
use crate::bytecode::TypeData;
use crate::error::RuntimeError;
use crate::memory::cells::Cell;
use crate::memory::text::TextStore;
use crate::value::Value;
use crate::vm::PreparedModule;

/// What the cells of one value hold, as the retained encoder reads them without building a `Value`.
#[derive(Debug, PartialEq)]
pub(crate) enum CellLeaf<'a> {
    /// Every cell is empty: the null value, which `read_value_in` reads for any type.
    Null,
    /// A scalar other than a text, copied out of its cell.
    Scalar(Value),
    /// A text: the bytes of its buffer, UTF-8 for a `STRING` and little-endian UTF-16 code units for a
    /// `WSTRING`.
    Text {
        /// Whether the buffer holds a `WSTRING`.
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
}

/// Classify the cells of one value of the `TYPE_TABLE` type `type_id`, with the region's text buffers
/// `text`.
///
/// # Errors
///
/// The refusal `read_value_in` gives for the same cells: a cell the type does not hold, an enumeration
/// cell that names no variant of its type, a slice that is not the layout's length, and a `STRUCT OVERLAP`
/// or a union (C1-15).
pub(crate) fn leaf_in<'a>(
    cells: &'a [Cell],
    type_id: u32,
    prepared: &'a PreparedModule,
    text: &'a TextStore,
) -> Result<CellLeaf<'a>, RuntimeError> {
    check_slice_length(cells.len(), type_id, prepared)?;
    if cells.iter().all(|cell| *cell == Cell::Null) {
        return Ok(CellLeaf::Null);
    }
    match shape(prepared, type_id)? {
        Shape::Scalar(prim_id) => {
            read_scalar(type_id, prim_id, cells, prepared).map(CellLeaf::Scalar)
        }
        Shape::Enumeration(enum_type) => enumeration_leaf(type_id, enum_type, cells, prepared),
        Shape::Structure(struct_type) => structure_leaf(type_id, struct_type, prepared),
        Shape::Array(_) => array_leaf(type_id, cells.len(), prepared),
        Shape::Reference => match cells.first() {
            Some(Cell::NullRef | Cell::Ref(_)) => Ok(CellLeaf::Reference),
            Some(_) => Err(refusal(
                prepared,
                type_id,
                REFERENCE_LEAF,
                REFERENCE_TARGET_REASON,
            )),
            None => Err(RuntimeError::InvalidExecutionState),
        },
        Shape::Instance => match cells.first() {
            Some(Cell::Instance(_)) => Ok(CellLeaf::Instance),
            Some(_) => Err(refusal(prepared, type_id, "cell", KIND_REASON)),
            None => Err(RuntimeError::InvalidExecutionState),
        },
        Shape::Text(prim_id) => {
            let Some(Cell::Text(slot)) = cells.first() else {
                return Err(refusal(prepared, type_id, "cell", KIND_REASON));
            };
            let bytes = text
                .bytes(*slot)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            Ok(CellLeaf::Text {
                wide: prim_id == WSTRING_PRIMITIVE,
                bytes,
            })
        }
        Shape::Overlay => Err(refusal(prepared, type_id, OVERLAP_LEAF, OVERLAP_REASON)),
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
        .map_or("", SmolStr::as_str)
}

/// The enumeration value one cell holds, with the names its type declares for it.
fn enumeration_leaf<'a>(
    type_id: u32,
    enum_type: u32,
    cells: &[Cell],
    prepared: &'a PreparedModule,
) -> Result<CellLeaf<'a>, RuntimeError> {
    let TypeData::Enum { variants, .. } = &type_entry(prepared, enum_type)?.data else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let Some(Cell::Enum { type_idx, value }) = cells.first() else {
        return Err(refusal(prepared, type_id, "cell", KIND_REASON));
    };
    if *type_idx != enum_type {
        return Err(refusal(prepared, type_id, "cell", KIND_REASON));
    }
    let Some(variant) = variants.iter().find(|variant| variant.value == *value) else {
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
        value: *value,
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

/// The array of one type: its bounds and its number of elements, which `cell_count` cells of the type's
/// stride must hold exactly.
fn array_leaf(
    type_id: u32,
    cell_count: usize,
    prepared: &PreparedModule,
) -> Result<CellLeaf<'_>, RuntimeError> {
    let Some(array) = layout_of(prepared, type_id)?.array.as_ref() else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let expected = element_count(&array.dims).ok_or(RuntimeError::InvalidExecutionState)?;
    let stride = usize::try_from(array.stride).map_err(|_| RuntimeError::Overflow)?;
    if stride == 0 || cell_count / stride != expected {
        return Err(RuntimeError::InvalidExecutionState);
    }
    Ok(CellLeaf::Array {
        dimensions: &array.dims,
        elements: expected,
    })
}
