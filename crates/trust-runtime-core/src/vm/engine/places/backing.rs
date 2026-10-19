//! Places of structures with a physical layout (specification 34 MEM-12, ARCH-05; C1-15b).
//!
//! On the flat store a `STRUCT OVERLAP`, and every structure nested in one, lies as its backing with
//! its texts beside it. A load of such a structure that holds no reference leaves it where it lies,
//! as its cells, and so does a load of a structure nested in a backing whose members all lie in the
//! backing: a nested structure starts and ends on a byte, so its bytes there are the cells of a
//! structure of its type. A store into a destination of the type copies those bytes. A copy into a
//! structure nested in a backing is charged the physical layout's write cost and checked as the
//! value store's physical writer checks the write of the same value there, before any byte
//! changes.

use super::*;
use crate::memory::cell_access::{CellsView, OverlayNode};
use crate::value::overlap::check_node_write;
use crate::vm::construction::values::{NodeTarget, Target};

impl EngineState<'_> {
    /// Whether a store may copy a structure of `type_id` that lies as its backing as its cells: one
    /// that holds no reference, so its copy is checked by visiting it, as a plain structure's is.
    pub(super) fn backing_place(&self, type_id: u32) -> bool {
        crate::vm::prepared::byte_types::backing_place(self.prepared, type_id)
    }

    /// The bytes and the type of the structure nested at the node `anchor`/`depth` of the
    /// structure `root_type` whose cells are `root`, when they are the cells of a structure of its
    /// type: a structure a store may copy as its cells, with no member beside the backing.
    pub(super) fn nested_cells(
        &self,
        cells: CellsView<'_>,
        root: ByteRange,
        root_type: u32,
        anchor: u32,
        depth: u8,
    ) -> Option<(ByteRange, u32)> {
        let node_type = cells.overlay_node_type(root_type, anchor, depth)?;
        if !self.backing_place(node_type) {
            return None;
        }
        let placed = cells.node_cells(root, root_type, Some((anchor, depth)))?;
        let own = self.prepared.type_layout(node_type)?;
        (placed.slots == 0 && placed.backing.bytes == own.shape.bytes)
            .then_some((placed.backing, node_type))
    }

    /// The charges of the texts beside the backing of the structure `type_id` whose cells are
    /// `range`, as the members of a structure, in the order a walk of its value reaches them.
    pub(super) fn charge_slot_texts(
        &self,
        walk: (CellReader<'_>, Option<&[u8]>),
        range: ByteRange,
        type_id: u32,
        rule: TextRule,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        let cells = walk.0.view_cells();
        let placed = cells
            .node_cells(range, type_id, None)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let rule = match rule {
            TextRule::Normalization { .. } => TextRule::Normalization { in_structure: true },
            TextRule::Canonical => TextRule::Canonical,
        };
        for index in placed.first_slot..placed.first_slot + placed.slots {
            let (member, member_type) = cells
                .beside_backing(range, type_id, index)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            if holds_text(self.prepared, member_type) {
                self.charge_texts(walk, member, member_type, rule, depth + 1)?;
            }
        }
        Ok(())
    }

    /// Charge and check the copy of `place` into the structure nested in a backing at `node`, whose
    /// bytes are `range`, as the value store charges and checks the write of the place's value
    /// there: the physical layout's write cost of the node's path, then each leaf of the node with
    /// the bits it takes and each leaf that shares a bit with the node.
    pub(super) fn charge_and_check_node_copy(
        &self,
        place: Place,
        range: ByteRange,
        node: PlaceNode,
    ) -> Result<(), RuntimeError> {
        let Routed::Cells(reader) = self.data.routed(self.prepared) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let cells = reader.view_cells();
        let layout = self
            .prepared
            .vm
            .physical_layout_ref(node.root_type)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let path = cells
            .overlay_node_path(node.root_type, node.anchor, node.depth)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let fields = match self.resolved_data(node.root_type) {
            Some(TypeData::Struct { fields }) => fields.len(),
            _ => return Err(RuntimeError::InvalidExecutionState),
        };
        let cost = layout.write_cost(path, fields, &mut |leaf| {
            self.enumeration_name_len(node.root_type, leaf)
        })?;
        if let Some(cost) = cost {
            self.charge_work_units(cost.work)?;
            self.charge_allocation_bytes(cost.allocation)?;
        }
        let backing = cells
            .node_cells(node.root, node.root_type, None)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .backing;
        let source = cells
            .bytes(place.range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let target = cells
            .bytes(backing)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if range.bytes != place.range.bytes {
            return Err(RuntimeError::InvalidExecutionState);
        }
        check_node_write(layout, path, source, target)
    }

    /// The target of member or element `ordinal` of the node `node`, of `type_id`, that a
    /// construction in place builds (C1-15b): a node of the backing, or the bytes of a member beside
    /// it.
    pub(in crate::vm::engine) fn construction_node_child(
        &self,
        node: NodeTarget,
        type_id: u32,
        ordinal: usize,
    ) -> Result<Target, RuntimeError> {
        let Routed::Cells(reader) = self.data.routed(self.prepared) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let child = reader.view_cells().overlay_child(
            node.root,
            node.root_type,
            node.anchor,
            node.depth,
            type_id,
            ordinal,
        )?;
        Ok(match child {
            OverlayNode::Bits { anchor, depth } => Target::Node(NodeTarget {
                anchor,
                depth,
                ..node
            }),
            OverlayNode::Beside { range, .. } => Target::Bytes(range),
        })
    }

    /// Write `bits` into the leaf `node` of its backing.
    pub(in crate::vm::engine) fn write_construction_leaf(
        &mut self,
        node: NodeTarget,
        bits: u64,
    ) -> Result<(), RuntimeError> {
        let layout = self
            .prepared
            .vm
            .physical_layout_ref(node.root_type)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if layout
            .leaf_path(node.anchor)
            .is_none_or(|leaf| leaf.len() != usize::from(node.depth))
        {
            return Err(RuntimeError::InvalidExecutionState);
        }
        let backing = {
            let Routed::Cells(reader) = self.data.routed(self.prepared) else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            reader
                .view_cells()
                .node_cells(node.root, node.root_type, None)
                .ok_or(RuntimeError::InvalidExecutionState)?
                .backing
        };
        let EngineStore::Cells(arm) = &mut self.data.store else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if arm.write_backing_leaf(backing, layout, node.anchor, bits) {
            Ok(())
        } else {
            Err(RuntimeError::InvalidExecutionState)
        }
    }

    /// Finish the structure of `type_id` a construction in place built at `node`, as
    /// `initialize_structure_layout` finishes its value: its backing's bytes, its staging and its
    /// construction work charged, then every leaf of it checked (C1-15b).
    pub(in crate::vm::engine) fn finish_construction_backing(
        &mut self,
        node: NodeTarget,
        type_id: u32,
    ) -> Result<(), RuntimeError> {
        let layout = self
            .prepared
            .vm
            .physical_layout_ref(type_id)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        self.charge_allocation_bytes(layout.byte_len()?)?;
        let staging = layout.initialization_staging_allocation_by(&mut |leaf| {
            self.enumeration_name_len(type_id, leaf)
        })?;
        self.charge_allocation_bytes(staging)?;
        self.charge_work_units(layout.construction_work()?)?;
        let Routed::Cells(reader) = self.data.routed(self.prepared) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let cells = reader.view_cells();
        let nested = (node.depth > 0).then_some((node.anchor, node.depth));
        let backing = cells
            .node_cells(node.root, node.root_type, nested)
            .ok_or(RuntimeError::InvalidExecutionState)?
            .backing;
        let bytes = cells
            .bytes(backing)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        layout.check_leaves(bytes)
    }

    /// The checks `check_value` makes of the value of `type_id` a construction in place built at
    /// `node` of a backing, charged as it charges them for that value: its texts beside the
    /// backing once each, then the lifetime check's visit (C1-15b).
    pub(in crate::vm::engine) fn check_constructed_node(
        &self,
        node: NodeTarget,
        type_id: u32,
    ) -> Result<(), RuntimeError> {
        if holds_text(self.prepared, type_id) {
            let Routed::Cells(reader) = self.data.routed(self.prepared) else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            let cells = reader.view_cells();
            let placed = cells
                .node_cells(node.root, node.root_type, Some((node.anchor, node.depth)))
                .ok_or(RuntimeError::InvalidExecutionState)?;
            for index in placed.first_slot..placed.first_slot + placed.slots {
                let (member, member_type) = cells
                    .beside_backing(node.root, node.root_type, index)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                if holds_text(self.prepared, member_type) {
                    self.charge_texts((reader, None), member, member_type, TextRule::Canonical, 1)?;
                }
            }
        }
        self.check_place_lifetime(type_id)
    }
}
