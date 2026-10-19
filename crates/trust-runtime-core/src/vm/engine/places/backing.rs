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
use crate::memory::cell_access::CellsView;
use crate::value::overlap::check_node_write;

impl EngineState<'_> {
    /// Whether a store may copy a structure of `type_id` that lies as its backing as its cells: one
    /// that holds no reference, so its copy is checked by visiting it, as a plain structure's is.
    pub(super) fn backing_place(&self, type_id: u32) -> bool {
        self.prepared.vm.physical_layout_ref(type_id).is_some()
            && self
                .prepared
                .type_layout(type_id)
                .is_some_and(|layout| layout.reference_leaves.is_empty())
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
        reader: CellReader<'_>,
        range: ByteRange,
        type_id: u32,
        rule: TextRule,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        let cells = reader.view_cells();
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
                self.charge_texts(reader, member, member_type, rule, depth + 1)?;
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
}
