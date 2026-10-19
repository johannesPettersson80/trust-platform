//! A structure with a physical layout in a flat block (specification 34 MEM-12; C1-15): a
//! `STRUCT OVERLAP`, or a structure nested in one.
//!
//! Its cells hold its backing (the bits of every member, the gaps between them and the bits of the
//! structures it nests), then each member that has no bits there (a text, a reference, a union, or
//! an array of them) at its own layout. A structure value is written as its backing and those
//! members, and read back with every other member decoded from the bits, so a read gives the value
//! that the value store keeps beside the same backing.

use super::*;
use crate::memory::layout::OverlayLayout;
use crate::value::overlap::{decode_overlay, PhysicalLayout};
use crate::value::{read_value_path_borrowed, RefPath, StructValue};
use alloc::sync::Arc;

/// Why a structure whose backing has another layout is refused.
const LAYOUT_REASON: &str = "the value's backing has another physical layout";

/// Check that `value` can be stored in a structure of `type_id`, resolved to `overlay`: a structure
/// whose backing has the type's physical layout, or one without a backing, whose backing the store
/// builds from its members as the value store admits it; and each member beside the backing a value
/// its own type takes.
pub(super) fn check(
    type_id: u32,
    overlay: u32,
    value: &Value,
    prepared: &PreparedModule,
    accept_references: bool,
) -> Result<(), RuntimeError> {
    let (physical, placed) = layouts(prepared, overlay)?;
    let Value::Struct(structure) = value else {
        return Err(refusal(prepared, type_id, value_kind(value), KIND_REASON));
    };
    match structure.physical_backing() {
        Some((own, _)) if !own.storage_compatible(&physical) => {
            return Err(refusal(prepared, type_id, OVERLAP_LEAF, LAYOUT_REASON));
        }
        Some(_) => {}
        None => {
            admitted(structure, physical)?;
        }
    }
    for slot in &placed.slots {
        let member = read_value_path_borrowed(value, &slot.path)
            .ok_or_else(|| refusal(prepared, type_id, OVERLAP_LEAF, MISSING_MEMBER_REASON))?;
        check_value(slot.type_id, member, prepared, accept_references)?;
    }
    Ok(())
}

/// Write the checked structure `value` into `bytes`, the cells of a structure resolved to
/// `overlay` that start at byte `start` of their block: its backing, then each member beside it.
pub(super) fn store(
    bytes: &mut [u8],
    overlay: u32,
    value: &Value,
    prepared: &PreparedModule,
    mut references: Option<&mut ReferenceTable>,
    start: u32,
) -> Result<(), RuntimeError> {
    let Value::Struct(structure) = value else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let (physical, placed) = layouts(prepared, overlay)?;
    let built;
    let backing = match structure.physical_backing() {
        Some((_, backing)) => backing,
        None => {
            built = admitted(structure, physical)?;
            built
                .physical_backing()
                .map(|(_, backing)| backing)
                .ok_or(RuntimeError::InvalidExecutionState)?
        }
    };
    let (own, beside) = bytes
        .split_at_mut_checked(placed.backing_bytes as usize)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    if own.len() != backing.len() {
        return Err(RuntimeError::InvalidExecutionState);
    }
    own.copy_from_slice(backing);
    beside.fill(0);
    for slot in &placed.slots {
        let member = read_value_path_borrowed(value, &slot.path)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let size = layout_of(prepared, slot.type_id)?.shape.bytes;
        store_value(
            part_mut(bytes, slot.byte_offset, size)?,
            slot.type_id,
            member,
            prepared,
            references.as_deref_mut(),
            offset_by(start, slot.byte_offset)?,
        )?;
    }
    Ok(())
}

/// The structure the cells `bytes` of a structure resolved to `overlay`, starting at byte `start` of
/// their block, hold: every member decoded from the backing, each member beside it read from its
/// own bytes with the region's reference table `references`.
pub(super) fn read(
    overlay: u32,
    bytes: &[u8],
    prepared: &PreparedModule,
    references: Option<&ReferenceTable>,
    start: u32,
) -> Result<Value, RuntimeError> {
    let (physical, placed) = layouts(prepared, overlay)?;
    let backing = part(bytes, 0, placed.backing_bytes)?;
    let mut refused = None;
    let value = decode_overlay(
        prepared,
        &physical,
        backing,
        overlay,
        &mut RefPath::new(),
        &mut |path, _| {
            let read = beside(bytes, placed, path, prepared, references, start);
            read.map_err(|error| refused = Some(error)).ok()
        },
    );
    value.ok_or_else(|| refused.unwrap_or(RuntimeError::InvalidExecutionState))
}

/// The member at `path` beside the backing in the cells `bytes`, read from its own bytes.
fn beside(
    bytes: &[u8],
    placed: &OverlayLayout,
    path: &[RefSegment],
    prepared: &PreparedModule,
    references: Option<&ReferenceTable>,
    start: u32,
) -> Result<Value, RuntimeError> {
    let slot = placed
        .slots
        .iter()
        .find(|slot| slot.path == path)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    let size = layout_of(prepared, slot.type_id)?.shape.bytes;
    super::read::read_one(
        part(bytes, slot.byte_offset, size)?,
        slot.type_id,
        prepared,
        references,
        offset_by(start, slot.byte_offset)?,
    )
}

/// The physical layout and the flat layout of the structure type `overlay`.
fn layouts(
    prepared: &PreparedModule,
    overlay: u32,
) -> Result<(Arc<PhysicalLayout>, &OverlayLayout), RuntimeError> {
    let physical = prepared
        .vm
        .physical_layout(overlay)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    let placed = layout_of(prepared, overlay)?
        .overlay
        .as_ref()
        .ok_or(RuntimeError::InvalidExecutionState)?;
    Ok((physical, placed))
}

/// `structure` given the physical layout `physical`, its backing built from its members as the value
/// store admits a structure without one.
fn admitted(
    structure: &StructValue,
    physical: Arc<PhysicalLayout>,
) -> Result<StructValue, RuntimeError> {
    structure.clone().with_physical_layout(Some(physical))
}
