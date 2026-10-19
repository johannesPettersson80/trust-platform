//! A structure with a physical layout written from its cells (specification 34 MEM-09; C1-19c4).
//!
//! A `STRUCT OVERLAP`, and every structure nested in one, lies in its cells as its backing, with each
//! member that has no bits there beside it (C1-15). The capture writes what `encode_value` writes for
//! the value those cells materialize: each member read from the backing's bits, a member beside the
//! backing from its own cells, and the physical checkpoint of the structure and of each structure it
//! nests that has a layout of its own, with the bits of the backing. It walks the nodes of the
//! backing by their anchors and builds no value and no path.

use alloc::format;
use alloc::vec::Vec;

use super::super::{
    encode_array_header, encode_enumeration, encode_string, encode_struct_header, encode_value,
    NO_PHYSICAL_CHECKPOINT, PHYSICAL_CHECKPOINT,
};
use super::encode_cells;
use crate::bytecode::{TypeData, BYTECODE_MAX_TYPE_NESTING};
use crate::error::RuntimeError;
use crate::memory::cell_access::{CellsView, OverlayNode};
use crate::memory::layout::ByteRange;
use crate::value::RefSegment;

/// The most dimensions of an array the walk names an element of without allocating.
const MOST_DIMENSIONS: usize = 8;

/// Append the encoding of the structure with a physical layout `type_id` whose cells are `range`.
pub(super) fn encode_overlay(
    out: &mut Vec<u8>,
    cells: CellsView<'_>,
    range: ByteRange,
    type_id: u32,
    depth: usize,
) -> Result<(), RuntimeError> {
    encode_node(out, cells, range, type_id, (0, 0), depth)
}

/// Append the encoding of the node `node` (an anchor and a depth; the root at a depth of zero) of the
/// structure `root_type` whose cells are `range`, as decoding it builds its value.
fn encode_node(
    out: &mut Vec<u8>,
    cells: CellsView<'_>,
    range: ByteRange,
    root_type: u32,
    (anchor, node_depth): (u32, u8),
    depth: usize,
) -> Result<(), RuntimeError> {
    if depth > BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let invalid = || RuntimeError::InvalidExecutionState;
    let vm = &cells.prepared().vm;
    let node_type = cells
        .overlay_node_type(root_type, anchor, node_depth)
        .ok_or_else(invalid)?;
    let resolved =
        crate::vm::type_policy::resolved_alias_type(&vm.types, node_type, 0).ok_or_else(invalid)?;
    let entry = vm
        .types
        .entries
        .get(resolved as usize)
        .ok_or_else(invalid)?;
    let type_name = || {
        entry
            .name_idx
            .and_then(|index| vm.strings.get(index as usize))
            .unwrap_or_default()
    };
    let member = |out: &mut Vec<u8>, names: &dyn Fn(&RefSegment) -> bool| match cells
        .overlay_step_by(range, root_type, anchor, node_depth, names)
    {
        Some(OverlayNode::Bits {
            anchor,
            depth: next,
        }) => encode_node(out, cells, range, root_type, (anchor, next), depth + 1),
        Some(OverlayNode::Beside { range, type_id }) => {
            encode_cells(out, cells, range, type_id, depth + 1)
        }
        None => Err(invalid()),
    };
    match &entry.data {
        TypeData::Struct { fields } => {
            encode_struct_header(out, type_name(), fields.len());
            for field in fields {
                let name = vm
                    .strings
                    .get(field.name_idx as usize)
                    .ok_or_else(invalid)?;
                encode_string(out, name);
                member(
                    out,
                    &|segment| matches!(segment, RefSegment::Field(member) if member.eq_ignore_ascii_case(name)),
                )?;
            }
            encode_checkpoint(out, cells, range, root_type, (anchor, node_depth), resolved)
        }
        TypeData::Array { dims, .. } => {
            let count = dims.iter().try_fold(1usize, |count, (lower, upper)| {
                let length = usize::try_from(upper.checked_sub(*lower)?.checked_add(1)?).ok()?;
                count.checked_mul(length)
            });
            let count = count.ok_or_else(invalid)?;
            if dims.len() > MOST_DIMENSIONS {
                return Err(RuntimeError::TypeMismatch);
            }
            encode_array_header(out, count, dims);
            let mut indices = [0i64; MOST_DIMENSIONS];
            for flat in 0..count {
                // The element's subscripts, the last dimension running fastest, as decoding counts.
                let mut remaining = flat;
                for (index, (lower, upper)) in indices.iter_mut().zip(dims).rev() {
                    let length = usize::try_from(upper - lower + 1).map_err(|_| invalid())?;
                    *index = lower
                        .checked_add(i64::try_from(remaining % length).map_err(|_| invalid())?)
                        .ok_or_else(invalid)?;
                    remaining /= length;
                }
                let subscripts = &indices[..dims.len()];
                member(
                    out,
                    &|segment| matches!(segment, RefSegment::Index(at) if at.as_slice() == subscripts),
                )?;
            }
            Ok(())
        }
        TypeData::Enum { .. } => {
            let layout = cells.overlay_layout(root_type).ok_or_else(invalid)?;
            let backing = cells.backing(range, root_type).ok_or_else(invalid)?;
            let (variant, numeric) = layout.enum_leaf(anchor, backing).ok_or_else(invalid)?;
            encode_enumeration(out, type_name(), variant, numeric);
            Ok(())
        }
        _ => {
            let value = cells
                .overlay_scalar(range, root_type, anchor, node_depth)?
                .ok_or_else(invalid)?;
            encode_value(out, &value)
        }
    }
}

/// Append the physical checkpoint of the structure node `node` of type `resolved`, as encoding its
/// value appends it: the whole backing with the root's layout for the root, the node's own bits
/// with its own layout for a structure nested in it that has one, and no checkpoint otherwise.
fn encode_checkpoint(
    out: &mut Vec<u8>,
    cells: CellsView<'_>,
    range: ByteRange,
    root_type: u32,
    (anchor, node_depth): (u32, u8),
    resolved: u32,
) -> Result<(), RuntimeError> {
    let invalid = || RuntimeError::InvalidExecutionState;
    let layout = cells.overlay_layout(root_type).ok_or_else(invalid)?;
    let backing = cells.backing(range, root_type).ok_or_else(invalid)?;
    let own = if node_depth == 0 {
        None
    } else {
        match cells.prepared().vm.physical_layout_ref(resolved) {
            Some(own) => Some(own.as_ref()),
            None => {
                out.push(NO_PHYSICAL_CHECKPOINT);
                return Ok(());
            }
        }
    };
    out.push(PHYSICAL_CHECKPOINT);
    let at = out.len();
    out.extend_from_slice(&[0; 4]);
    let written = match own {
        None => layout.write_checkpoint(out, backing).map(|()| true),
        Some(own) => {
            let path = cells
                .overlay_path(root_type, anchor, node_depth)
                .ok_or_else(invalid)?;
            layout.write_nested_checkpoint(out, backing, path, own)
        }
    }
    .map_err(|error| {
        RuntimeError::RetainStore(
            format!("cannot encode physical retain checkpoint: {error}").into(),
        )
    })?;
    if !written {
        // No structure of its own lies there: no checkpoint, as decoding gives none.
        out.truncate(at - 1);
        out.push(NO_PHYSICAL_CHECKPOINT);
        return Ok(());
    }
    let len = u32::try_from(out.len() - at - 4)
        .map_err(|_| RuntimeError::RetainStore("physical retain checkpoint is too large".into()))?;
    out[at..at + 4].copy_from_slice(&len.to_le_bytes());
    Ok(())
}
