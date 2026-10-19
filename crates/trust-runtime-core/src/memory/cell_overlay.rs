//! Structures with a physical layout in the cells (specification 34 MEM-12; C1-15).
//!
//! A `STRUCT OVERLAP`, and every structure nested in one, lies in its cells as its backing, with
//! each member that has no bits there (a text, a reference, a union, or an array of them) beside
//! it at its own layout. A member of the backing, an element of an array in it or a structure it
//! nests is then no byte range of its own but a node of the backing. A node is named by an anchor
//! below it (a scalar leaf of the physical layout, or a member beside the backing, numbered after
//! the leaves) and the number of the anchor's path segments that reach it. A view of a node
//! ([`ValueView::Bits`]) reads its value from the backing when it is asked, a scalar leaf without
//! allocating, and steps to a node's members and elements as a structure's or an array's view does;
//! a step to a member beside the backing gives the view of its own cells.

use super::cell_charge::CopyCharge;
use super::*;
use crate::memory::layout::{OverlayLayout, OverlaySlot};
use crate::value::overlap::{decode_overlay, overlay_node_type, paths_match, PhysicalLayout};

/// Where a step inside a structure with a physical layout lands.
#[derive(Clone, Copy)]
pub(crate) enum OverlayNode {
    /// A node of the backing: the first `depth` path segments of anchor `anchor`.
    Bits {
        /// A scalar leaf or a member beside the backing below the node.
        anchor: u32,
        /// The number of the anchor's path segments that name the node.
        depth: u8,
    },
    /// A member beside the backing, at its own cells.
    Beside {
        /// The member's cells.
        range: ByteRange,
        /// The member's type.
        type_id: u32,
    },
}

impl<'a> CellsView<'a> {
    /// The physical layout of `type_id` when it is a structure that has one, or `None`.
    pub(crate) fn overlay_layout(self, type_id: u32) -> Option<&'a PhysicalLayout> {
        self.prepared
            .vm
            .physical_layout_ref(type_id)
            .map(AsRef::as_ref)
    }

    /// The physical layout and the flat layout of the structure `type_id`.
    fn overlay_parts(self, type_id: u32) -> Option<(&'a PhysicalLayout, &'a OverlayLayout)> {
        let placed = self.prepared.type_layout(type_id)?.overlay.as_ref()?;
        Some((self.overlay_layout(type_id)?, placed))
    }

    /// The path of the node `anchor`/`depth` of the structure `type_id`: empty for the root, at a
    /// depth of zero.
    fn overlay_path(self, type_id: u32, anchor: u32, depth: u8) -> Option<&'a [RefSegment]> {
        if depth == 0 {
            return Some(&[]);
        }
        let (layout, placed) = self.overlay_parts(type_id)?;
        let path = match layout.leaf_path(anchor) {
            Some(path) => path,
            None => {
                let slot = (anchor as usize).checked_sub(layout.leaf_count())?;
                placed.slots.get(slot)?.path.as_slice()
            }
        };
        path.get(..usize::from(depth))
    }

    /// The cells of the member `slot` beside the backing of the structure whose cells are `range`.
    fn slot_range(self, range: ByteRange, slot: &OverlaySlot) -> Option<ByteRange> {
        let bytes = self.prepared.type_layout(slot.type_id)?.shape.bytes;
        Some(ByteRange {
            region: range.region,
            start: range.start.checked_add(slot.byte_offset)?,
            bytes,
        })
    }

    /// The backing of the structure `type_id` whose cells are `range`.
    fn backing(self, range: ByteRange, type_id: u32) -> Option<&'a [u8]> {
        let (_, placed) = self.overlay_parts(type_id)?;
        self.bytes(ByteRange {
            bytes: placed.backing_bytes,
            ..range
        })
    }

    /// The node that `segment` names inside the node `anchor`/`depth` (the root at a depth of zero)
    /// of the structure `type_id` whose cells are `range`.
    pub(crate) fn overlay_step(
        self,
        range: ByteRange,
        type_id: u32,
        anchor: u32,
        depth: u8,
        segment: &RefSegment,
    ) -> Option<OverlayNode> {
        let (layout, placed) = self.overlay_parts(type_id)?;
        let prefix = self.overlay_path(type_id, anchor, depth)?;
        let next = depth.checked_add(1)?;
        if let Some(leaf) = layout.step_below(prefix, segment) {
            return Some(OverlayNode::Bits {
                anchor: leaf,
                depth: next,
            });
        }
        let index = placed.slots.iter().position(|slot| {
            slot.path.len() > prefix.len()
                && paths_match(&slot.path[..prefix.len()], prefix)
                && paths_match(
                    core::slice::from_ref(&slot.path[prefix.len()]),
                    core::slice::from_ref(segment),
                )
        })?;
        let slot = &placed.slots[index];
        if slot.path.len() == usize::from(next) {
            return Some(OverlayNode::Beside {
                range: self.slot_range(range, slot)?,
                type_id: slot.type_id,
            });
        }
        Some(OverlayNode::Bits {
            anchor: u32::try_from(layout.leaf_count().checked_add(index)?).ok()?,
            depth: next,
        })
    }

    /// The type of the node `anchor`/`depth` of the structure `type_id`.
    pub(crate) fn overlay_node_type(self, type_id: u32, anchor: u32, depth: u8) -> Option<u32> {
        let path = self.overlay_path(type_id, anchor, depth)?;
        overlay_node_type(self.prepared, type_id, path)
    }

    /// The value of the node `anchor`/`depth` of the structure `type_id` whose cells are `range`.
    pub(crate) fn overlay_value(
        self,
        range: ByteRange,
        type_id: u32,
        anchor: u32,
        depth: u8,
    ) -> Result<Value, RuntimeError> {
        let layout = self
            .prepared
            .vm
            .physical_layout_ref(type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let (_, placed) = self
            .overlay_parts(type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let node = self
            .overlay_node_type(type_id, anchor, depth)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let backing = self
            .backing(range, type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let mut path = self
            .overlay_path(type_id, anchor, depth)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .to_vec();
        let mut refused = None;
        let value = decode_overlay(
            self.prepared,
            layout,
            backing,
            node,
            &mut path,
            &mut |member, _| {
                let read = placed
                    .slots
                    .iter()
                    .find(|slot| slot.path == member)
                    .and_then(|slot| self.slot_range(range, slot).map(|at| (at, slot.type_id)))
                    .ok_or(RuntimeError::InvalidExecutionState)
                    .and_then(|(at, slot_type)| self.materialize(at, slot_type));
                read.map_err(|error| refused = Some(error)).ok()
            },
        );
        value.ok_or_else(|| refused.unwrap_or(RuntimeError::InvalidExecutionState))
    }

    /// The scalar of the node `anchor`/`depth` when it is a scalar leaf, read without allocating;
    /// `None` for any other node, which owns storage or names an enumeration.
    pub(crate) fn overlay_scalar(
        self,
        range: ByteRange,
        type_id: u32,
        anchor: u32,
        depth: u8,
    ) -> Result<Option<Value>, RuntimeError> {
        let (layout, _) = self
            .overlay_parts(type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if layout
            .leaf_path(anchor)
            .is_none_or(|path| path.len() != usize::from(depth))
        {
            return Ok(None);
        }
        let backing = self
            .backing(range, type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        Ok(layout.scalar_leaf(anchor, backing))
    }

    /// The number of members of the structure, or elements of the array, that a node of type
    /// `node_type` is; `None` for a leaf.
    pub(crate) fn overlay_child_count(self, node_type: u32) -> Result<Option<usize>, RuntimeError> {
        let (_, entry) =
            resolve_entry(self.prepared, node_type).ok_or(RuntimeError::InvalidExecutionState)?;
        Ok(match &entry.data {
            TypeData::Struct { fields } => Some(fields.len()),
            TypeData::Array { dims, .. } => Some(
                dims.iter()
                    .try_fold(1usize, |count, (lower, upper)| {
                        let length =
                            usize::try_from(upper.checked_sub(*lower)?.checked_add(1)?).ok()?;
                        count.checked_mul(length)
                    })
                    .ok_or(RuntimeError::Overflow)?,
            ),
            _ => None,
        })
    }

    /// The node of member or element `ordinal` of the node `anchor`/`depth`, whose type is
    /// `node_type`, of the structure `type_id` whose cells are `range`: a member in declaration
    /// order, an element in flat order.
    pub(crate) fn overlay_child(
        self,
        range: ByteRange,
        type_id: u32,
        anchor: u32,
        depth: u8,
        node_type: u32,
        ordinal: usize,
    ) -> Result<OverlayNode, RuntimeError> {
        let (_, entry) =
            resolve_entry(self.prepared, node_type).ok_or(RuntimeError::InvalidExecutionState)?;
        let segment = match &entry.data {
            TypeData::Struct { fields } => {
                let field = fields
                    .get(ordinal)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                RefSegment::Field(SmolStr::new(
                    member_name(self.prepared, field.name_idx)
                        .ok_or(RuntimeError::InvalidExecutionState)?,
                ))
            }
            TypeData::Array { dims, .. } => {
                let mut indices = alloc::vec![0i64; dims.len()];
                let mut remaining = ordinal;
                for (index, (lower, upper)) in indices.iter_mut().zip(dims).rev() {
                    let length = usize::try_from(upper - lower + 1)
                        .map_err(|_| RuntimeError::InvalidExecutionState)?;
                    *index = lower
                        .checked_add(
                            i64::try_from(remaining % length)
                                .map_err(|_| RuntimeError::Overflow)?,
                        )
                        .ok_or(RuntimeError::Overflow)?;
                    remaining /= length;
                }
                RefSegment::Index(indices)
            }
            _ => return Err(RuntimeError::InvalidExecutionState),
        };
        self.overlay_step(range, type_id, anchor, depth, &segment)
            .ok_or(RuntimeError::InvalidExecutionState)
    }

    /// The cells and the type of member `index` beside the backing of the structure `type_id` whose
    /// cells are `range`, in the order the members are placed; `None` past the last.
    pub(crate) fn beside_backing(
        self,
        range: ByteRange,
        type_id: u32,
        index: usize,
    ) -> Option<(ByteRange, u32)> {
        let (_, placed) = self.overlay_parts(type_id)?;
        let slot = placed.slots.get(index)?;
        Some((self.slot_range(range, slot)?, slot.type_id))
    }

    /// Whether a value of `type_id` holds a reference leaf, null or not.
    pub(crate) fn holds_reference(self, type_id: u32) -> bool {
        self.prepared
            .type_layout(type_id)
            .is_some_and(|layout| !layout.reference_leaves.is_empty())
    }

    /// Whether a structure node of type `node_type` declares a member named `name`, in any case, as
    /// its structure value does by its field names.
    pub(crate) fn overlay_has_field(self, node_type: u32, name: &str) -> bool {
        let Some((_, entry)) = resolve_entry(self.prepared, node_type) else {
            return false;
        };
        let TypeData::Struct { fields } = &entry.data else {
            return false;
        };
        fields.iter().any(|field| {
            member_name(self.prepared, field.name_idx)
                .is_some_and(|declared| declared.eq_ignore_ascii_case(name))
        })
    }

    /// The first member beside the backing below the node at `prefix` of the structure `type_id`:
    /// the members beside a node follow each other, in the order a walk of its type meets them.
    fn first_slot_below(self, type_id: u32, prefix: &[RefSegment]) -> Option<usize> {
        let (_, placed) = self.overlay_parts(type_id)?;
        Some(
            placed
                .slots
                .iter()
                .position(|slot| {
                    slot.path.len() >= prefix.len()
                        && paths_match(&slot.path[..prefix.len()], prefix)
                })
                .unwrap_or(placed.slots.len()),
        )
    }
}

impl<'a> CellsView<'a> {
    /// The charge of one copy of the node `anchor`/`depth` (the root at a depth of zero) of the
    /// structure `type_id` whose cells are `range` (TIME-08; C1-15): what the copy of the value it
    /// decodes to charges. A structure or an array charges one unit for itself and one for each
    /// member or element, an enumeration one unit, each member or element that is no structure or
    /// array one cell, and each member beside the backing what the copy of its own cells charges.
    pub(crate) fn overlay_copy_charge(
        self,
        range: ByteRange,
        type_id: u32,
        anchor: u32,
        depth: u8,
        nesting: usize,
    ) -> Result<CopyCharge, RuntimeError> {
        let mut walk = self.overlay_walk(range, type_id, anchor, depth)?;
        let mut work = 0usize;
        let allocation = walk.charge(walk.node, nesting, &mut |units| {
            work = work.checked_add(units).ok_or(RuntimeError::Overflow)?;
            Ok(())
        })?;
        Ok(CopyCharge { work, allocation })
    }

    /// The work of one copy of the node `anchor`/`depth` of the structure `type_id` whose cells are
    /// `range`, charged through `charge` node by node in the order the copy of its value charges it.
    pub(crate) fn overlay_copy_work(
        self,
        range: ByteRange,
        type_id: u32,
        anchor: u32,
        depth: u8,
        nesting: usize,
        charge: &mut impl FnMut(usize) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        let mut walk = self.overlay_walk(range, type_id, anchor, depth)?;
        walk.charge(walk.node, nesting, charge).map(drop)
    }

    /// The walk of the copy charge of the node `anchor`/`depth`.
    fn overlay_walk(
        self,
        range: ByteRange,
        type_id: u32,
        anchor: u32,
        depth: u8,
    ) -> Result<CopyWalk<'a>, RuntimeError> {
        let prefix = self
            .overlay_path(type_id, anchor, depth)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let (_, placed) = self
            .overlay_parts(type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        Ok(CopyWalk {
            cells: self,
            range,
            slots: &placed.slots,
            next: self
                .first_slot_below(type_id, prefix)
                .ok_or(RuntimeError::InvalidExecutionState)?,
            node: overlay_node_type(self.prepared, type_id, prefix)
                .ok_or(RuntimeError::InvalidExecutionState)?,
        })
    }
}

/// A walk of the type of a node of a structure with a physical layout, in declaration and flat
/// order, which meets its members beside the backing in the order they are placed.
struct CopyWalk<'a> {
    cells: CellsView<'a>,
    /// The structure's cells.
    range: ByteRange,
    slots: &'a [OverlaySlot],
    /// The member beside the backing the walk meets next.
    next: usize,
    /// The node's type.
    node: u32,
}

impl CopyWalk<'_> {
    /// Charge the work of the copy of a node of `type_id` through `charge` and return its
    /// allocation.
    fn charge(
        &mut self,
        type_id: u32,
        nesting: usize,
        charge: &mut impl FnMut(usize) -> Result<(), RuntimeError>,
    ) -> Result<usize, RuntimeError> {
        if nesting > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let prepared = self.cells.prepared;
        if crate::memory::layout::lies_beside_backing(&prepared.vm.types, type_id) {
            let slot = self
                .slots
                .get(self.next)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            self.next += 1;
            let at = self
                .cells
                .slot_range(self.range, slot)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            self.cells.copy_work(at, slot.type_id, nesting, charge)?;
            return Ok(self
                .cells
                .copy_charge(at, slot.type_id, nesting)?
                .allocation);
        }
        let (_, entry) =
            resolve_entry(prepared, type_id).ok_or(RuntimeError::InvalidExecutionState)?;
        match &entry.data {
            TypeData::Struct { fields } => {
                charge(1)?;
                charge(fields.len())?;
                fields.iter().try_fold(0usize, |total, field| {
                    let child = self.child(field.type_id, nesting, charge)?;
                    total.checked_add(child).ok_or(RuntimeError::Overflow)
                })
            }
            TypeData::Array { elem_type_id, .. } => {
                let count = self
                    .cells
                    .overlay_child_count(type_id)?
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                charge(1)?;
                charge(count)?;
                (0..count).try_fold(0usize, |total, _| {
                    let child = self.child(*elem_type_id, nesting, charge)?;
                    total.checked_add(child).ok_or(RuntimeError::Overflow)
                })
            }
            TypeData::Enum { .. } => charge(1).map(|()| 0),
            _ => Ok(0),
        }
    }

    /// The allocation a parent's copy takes for one child of `type_id`, whose work is charged: the
    /// child's own, and one cell when the child is a leaf of the parent (`vm::engine::allocation`).
    fn child(
        &mut self,
        type_id: u32,
        nesting: usize,
        charge: &mut impl FnMut(usize) -> Result<(), RuntimeError>,
    ) -> Result<usize, RuntimeError> {
        let own = self.charge(type_id, nesting + 1, charge)?;
        if super::cell_charge::is_compound(self.cells.prepared, type_id) {
            return Ok(own);
        }
        own.checked_add(crate::memory::layout::CELL_BYTES)
            .ok_or(RuntimeError::Overflow)
    }
}
