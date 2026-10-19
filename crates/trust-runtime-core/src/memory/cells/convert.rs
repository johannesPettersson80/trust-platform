//! Converting a value of a prepared type into its layout's bytes and back (specification 34
//! MEM-12; design "Boundaries": an engineering write converts a `Value` into the destination's
//! bytes without allocating, and a read materializes a `Value`).
//!
//! A [`TypeLayout`](crate::memory::layout::TypeLayout) fixes where a structure's members and an
//! array's elements sit in the block, so every conversion below is driven by that layout and by the
//! `TYPE_TABLE` entry: a member's offset, an element's stride and an enumeration's variants are
//! taken from the prepared metadata rather than computed a second time. Each leaf is read and
//! written at its declared size through [`flat`].
//!
//! - [`write_value`](crate::memory::cells::write_value) checks the whole value against the layout
//!   and then writes it, so a refused write leaves the bytes as they were and allocates nothing;
//! - [`read_value`](crate::memory::cells::read_value) materializes the value of a byte range;
//! - [`byte_offset`](crate::memory::cells::byte_offset) maps a member-and-index path to a byte
//!   offset within a value of its type (C1-10).
//!
//! A `STRUCT OVERLAP` and a union are not converted in this slice, and each refusal names the type
//! and the reason: their members share their bytes (C1-15). A text leaf holds its length and its
//! characters inline, cut where assignment normalization cuts it.

use alloc::format;

use smol_str::SmolStr;

mod leaf;
mod read;
pub(crate) use leaf::{leaf_in, structure_member_name, CellLeaf};
pub(crate) use read::read_value_in;

use super::{Cell, ReferenceTable};
use crate::bytecode::{Field, TypeData, TypeEntry, TypeKind, BYTECODE_MAX_TYPE_NESTING};
use crate::error::RuntimeError;
use crate::memory::flat::{self, LeafKind};
use crate::memory::layout::TypeLayout;
use crate::value::{RefSegment, Value};
use crate::vm::type_policy::primitive_value_matches;
use crate::vm::PreparedModule;

/// The kind-mismatch reason: the value is one kind and the type's primitive encoding another.
pub(super) const KIND_REASON: &str = "the value's kind does not match the type";

/// Why a non-null reference is refused where no region's reference table holds it (O9).
const REFERENCE_REASON: &str = "a non-null reference is held by its region's reference table";

/// Why a reference leaf that holds a reference is refused where no reference table is given (O9).
pub(super) const REFERENCE_TARGET_REASON: &str =
    "a reference leaf's target is in its region's reference table";

/// Why a `STRUCT OVERLAP` is refused here (C1-15).
pub(super) const OVERLAP_REASON: &str =
    "an overlapping structure is written through its byte backing in C1-15";

/// Why a union is refused here (C1-15).
pub(super) const UNION_REASON: &str =
    "a union's members share their bytes; C1-15 owns shared storage";

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
pub(super) const REFERENCE_LEAF: &str = "REFERENCE";

/// The leaf name of a `STRUCT OVERLAP`.
pub(super) const OVERLAP_LEAF: &str = "OVERLAP";

/// The leaf name of a union.
pub(super) const UNION_LEAF: &str = "UNION";

/// The text primitive encodings (`STRING` and `WSTRING`).
const TEXT_PRIMITIVES: [u16; 2] = [24, 25];

/// The primitive encoding of `WSTRING`; `STRING` is 24.
pub(super) const WSTRING_PRIMITIVE: u16 = 25;

/// Write the value `value` of the `TYPE_TABLE` type `type_id` into `bytes`, the byte range of the
/// type's prepared layout (specification 34 MEM-12; design "Boundaries").
///
/// `bytes` is the whole range: its length is the layout's declared size. Every value kind of this
/// slice is converted — each scalar with its exact bits at its declared size, an enumeration as its
/// base type, an instance as its identity, a text inline with the normalization cut, a structure's
/// members and an array's elements at their layout offsets with nested compounds inline, a null
/// reference as an empty reference leaf, and [`Value::Null`] as zero bytes.
///
/// # Errors
///
/// Refused with [`RuntimeError::InitFailed`], naming the type, the refused value or leaf and the
/// reason, when a value's kind is not the type's, when a structure is missing a member or carries
/// one the type does not declare, when an array's bounds or element count are not the type's, and
/// when `bytes` is not the layout's length. A non-null reference, which a region's reference table
/// holds, and a `STRUCT OVERLAP` or union (C1-15) are refused. The check runs before the first
/// write, so a refusal leaves the bytes as they were, and no path allocates.
pub fn write_value(
    bytes: &mut [u8],
    type_id: u32,
    value: &Value,
    prepared: &PreparedModule,
) -> Result<(), RuntimeError> {
    write_value_in(bytes, type_id, value, prepared, None, 0)
}

/// [`write_value`] into a region that holds reference leaves: a non-null reference is kept in
/// `references` under its leaf's byte offset, `start` being the offset of `bytes` in its block (O9,
/// R3). `None` refuses a non-null reference, as [`write_value`] does.
pub(crate) fn write_value_in(
    bytes: &mut [u8],
    type_id: u32,
    value: &Value,
    prepared: &PreparedModule,
    references: Option<&mut ReferenceTable>,
    start: u32,
) -> Result<(), RuntimeError> {
    check_slice_length(bytes.len(), type_id, prepared)?;
    check_value(type_id, value, prepared, references.is_some())?;
    store_value(bytes, type_id, value, prepared, references, start)
}

/// Materialize the value of the `TYPE_TABLE` type `type_id` from `bytes`, the byte range of the
/// type's prepared layout (specification 34 MEM-12; design "Boundaries").
///
/// The inverse of [`write_value`]: a range [`write_value`] wrote reads back as the value it was
/// given, and an instance leaf that names no instance reads as [`Value::Null`].
///
/// # Errors
///
/// Refused with [`RuntimeError::InitFailed`], naming the type, the refused leaf and the reason,
/// when a leaf does not hold a value of the type, when an enumeration names no variant of its type,
/// when `bytes` is not the layout's length, and for a reference that holds a reference, which only
/// a region's table can read, and the overlapping structures and unions (C1-15) [`write_value`]
/// refuses.
pub fn read_value(
    bytes: &[u8],
    type_id: u32,
    prepared: &PreparedModule,
) -> Result<Value, RuntimeError> {
    read::read_value_in(bytes, type_id, prepared, None, 0)
}

/// The byte offset, within a value of the `TYPE_TABLE` type `type_id`, of the member-and-index
/// `path`, or `None` for a path the type does not have (specification 34 MEM-12; design "Operands
/// and references": C1-10 turns a reference's path into an offset).
///
/// [`RefSegment::Field`] resolves a member of a structure by name, and [`RefSegment::Index`]
/// resolves an array element by its IEC index, honouring the array's declared lower bounds.
#[must_use]
pub fn byte_offset(type_id: u32, path: &[RefSegment], prepared: &PreparedModule) -> Option<u32> {
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
                offset = offset.checked_add(layout.members.get(ordinal)?.byte_offset)?;
                current = fields.get(ordinal)?.type_id;
            }
            RefSegment::Index(indices) => {
                let TypeData::Array { elem_type_id, dims } = &entry.data else {
                    return None;
                };
                let element = crate::value::array_offset_i64(dims, indices)?;
                let stride = prepared.type_layout(resolved)?.array.as_ref()?.byte_stride;
                offset = offset.checked_add(u32::try_from(element).ok()?.checked_mul(stride)?)?;
                current = *elem_type_id;
            }
        }
    }
    Some(offset)
}

/// The length field's bytes, the characters' capacity in bytes and the width of the text leaf of
/// `type_id`, or `None` for a type that is not a text (MEM-12).
pub(crate) fn text_geometry(prepared: &PreparedModule, type_id: u32) -> Option<(u32, u32, bool)> {
    let prim_id = primitive_of(prepared, type_id)?;
    if !TEXT_PRIMITIVES.contains(&prim_id) {
        return None;
    }
    let layout = prepared.type_layout(type_id)?;
    let capacity = layout.text_leaves.first()?.capacity_bytes;
    let length = layout.shape.bytes.checked_sub(capacity)?;
    Some((length, capacity, prim_id == WSTRING_PRIMITIVE))
}

/// How one prepared type holds a value in a flat block.
pub(super) enum Shape {
    /// One scalar leaf with this primitive encoding.
    Scalar(u16),
    /// One text leaf of this primitive encoding (`STRING` 24 or `WSTRING` 25), its length and its
    /// characters inline.
    Text(u16),
    /// One reference leaf.
    Reference,
    /// One instance leaf.
    Instance,
    /// One enumeration leaf of this resolved enumeration type.
    Enumeration(u32),
    /// A structure of this resolved structure type's members.
    Structure(u32),
    /// A `STRUCT OVERLAP`, whose members share their bytes (C1-15).
    Overlay,
    /// A union, whose members share their bytes (C1-15).
    Union,
    /// An array of this element type.
    Array(u32),
}

/// Refuse a byte range that is not exactly the type's declared size.
pub(super) fn check_slice_length(
    bytes: usize,
    type_id: u32,
    prepared: &PreparedModule,
) -> Result<(), RuntimeError> {
    let layout = prepared
        .type_layout(type_id)
        .ok_or_else(|| unknown_type(type_id))?;
    if bytes != layout.shape.bytes as usize {
        return Err(refusal(
            prepared,
            type_id,
            "bytes",
            &format!(
                "the slice holds {bytes} bytes, the type's layout needs {}",
                layout.shape.bytes
            ),
        ));
    }
    Ok(())
}

/// Whether `type_id` resolves to a reference type, whose leaf's reference a region's table holds
/// (O9).
pub(crate) fn is_reference_type(prepared: &PreparedModule, type_id: u32) -> bool {
    resolve(prepared, type_id)
        .is_ok_and(|(_, entry)| matches!(entry.data, TypeData::Reference { .. }))
}

/// Check the whole value against the type before a byte is written, so a refusal writes nothing.
/// `accept_references` admits a non-null reference, which the region's table then holds (O9).
pub(crate) fn check_value(
    type_id: u32,
    value: &Value,
    prepared: &PreparedModule,
    accept_references: bool,
) -> Result<(), RuntimeError> {
    if matches!(value, Value::Null) {
        return Ok(());
    }
    let layout = layout_of(prepared, type_id)?;
    match shape(prepared, type_id)? {
        Shape::Scalar(prim_id) | Shape::Text(prim_id) => {
            if primitive_value_matches(prim_id, value) {
                Ok(())
            } else {
                Err(refusal(prepared, type_id, value_kind(value), KIND_REASON))
            }
        }
        Shape::Reference => check_reference(type_id, value, prepared, accept_references),
        Shape::Instance => match value {
            Value::Instance(_) => Ok(()),
            other => Err(refusal(prepared, type_id, value_kind(other), KIND_REASON)),
        },
        Shape::Enumeration(enum_type) => check_enumeration(type_id, enum_type, value, prepared),
        Shape::Structure(struct_type) => check_structure(
            type_id,
            struct_type,
            layout,
            value,
            prepared,
            accept_references,
        ),
        Shape::Array(elem_type) => check_array(
            type_id,
            elem_type,
            layout,
            value,
            prepared,
            accept_references,
        ),
        Shape::Overlay => Err(refusal(prepared, type_id, OVERLAP_LEAF, OVERLAP_REASON)),
        Shape::Union => Err(refusal(prepared, type_id, UNION_LEAF, UNION_REASON)),
    }
}

/// Check a reference leaf's value: a null reference, or a refusal.
fn check_reference(
    type_id: u32,
    value: &Value,
    prepared: &PreparedModule,
    accept_references: bool,
) -> Result<(), RuntimeError> {
    match value {
        Value::Reference(None) => Ok(()),
        Value::Reference(Some(_)) if accept_references => Ok(()),
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
    accept_references: bool,
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
        check_value(field.type_id, member_value, prepared, accept_references)?;
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
    accept_references: bool,
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
        check_value(elem_type, element, prepared, accept_references)?;
    }
    Ok(())
}

/// Write a checked value into the bytes of its type's layout; `start` is their offset in their
/// block.
fn store_value(
    bytes: &mut [u8],
    type_id: u32,
    value: &Value,
    prepared: &PreparedModule,
    references: Option<&mut ReferenceTable>,
    start: u32,
) -> Result<(), RuntimeError> {
    if matches!(value, Value::Null) {
        bytes.fill(0);
        return Ok(());
    }
    match shape(prepared, type_id)? {
        Shape::Scalar(prim_id) => {
            if !primitive_value_matches(prim_id, value) {
                return Err(RuntimeError::InvalidExecutionState);
            }
            let Some(cell) = scalar_slot(value) else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            flat::encode(LeafKind::Primitive(prim_id), cell, bytes)?;
        }
        Shape::Text(prim_id) => {
            let Some((stored, wide)) = text_value(prim_id, value) else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            let (length, capacity, _) =
                text_geometry(prepared, type_id).ok_or(RuntimeError::InvalidExecutionState)?;
            flat::write_text(bytes, length, capacity, stored, wide)?;
        }
        Shape::Enumeration(enum_type) => {
            let Value::Enum(enumeration) = value else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            let kind = enumeration_kind(prepared, enum_type)?;
            flat::encode(
                kind,
                Cell::Enum {
                    type_idx: enum_type,
                    value: enumeration.numeric_value(),
                },
                bytes,
            )?;
        }
        Shape::Reference => store_reference(bytes, value, references, start)?,
        Shape::Instance => {
            let Value::Instance(instance) = value else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            flat::encode(LeafKind::Instance, Cell::Instance(*instance), bytes)?;
        }
        Shape::Structure(struct_type) => store_structure(
            bytes,
            type_id,
            struct_type,
            value,
            prepared,
            references,
            start,
        )?,
        Shape::Array(elem_type) => store_array(
            bytes, type_id, elem_type, value, prepared, references, start,
        )?,
        Shape::Overlay | Shape::Union => return Err(RuntimeError::InvalidExecutionState),
    }
    Ok(())
}

/// The text of a value of a text leaf's primitive encoding, and whether its leaf is wide.
fn text_value(prim_id: u16, value: &Value) -> Option<(&str, bool)> {
    if !primitive_value_matches(prim_id, value) {
        return None;
    }
    let stored = match value {
        Value::String(stored) => stored.as_str(),
        Value::WString(stored) => stored.as_str(),
        _ => return None,
    };
    Some((stored, prim_id == WSTRING_PRIMITIVE))
}

/// How the leaf of the resolved enumeration type `enum_type` is stored: as its base primitive.
pub(super) fn enumeration_kind(
    prepared: &PreparedModule,
    enum_type: u32,
) -> Result<LeafKind, RuntimeError> {
    let TypeData::Enum { base_type_id, .. } = &type_entry(prepared, enum_type)?.data else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let base = primitive_of(prepared, *base_type_id).ok_or(RuntimeError::InvalidExecutionState)?;
    Ok(LeafKind::Enumeration {
        type_idx: enum_type,
        base,
    })
}

/// Write a checked reference leaf (O9, R3). A non-null reference is kept in the region's table under
/// the leaf's byte offset, and the leaf says that it holds one. The table is reserved with the
/// reference leaves, so a leaf outside it is refused.
fn store_reference(
    bytes: &mut [u8],
    value: &Value,
    references: Option<&mut ReferenceTable>,
    start: u32,
) -> Result<(), RuntimeError> {
    match value {
        Value::Reference(None) => {
            if let Some(references) = references {
                references.set(start, None);
            }
            flat::encode(LeafKind::Reference, Cell::NullRef, bytes)
        }
        Value::Reference(Some(reference)) => {
            let references = references.ok_or(RuntimeError::InvalidExecutionState)?;
            if !references.set(start, Some(reference.clone())) {
                return Err(RuntimeError::InvalidExecutionState);
            }
            flat::encode(LeafKind::Reference, Cell::Ref(super::RefSlot(start)), bytes)
        }
        _ => Err(RuntimeError::InvalidExecutionState),
    }
}

/// Write a checked structure's members at their layout offsets.
fn store_structure(
    bytes: &mut [u8],
    type_id: u32,
    struct_type: u32,
    value: &Value,
    prepared: &PreparedModule,
    mut references: Option<&mut ReferenceTable>,
    start: u32,
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
        let size = layout_of(prepared, field.type_id)?.shape.bytes;
        store_value(
            part_mut(bytes, member.byte_offset, size)?,
            field.type_id,
            member_value,
            prepared,
            references.as_deref_mut(),
            offset_by(start, member.byte_offset)?,
        )?;
    }
    Ok(())
}

/// Write a checked array's elements at their stride.
fn store_array(
    bytes: &mut [u8],
    type_id: u32,
    elem_type: u32,
    value: &Value,
    prepared: &PreparedModule,
    mut references: Option<&mut ReferenceTable>,
    start: u32,
) -> Result<(), RuntimeError> {
    let layout = layout_of(prepared, type_id)?;
    let Some(array) = layout.array.as_ref() else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let Value::Array(value) = value else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let size = layout_of(prepared, elem_type)?.shape.bytes;
    for (index, element) in value.elements().iter().enumerate() {
        let offset = element_offset(index, array.byte_stride)?;
        store_value(
            part_mut(bytes, offset, size)?,
            elem_type,
            element,
            prepared,
            references.as_deref_mut(),
            offset_by(start, offset)?,
        )?;
    }
    Ok(())
}

/// The byte offset of element `index` of an array whose elements are `stride` bytes apart.
pub(super) fn element_offset(index: usize, stride: u32) -> Result<u32, RuntimeError> {
    u32::try_from(index)
        .ok()
        .and_then(|index| index.checked_mul(stride))
        .ok_or(RuntimeError::Overflow)
}

/// `base` moved on by `offset` bytes.
pub(super) fn offset_by(base: u32, offset: u32) -> Result<u32, RuntimeError> {
    base.checked_add(offset).ok_or(RuntimeError::Overflow)
}

/// The `len` bytes of `bytes` from `offset`, or a refusal when they leave it.
pub(super) fn part(bytes: &[u8], offset: u32, len: u32) -> Result<&[u8], RuntimeError> {
    let start = usize::try_from(offset).map_err(|_| RuntimeError::Overflow)?;
    let end = start
        .checked_add(usize::try_from(len).map_err(|_| RuntimeError::Overflow)?)
        .ok_or(RuntimeError::Overflow)?;
    bytes
        .get(start..end)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The `len` bytes of `bytes` from `offset`, writable, or a refusal when they leave it.
pub(super) fn part_mut(bytes: &mut [u8], offset: u32, len: u32) -> Result<&mut [u8], RuntimeError> {
    let start = usize::try_from(offset).map_err(|_| RuntimeError::Overflow)?;
    let end = start
        .checked_add(usize::try_from(len).map_err(|_| RuntimeError::Overflow)?)
        .ok_or(RuntimeError::Overflow)?;
    bytes
        .get_mut(start..end)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// How a prepared type holds its value in a flat block.
pub(super) fn shape(prepared: &PreparedModule, type_id: u32) -> Result<Shape, RuntimeError> {
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
        Shape::Text(prim_id)
    } else {
        Shape::Scalar(prim_id)
    })
}

/// The primitive encoding a type resolves to through its aliases and subranges.
pub(super) fn primitive_of(prepared: &PreparedModule, type_id: u32) -> Option<u16> {
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
pub(super) fn type_entry(
    prepared: &PreparedModule,
    type_id: u32,
) -> Result<&TypeEntry, RuntimeError> {
    prepared
        .type_table()
        .entries
        .get(type_id as usize)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// One type's prepared layout.
pub(super) fn layout_of(
    prepared: &PreparedModule,
    type_id: u32,
) -> Result<&TypeLayout, RuntimeError> {
    prepared
        .type_layout(type_id)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The name of one `STRING_TABLE` record.
pub(super) fn string(prepared: &PreparedModule, name_idx: u32) -> Result<&str, RuntimeError> {
    prepared
        .strings()
        .get(name_idx as usize)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The name a value of this type carries: the `STRING_TABLE` name, or none for an anonymous type.
pub(super) fn value_type_name(prepared: &PreparedModule, type_id: u32) -> SmolStr {
    prepared
        .type_table()
        .entries
        .get(type_id as usize)
        .and_then(|entry| entry.name_idx)
        .and_then(|name_idx| prepared.strings().get(name_idx as usize))
        .map(SmolStr::new)
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
        .map(SmolStr::new)
        .unwrap_or_else(|| SmolStr::new(format!("#{type_id}")))
}

/// The refusal of one conversion, naming the type, the refused value or leaf and the reason.
///
/// A conversion is a boundary write or read of a destination variable, so the refusal is the
/// runtime's initialization failure, whose owner, variable and error fields carry those three
/// facts.
pub(super) fn refusal(
    prepared: &PreparedModule,
    type_id: u32,
    refused: &str,
    reason: &str,
) -> RuntimeError {
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

/// The decoded leaf of a scalar value, or `None` for a value that is not a scalar.
fn scalar_slot(value: &Value) -> Option<Cell> {
    Some(match value {
        Value::Bool(value) => Cell::Bool(*value),
        Value::Byte(value) => Cell::Byte(*value),
        Value::Word(value) => Cell::Word(*value),
        Value::DWord(value) => Cell::DWord(*value),
        Value::LWord(value) => Cell::LWord(*value),
        Value::SInt(value) => Cell::SInt(*value),
        Value::Int(value) => Cell::Int(*value),
        Value::DInt(value) => Cell::DInt(*value),
        Value::LInt(value) => Cell::LInt(*value),
        Value::USInt(value) => Cell::USInt(*value),
        Value::UInt(value) => Cell::UInt(*value),
        Value::UDInt(value) => Cell::UDInt(*value),
        Value::ULInt(value) => Cell::ULInt(*value),
        Value::Real(value) => Cell::Real(*value),
        Value::LReal(value) => Cell::LReal(*value),
        Value::Time(value) => Cell::Time(value.as_nanos()),
        Value::LTime(value) => Cell::LTime(value.as_nanos()),
        Value::Date(value) => Cell::Date(value.ticks()),
        Value::LDate(value) => Cell::LDate(value.nanos()),
        Value::Tod(value) => Cell::Tod(value.ticks()),
        Value::LTod(value) => Cell::LTod(value.nanos()),
        Value::Dt(value) => Cell::Dt(value.ticks()),
        Value::Ldt(value) => Cell::Ldt(value.nanos()),
        Value::Char(value) => Cell::Char(*value),
        Value::WChar(value) => Cell::WChar(*value),
        _ => return None,
    })
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
pub(super) fn element_count(dims: &[(i64, i64)]) -> Option<usize> {
    dims.iter().try_fold(1usize, |count, (lower, upper)| {
        let length = upper.checked_sub(*lower)?.checked_add(1)?;
        count.checked_mul(usize::try_from(length).ok()?)
    })
}
