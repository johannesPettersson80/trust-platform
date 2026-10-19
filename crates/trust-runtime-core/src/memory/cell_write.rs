//! The write side of the cell arm (specification 34 §6.2, MEM-10; C1-9c).
//!
//! The writer publishes a value into the cells of a global or of a persistent instance member. A write
//! is normalized by the declared type of its target (O11), checked against that type, and only then
//! copied, so a refused write leaves every cell unchanged (R4). A text leaf takes its capacity cut in
//! its buffer (R3). A non-null reference leaf holds its `ValueRef` on the `Value` side, keyed by its
//! cell (O9). Locals, per-call instances and the program root stay on the `Value` half, as before.

use alloc::vec::Vec;

use smol_str::SmolStr;

use super::{CellArm, CellReader, Target};
use crate::error::RuntimeError;
use crate::memory::cell_access::LeafPatch;
use crate::memory::cells::{
    check_value, flat_cell, is_reference_type, text_geometry, write_value_in, Cell, ReferenceTable,
};
use crate::memory::flat::{self, LeafKind, TextBytes};
use crate::memory::layout::ByteRange;
use crate::memory::view::{OverlayWrite, PreparedWrite, PreparedWriteKind, StoreMutate};
use crate::memory::{InstanceData, InstanceId, MemoryLocation, VariableStorage};
use crate::value::{
    replace_text_element, string_element_count, string_element_position_in, text_element_char,
    RefSegment, Value, ValueRefView,
};
use crate::vm::type_policy::{normalize_primitive_value, resolved_primitive_shape};
use crate::vm::PreparedModule;

/// A prepared write into the block: the range of the leaf, its declared type, the presence byte of
/// the declaration the range is when it keeps one, and the value the leaf will hold, normalized and
/// checked before anything is copied (R4). A reference leaf holds no value here: its check reads the
/// value by reference, and its commit moves the value it is given (R5), so a reference write in a
/// scan clones nothing.
pub(crate) struct CellPatch {
    pub(crate) range: ByteRange,
    pub(crate) type_id: u32,
    pub(crate) presence: Option<u32>,
    pub(crate) leaf: Option<Value>,
    /// For a character write, the character the text leaf at `range` takes and its position: the
    /// commit edits the leaf in place (C1-12).
    pub(crate) element: Option<TextElement>,
    /// Whether the value store stages the same write as a physical patch, which samples the
    /// deadline before it is published (C1-15): a write that reaches the leaf through a structure
    /// with a physical layout, or that replaces such a structure with a value that does not carry
    /// its backing.
    pub(crate) physical: bool,
}

/// One character write into a text leaf: the zero-based position of the character it replaces and
/// the character it writes (C1-12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TextElement {
    pub(crate) position: usize,
    pub(crate) character: char,
}

/// The writer of the cell arm: the writes of [`StoreMutate`] over one arm and the `Value` half the engine
/// lends it, placed by the prepared module (C1-9c). A writer borrows both for the one call that makes it.
pub(crate) struct CellWriter<'a> {
    prepared: &'a PreparedModule,
    arm: &'a mut CellArm,
    /// The `Value` half: the activations' locals and the per-call instances.
    values: &'a mut VariableStorage,
}

impl<'a> CellWriter<'a> {
    /// A writer of `arm` and of the `Value` half `values`, placing the arm's cells through `prepared`.
    pub(crate) fn new(
        prepared: &'a PreparedModule,
        arm: &'a mut CellArm,
        values: &'a mut VariableStorage,
    ) -> Self {
        Self {
            prepared,
            arm,
            values,
        }
    }

    /// A reader of the arm and the `Value` half this writer holds, for the locate and walk that the
    /// writes share.
    fn reader(&self) -> CellReader<'_> {
        CellReader::new(self.prepared, &*self.arm, &*self.values)
    }

    /// Record a persistent instance of template `pou` in the cells of the root at CONSTRUCTION_ROOTS
    /// index `root`, from the store's one identity counter (specification 34 §6.2; R2, O8). Its type
    /// name is the name of `pou`. Its range is the first `own` cells of its root's range, where `own` is
    /// the number of cells the template's own members take.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] when the root owns no range or the instance table is
    /// full, and [`RuntimeError::Overflow`] when the identity counter is exhausted.
    pub(crate) fn create_persistent_instance(
        &mut self,
        root: usize,
        pou: u32,
    ) -> Result<InstanceId, RuntimeError> {
        let root_range = self
            .arm
            .cells
            .regions()
            .root_range(root)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let own = crate::memory::regions::own_bytes(self.prepared, pou);
        let range = ByteRange {
            bytes: own,
            ..root_range
        };
        self.arm.create_persistent_instance(pou, range)
    }

    /// Give `child` the parent `parent`: a persistent instance records it in the arm, and a per-call
    /// instance in the `Value` half. `false` when `child` exists in neither.
    pub(crate) fn set_instance_parent(
        &mut self,
        child: InstanceId,
        parent: Option<InstanceId>,
    ) -> bool {
        if self.arm.set_instance_parent(child, parent) {
            return true;
        }
        match self.values.get_instance_mut(child) {
            Some(record) => {
                record.parent = parent;
                true
            }
            None => false,
        }
    }

    /// Create an instance of template `pou` that a frame on the activation stack owns, on top of the
    /// stack (specification 34 MEM-12; C1-11c): its own variables take their template's bytes.
    pub(crate) fn create_call_instance(&mut self, pou: u32) -> Result<InstanceId, RuntimeError> {
        let own = crate::memory::regions::own_bytes(self.prepared, pou);
        self.arm.create_call_instance(pou, own)
    }

    /// Replace one character of the text leaf at `range`, a value of `type_id`, as `element` says,
    /// the text then cut to the leaf's capacity at the last whole character that fits, and mark the
    /// presence byte at `presence`, when the declaration keeps one (C1-12). The text is edited in the
    /// arm's kept buffer, so the write allocates nothing once the engine is warm. `false` when the
    /// leaf disagrees with its layout.
    fn write_text_element(
        &mut self,
        range: ByteRange,
        type_id: u32,
        presence: Option<u32>,
        element: TextElement,
    ) -> bool {
        let Some(geometry @ (length, _, wide)) = text_geometry(self.prepared, type_id) else {
            return false;
        };
        let mut text = core::mem::take(&mut self.arm.text);
        text.clear();
        let current = self
            .arm
            .cells
            .regions()
            .block(range.region)
            .bytes(range)
            .and_then(|bytes| flat::read_text(bytes, length, wide).ok());
        let read = match current {
            Some(TextBytes::Narrow(current)) => {
                text.push_str(current);
                true
            }
            Some(units @ TextBytes::Wide(_)) => char::decode_utf16(units.units())
                .try_for_each(|character| character.map(|character| text.push(character)))
                .is_ok(),
            None => false,
        };
        let written = read && {
            replace_text_element(&mut text, element.position, element.character);
            self.arm.write_text(range, geometry, presence, &text)
        };
        self.arm.text = text;
        written
    }

    /// Store the checked `leaf` of `type_id` into the cells of `range`. The check ran before the copy
    /// (R4), so the store answers `false` only when the cells disagree with the layout.
    /// Write `leaf` into the bytes of `range`, a value of `type_id`, and mark the declaration's
    /// presence byte at `presence`, when it keeps one: set for a value and clear for the null value
    /// (MEM-12). `false` when the write is refused; nothing is marked then.
    fn store(
        &mut self,
        range: ByteRange,
        type_id: u32,
        presence: Option<u32>,
        leaf: Value,
    ) -> bool {
        let prepared = self.prepared;
        let holds_value = !matches!(leaf, Value::Null);
        // The activation stack keeps no reference table: its frames hold no reference leaf (C1-11).
        let (block, references) = self.arm.cells.regions_mut().block_mut(range.region);
        let written = match (block.bytes_mut(range), references) {
            (None, _) => false,
            (Some(bytes), references) if is_reference_type(prepared, type_id) => {
                store_reference_leaf(bytes, range.start, leaf, references)
            }
            (Some(bytes), references) => {
                write_value_in(bytes, type_id, &leaf, prepared, references, range.start).is_ok()
            }
        };
        let Some(offset) = presence.filter(|_| written) else {
            return written;
        };
        let flag = ByteRange {
            start: offset,
            bytes: 1,
            ..range
        };
        match block.bytes_mut(flag) {
            Some(byte) => {
                byte[0] = u8::from(holds_value);
                true
            }
            None => false,
        }
    }

    /// Make the staged write of one scalar leaf in its backing (C1-15b).
    fn write_leaf(&mut self, patch: &LeafPatch) -> bool {
        let (block, _) = self.arm.cells.regions_mut().block_mut(patch.backing.region);
        block
            .bytes_mut(patch.backing)
            .is_some_and(|bytes| patch.write.commit(bytes))
    }

    /// The write of `value` into the slot at `location`/`offset` and `path`, answering as the `Value`
    /// store answers a checked write: `Ok(false)` for a target that does not exist.
    fn write_path(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> Result<bool, RuntimeError> {
        let target = self.reader().locate(location, offset, path);
        match target {
            Ok(Target::Cells(range, type_id, presence, _)) => {
                let leaf = checked_leaf(self.prepared, type_id, value)?;
                Ok(self.store(range, type_id, presence, leaf))
            }
            Ok(Target::Values(location, offset, rest)) => self
                .values
                .write_by_ref_parts_checked(location, offset, rest, value),
            // A write into the backing of a structure with a physical layout patches the
            // structure's value as the value store patches it, then stores it (C1-15); a scalar
            // leaf is written where it lies (C1-15b).
            Ok(Target::Bits(range, type_id, presence, rest)) => {
                let cells = self.reader().view_cells();
                if let Some(patch) = cells.leaf_patch(range, type_id, rest, &value)? {
                    return Ok(self.write_leaf(&patch));
                }
                let mut root = self.reader().overlay_root(range, type_id, presence)?;
                let Some(staged) = root.prepare_physical_write(rest, &value)? else {
                    return Ok(false);
                };
                if !root.commit_physical_write(rest, value, staged) {
                    return Ok(false);
                }
                Ok(self.store(
                    range,
                    type_id,
                    presence,
                    Value::Struct(alloc::sync::Arc::new(root)),
                ))
            }
            Err(_) => {
                let patch = self
                    .reader()
                    .text_element_patch(location, offset, path, &value)?;
                // A text patch always carries its character; a path with none names no target.
                let Some(CellPatch {
                    range,
                    type_id,
                    presence,
                    element: Some(element),
                    ..
                }) = patch
                else {
                    return Ok(false);
                };
                Ok(self.write_text_element(range, type_id, presence, element))
            }
        }
    }
}

impl CellReader<'_> {
    /// Stage the write of `value` at `location`/`offset` and `path`, checking it without copying
    /// anything (R4). `None` when the path names no target.
    pub(crate) fn prepare_write(
        self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: &Value,
    ) -> Result<Option<PreparedWrite>, RuntimeError> {
        match self.locate(location, offset, path) {
            Ok(Target::Cells(range, type_id, presence, beside)) => {
                // A reference is checked by reference: cloning its path would allocate in a scan (R5).
                let leaf = if is_reference_type(self.prepared, type_id) {
                    check_value(type_id, value, self.prepared, true)?;
                    None
                } else {
                    Some(checked_leaf(self.prepared, type_id, value.clone())?)
                };
                let physical = beside || self.replaces_backing(type_id, value);
                Ok(Some(PreparedWrite(PreparedWriteKind::Cells(CellPatch {
                    range,
                    type_id,
                    presence,
                    leaf,
                    element: None,
                    physical,
                }))))
            }
            Ok(Target::Values(location, offset, rest)) => {
                let consumed = path.len() - rest.len();
                let staged = self
                    .values
                    .prepare_by_ref_parts_write(location, offset, rest, value)?;
                Ok(staged
                    .map(|staged| PreparedWrite(PreparedWriteKind::Values { consumed, staged })))
            }
            // The structure's value, read from its cells, is checked and patched as the value store
            // checks and patches its own (C1-15); a scalar leaf is staged where it lies (C1-15b).
            Ok(Target::Bits(range, type_id, presence, rest)) => {
                if let Some(patch) = self.view_cells().leaf_patch(range, type_id, rest, value)? {
                    return Ok(Some(PreparedWrite(PreparedWriteKind::Bits(patch))));
                }
                let root = self.overlay_root(range, type_id, presence)?;
                let Some(staged) = root.prepare_physical_write(rest, value)? else {
                    return Ok(None);
                };
                Ok(Some(PreparedWrite(PreparedWriteKind::Overlay(
                    OverlayWrite {
                        range,
                        type_id,
                        presence,
                        consumed: path.len() - rest.len(),
                        root,
                        staged,
                    },
                ))))
            }
            Err(_) => Ok(self
                .text_element_patch(location, offset, path, value)?
                .map(|patch| PreparedWrite(PreparedWriteKind::Cells(patch)))),
        }
    }

    /// The patch of one character write, `s[i] := c`, when `path` ends in an index of a text leaf in the
    /// cells: the text with that character replaced, cut to the leaf's capacity (C1-9c). `None` when the
    /// path names no such character.
    fn text_element_patch(
        self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: &Value,
    ) -> Result<Option<CellPatch>, RuntimeError> {
        let Some((last, parent)) = path.split_last() else {
            return Ok(None);
        };
        let RefSegment::Index(indices) = last else {
            return Ok(None);
        };
        let [index] = indices.as_slice() else {
            return Ok(None);
        };
        let Ok(Target::Cells(range, type_id, presence, _)) = self.locate(location, offset, parent)
        else {
            return Ok(None);
        };
        // A character of a text that holds no value yet names nothing, as in the null value.
        if !self.present(range.region, presence) {
            return Ok(None);
        }
        let Some((length, _, wide)) = text_geometry(self.prepared, type_id) else {
            return Ok(None);
        };
        let Some(bytes) = self.arm.cells.regions().block(range.region).bytes(range) else {
            return Ok(None);
        };
        // The text's characters are counted where they lie; the commit edits them there (C1-12).
        let count = match flat::read_text(bytes, length, wide) {
            Ok(TextBytes::Narrow(text)) => string_element_count(text),
            Ok(wide @ TextBytes::Wide(_)) => {
                let mut count = 0usize;
                for character in char::decode_utf16(wide.units()) {
                    if character.is_err() {
                        return Ok(None);
                    }
                    count += 1;
                }
                count
            }
            Err(_) => return Ok(None),
        };
        let Ok(position) = string_element_position_in(count, *index) else {
            return Ok(None);
        };
        let Ok(character) = text_element_char(value, wide) else {
            return Ok(None);
        };
        Ok(Some(CellPatch {
            range,
            type_id,
            presence,
            leaf: None,
            element: Some(TextElement {
                position,
                character,
            }),
            // The value store edits a character of a text directly.
            physical: false,
        }))
    }

    /// Whether `value` replaces a whole structure of `type_id` that has a physical layout without
    /// carrying that layout's backing, which the value store writes as a physical patch (C1-15).
    fn replaces_backing(self, type_id: u32, value: &Value) -> bool {
        let Some(layout) = self.prepared.vm.physical_layout_ref(type_id) else {
            return false;
        };
        !matches!(value, Value::Struct(source) if source
            .physical_backing()
            .is_some_and(|(own, _)| alloc::sync::Arc::ptr_eq(own, layout)))
    }
}

/// Store a reference leaf (O9, R5): a flat reference lies whole in the leaf (C1-11d), and any other
/// non-null reference moves into the region's reference table under the leaf's offset, the leaf
/// naming that entry. A region without a table, the activation stack, holds flat and null
/// references only. `false` when the leaf cannot hold the value.
fn store_reference_leaf(
    bytes: &mut [u8],
    offset: u32,
    leaf: Value,
    references: Option<&mut ReferenceTable>,
) -> bool {
    let flat = match &leaf {
        Value::Reference(Some(reference)) => flat_cell(reference),
        _ => None,
    };
    if let Some(cell) = flat {
        // The table's entry for the leaf, if any, is displaced; the engine takes its buffers.
        if let Some(references) = references {
            references.set(offset, None);
        }
        return flat::encode(LeafKind::Reference, cell, bytes).is_ok();
    }
    let cell = match (leaf, references) {
        (Value::Reference(Some(reference)), Some(references)) => {
            if !references.set(offset, Some(reference)) {
                return false;
            }
            Cell::Ref(crate::memory::cells::RefSlot(offset))
        }
        (Value::Reference(Some(_)), None) => return false,
        (Value::Reference(None) | Value::Null, Some(references)) => {
            if !references.set(offset, None) {
                return false;
            }
            Cell::NullRef
        }
        (Value::Reference(None) | Value::Null, None) => Cell::NullRef,
        _ => return false,
    };
    flat::encode(LeafKind::Reference, cell, bytes).is_ok()
}

/// The value a write leaves in a leaf of `type_id`: normalized by the declared type (O11) and checked
/// against it. A non-null reference is accepted, since the arm's table holds it (O9).
fn checked_leaf(
    prepared: &PreparedModule,
    type_id: u32,
    value: Value,
) -> Result<Value, RuntimeError> {
    let leaf = normalize_for_declared(prepared, type_id, value)?;
    check_value(type_id, &leaf, prepared, true)?;
    Ok(leaf)
}

/// Widen a scalar or text value to its declared type, and cut a text to the declared capacity (O11,
/// R2). A `Null` value is stored as the empty leaf, not widened to the type's zero.
fn normalize_for_declared(
    prepared: &PreparedModule,
    type_id: u32,
    value: Value,
) -> Result<Value, RuntimeError> {
    if matches!(value, Value::Null) {
        return Ok(value);
    }
    match resolved_primitive_shape(prepared.type_table(), type_id, 0) {
        Some((prim_id, max_length)) => normalize_primitive_value(prim_id, max_length, value),
        None => Ok(value),
    }
}

impl StoreMutate for CellWriter<'_> {
    fn write_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) -> bool {
        let target = self.reader().locate(location, offset, &[]);
        match target {
            Ok(Target::Cells(range, type_id, presence, _)) => {
                match checked_leaf(self.prepared, type_id, value) {
                    Ok(leaf) => self.store(range, type_id, presence, leaf),
                    Err(_) => false,
                }
            }
            Ok(Target::Values(location, offset, _)) => self
                .values
                .write_direct_slot_by_location(location, offset, value),
            // A slot is a whole declaration, never a node inside one.
            Ok(Target::Bits(..)) | Err(_) => false,
        }
    }

    fn write_reference(
        &mut self,
        reference: ValueRefView<'_>,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        if reference.generation != self.reader().generation() {
            return Err(RuntimeError::ReferenceLifetime);
        }
        self.write_path(reference.location, reference.offset, reference.path, value)
    }

    fn write_instance_var(
        &mut self,
        instance: InstanceId,
        name: impl Into<SmolStr>,
        value: Value,
    ) -> bool {
        let name = name.into();
        if !self.arm.is_persistent(instance) {
            return self.values.set_instance_var(instance, name, value);
        }
        let target = self
            .reader()
            .member_by(instance, |_, member| member == name.as_str())
            .map(|(_, _, range, type_id, presence)| (range, type_id, presence));
        let Some((range, type_id, presence)) = target else {
            return false;
        };
        match checked_leaf(self.prepared, type_id, value) {
            Ok(leaf) => self.store(range, type_id, presence, leaf),
            Err(_) => false,
        }
    }

    fn set_global(&mut self, name: impl Into<SmolStr>, value: Value) -> bool {
        let name = name.into();
        let Some(ordinal) = self.reader().global_index(name.as_str()) else {
            return false;
        };
        let declared = self.reader().global(ordinal);
        match declared {
            Some((range, type_id, presence)) => match checked_leaf(self.prepared, type_id, value) {
                Ok(leaf) => self.store(range, type_id, presence, leaf),
                Err(_) => false,
            },
            None => {
                self.values.set_global(name, value);
                true
            }
        }
    }

    fn try_create_instance(
        &mut self,
        type_name: impl Into<SmolStr>,
    ) -> Result<InstanceId, RuntimeError> {
        // Per-call and persistent instances draw on the one counter of the cell store (R4). A persistent
        // instance also needs its reserved root range, which the trait does not carry (O8). The counter
        // is advanced only once the record is placed, as the `Value` store does.
        let (id, following) = self.arm.cells.next_instance_id()?;
        self.values.insert_instance(id, type_name)?;
        self.arm.cells.issue_instance(id, following);
        Ok(id)
    }

    fn remove_instance(&mut self, id: InstanceId) -> bool {
        // A persistent instance ends with its generation, so only a per-call instance is released. The
        // release is recorded in the counter that issued the identity (R4).
        if self.arm.is_root_instance(id) {
            return false;
        }
        // A per-call instance on the activation stack leaves it (C1-11c).
        if self.arm.release_call_instance(id) {
            return true;
        }
        let removed = self.values.take_instance(id).is_some();
        if removed {
            self.arm.cells.release_instance(id);
        }
        removed
    }

    fn retain_instances(&mut self, mut keep: impl FnMut(InstanceId) -> bool) -> usize {
        // A persistent instance ends with its generation. The per-call instances lie on the activation
        // stack (C1-11c) or in the `Value` half; each release is recorded in the counter that issued the
        // identity, the `Value` half's against its entry floor (R4).
        let released = self.arm.retain_call_instances(&mut keep);
        let floor = self.arm.cells.entry_instance_floor();
        let (removed, released_since_floor) = self.values.retain_instances_above(floor, keep);
        self.arm.cells.release_instances(released_since_floor);
        released.saturating_add(removed)
    }

    fn restore_instance(&mut self, instance: InstanceId, data: InstanceData) {
        if !self.arm.is_persistent(instance) {
            StoreMutate::restore_instance(&mut *self.values, instance, data);
            return;
        }
        // Every member is checked before any is written, so a refused restore changes nothing.
        let mut leaves = Vec::with_capacity(data.variables.len());
        for (name, value) in data.variables.iter() {
            let target = self
                .reader()
                .member_by(instance, |_, member| member == name.as_str())
                .map(|(_, _, range, type_id, presence)| (range, type_id, presence));
            let Some((range, type_id, presence)) = target else {
                return;
            };
            let Ok(leaf) = checked_leaf(self.prepared, type_id, value.clone()) else {
                return;
            };
            leaves.push((range, type_id, presence, leaf));
        }
        for (range, type_id, presence, leaf) in leaves {
            self.store(range, type_id, presence, leaf);
        }
        self.arm.set_instance_parent(instance, data.parent);
    }

    fn commit_write(
        &mut self,
        path: &[RefSegment],
        value: Value,
        prepared: PreparedWrite,
        capacity: usize,
    ) -> bool {
        match prepared.0 {
            PreparedWriteKind::Values { consumed, staged } => match path.get(consumed..) {
                Some(path) => self
                    .values
                    .commit_prepared_write(path, value, staged, capacity),
                None => false,
            },
            // The leaf was checked when it was prepared, so the commit copies it and nothing else. A
            // reference was checked by reference, so the commit stores the value it is given.
            PreparedWriteKind::Cells(patch) => match (patch.element, patch.leaf) {
                (Some(element), _) => {
                    self.write_text_element(patch.range, patch.type_id, patch.presence, element)
                }
                (None, Some(leaf)) => self.store(patch.range, patch.type_id, patch.presence, leaf),
                (None, None) => self.store(patch.range, patch.type_id, patch.presence, value),
            },
            PreparedWriteKind::Bits(patch) => self.write_leaf(&patch),
            PreparedWriteKind::Overlay(mut write) => {
                let Some(rest) = path.get(write.consumed..) else {
                    return false;
                };
                if !write.root.commit_physical_write(rest, value, write.staged) {
                    return false;
                }
                self.store(
                    write.range,
                    write.type_id,
                    write.presence,
                    Value::Struct(alloc::sync::Arc::new(write.root)),
                )
            }
        }
    }

    fn restore_slot(&mut self, location: MemoryLocation, offset: usize, value: Value) {
        // The saved value came from this slot, so it is stored as it is; a refusal leaves the cells.
        let target = self.reader().locate(location, offset, &[]);
        match target {
            Ok(Target::Cells(range, type_id, presence, _)) => {
                self.store(range, type_id, presence, value);
            }
            Ok(Target::Values(location, offset, _)) => {
                self.values.restore_direct_slot(location, offset, value);
            }
            Ok(Target::Bits(..)) | Err(_) => {}
        }
    }
}

impl CellReader<'_> {
    /// The value of the structure with a physical layout whose cells are `range`, read whole so that
    /// a write into its backing can be checked and patched as the value store checks and patches its
    /// own (C1-15).
    fn overlay_root(
        self,
        range: ByteRange,
        type_id: u32,
        presence: Option<u32>,
    ) -> Result<crate::value::StructValue, RuntimeError> {
        match self.view(range, type_id, presence).materialize()? {
            Value::Struct(root) => Ok(alloc::sync::Arc::unwrap_or_clone(root)),
            _ => Err(RuntimeError::InvalidExecutionState),
        }
    }
}
