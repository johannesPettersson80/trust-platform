//! The retained transfer on the flat store, as bytes (specification 34 §11.3, MEM-12; C1-14b).
//!
//! When the source and the target of a warm restart or a congruent replacement both hold cells, a
//! retained value is copied from the source's cells into the target's instead of being built as a
//! `Value`. The walk of [`RetainedGraph::owned_value`] runs over the cells first, node for node, so it
//! copies the same instances and makes the same checks and charges in the same order; then the bytes
//! are written: an instance leaf with the identity the graph gave its instance, a flat reference with
//! the target's generation, and every other leaf as it is, since congruent stores have the same
//! layouts and so the same offsets. A value that holds a reference with a path, which the value
//! transfer carries by the names of its target, is copied as a value. A structure with a physical
//! layout is walked as its value is (C1-15): its members beside the backing as values of their own,
//! and each structure node then bound to the target's layout, charged as the value transfer charges
//! that binding; its backing is copied as it is.

use super::*;
use crate::memory::cell_access::CellsView;
use crate::memory::layout::{lies_beside_backing, ByteRange};
use crate::memory::view::ValueView;
use crate::value::{RefPath, ValueRef};

impl RetainedGraph<'_, '_> {
    /// Whether the candidate takes the retained value `view` in place: the value lies in cells and
    /// every reference it holds is flat. Charges nothing.
    pub(super) fn copies_in_place(&self, view: ValueView<'_>) -> Result<bool, RuntimeError> {
        if !matches!(view, ValueView::Cells { .. }) {
            return Ok(false);
        }
        references_are_flat(self.source.prepared, view, 0)
    }

    /// [`RetainedGraph::owned_value`] over the cells of `view`: the same steps, the same instances
    /// copied and bound, and the same reference checks, writing nothing of `view` itself.
    pub(super) fn owned_cells(
        &mut self,
        target: &mut EngineState<'_>,
        view: ValueView<'_>,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        Self::step(depth)?;
        let ValueView::Cells {
            cells,
            range,
            type_id,
        } = view
        else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if cells.overlay_layout(type_id).is_some() {
            let mut next = 0;
            return self.owned_overlay(target, cells, range, type_id, &mut next, type_id, depth);
        }
        // A value with no instance, reference or physical layout has nothing to copy, check or
        // bind, as its `Value` walk finds nothing.
        if !holds_instances_or_references(self.source.prepared, type_id)
            && !holds_backing(self.source.prepared, type_id, 0)
        {
            return Ok(());
        }
        match resolved_data(self.source.prepared, type_id) {
            Some(TypeData::Interface { .. }) => {
                if let Some(old) = view.instance()? {
                    self.binding(target, old, depth + 1)?;
                }
            }
            Some(TypeData::Pou { .. }) => {
                if let Some(old) = view.instance()? {
                    self.instance(target, old, depth + 1)?;
                }
            }
            Some(TypeData::Reference { .. }) => {
                if let Some(reference) = view.cell_reference()? {
                    self.check_flat_reference(&reference, depth + 1)?;
                }
            }
            Some(TypeData::Array { .. } | TypeData::Struct { .. } | TypeData::Union { .. }) => {
                let count = view.cell_child_count()?.ok_or(RuntimeError::TypeMismatch)?;
                for ordinal in 0..count {
                    self.owned_cells(target, view.cell_child(ordinal)?, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// [`RetainedGraph::owned_value`] over the node of type `node` of the structure `type_id` with a
    /// physical layout whose cells are `range`: the members of a structure node in declaration order
    /// and the elements of an array node in flat order, each member beside the backing (the
    /// `next` one the walk meets) as a value of its own, and each structure node then bound to the
    /// target's layout as [`RetainedGraph::owned_value`] binds it.
    #[allow(clippy::too_many_arguments)]
    fn owned_overlay(
        &mut self,
        target: &mut EngineState<'_>,
        cells: CellsView<'_>,
        range: ByteRange,
        type_id: u32,
        next: &mut usize,
        node: u32,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        Self::step(depth)?;
        let prepared = self.source.prepared;
        if lies_beside_backing(&prepared.vm.types, node) {
            let (member, member_type) = cells
                .beside_backing(range, type_id, *next)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            *next += 1;
            let view = ValueView::Cells {
                cells,
                range: member,
                type_id: member_type,
            };
            return self.owned_cells(target, view, depth);
        }
        match resolved_data(prepared, node) {
            Some(TypeData::Struct { fields }) => {
                for field in fields {
                    self.owned_overlay(
                        target,
                        cells,
                        range,
                        type_id,
                        next,
                        field.type_id,
                        depth + 1,
                    )?;
                }
                self.bind_layout(target, node)
            }
            Some(TypeData::Array { elem_type_id, .. }) => {
                let count = cells
                    .overlay_child_count(node)?
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                for _ in 0..count {
                    self.owned_overlay(
                        target,
                        cells,
                        range,
                        type_id,
                        next,
                        *elem_type_id,
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// The binding of a structure node of `type_id` to the target's physical layout, as
    /// [`RetainedGraph::owned_value`] binds the structure's value: nothing when both stores share
    /// the layout, else the target layout's construction work, and a refusal when the layouts do
    /// not hold the same storage.
    fn bind_layout(&self, target: &EngineState<'_>, type_id: u32) -> Result<(), RuntimeError> {
        let source = self.source.prepared.vm.physical_layout_ref(type_id);
        match (source, target.prepared.vm.physical_layout_ref(type_id)) {
            (Some(source), Some(layout)) if Arc::ptr_eq(source, layout) => Ok(()),
            (Some(source), Some(layout)) => {
                target.charge_work_units(layout.construction_work()?)?;
                if source.storage_compatible(layout) {
                    Ok(())
                } else {
                    Err(RuntimeError::TypeMismatch)
                }
            }
            (Some(_), None) => Err(RuntimeError::TypeMismatch),
            (None, _) => Err(RuntimeError::InvalidExecutionState),
        }
    }

    /// The checks [`RetainedGraph::reference`] makes of a flat reference before it rebinds it: one of
    /// the source's generation, naming a variable of its persistent block.
    fn check_flat_reference(
        &self,
        reference: &crate::value::ValueRefView<'_>,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        Self::step(depth)?;
        if reference.generation != StoreAccess::generation(self.source) {
            return Err(RuntimeError::ReferenceLifetime);
        }
        match reference.location {
            MemoryLocation::Flat {
                region: crate::memory::layout::LayoutRegion::Persistent,
                ..
            } => Ok(()),
            _ => Err(RuntimeError::ReferenceLifetime),
        }
    }

    /// Write the retained value `view`, which [`RetainedGraph::owned_cells`] walked, into the bytes `at`
    /// of the target's `store`, whose program is `prepared` and whose generation is `generation`: each
    /// instance leaf with its instance's identity in the target, each flat reference with the target's
    /// generation, and every other leaf as it is. Only the store is borrowed, so a marker reserved in
    /// the target's lifecycle tables stays reserved while the value is published.
    pub(super) fn write_cells(
        &mut self,
        store: &mut CellArm,
        prepared: &PreparedModule,
        generation: crate::memory::Generation,
        view: ValueView<'_>,
        at: ByteRange,
    ) -> Result<(), RuntimeError> {
        let ValueView::Cells {
            cells,
            range,
            type_id,
        } = view
        else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let rewritten = if holds_instances_or_references(self.source.prepared, type_id) {
            match resolved_data(self.source.prepared, type_id) {
                Some(TypeData::Interface { .. } | TypeData::Pou { .. }) => view
                    .instance()?
                    .map(|old| {
                        self.identities
                            .get(&old.0)
                            .map(|new| Value::Instance(InstanceId(*new)))
                            .ok_or(RuntimeError::NullReference)
                    })
                    .transpose()?,
                Some(TypeData::Reference { .. }) => match view.cell_reference()? {
                    Some(reference) => Some(Value::Reference(Some(ValueRef {
                        location: reference.location,
                        offset: u32::try_from(reference.offset)
                            .map_err(|_| RuntimeError::Overflow)?,
                        generation,
                        path: RefPath::new(),
                    }))),
                    None => None,
                },
                // The backing as it is, then each member beside it written as a value of its own.
                Some(TypeData::Struct { .. }) if cells.overlay_layout(type_id).is_some() => {
                    write_bytes(store, at, cells, range)?;
                    let mut index = 0;
                    while let Some((member, member_type)) =
                        cells.beside_backing(range, type_id, index)
                    {
                        index += 1;
                        let offset = member
                            .start
                            .checked_sub(range.start)
                            .ok_or(RuntimeError::InvalidExecutionState)?;
                        let member_at = ByteRange {
                            start: at.start.checked_add(offset).ok_or(RuntimeError::Overflow)?,
                            bytes: member.bytes,
                            ..at
                        };
                        let view = ValueView::Cells {
                            cells,
                            range: member,
                            type_id: member_type,
                        };
                        self.write_cells(store, prepared, generation, view, member_at)?;
                    }
                    return Ok(());
                }
                Some(TypeData::Array { .. } | TypeData::Struct { .. } | TypeData::Union { .. }) => {
                    let count = view.cell_child_count()?.ok_or(RuntimeError::TypeMismatch)?;
                    for ordinal in 0..count {
                        let child = view.cell_child(ordinal)?;
                        let ValueView::Cells { range: inner, .. } = child else {
                            return Err(RuntimeError::InvalidExecutionState);
                        };
                        let offset = inner
                            .start
                            .checked_sub(range.start)
                            .ok_or(RuntimeError::InvalidExecutionState)?;
                        let child_at = ByteRange {
                            start: at.start.checked_add(offset).ok_or(RuntimeError::Overflow)?,
                            bytes: inner.bytes,
                            ..at
                        };
                        self.write_cells(store, prepared, generation, child, child_at)?;
                    }
                    return Ok(());
                }
                _ => None,
            }
        } else {
            None
        };
        match rewritten {
            Some(value) => {
                let arm = store;
                arm.write_in_place(at, type_id, &value, prepared)
            }
            // A leaf that names no instance or reference, and every value without such leaves, has
            // the same bytes in congruent stores.
            None => write_bytes(store, at, cells, range),
        }
    }
}

/// Copy the source's cells `range` into the bytes `at` of the target's `store`.
fn write_bytes(
    store: &mut CellArm,
    at: ByteRange,
    cells: CellsView<'_>,
    range: ByteRange,
) -> Result<(), RuntimeError> {
    let arm = store;
    let bytes = cells
        .bytes(range)
        .ok_or(RuntimeError::InvalidExecutionState)?;
    if arm.write_bytes(&at, bytes) {
        Ok(())
    } else {
        Err(RuntimeError::InvalidExecutionState)
    }
}

/// Whether a value of `type_id` holds a structure with a physical layout, at any depth.
fn holds_backing(prepared: &PreparedModule, type_id: u32, depth: usize) -> bool {
    if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        return false;
    }
    match resolved_data(prepared, type_id) {
        Some(TypeData::Struct { .. }) if prepared.vm.physical_layout_ref(type_id).is_some() => true,
        Some(TypeData::Struct { fields } | TypeData::Union { fields }) => fields
            .iter()
            .any(|field| holds_backing(prepared, field.type_id, depth + 1)),
        Some(TypeData::Array { elem_type_id, .. }) => {
            holds_backing(prepared, *elem_type_id, depth + 1)
        }
        _ => false,
    }
}

/// The data of `type_id` with its aliases resolved.
fn resolved_data(prepared: &PreparedModule, type_id: u32) -> Option<&TypeData> {
    let resolved =
        super::super::super::type_policy::resolved_alias_type(&prepared.vm.types, type_id, 0)?;
    prepared
        .vm
        .types
        .entries
        .get(resolved as usize)
        .map(|entry| &entry.data)
}

/// Whether a value of `type_id` can hold an instance or a reference.
fn holds_instances_or_references(prepared: &PreparedModule, type_id: u32) -> bool {
    prepared.type_layout(type_id).is_none_or(|layout| {
        !layout.instance_leaves.is_empty() || !layout.reference_leaves.is_empty()
    })
}

/// Whether every reference the value `view` holds, outside the instances it holds, is flat.
fn references_are_flat(
    prepared: &PreparedModule,
    view: ValueView<'_>,
    depth: usize,
) -> Result<bool, RuntimeError> {
    if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    let ValueView::Cells {
        cells,
        range,
        type_id,
    } = view
    else {
        return Ok(false);
    };
    if prepared
        .type_layout(type_id)
        .is_some_and(|layout| layout.reference_leaves.is_empty())
    {
        return Ok(true);
    }
    // A structure with a physical layout holds its references beside its backing (C1-15).
    if cells.overlay_layout(type_id).is_some() {
        let mut index = 0;
        while let Some((member, member_type)) = cells.beside_backing(range, type_id, index) {
            index += 1;
            let view = ValueView::Cells {
                cells,
                range: member,
                type_id: member_type,
            };
            if !references_are_flat(prepared, view, depth + 1)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    match resolved_data(prepared, type_id) {
        Some(TypeData::Reference { .. }) => Ok(view
            .cell_reference()?
            .is_none_or(|reference| matches!(reference.location, MemoryLocation::Flat { .. }))),
        Some(TypeData::Array { .. } | TypeData::Struct { .. } | TypeData::Union { .. }) => {
            let count = view.cell_child_count()?.ok_or(RuntimeError::TypeMismatch)?;
            for ordinal in 0..count {
                if !references_are_flat(prepared, view.cell_child(ordinal)?, depth + 1)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(true),
    }
}
