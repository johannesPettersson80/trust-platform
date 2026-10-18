//! Converting a value of a prepared type into its layout's cells and back (specification 34 §6.2,
//! MEM-10; design "Boundaries": an engineering write converts a `Value` into the destination cells
//! without allocating, and a read materializes a `Value`).
//!
//! A [`TypeLayout`](crate::memory::layout::TypeLayout) fixes where a structure's members and an
//! array's elements sit in cells, so every conversion below is driven by that layout and by the
//! `TYPE_TABLE` entry: a member's offset, an element's stride and an enumeration's variants are
//! taken from the prepared metadata rather than computed a second time.
//!
//! - [`write_value`](crate::memory::cells::write_value) checks the whole value against the layout
//!   and then writes it, so a refused write leaves the cells as they were and allocates nothing;
//! - [`read_value`](crate::memory::cells::read_value) materializes the value of a cell range and
//!   refuses a cell the type's layout does not describe;
//! - [`cell_offset`](crate::memory::cells::cell_offset) maps a member-and-index path to a cell
//!   offset within a value of its type (C1-10).
//!
//! Three leaves are not converted in this slice, and each refusal names the type and the reason: a
//! text leaf (`STRING`, `WSTRING`), which gets its text buffers in C1-7; a non-null reference,
//! whose target is the reference record a store holds (C1-9); and a `STRUCT OVERLAP` or a union,
//! whose members share their cells (C1-15).

use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;

use smol_str::SmolStr;

use super::Cell;
use crate::bytecode::{Field, TypeData, TypeEntry, TypeKind, BYTECODE_MAX_TYPE_NESTING};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::memory::layout::TypeLayout;
use crate::value::{ArrayValue, EnumValue, RefSegment, StructValue, Value};
use crate::vm::PreparedModule;

/// The kind-mismatch reason: the value is one kind and the type's primitive encoding another.
const KIND_REASON: &str = "the value's kind does not match the type";

/// Why a text leaf is refused here (C1-7).
const TEXT_REASON: &str = "text is converted in C1-7";

/// Why a non-null reference is refused here (C1-9).
const REFERENCE_REASON: &str = "a non-null reference is stored in C1-9";

/// Why a reference cell's target is refused here (C1-9).
const REFERENCE_TARGET_REASON: &str = "a reference cell's target is its record, which C1-9 reads";

/// Why a `STRUCT OVERLAP` is refused here (C1-15).
const OVERLAP_REASON: &str =
    "an overlapping structure is written through its byte backing in C1-15";

/// Why a union is refused here (C1-15).
const UNION_REASON: &str = "a union's members share their cells; C1-15 owns shared storage";

/// Why a value of another structure type is refused.
const STRUCTURE_NAME_REASON: &str = "the value's type name is not the structure's";

/// Why a structure member the value does not carry is refused.
const MISSING_MEMBER_REASON: &str = "the value has no member of that name";

/// Why a value member the structure does not declare is refused.
const UNKNOWN_MEMBER_REASON: &str = "the structure has no member of that name";

/// Why an enumeration of another type is refused.
const ENUM_TYPE_REASON: &str = "the value's enumeration is not the type";

/// Why an enumeration value no variant declares is refused.
const ENUM_VARIANT_REASON: &str = "the enumeration has no such variant with that value";

/// The leaf name of a reference cell.
const REFERENCE_LEAF: &str = "REFERENCE";

/// The leaf name of a `STRUCT OVERLAP`.
const OVERLAP_LEAF: &str = "OVERLAP";

/// The leaf name of a union.
const UNION_LEAF: &str = "UNION";

/// The `Cell::Text` primitive encodings (`STRING` and `WSTRING`).
const TEXT_PRIMITIVES: [u16; 2] = [24, 25];

/// Write the value `value` of the `TYPE_TABLE` type `type_id` into `cells`, the cell range of the
/// type's prepared layout (specification 34 §6.2, MEM-10; design "Boundaries").
///
/// `cells` is the whole range: its length is the layout's cell count. Every value kind of this
/// slice is converted — each scalar with its exact bits, an enumeration as its type and value, an
/// instance as its identity, a structure's members and an array's elements at their layout offsets
/// with nested compounds inline, a null reference as an empty reference cell, and
/// [`Value::Null`] as an empty cell.
///
/// # Errors
///
/// Refused with [`RuntimeError::InitFailed`], naming the type, the refused value or leaf and the
/// reason, when a value's kind is not the type's, when a structure is missing a member or carries
/// one the type does not declare, when an array's bounds or element count are not the type's, and
/// when `cells` is not the layout's length. A text leaf (C1-7), a non-null reference (C1-9) and a
/// `STRUCT OVERLAP` or union (C1-15) are refused until their slices convert them. The check runs
/// before the first write, so a refusal leaves the cells as they were, and no path allocates.
pub fn write_value(
    cells: &mut [Cell],
    type_id: u32,
    value: &Value,
    prepared: &PreparedModule,
) -> Result<(), RuntimeError> {
    check_slice_length(cells.len(), type_id, prepared)?;
    check_value(type_id, value, prepared)?;
    store_value(cells, type_id, value, prepared)
}

/// Materialize the value of the `TYPE_TABLE` type `type_id` from `cells`, the cell range of the
/// type's prepared layout (specification 34 §6.2, MEM-10; design "Boundaries").
///
/// The inverse of [`write_value`]: a range [`write_value`] wrote reads back as the value it was
/// given, and a range whose cells are all [`Cell::Null`] reads as
/// [`Value::Null`], the empty value.
///
/// # Errors
///
/// Refused with [`RuntimeError::InitFailed`], naming the type, the refused cell or leaf and the
/// reason, when a cell does not hold a value of the type, when an enumeration cell names no variant
/// of its type, when `cells` is not the layout's length, and for the text leaves (C1-7), non-null
/// references (C1-9) and overlapping structures and unions (C1-15) [`write_value`] refuses.
pub fn read_value(
    cells: &[Cell],
    type_id: u32,
    prepared: &PreparedModule,
) -> Result<Value, RuntimeError> {
    check_slice_length(cells.len(), type_id, prepared)?;
    read_one(cells, type_id, prepared)
}

/// The cell offset, within a value of the `TYPE_TABLE` type `type_id`, of the member-and-index
/// `path`, or `None` for a path the type does not have (specification 34 §6.2; design "Operands and
/// references": C1-10 turns a reference's path into a cell).
///
/// [`RefSegment::Field`] resolves a member of a structure by name, and [`RefSegment::Index`]
/// resolves an array element by its IEC index, honouring the array's declared lower bounds.
#[must_use]
pub fn cell_offset(type_id: u32, path: &[RefSegment], prepared: &PreparedModule) -> Option<u32> {
    let mut offset = 0u32;
    let mut current = type_id;
    for segment in path {
        let (resolved, entry) = resolve(prepared, current).ok()?;
        match segment {
            RefSegment::Field(name) => {
                let TypeData::Struct { fields } = &entry.data else {
                    return None;
                };
                let layout = prepared.type_layout(resolved)?;
                let ordinal = layout.members.iter().position(|member| {
                    string(prepared, member.name_idx)
                        .is_ok_and(|member_name| member_name.eq_ignore_ascii_case(name.as_str()))
                })?;
                offset = offset.checked_add(layout.members.get(ordinal)?.offset)?;
                current = fields.get(ordinal)?.type_id;
            }
            RefSegment::Index(indices) => {
                let TypeData::Array { elem_type_id, dims } = &entry.data else {
                    return None;
                };
                let element = crate::value::array_offset_i64(dims, indices)?;
                let stride = prepared.type_layout(resolved)?.array.as_ref()?.stride;
                offset = offset.checked_add(u32::try_from(element).ok()?.checked_mul(stride)?)?;
                current = *elem_type_id;
            }
        }
    }
    Some(offset)
}

/// How one prepared type holds a value in cells.
enum Shape {
    /// One scalar cell with this primitive encoding.
    Scalar(u16),
    /// A text leaf; its text arrives with C1-7.
    Text,
    /// One reference cell.
    Reference,
    /// One instance cell.
    Instance,
    /// One enumeration cell of this resolved enumeration type.
    Enumeration(u32),
    /// A structure of this resolved structure type's members.
    Structure(u32),
    /// A `STRUCT OVERLAP`, whose members share their cells (C1-15).
    Overlay,
    /// A union, whose members share their cells (C1-15).
    Union,
    /// An array of this element type.
    Array(u32),
}

/// Refuse a cell range that is not exactly the type's layout's cells.
fn check_slice_length(
    cells: usize,
    type_id: u32,
    prepared: &PreparedModule,
) -> Result<(), RuntimeError> {
    let layout = prepared
        .type_layout(type_id)
        .ok_or_else(|| unknown_type(type_id))?;
    if cells != layout.cells as usize {
        return Err(refusal(
            prepared,
            type_id,
            "cells",
            &format!(
                "the slice holds {cells} cells, the type's layout needs {}",
                layout.cells
            ),
        ));
    }
    Ok(())
}

/// Check the whole value against the type before a cell is written, so a refusal writes nothing.
fn check_value(type_id: u32, value: &Value, prepared: &PreparedModule) -> Result<(), RuntimeError> {
    if matches!(value, Value::Null) {
        return Ok(());
    }
    let layout = layout_of(prepared, type_id)?;
    match shape(prepared, type_id)? {
        Shape::Scalar(prim_id) => {
            let Some(stored) = scalar_slot(value).map(|(stored, _)| stored) else {
                return Err(refusal(prepared, type_id, value_kind(value), KIND_REASON));
            };
            if !scalar_kinds_match(prim_id, stored) {
                return Err(refusal(prepared, type_id, value_kind(value), KIND_REASON));
            }
            Ok(())
        }
        Shape::Text => Err(refusal(
            prepared,
            type_id,
            text_leaf(prepared, type_id),
            TEXT_REASON,
        )),
        Shape::Reference => check_reference(type_id, value, prepared),
        Shape::Instance => match value {
            Value::Instance(_) => Ok(()),
            other => Err(refusal(prepared, type_id, value_kind(other), KIND_REASON)),
        },
        Shape::Enumeration(enum_type) => check_enumeration(type_id, enum_type, value, prepared),
        Shape::Structure(struct_type) => {
            check_structure(type_id, struct_type, layout, value, prepared)
        }
        Shape::Array(elem_type) => check_array(type_id, elem_type, layout, value, prepared),
        Shape::Overlay => Err(refusal(prepared, type_id, OVERLAP_LEAF, OVERLAP_REASON)),
        Shape::Union => Err(refusal(prepared, type_id, UNION_LEAF, UNION_REASON)),
    }
}

/// Check a reference leaf's value: a null reference, or a refusal.
fn check_reference(
    type_id: u32,
    value: &Value,
    prepared: &PreparedModule,
) -> Result<(), RuntimeError> {
    match value {
        Value::Reference(None) => Ok(()),
        Value::Reference(Some(_)) => {
            Err(refusal(prepared, type_id, REFERENCE_LEAF, REFERENCE_REASON))
        }
        other => Err(refusal(prepared, type_id, value_kind(other), KIND_REASON)),
    }
}

/// Check an enumeration value against its type's declared variants.
fn check_enumeration(
    type_id: u32,
    enum_type: u32,
    value: &Value,
    prepared: &PreparedModule,
) -> Result<(), RuntimeError> {
    let TypeData::Enum { variants, .. } = &type_entry(prepared, enum_type)?.data else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let Value::Enum(enumeration) = value else {
        return Err(refusal(prepared, type_id, value_kind(value), KIND_REASON));
    };
    // ST names are case-insensitive, so a value of another spelling is still the type's.
    if !enumeration
        .type_name()
        .eq_ignore_ascii_case(&value_type_name(prepared, enum_type))
    {
        return Err(refusal(
            prepared,
            type_id,
            enumeration.type_name().as_str(),
            ENUM_TYPE_REASON,
        ));
    }
    let declared = variants.iter().any(|variant| {
        variant.value == enumeration.numeric_value()
            && string(prepared, variant.name_idx)
                .is_ok_and(|name| name.eq_ignore_ascii_case(enumeration.variant_name()))
    });
    if !declared {
        return Err(refusal(
            prepared,
            type_id,
            enumeration.variant_name().as_str(),
            ENUM_VARIANT_REASON,
        ));
    }
    Ok(())
}

/// Check a structure's value: its type name, its members and their values.
fn check_structure(
    type_id: u32,
    struct_type: u32,
    layout: &TypeLayout,
    value: &Value,
    prepared: &PreparedModule,
) -> Result<(), RuntimeError> {
    let TypeData::Struct { fields } = &type_entry(prepared, struct_type)?.data else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let Value::Struct(structure) = value else {
        return Err(refusal(prepared, type_id, value_kind(value), KIND_REASON));
    };
    if structure.type_name() != &value_type_name(prepared, struct_type) {
        return Err(refusal(
            prepared,
            type_id,
            structure.type_name().as_str(),
            STRUCTURE_NAME_REASON,
        ));
    }
    if layout.members.len() != fields.len() {
        return Err(RuntimeError::InvalidExecutionState);
    }
    for (member, field) in layout.members.iter().zip(fields) {
        let name = string(prepared, member.name_idx)?;
        let Some(member_value) = structure.field(name) else {
            return Err(refusal(prepared, type_id, name, MISSING_MEMBER_REASON));
        };
        check_value(field.type_id, member_value, prepared)?;
    }
    for name in structure.fields().keys() {
        if !declares_member(prepared, fields, name)? {
            return Err(refusal(prepared, type_id, name, UNKNOWN_MEMBER_REASON));
        }
    }
    Ok(())
}

/// Whether a structure declares a member with this name.
fn declares_member(
    prepared: &PreparedModule,
    fields: &[Field],
    name: &SmolStr,
) -> Result<bool, RuntimeError> {
    for field in fields {
        if string(prepared, field.name_idx)? == name.as_str() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Check an array's value: its bounds, its element count and its elements.
fn check_array(
    type_id: u32,
    elem_type: u32,
    layout: &TypeLayout,
    value: &Value,
    prepared: &PreparedModule,
) -> Result<(), RuntimeError> {
    let Some(array) = layout.array.as_ref() else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let Value::Array(value) = value else {
        return Err(refusal(prepared, type_id, value_kind(value), KIND_REASON));
    };
    if value.dimensions() != array.dims.as_slice() {
        return Err(refusal(
            prepared,
            type_id,
            "bounds",
            &format!(
                "the value's bounds {:?} are not the type's {:?}",
                value.dimensions(),
                array.dims
            ),
        ));
    }
    let expected = element_count(&array.dims).ok_or(RuntimeError::InvalidExecutionState)?;
    if value.elements().len() != expected {
        return Err(refusal(
            prepared,
            type_id,
            "elements",
            &format!(
                "the value holds {} elements, the type holds {expected}",
                value.elements().len()
            ),
        ));
    }
    for element in value.elements() {
        check_value(elem_type, element, prepared)?;
    }
    Ok(())
}

/// Write a checked value into the cells of its type's layout.
fn store_value(
    cells: &mut [Cell],
    type_id: u32,
    value: &Value,
    prepared: &PreparedModule,
) -> Result<(), RuntimeError> {
    if matches!(value, Value::Null) {
        cells.fill(Cell::Null);
        return Ok(());
    }
    match shape(prepared, type_id)? {
        Shape::Scalar(prim_id) => {
            let Some((stored, cell)) = scalar_slot(value) else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            if !scalar_kinds_match(prim_id, stored) {
                return Err(RuntimeError::InvalidExecutionState);
            }
            *first_mut(cells)? = cell;
        }
        Shape::Enumeration(enum_type) => {
            let Value::Enum(enumeration) = value else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            *first_mut(cells)? = Cell::Enum {
                type_idx: enum_type,
                value: enumeration.numeric_value(),
            };
        }
        Shape::Reference => *first_mut(cells)? = Cell::NullRef,
        Shape::Instance => {
            let Value::Instance(instance) = value else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            *first_mut(cells)? = Cell::Instance(*instance);
        }
        Shape::Structure(struct_type) => {
            store_structure(cells, type_id, struct_type, value, prepared)?
        }
        Shape::Array(elem_type) => store_array(cells, type_id, elem_type, value, prepared)?,
        Shape::Text | Shape::Overlay | Shape::Union => {
            return Err(RuntimeError::InvalidExecutionState)
        }
    }
    Ok(())
}

/// Write a checked structure's members at their layout offsets.
fn store_structure(
    cells: &mut [Cell],
    type_id: u32,
    struct_type: u32,
    value: &Value,
    prepared: &PreparedModule,
) -> Result<(), RuntimeError> {
    let layout = layout_of(prepared, type_id)?;
    let TypeData::Struct { fields } = &type_entry(prepared, struct_type)?.data else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let Value::Struct(structure) = value else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    for (member, field) in layout.members.iter().zip(fields) {
        let name = string(prepared, member.name_idx)?;
        let Some(member_value) = structure.field(name) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let member_cells = layout_of(prepared, field.type_id)?.cells as usize;
        let start = member.offset as usize;
        let end = start
            .checked_add(member_cells)
            .ok_or(RuntimeError::Overflow)?;
        let range = cells
            .get_mut(start..end)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        store_value(range, field.type_id, member_value, prepared)?;
    }
    Ok(())
}

/// Write a checked array's elements at their stride.
fn store_array(
    cells: &mut [Cell],
    type_id: u32,
    elem_type: u32,
    value: &Value,
    prepared: &PreparedModule,
) -> Result<(), RuntimeError> {
    let layout = layout_of(prepared, type_id)?;
    let Some(array) = layout.array.as_ref() else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let Value::Array(value) = value else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let stride = usize::try_from(array.stride).map_err(|_| RuntimeError::Overflow)?;
    for (index, element) in value.elements().iter().enumerate() {
        let start = index.checked_mul(stride).ok_or(RuntimeError::Overflow)?;
        let end = start.checked_add(stride).ok_or(RuntimeError::Overflow)?;
        let range = cells
            .get_mut(start..end)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        store_value(range, elem_type, element, prepared)?;
    }
    Ok(())
}

/// Materialize the value of one cell range, refusing a cell the type does not hold.
fn read_one(
    cells: &[Cell],
    type_id: u32,
    prepared: &PreparedModule,
) -> Result<Value, RuntimeError> {
    if cells.iter().all(|cell| *cell == Cell::Null) {
        return Ok(Value::Null);
    }
    match shape(prepared, type_id)? {
        Shape::Scalar(prim_id) => read_scalar(type_id, prim_id, cells, prepared),
        Shape::Enumeration(enum_type) => read_enumeration(type_id, enum_type, cells, prepared),
        Shape::Structure(struct_type) => read_structure(type_id, struct_type, cells, prepared),
        Shape::Array(elem_type) => read_array(type_id, elem_type, cells, prepared),
        Shape::Reference => match cells.first() {
            Some(Cell::NullRef) => Ok(Value::Reference(None)),
            Some(_) => Err(refusal(
                prepared,
                type_id,
                REFERENCE_LEAF,
                REFERENCE_TARGET_REASON,
            )),
            None => Err(RuntimeError::InvalidExecutionState),
        },
        Shape::Instance => match cells.first() {
            Some(Cell::Instance(instance)) => Ok(Value::Instance(*instance)),
            Some(_) => Err(refusal(prepared, type_id, "cell", KIND_REASON)),
            None => Err(RuntimeError::InvalidExecutionState),
        },
        Shape::Text => Err(refusal(
            prepared,
            type_id,
            text_leaf(prepared, type_id),
            TEXT_REASON,
        )),
        Shape::Overlay => Err(refusal(prepared, type_id, OVERLAP_LEAF, OVERLAP_REASON)),
        Shape::Union => Err(refusal(prepared, type_id, UNION_LEAF, UNION_REASON)),
    }
}

/// Materialize one scalar cell as the value of the type's primitive encoding.
fn read_scalar(
    type_id: u32,
    prim_id: u16,
    cells: &[Cell],
    prepared: &PreparedModule,
) -> Result<Value, RuntimeError> {
    let cell = *cells.first().ok_or(RuntimeError::InvalidExecutionState)?;
    let Some((stored, value)) = cell_slot(cell) else {
        return Err(refusal(prepared, type_id, "cell", KIND_REASON));
    };
    if !scalar_kinds_match(prim_id, stored) {
        return Err(refusal(prepared, type_id, "cell", KIND_REASON));
    }
    Ok(value)
}

/// Materialize one enumeration cell as the bracket the type declares for its value.
fn read_enumeration(
    type_id: u32,
    enum_type: u32,
    cells: &[Cell],
    prepared: &PreparedModule,
) -> Result<Value, RuntimeError> {
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
    let name = SmolStr::new(string(prepared, variant.name_idx)?);
    Ok(Value::Enum(alloc::boxed::Box::new(
        EnumValue::from_canonical_parts(value_type_name(prepared, enum_type), name, *value),
    )))
}

/// Materialize a structure's members from their layout offsets.
fn read_structure(
    type_id: u32,
    struct_type: u32,
    cells: &[Cell],
    prepared: &PreparedModule,
) -> Result<Value, RuntimeError> {
    let layout = layout_of(prepared, type_id)?;
    let TypeData::Struct { fields } = &type_entry(prepared, struct_type)?.data else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let mut members: OrderedMap<SmolStr, Value> = OrderedMap::default();
    for (member, field) in layout.members.iter().zip(fields) {
        let member_cells = layout_of(prepared, field.type_id)?.cells as usize;
        let start = member.offset as usize;
        let end = start
            .checked_add(member_cells)
            .ok_or(RuntimeError::Overflow)?;
        let range = cells
            .get(start..end)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let value = read_one(range, field.type_id, prepared)?;
        members.insert(SmolStr::new(string(prepared, member.name_idx)?), value);
    }
    Ok(Value::Struct(Arc::new(StructValue::from_canonical_parts(
        value_type_name(prepared, struct_type),
        members,
    ))))
}

/// Materialize an array's elements from their stride.
fn read_array(
    type_id: u32,
    elem_type: u32,
    cells: &[Cell],
    prepared: &PreparedModule,
) -> Result<Value, RuntimeError> {
    let layout = layout_of(prepared, type_id)?;
    let Some(array) = layout.array.as_ref() else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let expected = element_count(&array.dims).ok_or(RuntimeError::InvalidExecutionState)?;
    let stride = usize::try_from(array.stride).map_err(|_| RuntimeError::Overflow)?;
    if stride == 0 {
        return Err(RuntimeError::InvalidExecutionState);
    }
    let chunks = cells.chunks_exact(stride);
    if chunks.len() != expected {
        return Err(RuntimeError::InvalidExecutionState);
    }
    let elements = chunks
        .map(|element| read_one(element, elem_type, prepared))
        .collect::<Result<Vec<Value>, RuntimeError>>()?;
    Ok(Value::Array(alloc::boxed::Box::new(
        ArrayValue::from_canonical_parts(elements, array.dims.clone()),
    )))
}

/// The first cell of a range, or a refusal when the range holds none.
fn first_mut(cells: &mut [Cell]) -> Result<&mut Cell, RuntimeError> {
    cells.first_mut().ok_or(RuntimeError::InvalidExecutionState)
}

/// How a prepared type holds its value in cells.
fn shape(prepared: &PreparedModule, type_id: u32) -> Result<Shape, RuntimeError> {
    let (resolved, entry) = resolve(prepared, type_id)?;
    Ok(match &entry.data {
        TypeData::Primitive { .. } | TypeData::Subrange { .. } => {
            primitive_shape(prepared, type_id)?
        }
        TypeData::Enum { .. } => Shape::Enumeration(resolved),
        TypeData::Struct { .. } => {
            if entry.kind == TypeKind::OverlappingStruct {
                Shape::Overlay
            } else {
                Shape::Structure(resolved)
            }
        }
        TypeData::Union { .. } => Shape::Union,
        TypeData::Array { elem_type_id, .. } => Shape::Array(*elem_type_id),
        TypeData::Reference { .. } => Shape::Reference,
        TypeData::Pou { .. } | TypeData::Interface { .. } => Shape::Instance,
        TypeData::Alias { .. } => return Err(RuntimeError::InvalidExecutionState),
    })
}

/// The shape of a scalar or a text leaf: the primitive behind its aliases and subranges.
///
/// A subrange is its base primitive here: its bounds are the assignment path's check, so storing a
/// value through the layout neither re-checks nor clamps it.
fn primitive_shape(prepared: &PreparedModule, type_id: u32) -> Result<Shape, RuntimeError> {
    let prim_id = primitive_of(prepared, type_id).ok_or(RuntimeError::InvalidExecutionState)?;
    Ok(if TEXT_PRIMITIVES.contains(&prim_id) {
        Shape::Text
    } else {
        Shape::Scalar(prim_id)
    })
}

/// The primitive encoding a type resolves to through its aliases and subranges.
fn primitive_of(prepared: &PreparedModule, type_id: u32) -> Option<u16> {
    let types = prepared.type_table();
    let mut current = type_id;
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        let entry = types.entries.get(current as usize)?;
        match &entry.data {
            TypeData::Primitive { prim_id, .. } => return Some(*prim_id),
            TypeData::Alias { target_type_id }
            | TypeData::Subrange {
                base_type_id: target_type_id,
                ..
            } => current = *target_type_id,
            _ => return None,
        }
    }
    None
}

/// The `TYPE_TABLE` ordinal of a type with its aliases resolved, and its entry.
fn resolve(prepared: &PreparedModule, type_id: u32) -> Result<(u32, &TypeEntry), RuntimeError> {
    let mut current = type_id;
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        let entry = type_entry(prepared, current)?;
        let TypeData::Alias { target_type_id } = &entry.data else {
            return Ok((current, entry));
        };
        current = *target_type_id;
    }
    Err(RuntimeError::InvalidExecutionState)
}

/// The `TYPE_TABLE` entry of one admitted type ordinal.
fn type_entry(prepared: &PreparedModule, type_id: u32) -> Result<&TypeEntry, RuntimeError> {
    prepared
        .type_table()
        .entries
        .get(type_id as usize)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// One type's prepared layout.
fn layout_of(prepared: &PreparedModule, type_id: u32) -> Result<&TypeLayout, RuntimeError> {
    prepared
        .type_layout(type_id)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The name of one `STRING_TABLE` record.
fn string(prepared: &PreparedModule, name_idx: u32) -> Result<&str, RuntimeError> {
    prepared
        .strings()
        .get(name_idx as usize)
        .map(SmolStr::as_str)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The name a value of this type carries: the `STRING_TABLE` name, or none for an anonymous type.
fn value_type_name(prepared: &PreparedModule, type_id: u32) -> SmolStr {
    prepared
        .type_table()
        .entries
        .get(type_id as usize)
        .and_then(|entry| entry.name_idx)
        .and_then(|name_idx| prepared.strings().get(name_idx as usize))
        .cloned()
        .unwrap_or_default()
}

/// How a refusal names a type in an error: its name, or its ordinal when it has none.
fn type_label(prepared: &PreparedModule, type_id: u32) -> SmolStr {
    prepared
        .type_table()
        .entries
        .get(type_id as usize)
        .and_then(|entry| entry.name_idx)
        .and_then(|name_idx| prepared.strings().get(name_idx as usize))
        .cloned()
        .unwrap_or_else(|| SmolStr::new(format!("#{type_id}")))
}

/// The refusal of one conversion, naming the type, the refused value or leaf and the reason.
///
/// A cell conversion is a boundary write or read of a destination variable, so the refusal is the
/// runtime's initialization failure, whose owner, variable and error fields carry those three
/// facts.
fn refusal(prepared: &PreparedModule, type_id: u32, refused: &str, reason: &str) -> RuntimeError {
    RuntimeError::InitFailed {
        owner: type_label(prepared, type_id),
        variable: SmolStr::new(refused),
        error: SmolStr::new(reason),
    }
}

/// The refusal of a type ordinal no `TYPE_TABLE` entry holds.
fn unknown_type(type_id: u32) -> RuntimeError {
    RuntimeError::InitFailed {
        owner: SmolStr::new(format!("#{type_id}")),
        variable: SmolStr::new("type"),
        error: SmolStr::new("the type is not a prepared TYPE_TABLE entry"),
    }
}

/// The leaf name of a text type: `WSTRING` for primitive 25, `STRING` otherwise.
fn text_leaf(prepared: &PreparedModule, type_id: u32) -> &'static str {
    if primitive_of(prepared, type_id) == Some(25) {
        "WSTRING"
    } else {
        "STRING"
    }
}

/// The scalar cell of a value and the primitive encoding it belongs to, or `None` for a value that
/// is not a scalar.
fn scalar_slot(value: &Value) -> Option<(u16, Cell)> {
    Some(match value {
        Value::Bool(value) => (1, Cell::Bool(*value)),
        Value::Byte(value) => (2, Cell::Byte(*value)),
        Value::Word(value) => (3, Cell::Word(*value)),
        Value::DWord(value) => (4, Cell::DWord(*value)),
        Value::LWord(value) => (5, Cell::LWord(*value)),
        Value::SInt(value) => (6, Cell::SInt(*value)),
        Value::Int(value) => (7, Cell::Int(*value)),
        Value::DInt(value) => (8, Cell::DInt(*value)),
        Value::LInt(value) => (9, Cell::LInt(*value)),
        Value::USInt(value) => (10, Cell::USInt(*value)),
        Value::UInt(value) => (11, Cell::UInt(*value)),
        Value::UDInt(value) => (12, Cell::UDInt(*value)),
        Value::ULInt(value) => (13, Cell::ULInt(*value)),
        Value::Real(value) => (14, Cell::Real(*value)),
        Value::LReal(value) => (15, Cell::LReal(*value)),
        Value::Time(value) => (16, Cell::Time(value.as_nanos())),
        Value::LTime(value) => (17, Cell::LTime(value.as_nanos())),
        Value::Date(value) => (18, Cell::Date(value.ticks())),
        Value::LDate(value) => (19, Cell::LDate(value.nanos())),
        Value::Tod(value) => (20, Cell::Tod(value.ticks())),
        Value::LTod(value) => (21, Cell::LTod(value.nanos())),
        Value::Dt(value) => (22, Cell::Dt(value.ticks())),
        Value::Ldt(value) => (23, Cell::Ldt(value.nanos())),
        Value::Char(value) => (26, Cell::Char(*value)),
        Value::WChar(value) => (27, Cell::WChar(*value)),
        Value::Null => (0x0100, Cell::Null),
        _ => return None,
    })
}

/// The scalar value of a scalar cell and the primitive encoding it belongs to, or `None` for a
/// cell that holds no scalar.
fn cell_slot(cell: Cell) -> Option<(u16, Value)> {
    Some(match cell {
        Cell::Bool(value) => (1, Value::Bool(value)),
        Cell::Byte(value) => (2, Value::Byte(value)),
        Cell::Word(value) => (3, Value::Word(value)),
        Cell::DWord(value) => (4, Value::DWord(value)),
        Cell::LWord(value) => (5, Value::LWord(value)),
        Cell::SInt(value) => (6, Value::SInt(value)),
        Cell::Int(value) => (7, Value::Int(value)),
        Cell::DInt(value) => (8, Value::DInt(value)),
        Cell::LInt(value) => (9, Value::LInt(value)),
        Cell::USInt(value) => (10, Value::USInt(value)),
        Cell::UInt(value) => (11, Value::UInt(value)),
        Cell::UDInt(value) => (12, Value::UDInt(value)),
        Cell::ULInt(value) => (13, Value::ULInt(value)),
        Cell::Real(value) => (14, Value::Real(value)),
        Cell::LReal(value) => (15, Value::LReal(value)),
        Cell::Time(value) => (16, Value::Time(crate::value::Duration::from_nanos(value))),
        Cell::LTime(value) => (17, Value::LTime(crate::value::Duration::from_nanos(value))),
        Cell::Date(value) => (18, Value::Date(crate::value::DateValue::new(value))),
        Cell::LDate(value) => (19, Value::LDate(crate::value::LDateValue::new(value))),
        Cell::Tod(value) => (20, Value::Tod(crate::value::TimeOfDayValue::new(value))),
        Cell::LTod(value) => (21, Value::LTod(crate::value::LTimeOfDayValue::new(value))),
        Cell::Dt(value) => (22, Value::Dt(crate::value::DateTimeValue::new(value))),
        Cell::Ldt(value) => (23, Value::Ldt(crate::value::LDateTimeValue::new(value))),
        Cell::Char(value) => (26, Value::Char(value)),
        Cell::WChar(value) => (27, Value::WChar(value)),
        Cell::Enum { .. }
        | Cell::Instance(_)
        | Cell::Text(_)
        | Cell::Ref(_)
        | Cell::Null
        | Cell::NullRef => return None,
    })
}

/// Whether a value's primitive encoding is the type's: equal, or any of the eight integer
/// encodings for the generic native state slot (`0x0100`) the value model admits an integer into.
fn scalar_kinds_match(prim_id: u16, stored: u16) -> bool {
    stored == prim_id || (prim_id == 0x0100 && matches!(stored, 6..=13))
}

/// The value model's kind name for a refusal.
fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Bool(_) => "BOOL",
        Value::SInt(_) => "SINT",
        Value::Int(_) => "INT",
        Value::DInt(_) => "DINT",
        Value::LInt(_) => "LINT",
        Value::USInt(_) => "USINT",
        Value::UInt(_) => "UINT",
        Value::UDInt(_) => "UDINT",
        Value::ULInt(_) => "ULINT",
        Value::Real(_) => "REAL",
        Value::LReal(_) => "LREAL",
        Value::Byte(_) => "BYTE",
        Value::Word(_) => "WORD",
        Value::DWord(_) => "DWORD",
        Value::LWord(_) => "LWORD",
        Value::Time(_) => "TIME",
        Value::LTime(_) => "LTIME",
        Value::Date(_) => "DATE",
        Value::LDate(_) => "LDATE",
        Value::Tod(_) => "TOD",
        Value::LTod(_) => "LTOD",
        Value::Dt(_) => "DT",
        Value::Ldt(_) => "LDT",
        Value::String(_) => "STRING",
        Value::WString(_) => "WSTRING",
        Value::Char(_) => "CHAR",
        Value::WChar(_) => "WCHAR",
        Value::Array(_) => "ARRAY",
        Value::Struct(_) => "STRUCT",
        Value::Enum(_) => "ENUM",
        Value::Reference(_) => "REFERENCE",
        Value::Instance(_) => "INSTANCE",
        Value::Null => "NULL",
    }
}

/// The number of elements an array's declared bounds hold, or `None` when they hold none.
fn element_count(dims: &[(i64, i64)]) -> Option<usize> {
    dims.iter().try_fold(1usize, |count, (lower, upper)| {
        let length = upper.checked_sub(*lower)?.checked_add(1)?;
        count.checked_mul(usize::try_from(length).ok()?)
    })
}
