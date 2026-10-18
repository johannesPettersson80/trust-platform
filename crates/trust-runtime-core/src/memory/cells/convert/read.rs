//! The read side of the cell conversion (specification 34 §6.2, MEM-10; C1-7, C1-9c): a cell range
//! materialized as a `Value`. A reference leaf reads the `ValueRef` the region's table holds under its
//! cell (O9, R3). The write side and the layouts are in the parent module.

use alloc::format;
use alloc::sync::Arc;
use alloc::vec::Vec;

use smol_str::SmolStr;

use super::{
    check_slice_length, element_count, layout_of, refusal, shape, string, type_entry,
    value_type_name, Shape, KIND_REASON, OVERLAP_LEAF, OVERLAP_REASON, REFERENCE_LEAF,
    REFERENCE_TARGET_REASON, UNION_LEAF, UNION_REASON, WSTRING_PRIMITIVE,
};
use crate::bytecode::TypeData;
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::memory::cells::{Cell, RefSlot, ReferenceTable};
use crate::memory::text::TextStore;
use crate::value::{ArrayValue, EnumValue, StructValue, Value};
use crate::vm::type_policy::primitive_value_matches;
use crate::vm::PreparedModule;

/// Materialize the value of the `TYPE_TABLE` type `type_id` from `cells`, with the region's reference
/// table `references` for the reference leaves (O9, R3).
pub(crate) fn read_value_in(
    cells: &[Cell],
    type_id: u32,
    prepared: &PreparedModule,
    text: &TextStore,
    references: Option<&ReferenceTable>,
) -> Result<Value, RuntimeError> {
    check_slice_length(cells.len(), type_id, prepared)?;
    read_one(cells, type_id, prepared, text, references)
}

/// Materialize the value of one cell range, refusing a cell the type does not hold.
fn read_one(
    cells: &[Cell],
    type_id: u32,
    prepared: &PreparedModule,
    text: &TextStore,
    references: Option<&ReferenceTable>,
) -> Result<Value, RuntimeError> {
    if cells.iter().all(|cell| *cell == Cell::Null) {
        return Ok(Value::Null);
    }
    match shape(prepared, type_id)? {
        Shape::Scalar(prim_id) => read_scalar(type_id, prim_id, cells, prepared),
        Shape::Enumeration(enum_type) => read_enumeration(type_id, enum_type, cells, prepared),
        Shape::Structure(struct_type) => {
            read_structure(type_id, struct_type, cells, prepared, text, references)
        }
        Shape::Array(elem_type) => {
            read_array(type_id, elem_type, cells, prepared, text, references)
        }
        Shape::Reference => read_reference(type_id, cells, prepared, references),
        Shape::Instance => match cells.first() {
            Some(Cell::Instance(instance)) => Ok(Value::Instance(*instance)),
            Some(_) => Err(refusal(prepared, type_id, "cell", KIND_REASON)),
            None => Err(RuntimeError::InvalidExecutionState),
        },
        Shape::Text(prim_id) => read_text(type_id, prim_id, cells, prepared, text),
        Shape::Overlay => Err(refusal(prepared, type_id, OVERLAP_LEAF, OVERLAP_REASON)),
        Shape::Union => Err(refusal(prepared, type_id, UNION_LEAF, UNION_REASON)),
    }
}

/// Materialize one reference leaf (O9, R3): a null reference, or the reference its cell names in the
/// region's table.
fn read_reference(
    type_id: u32,
    cells: &[Cell],
    prepared: &PreparedModule,
    references: Option<&ReferenceTable>,
) -> Result<Value, RuntimeError> {
    match cells.first() {
        Some(Cell::NullRef) => Ok(Value::Reference(None)),
        Some(Cell::Ref(RefSlot(cell))) => references
            .and_then(|table| table.get(*cell))
            .map(|reference| Value::Reference(Some(reference.clone())))
            .ok_or(RuntimeError::InvalidExecutionState),
        Some(_) => Err(refusal(
            prepared,
            type_id,
            REFERENCE_LEAF,
            REFERENCE_TARGET_REASON,
        )),
        None => Err(RuntimeError::InvalidExecutionState),
    }
}

/// Materialize one text leaf from the buffer its cell names (C1-7).
fn read_text(
    type_id: u32,
    prim_id: u16,
    cells: &[Cell],
    prepared: &PreparedModule,
    text: &TextStore,
) -> Result<Value, RuntimeError> {
    let Some(Cell::Text(slot)) = cells.first() else {
        return Err(refusal(prepared, type_id, "cell", KIND_REASON));
    };
    let wide = prim_id == WSTRING_PRIMITIVE;
    let stored = text
        .text(*slot, wide)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    Ok(if wide {
        Value::WString(stored)
    } else {
        Value::String(SmolStr::new(stored))
    })
}

/// Materialize one scalar cell as the value of the type's primitive encoding.
pub(super) fn read_scalar(
    type_id: u32,
    prim_id: u16,
    cells: &[Cell],
    prepared: &PreparedModule,
) -> Result<Value, RuntimeError> {
    let cell = *cells.first().ok_or(RuntimeError::InvalidExecutionState)?;
    let Some(value) = cell_slot(cell) else {
        return Err(refusal(prepared, type_id, "cell", KIND_REASON));
    };
    if !primitive_value_matches(prim_id, &value) {
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
    text: &TextStore,
    references: Option<&ReferenceTable>,
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
        let value = read_one(range, field.type_id, prepared, text, references)?;
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
    text: &TextStore,
    references: Option<&ReferenceTable>,
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
        .map(|element| read_one(element, elem_type, prepared, text, references))
        .collect::<Result<Vec<Value>, RuntimeError>>()?;
    Ok(Value::Array(alloc::boxed::Box::new(
        ArrayValue::from_canonical_parts(elements, array.dims.clone()),
    )))
}

/// The scalar value of a scalar cell, or `None` for a cell that holds no scalar.
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
        | Cell::Text(_)
        | Cell::Ref(_)
        | Cell::Null
        | Cell::NullRef => return None,
    })
}
