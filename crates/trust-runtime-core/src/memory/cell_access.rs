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

use alloc::string::String;
use smol_str::SmolStr;

use crate::bytecode::{StorageOwner, TypeData, TypeEntry, BYTECODE_MAX_TYPE_NESTING};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::memory::cells::{
    flat_reference, is_reference_type, leaf_in, read_value_in, structure_member_name,
    text_geometry, Cell, CellLeaf,
};
use crate::memory::flat::{self, LeafKind, TextBytes};
use crate::memory::instance_table::PersistentInstance;
use crate::memory::layout::{ByteRange, LayoutRegion};
use crate::memory::regions::{CellRegions, CellStorage, FrameBlock, ResultBlock};
use crate::memory::view::ValueView;
use crate::memory::{
    FrameId, Generation, InstanceData, InstanceId, MemoryLocation, VariableStorage,
};
use crate::value::{array_offset_i64, RefPath, RefSegment, Value, ValueRef, ValueRefView};
use crate::vm::{PreparedModule, VmTrap};

pub(crate) use cell_overlay::{LeafPatch, OverlayNode};
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
    /// The bytes of `range` in the block that holds them, or `None` when the block refuses the range.
    pub(crate) fn bytes(self, range: ByteRange) -> Option<&'a [u8]> {
        self.regions.block(range.region).bytes(range)
    }

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
        match flat::read_text(
            self.regions.block(range.region).bytes(range)?,
            length,
            false,
        )
        .ok()?
        {
            TextBytes::Narrow(text) => Some(text),
            TextBytes::Wide(_) => None,
        }
    }

    /// The owned value of the bytes at `range`, read with `read_value` (the only allocating read).
    /// A reference leaf holding a reference answers the `ValueRef` it keeps (O9).
    pub(crate) fn materialize(self, range: ByteRange, type_id: u32) -> Result<Value, RuntimeError> {
        let bytes = self
            .regions
            .block(range.region)
            .bytes(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        // Only the persistent block keeps reference leaves (C1-11).
        let references =
            (range.region == LayoutRegion::Persistent).then(|| self.regions.references());
        read_value_in(bytes, type_id, self.prepared, references, range.start)
    }

    /// The reference a reference leaf at `range` holds, when it holds one, borrowed: `None` for a
    /// leaf of another type and for a null reference. A flat reference is read from the leaf
    /// itself and names its target with no path (C1-11d).
    pub(crate) fn reference_leaf(
        self,
        range: ByteRange,
        type_id: u32,
    ) -> Result<Option<ValueRefView<'a>>, RuntimeError> {
        if !is_reference_type(self.prepared, type_id) {
            return Ok(None);
        }
        let bytes = self
            .regions
            .block(range.region)
            .bytes(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        match flat::decode(LeafKind::Reference, bytes)? {
            Cell::NullRef => Ok(None),
            cell @ Cell::Flat { .. } => Ok(flat_reference(self.prepared, type_id, cell)?.map(
                |reference| ValueRefView {
                    location: reference.location,
                    offset: reference.offset as usize,
                    generation: reference.generation,
                    path: &[],
                },
            )),
            _ => self
                .regions
                .reference(range.start)
                .map(|reference| Some(reference.as_view()))
                .ok_or(RuntimeError::InvalidExecutionState),
        }
    }

    /// What the bytes at `range`, a value of `type_id`, hold, classified for the retained encoder: the
    /// parts of the `Value` that [`CellsView::materialize`] would build, without building it
    /// (specification 11 §6.7; C1-9d2b-2). Nothing is allocated.
    pub(crate) fn leaf(self, range: ByteRange, type_id: u32) -> Result<CellLeaf<'a>, RuntimeError> {
        let bytes = self
            .regions
            .block(range.region)
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
    /// The characters of a text a character write edits, kept between writes (C1-12).
    text: String,
}

impl CellArm {
    /// An arm over `cells`, with no persistent instance recorded yet.
    pub(crate) fn new(cells: CellStorage) -> Self {
        Self {
            cells,
            text: String::new(),
        }
    }

    /// The reference leaves of the persistent block, each of which may hold a reference that keeps a
    /// path (C1-19c3).
    pub(crate) fn persistent_reference_leaves(&self) -> usize {
        self.cells.regions().references().leaf_count()
    }

    /// Room for `bytes` bytes of the text a character write edits, so that no write grows it
    /// (spec 34 MEM-09; C1-19c).
    pub(crate) fn reserve_text_edit(&mut self, bytes: usize) -> Result<(), RuntimeError> {
        self.text
            .try_reserve_exact(bytes)
            .map_err(|_| RuntimeError::Overflow)
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

    /// The number of instances the arm records: the persistent ones and those frames on the
    /// activation stack own (C1-11c).
    pub(crate) fn persistent_count(&self) -> usize {
        self.cells
            .regions()
            .instance_count()
            .saturating_add(self.cells.regions().call_instance_count())
    }

    /// The instance `instance` the arm holds in its blocks, persistent or owned by a frame on the
    /// activation stack, or `None` for an identity it does not hold.
    fn instance(&self, instance: InstanceId) -> Option<&PersistentInstance> {
        self.cells.regions().instance(instance)
    }

    /// Whether the arm holds `instance` in its blocks: its members are read and written there.
    fn is_persistent(&self, instance: InstanceId) -> bool {
        self.instance(instance).is_some()
    }

    /// Create an instance of template `pou` that a frame on the activation stack owns, on top of the
    /// stack, its own variables' bytes zero, with an identity from the store's one counter
    /// (specification 34 MEM-12; C1-11c).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Overflow`] when the counter is exhausted, and the call stack overflow when
    /// the stack or its instance records are full; nothing is issued then.
    pub(crate) fn create_call_instance(
        &mut self,
        pou: u32,
        own: u32,
    ) -> Result<InstanceId, RuntimeError> {
        let range = ByteRange {
            region: LayoutRegion::Activation,
            start: self.cells.regions().activation_top(),
            bytes: own,
        };
        let (instance, following) = self.cells.next_instance_id()?;
        if !self.clear_activation(range) {
            return Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error());
        }
        self.cells.regions_mut().push_call_instance(
            instance,
            PersistentInstance {
                pou,
                range,
                parent: None,
            },
        )?;
        self.cells.issue_instance(instance, following);
        Ok(instance)
    }

    /// Whether a frame is on the activation stack, that is, a call runs (C1-11c).
    pub(crate) fn holds_frames(&self) -> bool {
        self.cells.regions().holds_frames()
    }

    /// Release every instance that a frame on the activation stack owns and `keep` refuses, each
    /// recorded in the counter that issued it; answers how many were released (C1-11c).
    pub(crate) fn retain_call_instances(&mut self, keep: impl FnMut(InstanceId) -> bool) -> usize {
        self.cells.retain_call_instances(keep)
    }

    /// Release the instance `instance` that a frame on the activation stack owned, and record the
    /// release in the counter that issued it. `false` when it is not one.
    pub(crate) fn release_call_instance(&mut self, instance: InstanceId) -> bool {
        let released = self.cells.regions_mut().release_call_instance(instance);
        if released {
            self.cells.release_instance(instance);
        }
        released
    }

    /// Whether `instance` is a persistent instance, which lives as long as its generation.
    fn is_root_instance(&self, instance: InstanceId) -> bool {
        self.cells.regions().is_persistent_instance(instance)
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

    /// Copy the bytes of `source` to `destination` in the block that holds them both, the
    /// persistent block or the activation stack, and mark the presence byte at `presence`, when the
    /// destination's declaration keeps one, as holding a value (specification 34 MEM-12; C1-10c,
    /// C1-13). `false`, and nothing changed, when the block refuses either range.
    pub(crate) fn copy_place(
        &mut self,
        source: ByteRange,
        destination: ByteRange,
        presence: Option<u32>,
    ) -> bool {
        let (block, _) = self.cells.regions_mut().block_mut(destination.region);
        let flag = presence.map(|start| ByteRange {
            start,
            bytes: 1,
            ..destination
        });
        if flag.is_some_and(|flag| block.bytes(flag).is_none()) {
            return false;
        }
        if !block.copy_within(source, destination) {
            return false;
        }
        if let Some(byte) = flag.and_then(|flag| block.bytes_mut(flag)) {
            byte[0] = 1;
        }
        true
    }

    /// Write `bytes`, a copy of a value the operand stack keeps, over `destination` and mark the
    /// presence byte at `presence`, when the destination's declaration keeps one, as holding a value
    /// (specification 34 MEM-12; C1-19b). `false`, and nothing changed, when the block refuses
    /// either range or `bytes` is not as long as `destination`.
    pub(crate) fn write_copy(
        &mut self,
        destination: ByteRange,
        presence: Option<u32>,
        bytes: &[u8],
    ) -> bool {
        let (block, _) = self.cells.regions_mut().block_mut(destination.region);
        let flag = presence.map(|start| ByteRange {
            start,
            bytes: 1,
            ..destination
        });
        if flag.is_some_and(|flag| block.bytes(flag).is_none()) {
            return false;
        }
        let Some(target) = block
            .bytes_mut(destination)
            .filter(|target| target.len() == bytes.len())
        else {
            return false;
        };
        target.copy_from_slice(bytes);
        if let Some(byte) = flag.and_then(|flag| block.bytes_mut(flag)) {
            byte[0] = 1;
        }
        true
    }

    /// Write `text`, UTF-8, into the text leaf at `range` with the length field of `length` bytes and
    /// room for `capacity` bytes of characters, cut to the last whole character that fits, a
    /// `WSTRING`'s (`wide`) as UTF-16 code units, and mark the presence byte at `presence`, when the
    /// declaration keeps one, as holding a value (MEM-12; C1-12). `false`, and nothing written, when
    /// the block refuses either range.
    pub(crate) fn write_text(
        &mut self,
        range: ByteRange,
        (length, capacity, wide): (u32, u32, bool),
        presence: Option<u32>,
        text: &str,
    ) -> bool {
        let (block, _) = self.cells.regions_mut().block_mut(range.region);
        let flag = presence.map(|start| ByteRange {
            start,
            bytes: 1,
            ..range
        });
        if flag.is_some_and(|flag| block.bytes(flag).is_none()) {
            return false;
        }
        let written = block
            .bytes_mut(range)
            .is_some_and(|bytes| flat::write_text(bytes, length, capacity, text, wide).is_ok());
        if !written {
            return false;
        }
        if let Some(byte) = flag.and_then(|flag| block.bytes_mut(flag)) {
            byte[0] = 1;
        }
        true
    }

    /// The first free byte of the activation stack (C1-11).
    pub(crate) fn activation_top(&self) -> u32 {
        self.cells.regions().activation_top()
    }

    /// Record `frame` on top of the activation stack (C1-11).
    pub(crate) fn push_frame(&mut self, frame: FrameBlock) -> Result<(), RuntimeError> {
        self.cells.regions_mut().push_frame(frame)
    }

    /// The frame of activation `id`, when it is on the activation stack (C1-11).
    pub(crate) fn frame(&self, id: FrameId) -> Option<FrameBlock> {
        self.cells.regions().frame(id)
    }

    /// Take the frame of activation `id` off the activation stack (C1-11).
    pub(crate) fn release_frame(&mut self, id: FrameId) {
        self.cells.regions_mut().release_frame(id);
    }

    /// Write `value`, of `type_id`, into its bytes `range` in the block that holds them, as a
    /// construction in place writes each leaf it builds (C1-13).
    pub(crate) fn write_in_place(
        &mut self,
        range: ByteRange,
        type_id: u32,
        value: &Value,
        prepared: &PreparedModule,
    ) -> Result<(), RuntimeError> {
        let (block, references) = self.cells.regions_mut().block_mut(range.region);
        let bytes = block
            .bytes_mut(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        crate::memory::cells::write_value_in(
            bytes,
            type_id,
            value,
            prepared,
            references,
            range.start,
        )
    }

    /// Copy `bytes` into `range` of the block that holds it (C1-14b). `false`, and nothing written,
    /// when the block refuses the range or the lengths differ.
    pub(crate) fn write_bytes(&mut self, range: ByteRange, bytes: &[u8]) -> bool {
        let (block, _) = self.cells.regions_mut().block_mut(range.region);
        match block.bytes_mut(range) {
            Some(target) if target.len() == bytes.len() => {
                target.copy_from_slice(bytes);
                true
            }
            _ => false,
        }
    }

    /// Write `bits` into scalar leaf `leaf` of the backing `backing` of a structure with the physical
    /// layout `layout`, as building the backing writes it (C1-15b). `false` when the block refuses
    /// the backing.
    pub(crate) fn write_backing_leaf(
        &mut self,
        backing: ByteRange,
        layout: &crate::value::overlap::PhysicalLayout,
        leaf: u32,
        bits: u64,
    ) -> bool {
        let (block, _) = self.cells.regions_mut().block_mut(backing.region);
        block
            .bytes_mut(backing)
            .is_some_and(|bytes| layout.write_leaf_bits(leaf, bits, bytes))
    }

    /// Write the enumeration value `numeric` of `type_id` into its leaf `range` (C1-13).
    pub(crate) fn write_enum_in_place(
        &mut self,
        range: ByteRange,
        type_id: u32,
        numeric: i64,
        prepared: &PreparedModule,
    ) -> Result<(), RuntimeError> {
        let (block, _) = self.cells.regions_mut().block_mut(range.region);
        let bytes = block
            .bytes_mut(range)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        crate::memory::cells::write_enum_in(bytes, type_id, numeric, prepared)
    }

    /// Make the staged write `patch` of one scalar leaf in its backing, as a committed write of the
    /// leaf makes it (C1-15b, C1-19a). `false` when the block refuses the backing.
    pub(crate) fn write_leaf_patch(&mut self, patch: &LeafPatch) -> bool {
        let (block, _) = self.cells.regions_mut().block_mut(patch.backing.region);
        block
            .bytes_mut(patch.backing)
            .is_some_and(|bytes| patch.write.commit(bytes))
    }

    /// Write the variant numbered `numeric` of the enumeration `type_id` into its leaf at `range`, and
    /// mark the presence byte at `presence`, when the declaration keeps one, as holding a value
    /// (C1-19a). `Ok(false)`, and nothing written, when the block refuses either range; a number no
    /// variant of the type has is refused before anything is written.
    pub(crate) fn write_enum(
        &mut self,
        range: ByteRange,
        presence: Option<u32>,
        type_id: u32,
        numeric: i64,
        prepared: &PreparedModule,
    ) -> Result<bool, RuntimeError> {
        let (block, _) = self.cells.regions_mut().block_mut(range.region);
        let flag = presence.map(|start| ByteRange {
            start,
            bytes: 1,
            ..range
        });
        if flag.is_some_and(|flag| block.bytes(flag).is_none()) {
            return Ok(false);
        }
        let Some(bytes) = block.bytes_mut(range) else {
            return Ok(false);
        };
        crate::memory::cells::write_enum_in(bytes, type_id, numeric, prepared)?;
        if let Some(byte) = flag.and_then(|flag| block.bytes_mut(flag)) {
            byte[0] = 1;
        }
        Ok(true)
    }

    /// The bytes a staged result of `bytes` bytes would take at the top of the activation stack,
    /// when they and a record are free (C1-13).
    pub(crate) fn result_room(&self, bytes: u32) -> Option<ByteRange> {
        self.cells.regions().result_room(bytes)
    }

    /// Record the staged result `result` on top of the activation stack (C1-13).
    pub(crate) fn push_result(&mut self, result: ResultBlock) -> Result<(), RuntimeError> {
        self.cells.regions_mut().push_result(result)
    }

    /// The staged result of activation `id`, when it is on the activation stack (C1-13).
    pub(crate) fn result(&self, id: FrameId) -> Option<ResultBlock> {
        self.cells.regions().result(id)
    }

    /// Take the staged result of activation `id` off the activation stack (C1-13).
    pub(crate) fn release_result(&mut self, id: FrameId) {
        self.cells.regions_mut().release_result(id);
    }

    /// Set every byte of `range` on the activation stack to zero, so a new frame starts from no
    /// value of an earlier one (C1-11). `false` when the range leaves the stack's block.
    pub(crate) fn clear_activation(&mut self, range: ByteRange) -> bool {
        match self.cells.regions_mut().activation_bytes_mut(range) {
            Some(bytes) => {
                bytes.fill(0);
                true
            }
            None => false,
        }
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

/// Where a store copies a place (C1-15b): cells of their own, with their type, the presence byte of
/// their declaration and whether they lie beside a backing, or a structure nested in a backing.
pub(crate) enum PlaceTarget {
    Cells {
        range: ByteRange,
        type_id: u32,
        presence: Option<u32>,
        beside: bool,
    },
    Node {
        /// The cells of the structure that holds the node.
        range: ByteRange,
        root_type: u32,
        anchor: u32,
        depth: u8,
        presence: Option<u32>,
    },
}

/// Where a path ends: in the arm's block, or in a place of the `Value` half.
enum Target<'p> {
    /// The bytes of a value of this type at this range, the block offset of the presence byte of
    /// the declaration the range is, when it keeps one (MEM-12), and whether the path reached them
    /// through a structure with a physical layout, as a member beside its backing (C1-15).
    Cells(ByteRange, u32, Option<u32>, bool),
    /// A place the `Value` half reads: location, slot and the path still to walk.
    Values(MemoryLocation, usize, &'p [RefSegment]),
    /// A node of the backing of a structure with a physical layout (C1-15): the structure's cells,
    /// its type and the presence byte of the declaration it is, and the path from the structure to
    /// the node.
    Bits(ByteRange, u32, Option<u32>, &'p [RefSegment]),
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

impl core::fmt::Debug for CellReader<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CellReader").finish_non_exhaustive()
    }
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
            Target::Cells(range, type_id, presence, _) => Ok(self.view(range, type_id, presence)),
            Target::Values(location, offset, rest) => self.values_slot(location, offset, rest),
            Target::Bits(range, type_id, presence, rest) => self
                .view(range, type_id, presence)
                .walk(rest)
                .ok_or(VmTrap::NullReference),
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
            Target::Cells(range, type_id, presence, _) => Ok(self.view(range, type_id, presence)),
            Target::Values(location, offset, rest) => self.values_slot(location, offset, rest),
            Target::Bits(range, type_id, presence, rest) => self
                .view(range, type_id, presence)
                .walk(rest)
                .ok_or(VmTrap::NullReference),
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

    /// Where a store through the slot `offset` of `location` and the path copies a place: cells of
    /// their own, or a structure nested in a backing (C1-15b). `None` for a place of the `Value`
    /// half, for a path that names nothing, and for any other node of a backing.
    pub(crate) fn place_target(
        self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
    ) -> Option<PlaceTarget> {
        match self.locate(location, offset, path) {
            Ok(Target::Cells(range, type_id, presence, beside)) => Some(PlaceTarget::Cells {
                range,
                type_id,
                presence,
                beside,
            }),
            Ok(Target::Bits(range, root_type, presence, rest)) => {
                let cells = self.view_cells();
                let (mut anchor, mut depth) = (0, 0);
                for segment in rest {
                    match cells.overlay_step(range, root_type, anchor, depth, segment)? {
                        OverlayNode::Bits {
                            anchor: next,
                            depth: deeper,
                        } => (anchor, depth) = (next, deeper),
                        OverlayNode::Beside { .. } => return None,
                    }
                }
                Some(PlaceTarget::Node {
                    range,
                    root_type,
                    anchor,
                    depth,
                    presence,
                })
            }
            _ => None,
        }
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
            Ok(Target::Cells(range, type_id, presence, _)) => Some((range, type_id, presence)),
            _ => None,
        }
    }

    /// The cells of this reader's region, for the questions a view asks of them.
    pub(crate) fn view_cells(self) -> CellsView<'a> {
        CellsView {
            prepared: self.prepared,
            regions: self.arm.cells.regions(),
        }
    }

    /// The view of the cells of `range`, a value of `type_id`, through this reader's region.
    fn view(self, range: ByteRange, type_id: u32, presence: Option<u32>) -> ValueView<'a> {
        if !self.present(range.region, presence) {
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

    /// Whether the declaration whose presence byte is at offset `presence` of the block of `region`
    /// holds a value: always for a declaration that keeps no presence byte (MEM-12).
    fn present(self, region: LayoutRegion, presence: Option<u32>) -> bool {
        presence.is_none_or(|offset| {
            self.arm
                .cells
                .regions()
                .block(region)
                .bytes(ByteRange {
                    region,
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

    /// The root-and-path form of the flat reference `reference` (C1-11d): the global or persistent
    /// instance variable whose bytes hold its target, and the path from that variable to the
    /// target, as the value store names it. It is read at the boundary, where a retained reference
    /// is carried into a new generation or a value is shown to a tool, not in a scan. `None` for a
    /// reference that is not flat or names no variable.
    pub(crate) fn path_reference(self, reference: &ValueRef) -> Option<ValueRef> {
        let MemoryLocation::Flat {
            region: LayoutRegion::Persistent,
            type_id,
            ..
        } = reference.location
        else {
            return None;
        };
        let (location, slot, range, root_type) = self.root_holding(reference.offset)?;
        let mut path = RefPath::new();
        let (location, slot) = match location {
            MemoryLocation::Instance(instance) => self.holder_form(instance, slot, &mut path)?,
            location => (location, slot),
        };
        self.path_to(range, root_type, reference.offset, type_id, &mut path, 0)?;
        // A character reference keeps its index after the text's path.
        path.extend(reference.path.iter().cloned());
        Some(ValueRef {
            location,
            offset: u32::try_from(slot).ok()?,
            generation: reference.generation,
            path,
        })
    }

    /// Member `slot` of the persistent instance `instance` named as the value store names a reference
    /// taken through the variables that hold it (C1-14b): a function block that a global holds is
    /// named through that global, and one that a member of another instance holds through that
    /// member, the members between them pushed onto `path` in order; a program instance, and an
    /// inheritance parent, name their own members. Answers the location and the slot the name starts
    /// at.
    fn holder_form(
        self,
        mut instance: InstanceId,
        mut slot: usize,
        path: &mut RefPath,
    ) -> Option<(MemoryLocation, usize)> {
        let regions = self.arm.cells.regions();
        let mut members = RefPath::new();
        for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
            let own = regions.instance(instance)?;
            let Some(root) = (0..self.prepared.roots.entries.len()).find(|&index| {
                regions.root_range(index).map(|range| range.start) == Some(own.range.start)
            }) else {
                break;
            };
            let entry = self.prepared.roots.entries.get(root)?;
            if entry.is_inheritance_parent() {
                break;
            }
            let holder = self
                .prepared
                .layout
                .entries
                .get(entry.declaration_idx as usize)?;
            match (holder.owner, holder.role) {
                (StorageOwner::Global, crate::bytecode::StorageRole::Variable) => {
                    members.push(RefSegment::Field(self.member_named(own.pou, slot)?));
                    members.reverse();
                    path.extend(members);
                    let ordinal = self
                        .prepared
                        .declarations(StorageOwner::Global, None)
                        .iter()
                        .position(|&id| id == entry.declaration_idx)?;
                    return Some((MemoryLocation::Global, ordinal));
                }
                (StorageOwner::Instance, _) => {
                    members.push(RefSegment::Field(self.member_named(own.pou, slot)?));
                    let parent = entry.parent_root_idx? as usize;
                    let start = regions.root_range(parent)?.start;
                    let (outer, outer_record) = regions.instance_holding(start)?;
                    slot = self.member_ordinal(outer_record.pou, entry.declaration_idx)?;
                    instance = outer;
                }
                _ => break,
            }
        }
        members.reverse();
        path.extend(members);
        Some((MemoryLocation::Instance(instance), slot))
    }

    /// The name of member `slot`, counted among the declarations that own storage, of an instance of
    /// `pou`.
    fn member_named(self, pou: u32, slot: usize) -> Option<SmolStr> {
        let id = self
            .prepared
            .declarations(StorageOwner::Instance, Some(pou))
            .iter()
            .copied()
            .filter(|&id| {
                self.prepared
                    .layout
                    .entries
                    .get(id as usize)
                    .is_some_and(|entry| entry.owns_storage() && entry.type_id.is_some())
            })
            .nth(slot)?;
        let entry = self.prepared.layout.entries.get(id as usize)?;
        self.prepared
            .vm
            .strings
            .get(entry.name_idx as usize)
            .map(SmolStr::new)
    }

    /// The slot of the declaration `declaration` among the members of an instance of `pou` that own
    /// storage.
    fn member_ordinal(self, pou: u32, declaration: u32) -> Option<usize> {
        self.prepared
            .declarations(StorageOwner::Instance, Some(pou))
            .iter()
            .copied()
            .filter(|&id| {
                self.prepared
                    .layout
                    .entries
                    .get(id as usize)
                    .is_some_and(|entry| entry.owns_storage() && entry.type_id.is_some())
            })
            .position(|id| id == declaration)
    }

    /// The variable whose bytes hold the byte at `offset` of the persistent block: its location, its
    /// slot, its bytes and its type.
    fn root_holding(self, offset: u32) -> Option<(MemoryLocation, usize, ByteRange, u32)> {
        let contains =
            |range: ByteRange| offset >= range.start && offset - range.start < range.bytes;
        let globals = self.prepared.declarations(StorageOwner::Global, None).len();
        for ordinal in 0..globals {
            if let Some((range, type_id, _)) = self.global(ordinal)
                && contains(range)
            {
                return Some((MemoryLocation::Global, ordinal, range, type_id));
            }
        }
        let (instance, persistent) = self.arm.cells.regions().instance_holding(offset)?;
        let mut ordinal = 0usize;
        for &id in self
            .prepared
            .declarations(StorageOwner::Instance, Some(persistent.pou))
        {
            let declaration = self.prepared.layout.entries.get(id as usize)?;
            let Some(type_id) = declaration.type_id.filter(|_| declaration.owns_storage()) else {
                continue;
            };
            let range = ByteRange {
                region: persistent.range.region,
                start: persistent
                    .range
                    .start
                    .checked_add(self.prepared.declaration_offset(id)?)?,
                bytes: self.prepared.type_layout(type_id)?.shape.bytes,
            };
            if contains(range) {
                return Some((MemoryLocation::Instance(instance), ordinal, range, type_id));
            }
            ordinal += 1;
        }
        None
    }

    /// Append to `path` the steps from the value of `type_id` at `range` to the target of
    /// `target_type` at byte `offset`: a structure's member, or an array's element with all its
    /// subscripts, until a value of the target's type starts at the target's byte.
    fn path_to(
        self,
        range: ByteRange,
        type_id: u32,
        offset: u32,
        target_type: u32,
        path: &mut RefPath,
        depth: usize,
    ) -> Option<()> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return None;
        }
        let (resolved, data) = resolve_alias(self.prepared, type_id)?;
        if range.start == offset
            && resolve_alias(self.prepared, target_type).map(|(id, _)| id) == Some(resolved)
        {
            return Some(());
        }
        let delta = offset.checked_sub(range.start)?;
        let layout = self.prepared.type_layout(resolved)?;
        // Inside a structure with a physical layout only a member beside the backing has bytes of
        // its own (C1-15).
        if let Some(overlay) = &layout.overlay {
            let slot = overlay.slots.iter().find(|slot| {
                self.prepared.type_layout(slot.type_id).is_some_and(|own| {
                    delta >= slot.byte_offset && delta - slot.byte_offset < own.shape.bytes
                })
            })?;
            path.extend(slot.path.iter().cloned());
            let child = ByteRange {
                start: range.start.checked_add(slot.byte_offset)?,
                bytes: self.prepared.type_layout(slot.type_id)?.shape.bytes,
                ..range
            };
            return self.path_to(child, slot.type_id, offset, target_type, path, depth + 1);
        }
        match data {
            TypeData::Struct { fields } | TypeData::Union { fields } => {
                for (member, field) in layout.members.iter().zip(fields) {
                    let bytes = self.prepared.type_layout(field.type_id)?.shape.bytes;
                    if delta >= member.byte_offset && delta - member.byte_offset < bytes {
                        path.push(RefSegment::Field(SmolStr::new(member_name(
                            self.prepared,
                            member.name_idx,
                        )?)));
                        let child = ByteRange {
                            start: range.start.checked_add(member.byte_offset)?,
                            bytes,
                            ..range
                        };
                        return self.path_to(
                            child,
                            field.type_id,
                            offset,
                            target_type,
                            path,
                            depth + 1,
                        );
                    }
                }
                None
            }
            TypeData::Array { elem_type_id, dims } => {
                let stride = layout.array.as_ref()?.byte_stride;
                let element = delta.checked_div(stride)?;
                // The element's subscripts, the last dimension varying fastest.
                let mut remaining = u64::from(element);
                let mut indices = alloc::vec![0i64; dims.len()];
                for (index, (lower, upper)) in indices.iter_mut().zip(dims).rev() {
                    let length = u64::try_from(upper.checked_sub(*lower)?.checked_add(1)?).ok()?;
                    *index = lower.checked_add(i64::try_from(remaining % length).ok()?)?;
                    remaining /= length;
                }
                path.push(RefSegment::Index(indices));
                let child = ByteRange {
                    start: range.start.checked_add(element.checked_mul(stride)?)?,
                    bytes: stride,
                    ..range
                };
                self.path_to(child, *elem_type_id, offset, target_type, path, depth + 1)
            }
            _ => None,
        }
    }

    /// The bytes and the type of slot `slot` of activation `id`, when that activation's frame is on
    /// the activation stack and the slot lies in it (C1-11), or when the activation is the staged
    /// result of an initializer evaluated in place, whose only slot is its result (C1-13).
    fn frame_slot(self, id: FrameId, slot: usize) -> Option<(ByteRange, u32)> {
        let regions = self.arm.cells.regions();
        if let Some(frame) = regions.frame(id) {
            return self.slot_in(frame.range, frame.pou, slot);
        }
        let result = regions.result(id)?;
        (slot == 0).then_some((result.range, result.type_id))
    }

    /// The bytes and the type of slot `slot` of a frame of `pou` whose bytes are `frame` (C1-11).
    fn slot_in(self, frame: ByteRange, pou: u32, slot: usize) -> Option<(ByteRange, u32)> {
        let (offset, type_id) = self.prepared.frame_slot(pou, slot)?;
        let bytes = self.prepared.type_layout(type_id)?.shape.bytes;
        Some((
            ByteRange {
                region: LayoutRegion::Activation,
                start: frame.start.checked_add(offset)?,
                bytes,
            },
            type_id,
        ))
    }

    /// The view of slot `slot` of a frame of `pou` whose bytes are `frame`, read where the frame
    /// lies even after its activation has ended, until the next frame takes its bytes (C1-11).
    pub(crate) fn frame_slot_view(
        self,
        frame: ByteRange,
        pou: u32,
        slot: usize,
    ) -> Option<ValueView<'a>> {
        let (range, type_id) = self.slot_in(frame, pou, slot)?;
        Some(self.view(range, type_id, None))
    }

    /// The instance an instance leaf names.
    fn instance_at(self, range: ByteRange) -> Result<InstanceId, VmTrap> {
        let bytes = self
            .arm
            .cells
            .regions()
            .block(range.region)
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
            MemoryLocation::Local(id) => match self.frame_slot(id, offset) {
                Some((range, type_id)) => self.walk(range, type_id, None, path),
                None => Ok(Target::Values(location, offset, path)),
            },
            // A flat reference names its target's bytes and type (C1-11d).
            MemoryLocation::Flat {
                region, type_id, ..
            } => {
                let range = ByteRange {
                    region,
                    start: u32::try_from(offset).map_err(|_| VmTrap::NullReference)?,
                    bytes: self
                        .prepared
                        .type_layout(type_id)
                        .ok_or(VmTrap::NullReference)?
                        .shape
                        .bytes,
                };
                self.walk(range, type_id, None, path)
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
            return Ok(Target::Cells(range, type_id, presence, false));
        }
        if !self.present(range.region, presence) {
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
            // Inside a structure with a physical layout the path names a node of its backing, or a
            // member beside the backing, from which it goes on (C1-15). Only the declaration itself
            // keeps a presence byte.
            if self.view_cells().overlay_layout(type_id).is_some() {
                let cells = self.view_cells();
                let (mut anchor, mut depth) = (0, 0);
                for (step, segment) in path[index..].iter().enumerate() {
                    match cells
                        .overlay_step(range, type_id, anchor, depth, segment)
                        .ok_or(VmTrap::NullReference)?
                    {
                        OverlayNode::Bits {
                            anchor: next,
                            depth: deeper,
                        } => (anchor, depth) = (next, deeper),
                        OverlayNode::Beside {
                            range: member,
                            type_id: member_type,
                        } => {
                            let rest = &path[index + step + 1..];
                            return Ok(match self.walk(member, member_type, None, rest)? {
                                Target::Cells(range, type_id, presence, _) => {
                                    Target::Cells(range, type_id, presence, true)
                                }
                                other => other,
                            });
                        }
                    }
                }
                let presence = if index == 0 { presence } else { None };
                return Ok(Target::Bits(range, type_id, presence, &path[index..]));
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
        Ok(Target::Cells(range, type_id, None, false))
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
        (RefSegment::Field(name), TypeData::Struct { fields } | TypeData::Union { fields }) => {
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
/// The nodes of structures with a physical layout (C1-15), in their own file.
#[path = "cell_overlay.rs"]
mod cell_overlay;
/// The flat store as the debugger reads it (C1-17), in its own file.
#[path = "cell_projection.rs"]
mod cell_projection;
/// The bytecode fixture of the cell arm's tests, whose builders the engine's construction tests reuse.
#[cfg(all(test, feature = "std"))]
pub(crate) mod fixture;
#[cfg(all(test, feature = "std"))]
mod retain_tests;
#[cfg(all(test, feature = "std"))]
mod tests;
#[cfg(all(test, feature = "std"))]
mod write_tests;
