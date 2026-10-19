//! The cell arm of the store-access seam (specification 34 §6.2, MEM-12; C1-9b).
//!
//! The arm reads the globals and the persistent instances from the persistent flat block, through
//! the prepared layouts. A [`crate::memory::view::ValueView::Cells`] names the bytes of one value and
//! reads them without copying.
//! Locals, per-call instances and the rest of the `Value` half stay in today's
//! [`crate::memory::VariableStorage`], which the engine owns beside the arm and lends to the reader and
//! the writer of each call (C1-9d2b-1), so the engine's activations and the arm's reads of locals and
//! per-call instances address one `Value` half and their reads are unchanged. The engine builds the arm
//! at construction when it is asked to (`vm::engine`), and builds only the `Value` store otherwise.
//!
//! The reads take the prepared module through `CellReader`: a cell view needs the layouts that
//! place its members and elements, and the store-access trait carries no prepared module. The
//! routing in `vm::engine::store_access` supplies it. The writes are in `cell_write` (C1-9c), which
//! checks a value before it copies it into the block.

use smol_str::SmolStr;

use crate::bytecode::{StorageOwner, TypeData, TypeEntry, BYTECODE_MAX_TYPE_NESTING};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::memory::cells::{
    is_reference_type, leaf_in, read_value_in, structure_member_name, text_geometry, Cell, CellLeaf,
};
use crate::memory::flat::{self, LeafKind, TextBytes};
use crate::memory::instance_table::PersistentInstance;
use crate::memory::layout::ByteRange;
use crate::memory::regions::{CellRegions, CellStorage};
use crate::memory::view::ValueView;
use crate::memory::{Generation, InstanceData, InstanceId, MemoryLocation, VariableStorage};
use crate::value::{array_offset_i64, RefPath, RefSegment, Value, ValueRef, ValueRefView};
use crate::vm::{PreparedModule, VmTrap};

pub(crate) use cell_write::{CellPatch, CellWriter};

/// The write side of the arm (C1-9c), in its own file.
#[path = "cell_write.rs"]
mod cell_write;

/// The primitive encoding of `STRING` (`prim_id` 24; the encodings of
/// `vm::type_policy::primitive_value_matches`).
const STRING_PRIMITIVE: u16 = 24;

/// The primitive encoding of `WSTRING` (`prim_id` 25).
const WSTRING_PRIMITIVE: u16 = 25;

/// The null value a declaration that holds no value yet reads as (MEM-12).
static NULL_VALUE: Value = Value::Null;

/// The layouts and the region a cell view reads through: the prepared module places its members
/// and elements, and the region holds its cells and text buffers.
#[derive(Clone, Copy)]
pub struct CellsView<'a> {
    prepared: &'a PreparedModule,
    regions: &'a CellRegions,
}

impl<'a> CellsView<'a> {
    /// The member or element `segment` names inside the value of `type_id` at `range`, as its range
    /// and type. The member's bytes must lie inside `range` (specification 34 §6.2, MEM-12).
    pub(crate) fn member(
        self,
        range: ByteRange,
        type_id: u32,
        segment: &RefSegment,
    ) -> Option<(ByteRange, u32)> {
        let (offset, child) = step(self.prepared, type_id, segment)?;
        let bytes = self.prepared.type_layout(child)?.shape.bytes;
        if offset.checked_add(bytes)? > range.bytes {
            return None;
        }
        let start = range.start.checked_add(offset)?;
        Some((
            ByteRange {
                region: range.region,
                start,
                bytes,
            },
            child,
        ))
    }

    /// The UTF-8 text of the `STRING` at `range`, borrowed from the block. `None` for a `WSTRING`,
    /// whose leaf holds UTF-16 code units and has no UTF-8 text (specification 34 MEM-12).
    pub(crate) fn string_text(self, range: ByteRange, type_id: u32) -> Option<&'a str> {
        let (length, _, wide) = text_geometry(self.prepared, type_id)?;
        if wide {
            return None;
        }
        match flat::read_text(self.regions.persistent().bytes(range)?, length, false).ok()? {
            TextBytes::Narrow(text) => Some(text),
            TextBytes::Wide(_) => None,
        }
    }

    /// The owned value of the bytes at `range`, read with `read_value` (the only allocating read).
    /// A reference leaf holding a reference answers the `ValueRef` it keeps (O9).
    pub(crate) fn materialize(self, range: ByteRange, type_id: u32) -> Result<Value, RuntimeError> {
        let bytes = self
            .regions
            .persistent()
            .bytes(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        read_value_in(
            bytes,
            type_id,
            self.prepared,
            Some(self.regions.references()),
            range.start,
        )
    }

    /// The `ValueRef` a reference leaf at `range` holds, when it holds one: `None` for a leaf of
    /// another type and for a null reference.
    pub(crate) fn reference_leaf(
        self,
        range: ByteRange,
        type_id: u32,
    ) -> Result<Option<&'a ValueRef>, RuntimeError> {
        if !is_reference_type(self.prepared, type_id) {
            return Ok(None);
        }
        let bytes = self
            .regions
            .persistent()
            .bytes(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        match flat::decode(LeafKind::Reference, bytes)? {
            Cell::NullRef => Ok(None),
            _ => self
                .regions
                .reference(range.start)
                .map(Some)
                .ok_or(RuntimeError::InvalidExecutionState),
        }
    }

    /// What the bytes at `range`, a value of `type_id`, hold, classified for the retained encoder: the
    /// parts of the `Value` that [`CellsView::materialize`] would build, without building it
    /// (specification 11 §6.7; C1-9d2b-2). Nothing is allocated.
    pub(crate) fn leaf(self, range: ByteRange, type_id: u32) -> Result<CellLeaf<'a>, RuntimeError> {
        let bytes = self
            .regions
            .persistent()
            .bytes(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        leaf_in(bytes, type_id, self.prepared)
    }

    /// The declared name of member `ordinal`, in declaration order, of the structure `type_id`.
    pub(crate) fn structure_member_name(
        self,
        type_id: u32,
        ordinal: usize,
    ) -> Result<&'a str, RuntimeError> {
        structure_member_name(self.prepared, type_id, ordinal)
    }
}

/// The cell arm's state (specification 34 §6.2; C1-9b): the cell store, whose one identity counter issues
/// every instance identity and whose fixed instance table records the persistent instances (C1-9d2b-2).
/// The `Value` half that holds locals and per-call instances is the engine's own `VariableStorage`, lent to
/// each reader and writer.
#[derive(Debug, Default)]
pub(crate) struct CellArm {
    cells: CellStorage,
}

impl CellArm {
    /// An arm over `cells`, with no persistent instance recorded yet.
    pub(crate) fn new(cells: CellStorage) -> Self {
        Self { cells }
    }

    /// Record that `instance`, of template `pou`, occupies the cells of `range` with `parent` as its
    /// parent. Construction records the persistent instances as it reserves them
    /// ([`CellArm::create_persistent_instance`]); the tests record the identities they issued.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] when `instance` is new and the instance table, reserved at
    /// construction from the roots, is full.
    pub(crate) fn register_instance(
        &mut self,
        instance: InstanceId,
        pou: u32,
        range: ByteRange,
        parent: Option<InstanceId>,
    ) -> Result<(), RuntimeError> {
        self.cells
            .regions_mut()
            .register_instance(instance, PersistentInstance { pou, range, parent })
    }

    /// Issue the next identity from the cell store's one counter and record the persistent instance
    /// that holds it (specification 34 §6.2; R2, O8). Per-call instances draw on the same counter, so no
    /// identity is issued twice. `range` is the instance's own cells within its root's range.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Overflow`] when the counter is exhausted, and the error of
    /// [`CellArm::register_instance`]; nothing is issued or recorded then.
    pub(crate) fn create_persistent_instance(
        &mut self,
        pou: u32,
        range: ByteRange,
    ) -> Result<InstanceId, RuntimeError> {
        let (instance, following) = self.cells.next_instance_id()?;
        self.register_instance(instance, pou, range, None)?;
        self.cells.issue_instance(instance, following);
        Ok(instance)
    }

    /// Give the persistent instance `child` the parent `parent`. `false` when `child` is not a persistent
    /// instance of the arm.
    pub(crate) fn set_instance_parent(
        &mut self,
        child: InstanceId,
        parent: Option<InstanceId>,
    ) -> bool {
        self.cells.regions_mut().set_instance_parent(child, parent)
    }

    /// The number of persistent instances the arm records.
    pub(crate) fn persistent_count(&self) -> usize {
        self.cells.regions().instance_count()
    }

    /// The persistent instance `instance`, or `None` for an identity that is not one.
    fn instance(&self, instance: InstanceId) -> Option<&PersistentInstance> {
        self.cells.regions().instance(instance)
    }

    /// Whether `instance` is a persistent instance of the arm.
    fn is_persistent(&self, instance: InstanceId) -> bool {
        self.instance(instance).is_some()
    }

    /// Begin a top-level execution entry for the instance identities the cell store issues
    /// (specification 34 §4.3.1, HOST-02; C1-9d2b-2, R4).
    pub(crate) fn begin_execution_entry(&mut self) {
        self.cells.begin_execution_entry();
    }

    /// The cells of a persistent instance's own members, or `None` for an identity that is not a
    /// persistent instance of the arm.
    #[cfg(test)]
    pub(crate) fn instance_range(&self, instance: InstanceId) -> Option<ByteRange> {
        self.instance(instance).map(|persistent| persistent.range)
    }

    /// Copy the bytes of `source` to `destination` in the persistent block and mark the presence
    /// byte at `presence`, when the destination's declaration keeps one, as holding a value
    /// (specification 34 MEM-12; C1-10c). `false`, and nothing changed, when the block refuses
    /// either range.
    pub(crate) fn copy_place(
        &mut self,
        source: ByteRange,
        destination: ByteRange,
        presence: Option<u32>,
    ) -> bool {
        let (persistent, _) = self.cells.regions_mut().split_mut();
        let flag = presence.map(|start| ByteRange {
            start,
            bytes: 1,
            ..destination
        });
        if flag.is_some_and(|flag| persistent.bytes(flag).is_none()) {
            return false;
        }
        if !persistent.copy_within(source, destination) {
            return false;
        }
        if let Some(byte) = flag.and_then(|flag| persistent.bytes_mut(flag)) {
            byte[0] = 1;
        }
        true
    }

    /// The reference the last write into a reference leaf replaced, for the engine to take its path
    /// buffers back (spec 34 ARCH-05).
    pub(crate) fn take_displaced_reference(&mut self) -> Option<ValueRef> {
        self.cells.regions_mut().split_mut().1.take_displaced()
    }

    /// The cell store the arm reads and writes, for the tests.
    #[cfg(test)]
    pub(crate) fn cells(&self) -> &CellStorage {
        &self.cells
    }

    /// The cell store the arm reads and writes, mutably, for the tests.
    #[cfg(test)]
    pub(crate) fn cells_mut(&mut self) -> &mut CellStorage {
        &mut self.cells
    }
}

/// Where a path ends: in the arm's block, or in a place of the `Value` half.
enum Target<'p> {
    /// The bytes of a value of this type at this range, and the block offset of the presence byte
    /// of the declaration the range is, when it keeps one (MEM-12).
    Cells(ByteRange, u32, Option<u32>),
    /// A place the `Value` half reads: location, slot and the path still to walk.
    Values(MemoryLocation, usize, &'p [RefSegment]),
}

/// The reads of the cell arm (specification 34 §6.2, MEM-10; C1-9b): the reads of
/// [`crate::memory::view::StoreAccess`] over the cells of an arm, with the prepared module that places
/// them. Each read takes the reader by value, so a view it returns borrows the arm and not the reader.
#[derive(Clone, Copy)]
pub(crate) struct CellReader<'a> {
    prepared: &'a PreparedModule,
    arm: &'a CellArm,
    /// The `Value` half the engine lends: the activations' locals and the per-call instances.
    values: &'a VariableStorage,
}

impl<'a> CellReader<'a> {
    /// A reader of `arm` and of the `Value` half `values`, placing the arm's cells through `prepared`.
    pub(crate) fn new(
        prepared: &'a PreparedModule,
        arm: &'a CellArm,
        values: &'a VariableStorage,
    ) -> Self {
        Self {
            prepared,
            arm,
            values,
        }
    }

    /// The installation generation of the arm's references (specification 34 §11.3, MEM-07).
    pub(crate) fn generation(self) -> Generation {
        self.arm.cells.generation()
    }

    /// Read the view of the path `path` inside the slot at `location`/`offset`.
    ///
    /// # Errors
    ///
    /// [`VmTrap::NullReference`] when the slot or a step of the path names no value.
    pub(crate) fn read_path(
        self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<ValueView<'a>, VmTrap> {
        match self.locate(location, offset, path)? {
            Target::Cells(range, type_id, presence) => Ok(self.view(range, type_id, presence)),
            Target::Values(location, offset, rest) => self.values_slot(location, offset, rest),
        }
    }

    /// The view of field `name` of `instance`, searched through its parents: the field the instance
    /// declares, else the nearest ancestor's.
    ///
    /// # Errors
    ///
    /// [`VmTrap::NullReference`] when no instance on the chain declares `name`.
    pub(crate) fn read_instance_field(
        self,
        instance: InstanceId,
        name: &str,
    ) -> Result<ValueView<'a>, VmTrap> {
        match self.field(instance, name, &[])? {
            Target::Cells(range, type_id, presence) => Ok(self.view(range, type_id, presence)),
            Target::Values(location, offset, rest) => self.values_slot(location, offset, rest),
        }
    }

    /// The view of a whole slot, without a path.
    ///
    /// # Errors
    ///
    /// [`VmTrap::NullReference`] when the slot names no value.
    pub(crate) fn read_slot(
        self,
        location: MemoryLocation,
        offset: usize,
    ) -> Result<ValueView<'a>, VmTrap> {
        self.read_path(location, offset, &[])
    }

    /// The view of a reference's slot and path, in this arm's generation. A reference of another
    /// generation names no state here (specification 34 §11.3, MEM-07).
    ///
    /// # Errors
    ///
    /// [`VmTrap::NullReference`] for a foreign generation, or when its target names no value.
    pub(crate) fn read_reference(
        self,
        reference: ValueRefView<'_>,
    ) -> Result<ValueView<'a>, VmTrap> {
        if reference.generation != self.generation() {
            return Err(VmTrap::NullReference);
        }
        self.read_path(reference.location, reference.offset, reference.path)
    }

    /// The view of one variable of the persistent instance `instance`, or of a `Value`-half
    /// instance, selected by its declared name.
    pub(crate) fn read_instance_var(
        self,
        instance: InstanceId,
        name: &str,
    ) -> Option<ValueView<'a>> {
        if self.arm.is_persistent(instance) {
            let (_, _, range, type_id, presence) =
                self.member_by(instance, |_, member| member == name)?;
            return Some(self.view(range, type_id, presence));
        }
        self.values
            .get_instance_var(instance, name)
            .map(ValueView::Value)
    }

    /// The view of the global named `name`. A declared global that owns no cells, the program root,
    /// is read from the `Value` half.
    pub(crate) fn read_global(self, name: &str) -> Option<ValueView<'a>> {
        let ordinal = self.global_index(name)?;
        match self.global(ordinal) {
            Some((range, type_id, presence)) => Some(self.view(range, type_id, presence)),
            None => self.values.get_global(name).map(ValueView::Value),
        }
    }

    /// The declared name of the global at `offset`, in declaration order.
    pub(crate) fn global_name(self, offset: usize) -> Option<&'a str> {
        let declaration = *self
            .prepared
            .declarations(StorageOwner::Global, None)
            .get(offset)?;
        let name_idx = self
            .prepared
            .layout
            .entries
            .get(declaration as usize)?
            .name_idx;
        self.prepared.strings().get(name_idx as usize)
    }

    /// The reference to the global named `name`, whose ordinal is its declaration order.
    pub(crate) fn ref_for_global(self, name: &str) -> Option<ValueRef> {
        let ordinal = self.global_index(name)?;
        self.value_ref(MemoryLocation::Global, ordinal)
    }

    /// The reference to the variable `name` of `instance`, in its own variables.
    pub(crate) fn ref_for_instance(self, instance: InstanceId, name: &str) -> Option<ValueRef> {
        if self.arm.is_persistent(instance) {
            let (ordinal, _, _, _, _) = self.member_by(instance, |_, member| member == name)?;
            return self.value_ref(MemoryLocation::Instance(instance), ordinal);
        }
        self.values.ref_for_instance(instance, name)
    }

    /// The reference to the variable `name` of `instance` or of its nearest ancestor that declares it.
    pub(crate) fn ref_for_instance_recursive(
        self,
        instance: InstanceId,
        name: &str,
    ) -> Option<ValueRef> {
        let mut current = instance;
        for _ in 0..=self.arm.persistent_count() {
            if !self.arm.is_persistent(current) {
                return self.values.ref_for_instance_recursive(current, name);
            }
            if let Some((ordinal, _, _, _, _)) = self.member_by(current, |_, member| member == name)
            {
                return self.value_ref(MemoryLocation::Instance(current), ordinal);
            }
            current = self.arm.instance(current)?.parent?;
        }
        None
    }

    /// The reference to field `name` of `instance`: the field it declares, else an inherited one.
    pub(crate) fn resolved_instance_field_ref(
        self,
        instance: InstanceId,
        name: &str,
    ) -> Option<ValueRef> {
        self.ref_for_instance(instance, name)
            .or_else(|| self.ref_for_instance_recursive(instance, name))
    }

    /// The position of `name` among the variables `instance` declares itself.
    pub(crate) fn declared_instance_field_offset(
        self,
        instance: InstanceId,
        name: &str,
    ) -> Option<usize> {
        if self.arm.is_persistent(instance) {
            return self
                .member_by(instance, |_, member| member == name)
                .map(|(ordinal, _, _, _, _)| ordinal);
        }
        self.values.declared_instance_field_offset(instance, name)
    }

    /// An owned copy of the instance record: its members materialized from their cells.
    pub(crate) fn snapshot_instance(self, instance: InstanceId) -> Option<InstanceData> {
        let Some(persistent) = self.arm.instance(instance) else {
            return self.values.get_instance(instance).cloned();
        };
        let mut variables = OrderedMap::default();
        let mut ordinal = 0usize;
        while let Some((_, name, range, type_id, presence)) =
            self.member_by(instance, |index, _| index == ordinal)
        {
            let value = self.view(range, type_id, presence).materialize().ok()?;
            variables.insert(SmolStr::new(name), value);
            ordinal += 1;
        }
        Some(InstanceData {
            type_name: self.persistent_type_name(persistent)?.clone(),
            variables,
            parent: persistent.parent,
        })
    }

    /// The declared type name of the persistent instance `persistent`: the name of its template POU,
    /// which the table does not repeat.
    fn persistent_type_name(self, persistent: &PersistentInstance) -> Option<&'a SmolStr> {
        self.prepared
            .vm
            .pou(persistent.pou)
            .map(|entry| &entry.name)
    }

    /// The declared type name of `instance`, or `None` when no such instance exists.
    pub(crate) fn instance_type_name(self, instance: InstanceId) -> Option<&'a SmolStr> {
        match self.arm.instance(instance) {
            Some(persistent) => self.persistent_type_name(persistent),
            None => self
                .values
                .get_instance(instance)
                .map(|data| &data.type_name),
        }
    }

    /// The parent of `instance`: `Ok(None)` when it has none, `NullReference` when no such instance
    /// exists.
    pub(crate) fn instance_parent(
        self,
        instance: InstanceId,
    ) -> Result<Option<InstanceId>, VmTrap> {
        match self.arm.instance(instance) {
            Some(persistent) => Ok(persistent.parent),
            None => self
                .values
                .get_instance(instance)
                .map(|data| data.parent)
                .ok_or(VmTrap::NullReference),
        }
    }

    /// The number of members `instance` holds: the members of a persistent instance's template that own
    /// storage, or the variables of a `Value`-half instance. `None` when no such instance exists. A walk
    /// of an instance visits ordinals `0..count` (specification 34 MEM-09; C1-9d2b-1).
    pub(crate) fn instance_member_count(self, instance: InstanceId) -> Option<usize> {
        match self.arm.instance(instance) {
            Some(persistent) => Some(
                self.prepared
                    .instance_declarations(persistent.pou)
                    .filter(|declaration| {
                        declaration.type_id.is_some() && declaration.owns_storage()
                    })
                    .count(),
            ),
            None => self
                .values
                .get_instance(instance)
                .map(|data| data.variables.len()),
        }
    }

    /// The declared name of member `ordinal` of `instance`, in declaration order. `None` when the
    /// instance or the member does not exist.
    pub(crate) fn instance_member_name(
        self,
        instance: InstanceId,
        ordinal: usize,
    ) -> Option<&'a str> {
        if self.arm.is_persistent(instance) {
            let (_, name, _, _, _) = self.member_by(instance, |index, _| index == ordinal)?;
            return Some(name);
        }
        self.values
            .get_instance(instance)?
            .variables
            .get_index(ordinal)
            .map(|(name, _)| name.as_str())
    }

    /// The view of member `ordinal` of `instance`, in declaration order. `None` when the instance or the
    /// member does not exist.
    pub(crate) fn instance_member_view(
        self,
        instance: InstanceId,
        ordinal: usize,
    ) -> Option<ValueView<'a>> {
        if self.arm.is_persistent(instance) {
            let (_, _, range, type_id, presence) =
                self.member_by(instance, |index, _| index == ordinal)?;
            return Some(self.view(range, type_id, presence));
        }
        self.values
            .read_instance_field_by_offset(instance, ordinal)
            .map(ValueView::Value)
    }

    /// The view of the compound a place stands for: the cells of `range`, a value of `type_id`
    /// (C1-10c).
    pub(crate) fn place_view(self, range: ByteRange, type_id: u32) -> ValueView<'a> {
        self.view(range, type_id, None)
    }

    /// The cells the slot `offset` of `location` and the path lead to, as their range, their type
    /// and the presence byte of the declaration they are, when it keeps one. `None` for a place of
    /// the `Value` half and for a path that names nothing (C1-10c).
    pub(crate) fn cells_target(
        self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Option<(ByteRange, u32, Option<u32>)> {
        match self.locate(location, offset, path) {
            Ok(Target::Cells(range, type_id, presence)) => Some((range, type_id, presence)),
            _ => None,
        }
    }

    /// The view of the cells of `range`, a value of `type_id`, through this reader's region.
    fn view(self, range: ByteRange, type_id: u32, presence: Option<u32>) -> ValueView<'a> {
        if !self.present(presence) {
            return ValueView::Value(&NULL_VALUE);
        }
        ValueView::Cells {
            cells: CellsView {
                prepared: self.prepared,
                regions: self.arm.cells.regions(),
            },
            range,
            type_id,
        }
    }

    /// Whether the declaration whose presence byte is at block offset `presence` holds a value:
    /// always for a declaration that keeps no presence byte (MEM-12).
    fn present(self, presence: Option<u32>) -> bool {
        presence.is_none_or(|offset| {
            self.arm
                .cells
                .regions()
                .persistent()
                .bytes(ByteRange {
                    region: crate::memory::layout::LayoutRegion::Persistent,
                    start: offset,
                    bytes: 1,
                })
                .is_some_and(|byte| byte[0] != 0)
        })
    }

    /// The `Value` half's view of a slot, or `NullReference` when the slot holds no value.
    fn values_slot(
        self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Result<ValueView<'a>, VmTrap> {
        self.values
            .read_by_ref_parts(location, offset, path)
            .map(ValueView::Value)
            .ok_or(VmTrap::NullReference)
    }

    /// The bytes, the type and the presence byte's offset of the global at `ordinal` in declaration
    /// order, or `None` when that declaration owns no bytes in the block.
    fn global(self, ordinal: usize) -> Option<(ByteRange, u32, Option<u32>)> {
        let declaration = *self
            .prepared
            .declarations(StorageOwner::Global, None)
            .get(ordinal)?;
        let range = self
            .arm
            .cells
            .regions()
            .global_range(declaration as usize)?;
        let type_id = self
            .prepared
            .layout
            .entries
            .get(declaration as usize)?
            .type_id?;
        Some((
            range,
            type_id,
            self.prepared.presence_offset(declaration, range.start),
        ))
    }

    /// The `Value` half's index of the global at declaration ordinal `ordinal`, which owns no cells:
    /// the half holds such a global under its name (C1-9c).
    fn values_global(self, ordinal: usize) -> Option<usize> {
        let name = self.global_name(ordinal)?;
        self.values.globals().get_index_of(name)
    }

    /// The declaration-order ordinal of the global named `name`.
    fn global_index(self, name: &str) -> Option<usize> {
        self.prepared
            .declarations(StorageOwner::Global, None)
            .iter()
            .position(|&declaration| {
                self.prepared
                    .layout
                    .entries
                    .get(declaration as usize)
                    .and_then(|entry| member_name(self.prepared, entry.name_idx))
                    == Some(name)
            })
    }

    /// The reference of this arm's generation to the slot `ordinal` of `location`.
    fn value_ref(self, location: MemoryLocation, ordinal: usize) -> Option<ValueRef> {
        Some(ValueRef {
            location,
            offset: u32::try_from(ordinal).ok()?,
            generation: self.generation(),
            path: RefPath::new(),
        })
    }

    /// The member of persistent `instance` that `select` accepts, as its ordinal among the members
    /// that own storage, its name, its range and its type. Members lie in declaration order, each at
    /// the offset preparation gave it within its template (specification 34 §6.2, MEM-12, O6).
    fn member_by(
        self,
        instance: InstanceId,
        select: impl Fn(usize, &str) -> bool,
    ) -> Option<(usize, &'a str, ByteRange, u32, Option<u32>)> {
        let persistent = self.arm.instance(instance)?;
        let prepared = self.prepared;
        let mut ordinal = 0usize;
        for &id in prepared.declarations(StorageOwner::Instance, Some(persistent.pou)) {
            let declaration = prepared.layout.entries.get(id as usize)?;
            let Some(type_id) = declaration.type_id.filter(|_| declaration.owns_storage()) else {
                continue;
            };
            let name = member_name(prepared, declaration.name_idx)?;
            if select(ordinal, name) {
                let range = ByteRange {
                    region: persistent.range.region,
                    start: persistent
                        .range
                        .start
                        .checked_add(prepared.declaration_offset(id)?)?,
                    bytes: prepared.type_layout(type_id)?.shape.bytes,
                };
                let presence = prepared.presence_offset(id, range.start);
                return Some((ordinal, name, range, type_id, presence));
            }
            ordinal += 1;
        }
        None
    }

    /// The instance an instance leaf names.
    fn instance_at(self, range: ByteRange) -> Result<InstanceId, VmTrap> {
        let bytes = self
            .arm
            .cells
            .regions()
            .persistent()
            .bytes(range)
            .ok_or(VmTrap::NullReference)?;
        match flat::decode(LeafKind::Instance, bytes) {
            Ok(Cell::Instance(instance)) => Ok(instance),
            _ => Err(VmTrap::NullReference),
        }
    }

    /// Where the slot `offset` of `location` and the path lead: the cells of a global or of a
    /// persistent instance's member, or a place of the `Value` half.
    fn locate<'p>(
        self,
        location: MemoryLocation,
        offset: usize,
        path: &'p [RefSegment],
    ) -> Result<Target<'p>, VmTrap> {
        match location {
            MemoryLocation::Global => match self.global(offset) {
                Some((range, type_id, presence)) => self.walk(range, type_id, presence, path),
                None => {
                    let index = self.values_global(offset).ok_or(VmTrap::NullReference)?;
                    Ok(Target::Values(MemoryLocation::Global, index, path))
                }
            },
            MemoryLocation::Instance(instance) if self.arm.is_persistent(instance) => {
                let (_, _, range, type_id, presence) = self
                    .member_by(instance, |ordinal, _| ordinal == offset)
                    .ok_or(VmTrap::NullReference)?;
                self.walk(range, type_id, presence, path)
            }
            _ => Ok(Target::Values(location, offset, path)),
        }
    }

    /// Walk `path` from the value of `type_id` at `range`, a declaration whose presence byte is at
    /// `presence` when it keeps one. A field of a function block or an interface value is read
    /// through the instance its leaf names, as the `Value` tree does, and a path into a declaration
    /// that holds no value yet names nothing, as a path into the null value does.
    fn walk<'p>(
        self,
        mut range: ByteRange,
        mut type_id: u32,
        presence: Option<u32>,
        path: &'p [RefSegment],
    ) -> Result<Target<'p>, VmTrap> {
        if path.is_empty() {
            return Ok(Target::Cells(range, type_id, presence));
        }
        if !self.present(presence) {
            return Err(VmTrap::NullReference);
        }
        for (index, segment) in path.iter().enumerate() {
            if is_instance(self.prepared, type_id) {
                let RefSegment::Field(name) = segment else {
                    return Err(VmTrap::NullReference);
                };
                let instance = self.instance_at(range)?;
                return self.field(instance, name.as_str(), &path[index + 1..]);
            }
            let (offset, child) =
                step(self.prepared, type_id, segment).ok_or(VmTrap::NullReference)?;
            let bytes = self
                .prepared
                .type_layout(child)
                .ok_or(VmTrap::NullReference)?
                .shape
                .bytes;
            let start = range
                .start
                .checked_add(offset)
                .ok_or(VmTrap::NullReference)?;
            range = ByteRange {
                region: range.region,
                start,
                bytes,
            };
            type_id = child;
        }
        Ok(Target::Cells(range, type_id, None))
    }

    /// The field `name` of `instance`, searched through its parents, then the rest of `rest`
    /// walked from it. An instance of the `Value` half is searched there.
    fn field<'p>(
        self,
        instance: InstanceId,
        name: &str,
        rest: &'p [RefSegment],
    ) -> Result<Target<'p>, VmTrap> {
        let mut current = instance;
        for _ in 0..=self.arm.persistent_count() {
            let Some(persistent) = self.arm.instance(current) else {
                let reference = self
                    .values
                    .ref_for_instance_recursive(current, name)
                    .ok_or(VmTrap::NullReference)?;
                return Ok(Target::Values(
                    reference.location,
                    reference.offset as usize,
                    rest,
                ));
            };
            if let Some((_, _, range, type_id, presence)) =
                self.member_by(current, |_, member| member == name)
            {
                return self.walk(range, type_id, presence, rest);
            }
            current = persistent.parent.ok_or(VmTrap::NullReference)?;
        }
        Err(VmTrap::NullReference)
    }
}

/// The member name of a `STRING_TABLE` record.
fn member_name(prepared: &PreparedModule, name_idx: u32) -> Option<&str> {
    prepared.strings().get(name_idx as usize)
}

/// The `TYPE_TABLE` ordinal of `type_id` with its aliases resolved, and the type's data.
fn resolve_alias(prepared: &PreparedModule, type_id: u32) -> Option<(u32, &TypeData)> {
    resolve_entry(prepared, type_id).map(|(resolved, entry)| (resolved, &entry.data))
}

/// The `TYPE_TABLE` ordinal of `type_id` with its aliases resolved, and the resolved entry.
fn resolve_entry(prepared: &PreparedModule, type_id: u32) -> Option<(u32, &TypeEntry)> {
    let mut current = type_id;
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        let entry = prepared.type_table().entries.get(current as usize)?;
        match &entry.data {
            TypeData::Alias { target_type_id } => current = *target_type_id,
            _ => return Some((current, entry)),
        }
    }
    None
}

/// The primitive encoding `type_id` resolves to through its aliases and subranges.
fn primitive_of(prepared: &PreparedModule, type_id: u32) -> Option<u16> {
    let mut current = type_id;
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        let entry = prepared.type_table().entries.get(current as usize)?;
        match &entry.data {
            TypeData::Primitive { prim_id, .. } => return Some(*prim_id),
            TypeData::Alias { target_type_id }
            | TypeData::Subrange {
                base_type_id: target_type_id,
                ..
            } => current = *target_type_id,
            _ => return None,
        }
    }
    None
}

/// Whether `type_id` is a function block or an interface, whose cell holds an instance.
fn is_instance(prepared: &PreparedModule, type_id: u32) -> bool {
    resolve_alias(prepared, type_id)
        .is_some_and(|(_, data)| matches!(data, TypeData::Pou { .. } | TypeData::Interface { .. }))
}

/// The offset within a value of `type_id` of the member or element `segment` names, and the type of
/// that member or element. A member is matched by its exact name, as the `Value` tree matches it;
/// an element is found by its IEC index against the declared bounds.
fn step(prepared: &PreparedModule, type_id: u32, segment: &RefSegment) -> Option<(u32, u32)> {
    let (resolved, data) = resolve_alias(prepared, type_id)?;
    match (segment, data) {
        (RefSegment::Field(name), TypeData::Struct { fields }) => {
            let layout = prepared.type_layout(resolved)?;
            let ordinal = layout
                .members
                .iter()
                .position(|member| member_name(prepared, member.name_idx) == Some(name.as_str()))?;
            Some((
                layout.members.get(ordinal)?.byte_offset,
                fields.get(ordinal)?.type_id,
            ))
        }
        (RefSegment::Index(indices), TypeData::Array { elem_type_id, dims }) => {
            let element = array_offset_i64(dims, indices)?;
            let stride = prepared.type_layout(resolved)?.array.as_ref()?.byte_stride;
            Some((
                u32::try_from(element).ok()?.checked_mul(stride)?,
                *elem_type_id,
            ))
        }
        _ => None,
    }
}

#[path = "cell_charge.rs"]
mod cell_charge;
/// The bytecode fixture of the cell arm's tests, whose builders the engine's construction tests reuse.
#[cfg(all(test, feature = "std"))]
pub(crate) mod fixture;
#[cfg(all(test, feature = "std"))]
mod retain_tests;
#[cfg(all(test, feature = "std"))]
mod tests;
#[cfg(all(test, feature = "std"))]
mod write_tests;
