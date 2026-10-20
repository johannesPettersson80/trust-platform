//! Retained values written from a store view, and the rule that decides which are written
//! (specification 11 §6.7, specification 34 §6.2 and §10; C1-9d2b-2).
//!
//! A view of the `Value` tree is written by [`encode_value`](super::encode_value). A view of cells is
//! written from its cells, its text buffers and its compound members, without building a `Value` and
//! without allocating: the bytes are those of the `Value` the same cells would materialize, so a
//! checkpoint does not depend on where the runtime keeps its variables. The two arms of this module
//! walk the same structure: a value that a durable snapshot cannot hold is a reference or an instance,
//! at any depth.

use alloc::vec::Vec;

use super::{
    encode_array_header, encode_enumeration, encode_string, encode_struct_header, encode_value,
    reference_or_instance_refused, ValueTag, NO_PHYSICAL_CHECKPOINT,
};
use crate::bytecode::BYTECODE_MAX_TYPE_NESTING;
use crate::error::RuntimeError;
use crate::memory::cell_access::CellsView;
use crate::memory::cells::CellLeaf;
use crate::memory::layout::ByteRange;
use crate::memory::view::ValueView;
use crate::value::Value;

/// A structure with a physical layout written from its cells (C1-19c4).
mod overlay;

/// Append the STRN v3 encoding of the value `view` names, to the bytes `encode_value` writes for the
/// same value.
///
/// # Errors
///
/// [`RuntimeError::RetainStore`] for a value that cannot be persisted: a non-finite `REAL` or `LREAL`, a
/// reference or an instance. A cell the view's type does not hold is refused as reading it is.
pub(super) fn encode_view(out: &mut Vec<u8>, view: &ValueView<'_>) -> Result<(), RuntimeError> {
    match *view {
        ValueView::Value(value) => encode_value(out, value),
        ValueView::Cells {
            cells,
            range,
            type_id,
        } => encode_cells(out, cells, range, type_id, 0),
        // A node of a backing is written as the value it decodes to (C1-15).
        ValueView::Bits { .. } => encode_value(out, &view.materialize()?),
        // A program root's instance, refused as an instance value is.
        ValueView::Instance(instance) => encode_value(out, &Value::Instance(instance)),
    }
}

/// Whether the value `view` names may go into a durable snapshot: a value that holds a reference or an
/// instance, at any depth, may not, and its variable is left out of the checkpoint (specification 11
/// §6.7). A cell range with no value is the null value, which may.
///
/// # Errors
///
/// [`RuntimeError::TypeMismatch`] for a value nested deeper than the admitted type nesting, and the
/// refusal of a cell the view's type does not hold.
pub(crate) fn view_is_retainable(view: &ValueView<'_>) -> Result<bool, RuntimeError> {
    match *view {
        ValueView::Value(value) => value_is_retainable(value, 0),
        ValueView::Cells {
            cells,
            range,
            type_id,
        } => cells_are_retainable(cells, range, type_id, 0),
        ValueView::Bits { .. } => value_is_retainable(&view.materialize()?, 0),
        ValueView::Instance(_) => Ok(false),
    }
}

/// The rule of [`view_is_retainable`] over a `Value`: a reference or an instance is not retainable, an
/// array or a structure is when everything in it is.
fn value_is_retainable(value: &Value, depth: usize) -> Result<bool, RuntimeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    match value {
        Value::Reference(_) | Value::Instance(_) => Ok(false),
        Value::Array(array) => {
            for element in array.elements() {
                if !value_is_retainable(element, depth + 1)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Value::Struct(structure) => {
            for field in structure.fields().values() {
                if !value_is_retainable(field, depth + 1)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(true),
    }
}

/// The rule of [`view_is_retainable`] over the cells at `range`, a value of `type_id`.
fn cells_are_retainable(
    cells: CellsView<'_>,
    range: ByteRange,
    type_id: u32,
    depth: usize,
) -> Result<bool, RuntimeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let children = match cells.leaf(range, type_id)? {
        CellLeaf::Reference | CellLeaf::Instance => return Ok(false),
        CellLeaf::Structure { members, .. } => members,
        CellLeaf::Array { elements, .. } => elements,
        // A structure with a physical layout holds no instance, and a reference only beside its
        // backing (C1-15).
        CellLeaf::Overlay => return Ok(!cells.holds_reference(type_id)),
        _ => return Ok(true),
    };
    for ordinal in 0..children {
        let (child, child_type) = cells.child(range, type_id, ordinal)?;
        if !cells_are_retainable(cells, child, child_type, depth + 1)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Append the encoding of the cells at `range`, a value of `type_id`: what `encode_value` writes for the
/// value `read_value_in` would build from them.
fn encode_cells(
    out: &mut Vec<u8>,
    cells: CellsView<'_>,
    range: ByteRange,
    type_id: u32,
    depth: usize,
) -> Result<(), RuntimeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    match cells.leaf(range, type_id)? {
        CellLeaf::Null => out.push(ValueTag::Null as u8),
        // The scalar is encoded as a `Value`, so its bytes and its refusal of a non-finite `REAL` or
        // `LREAL` are the codec's own.
        CellLeaf::Scalar(value) => encode_value(out, &value)?,
        CellLeaf::Text { wide: false, bytes } => {
            let text =
                core::str::from_utf8(bytes).map_err(|_| RuntimeError::InvalidExecutionState)?;
            out.push(ValueTag::String as u8);
            encode_string(out, text);
        }
        CellLeaf::Text { wide: true, bytes } => encode_wide_text(out, bytes)?,
        CellLeaf::Enumeration {
            type_name,
            variant,
            value,
        } => encode_enumeration(out, type_name, variant, value),
        CellLeaf::Structure { type_name, members } => {
            encode_struct_header(out, type_name, members);
            for ordinal in 0..members {
                encode_string(out, cells.structure_member_name(type_id, ordinal)?);
                let (member, member_type) = cells.child(range, type_id, ordinal)?;
                encode_cells(out, cells, member, member_type, depth + 1)?;
            }
            // A structure in cells has no physical backing, so no checkpoint follows its members.
            out.push(NO_PHYSICAL_CHECKPOINT);
        }
        CellLeaf::Array {
            dimensions,
            elements,
        } => {
            encode_array_header(out, elements, dimensions);
            for ordinal in 0..elements {
                let (element, element_type) = cells.child(range, type_id, ordinal)?;
                encode_cells(out, cells, element, element_type, depth + 1)?;
            }
        }
        CellLeaf::Reference | CellLeaf::Instance => return Err(reference_or_instance_refused()),
        // An overlapping structure is written from its backing as its value would be, its physical
        // checkpoint carrying the backing (C1-15, C1-19c4).
        CellLeaf::Overlay => overlay::encode_overlay(out, cells, range, type_id, depth)?,
    }
    Ok(())
}

/// Append a `WSTRING` held as little-endian UTF-16 code units: its tag, the byte length of its UTF-8
/// text, and that text, which is how `encode_value` writes a `Value::WString`. The text is transcoded
/// into the payload in two passes over the buffer, the first for the length, so nothing is allocated.
fn encode_wide_text(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), RuntimeError> {
    let (units, remainder) = bytes.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(RuntimeError::InvalidExecutionState);
    }
    let characters = || {
        char::decode_utf16(units.iter().map(|unit| u16::from_le_bytes(*unit)))
            .map(|decoded| decoded.map_err(|_| RuntimeError::InvalidExecutionState))
    };
    let mut length = 0usize;
    for character in characters() {
        length += character?.len_utf8();
    }
    out.push(ValueTag::WString as u8);
    out.extend_from_slice(&(length as u32).to_le_bytes());
    for character in characters() {
        out.extend_from_slice(character?.encode_utf8(&mut [0; 4]).as_bytes());
    }
    Ok(())
}
