//! The retained transfer on the flat store, as bytes (specification 34 §11.3, MEM-12; C1-14b).
//!
//! When the source and the target of a warm restart or a congruent replacement both hold cells, a
//! retained value is copied from the source's cells into the target's instead of being built as a
//! `Value`. The walk of [`RetainedGraph::owned_value`] runs over the cells first, node for node, so it
//! copies the same instances and makes the same checks and charges in the same order; then the bytes
//! are written: an instance leaf with the identity the graph gave its instance, a flat reference with
//! the target's generation, and every other leaf as it is, since congruent stores have the same
//! layouts and so the same offsets. A value that holds a reference with a path, which the value
//! transfer carries by the names of its target, is copied as a value.

use super::*;
use crate::memory::layout::ByteRange;
use crate::memory::view::ValueView;
use crate::value::{RefPath, ValueRef};

impl RetainedGraph<'_, '_> {
    /// Whether `target` takes the retained value `view` in place: both stores hold cells and every
    /// reference the value holds is flat. Charges nothing.
    pub(super) fn copies_in_place(
        &self,
        target: &EngineState<'_>,
        view: ValueView<'_>,
    ) -> Result<bool, RuntimeError> {
        if !target.holds_cells() || !matches!(view, ValueView::Cells { .. }) {
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
        let ValueView::Cells { type_id, .. } = view else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        // A value with no instance or reference leaf has nothing to copy or check, as its `Value`
        // walk finds nothing.
        if !holds_instances_or_references(self.source.prepared, type_id) {
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
            Some(TypeData::Array { .. } | TypeData::Struct { .. }) => {
                let count = view.cell_child_count()?.ok_or(RuntimeError::TypeMismatch)?;
                for ordinal in 0..count {
                    self.owned_cells(target, view.cell_child(ordinal)?, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
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
        store: &mut EngineStore,
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
                Some(TypeData::Array { .. } | TypeData::Struct { .. }) => {
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
        let EngineStore::Cells(arm) = store else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        match rewritten {
            Some(value) => arm.write_in_place(at, type_id, &value, prepared),
            // A leaf that names no instance or reference, and every value without such leaves, has
            // the same bytes in congruent stores.
            None => {
                let bytes = cells
                    .bytes(range)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                if arm.write_bytes(at, bytes) {
                    Ok(())
                } else {
                    Err(RuntimeError::InvalidExecutionState)
                }
            }
        }
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
    let ValueView::Cells { type_id, .. } = view else {
        return Ok(false);
    };
    if prepared
        .type_layout(type_id)
        .is_some_and(|layout| layout.reference_leaves.is_empty())
    {
        return Ok(true);
    }
    match resolved_data(prepared, type_id) {
        Some(TypeData::Reference { .. }) => Ok(view
            .cell_reference()?
            .is_none_or(|reference| matches!(reference.location, MemoryLocation::Flat { .. }))),
        Some(TypeData::Array { .. } | TypeData::Struct { .. }) => {
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
