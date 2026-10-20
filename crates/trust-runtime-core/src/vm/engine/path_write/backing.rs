//! The charge of a write into a structure that lies as its backing (specification 34 MEM-12,
//! TIME-08; C1-15b).
//!
//! On the flat store a `STRUCT OVERLAP`, and every structure nested in one, lies as its backing. A
//! write into it is charged what the value store charges for the same write into the value the
//! backing decodes to: at the first structure on the path, the physical layout's write cost of the
//! rest of the path, and until one is reached, a step into the member or element the next segment
//! names. The cost is computed from the layout, so the structure is not decoded.

use super::*;
use crate::value::overlap::overlay_node_type;

impl EngineState<'_> {
    /// The charge of a write to `path` inside `view`, a structure with a physical layout or a node of
    /// its backing: what [`Self::charge_path_write_inner`] charges for the value it decodes to.
    pub(super) fn charge_backing_path_write<S: StoreAccess>(
        &self,
        storage: &S,
        view: ValueView<'_>,
        path: &[RefSegment],
        input: &Value,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        let node = match view {
            ValueView::Cells { type_id, .. } => type_id,
            ValueView::Bits {
                cells,
                type_id,
                leaf,
                depth: node_depth,
                ..
            } => cells
                .overlay_node_type(type_id, leaf, node_depth)
                .ok_or(RuntimeError::InvalidExecutionState)?,
            ValueView::Value(_) | ValueView::Instance(_) => {
                return Err(RuntimeError::InvalidExecutionState);
            }
        };
        // Every structure in a backing has a physical layout of its own, as its value does.
        if let Some(layout) = self.prepared.vm.physical_layout_ref(node) {
            let path = layout.canonical_path(path).unwrap_or(path);
            if path.is_empty() && whole_assignment_is_admitted(layout, input) {
                return Ok(());
            }
            let fields = match self.resolved_data(node) {
                Some(crate::bytecode::TypeData::Struct { fields }) => fields.len(),
                _ => return Err(RuntimeError::InvalidExecutionState),
            };
            let cost = layout.write_cost(path, fields, &mut |leaf| {
                self.enumeration_name_len(node, leaf)
            })?;
            if let Some(cost) = cost {
                self.charge_work_units(cost.work)?;
                self.charge_allocation_bytes(cost.allocation)?;
                if let Some(children) = layout.affected_child_paths(path) {
                    for child in children {
                        match child.strip_prefix(path) {
                            Some(suffix) => validate_physical_child(input, suffix, depth)?,
                            // The stored structure holds a structure at every child path of its
                            // layout; only the walk's depth is checked.
                            None if !child.is_empty()
                                && depth + child.len()
                                    > crate::bytecode::BYTECODE_MAX_TYPE_NESTING =>
                            {
                                return Err(RuntimeError::TypeMismatch);
                            }
                            None => {}
                        }
                    }
                }
                return Ok(());
            }
        }
        let Some((segment, rest)) = path.split_first() else {
            return Ok(());
        };
        if depth >= crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let child = view
            .walk(core::slice::from_ref(segment))
            .ok_or(RuntimeError::NullReference)?;
        self.charge_path_write_inner(storage, child, rest, input, depth + 1)
    }

    /// The data of `type_id` with its aliases resolved.
    pub(in crate::vm::engine) fn resolved_data(
        &self,
        type_id: u32,
    ) -> Option<&crate::bytecode::TypeData> {
        let resolved =
            crate::vm::type_policy::resolved_alias_type(&self.prepared.vm.types, type_id, 0)?;
        self.prepared
            .vm
            .types
            .entries
            .get(resolved as usize)
            .map(|entry| &entry.data)
    }

    /// The bytes of the type name of the enumeration at `leaf` inside a structure of `type_id`, as
    /// the value the backing decodes to names it; 0 for a leaf of another type.
    pub(in crate::vm::engine) fn enumeration_name_len(
        &self,
        type_id: u32,
        leaf: &[RefSegment],
    ) -> usize {
        let Some(node) = overlay_node_type(self.prepared, type_id, leaf) else {
            return 0;
        };
        let Some(resolved) =
            crate::vm::type_policy::resolved_alias_type(&self.prepared.vm.types, node, 0)
        else {
            return 0;
        };
        match self.prepared.vm.types.entries.get(resolved as usize) {
            Some(entry) if matches!(entry.data, crate::bytecode::TypeData::Enum { .. }) => entry
                .name_idx
                .and_then(|name| self.prepared.vm.strings.get(name as usize))
                .map_or(0, str::len),
            _ => 0,
        }
    }
}

/// Whether `input` replaces a whole structure of `layout` with its own backing, as
/// `StructValue::physical_whole_assignment_is_admitted` admits it: a structure whose backing has
/// the same layout.
fn whole_assignment_is_admitted(
    layout: &alloc::sync::Arc<crate::value::overlap::PhysicalLayout>,
    input: &Value,
) -> bool {
    match input {
        Value::Struct(source) => source
            .physical_backing()
            .is_some_and(|(own, _)| alloc::sync::Arc::ptr_eq(own, layout)),
        _ => false,
    }
}
