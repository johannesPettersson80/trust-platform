//! The read side of the conversion (specification 34 MEM-12; C1-7, C1-9c): a byte range
//! materialized as a `Value`. A reference leaf reads the `ValueRef` the region's table holds under
//! its byte offset (O9, R3). The write side and the layouts are in the parent module.

use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;

use smol_str::SmolStr;

use super::{
    check_slice_length, element_count, element_offset, enumeration_kind, layout_of, offset_by,
    part, refusal, shape, string, text_geometry, type_entry, value_type_name, Shape, KIND_REASON,
    REFERENCE_LEAF, REFERENCE_TARGET_REASON, UNION_LEAF, UNION_REASON,
};
use crate::bytecode::TypeData;
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::memory::cells::{Cell, ReferenceTable};
use crate::memory::flat::{self, LeafKind, TextBytes};
use crate::value::{ArrayValue, EnumValue, StructValue, Value};
use crate::vm::type_policy::primitive_value_matches;
use crate::vm::PreparedModule;

/// Materialize the value of the `TYPE_TABLE` type `type_id` from `bytes`, whose offset in their
/// block is `start`, with the region's reference table `references` for the reference leaves (O9,
/// R3).
pub(crate) fn read_value_in(
    bytes: &[u8],
    type_id: u32,
    prepared: &PreparedModule,
    references: Option<&ReferenceTable>,
    start: u32,
) -> Result<Value, RuntimeError> {
    check_slice_length(bytes.len(), type_id, prepared)?;
    read_one(bytes, type_id, prepared, references, start)
}

/// Materialize the value of one byte range, refusing a leaf the type does not hold.
pub(super) fn read_one(
    bytes: &[u8],
    type_id: u32,
    prepared: &PreparedModule,
    references: Option<&ReferenceTable>,
    start: u32,
) -> Result<Value, RuntimeError> {
    match shape(prepared, type_id)? {
        Shape::Scalar(prim_id) => read_scalar(type_id, prim_id, bytes, prepared),
        Shape::Enumeration(enum_type) => read_enumeration(type_id, enum_type, bytes, prepared),
        Shape::Structure(struct_type) => {
            read_structure(type_id, struct_type, bytes, prepared, references, start)
        }
        Shape::Array(elem_type) => {
            read_array(type_id, elem_type, bytes, prepared, references, start)
        }
        Shape::Reference => read_reference(type_id, bytes, prepared, references, start),
        Shape::Instance => match flat::decode(LeafKind::Instance, bytes)? {
            Cell::Instance(instance) => Ok(Value::Instance(instance)),
            _ => Ok(Value::Null),
        },
        Shape::Text(_) => read_text(type_id, bytes, prepared),
        Shape::Overlay(overlay) => {
            super::overlay::read(overlay, bytes, prepared, references, start)
        }
        Shape::Union => Err(refusal(prepared, type_id, UNION_LEAF, UNION_REASON)),
    }
}

/// Materialize one reference leaf (O9, R3): a null reference, or the reference the region's table
/// holds under the leaf's byte offset `start`.
fn read_reference(
    type_id: u32,
    bytes: &[u8],
    prepared: &PreparedModule,
    references: Option<&ReferenceTable>,
    start: u32,
) -> Result<Value, RuntimeError> {
    match flat::decode(LeafKind::Reference, bytes)? {
        Cell::NullRef => Ok(Value::Reference(None)),
        cell @ Cell::Flat { .. } => {
            super::flat_reference(prepared, type_id, cell).map(Value::Reference)
        }
        _ => match references {
            Some(table) => table
                .get(start)
                .map(|reference| Value::Reference(Some(reference.clone())))
                .ok_or(RuntimeError::InvalidExecutionState),
            None => Err(refusal(
                prepared,
                type_id,
                REFERENCE_LEAF,
                REFERENCE_TARGET_REASON,
            )),
        },
    }
}

/// Materialize one text leaf from its length and its characters inline.
fn read_text(type_id: u32, bytes: &[u8], prepared: &PreparedModule) -> Result<Value, RuntimeError> {
    let (length, _, wide) =
        text_geometry(prepared, type_id).ok_or(RuntimeError::InvalidExecutionState)?;
    Ok(match flat::read_text(bytes, length, wide)? {
        TextBytes::Narrow(text) => Value::String(SmolStr::new(text)),
        wide => Value::WString(
            char::decode_utf16(wide.units())
                .collect::<Result<_, _>>()
                .map_err(|_| RuntimeError::InvalidExecutionState)?,
        ),
    })
}

/// Materialize one scalar leaf as the value of the type's primitive encoding. An unbound native
/// counter slot reads as [`Value::Null`].
pub(super) fn read_scalar(
    type_id: u32,
    prim_id: u16,
    bytes: &[u8],
    prepared: &PreparedModule,
) -> Result<Value, RuntimeError> {
    let cell = flat::decode(LeafKind::Primitive(prim_id), bytes)?;
    if cell == Cell::Null {
        return Ok(Value::Null);
    }
    let Some(value) = cell_slot(cell) else {
        return Err(refusal(prepared, type_id, "leaf", KIND_REASON));
    };
    if !primitive_value_matches(prim_id, &value) {
        return Err(refusal(prepared, type_id, "leaf", KIND_REASON));
    }
    Ok(value)
}

/// Materialize one enumeration leaf as the variant the type declares for its value.
fn read_enumeration(
    type_id: u32,
    enum_type: u32,
    bytes: &[u8],
    prepared: &PreparedModule,
) -> Result<Value, RuntimeError> {
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
    let name = SmolStr::new(string(prepared, variant.name_idx)?);
    Ok(Value::Enum(alloc::boxed::Box::new(
        EnumValue::from_canonical_parts(value_type_name(prepared, enum_type), name, value),
    )))
}

/// Materialize a structure's members from their layout offsets.
fn read_structure(
    type_id: u32,
    struct_type: u32,
    bytes: &[u8],
    prepared: &PreparedModule,
    references: Option<&ReferenceTable>,
    start: u32,
) -> Result<Value, RuntimeError> {
    let layout = layout_of(prepared, type_id)?;
    let TypeData::Struct { fields } = &type_entry(prepared, struct_type)?.data else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let mut members: OrderedMap<SmolStr, Value> = OrderedMap::default();
    for (member, field) in layout.members.iter().zip(fields) {
        let size = layout_of(prepared, field.type_id)?.shape.bytes;
        let value = read_one(
            part(bytes, member.byte_offset, size)?,
            field.type_id,
            prepared,
            references,
            offset_by(start, member.byte_offset)?,
        )?;
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
    bytes: &[u8],
    prepared: &PreparedModule,
    references: Option<&ReferenceTable>,
    start: u32,
) -> Result<Value, RuntimeError> {
    let layout = layout_of(prepared, type_id)?;
    let Some(array) = layout.array.as_ref() else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let expected = element_count(&array.dims).ok_or(RuntimeError::InvalidExecutionState)?;
    let size = layout_of(prepared, elem_type)?.shape.bytes;
    let mut elements = Vec::with_capacity(expected);
    for index in 0..expected {
        let offset = element_offset(index, array.byte_stride)?;
        elements.push(read_one(
            part(bytes, offset, size)?,
            elem_type,
            prepared,
            references,
            offset_by(start, offset)?,
        )?);
    }
    Ok(Value::Array(alloc::boxed::Box::new(
        ArrayValue::from_canonical_parts(elements, array.dims.clone()),
    )))
}

/// The scalar value of a decoded scalar leaf, or `None` for a leaf that holds no scalar.
fn cell_slot(cell: Cell) -> Option<Value> {
    Some(match cell {
        Cell::Bool(value) => Value::Bool(value),
        Cell::Byte(value) => Value::Byte(value),
        Cell::Word(value) => Value::Word(value),
        Cell::DWord(value) => Value::DWord(value),
        Cell::LWord(value) => Value::LWord(value),
        Cell::SInt(value) => Value::SInt(value),
        Cell::Int(value) => Value::Int(value),
        Cell::DInt(value) => Value::DInt(value),
        Cell::LInt(value) => Value::LInt(value),
        Cell::USInt(value) => Value::USInt(value),
        Cell::UInt(value) => Value::UInt(value),
        Cell::UDInt(value) => Value::UDInt(value),
        Cell::ULInt(value) => Value::ULInt(value),
        Cell::Real(value) => Value::Real(value),
        Cell::LReal(value) => Value::LReal(value),
        Cell::Time(value) => Value::Time(crate::value::Duration::from_nanos(value)),
        Cell::LTime(value) => Value::LTime(crate::value::Duration::from_nanos(value)),
        Cell::Date(value) => Value::Date(crate::value::DateValue::new(value)),
        Cell::LDate(value) => Value::LDate(crate::value::LDateValue::new(value)),
        Cell::Tod(value) => Value::Tod(crate::value::TimeOfDayValue::new(value)),
        Cell::LTod(value) => Value::LTod(crate::value::LTimeOfDayValue::new(value)),
        Cell::Dt(value) => Value::Dt(crate::value::DateTimeValue::new(value)),
        Cell::Ldt(value) => Value::Ldt(crate::value::LDateTimeValue::new(value)),
        Cell::Char(value) => Value::Char(value),
        Cell::WChar(value) => Value::WChar(value),
        Cell::Enum { .. }
        | Cell::Instance(_)
        | Cell::Ref(_)
        | Cell::Flat { .. }
        | Cell::Null
        | Cell::NullRef => {
            return None;
        }
    })
}
