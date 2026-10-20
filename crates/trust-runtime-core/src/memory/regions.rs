//! The persistent block of the bounded value store (specification 34 §6.2, MEM-12; C1-9a).
//!
//! [`crate::memory::regions::CellStorage`] is the store the engine runs on. Its construction reads
//! the declarations and the prepared layouts, charges the bytes of the persistent region (its flat
//! block, its reference table, its range tables, its instance table and the leaf lists the
//! construction builds) through the construction allowance, and only then reserves the region,
//! once. Every variable lies in the block at its declared size and alignment, texts inline, and
//! each persistent root gets its own byte range (O8). Construction then writes every declared
//! leaf's initial value into its bytes (R2), with the date/time profile of the engine that
//! constructs it (C1-9d2b-1).

use alloc::vec::Vec;

use smol_str::SmolStr;

use crate::bytecode::{StorageOwner, StorageRole, TypeData, TypeEntry, BYTECODE_MAX_TYPE_NESTING};
use crate::error::RuntimeError;
use crate::memory::cells::{write_value_in, FlatBlock, ReferenceTable};
use crate::memory::instance_table::{InstanceTable, PersistentInstance};
use crate::memory::layout::{align_up, ByteRange, LayoutRegion, INSTANCE_ALIGN};
use crate::memory::{
    ActivationStore, FrameId, Generation, Identities, InstanceId, VariableStorage,
};
use crate::value::{DateTimeProfile, EnumValue, Value, ValueRef};
use crate::vm::construction::values::scalar;
use crate::vm::type_policy::resolved_primitive_shape;
use crate::vm::PreparedModule;

/// One frame on the activation stack (C1-11): its activation identity, its bytes and its POU,
/// whose frame layout places each variable in those bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameBlock {
    pub(crate) id: FrameId,
    pub(crate) range: ByteRange,
    pub(crate) pou: u32,
}

/// The staged result of an initializer evaluated in place on the activation stack (C1-13): a value of
/// `type_id` in its bytes, which the store reads and writes as slot 0 of activation `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultBlock {
    pub(crate) id: FrameId,
    pub(crate) range: ByteRange,
    pub(crate) type_id: u32,
}

/// The first byte above a staged result at `range`: its end rounded up to [`INSTANCE_ALIGN`], so that
/// a frame placed above it stays aligned as frames are (C1-13). Saturates, so that an end beyond the
/// block is refused as one.
fn result_end(range: ByteRange) -> u32 {
    let end = range.start.saturating_add(range.bytes);
    align_up(end, INSTANCE_ALIGN).unwrap_or(u32::MAX)
}

/// The persistent region's byte ranges, its flat block and its reference table (specification 34
/// §6.2, MEM-12).
#[derive(Debug, Default)]
pub struct CellRegions {
    /// The byte range of each global declaration, by its STORAGE_LAYOUT index.
    globals: Vec<Option<ByteRange>>,
    /// The byte range of each persistent root, by its CONSTRUCTION_ROOTS index. Every root has
    /// its own range (O8), so two instances of one function block type get two ranges.
    roots: Vec<Option<ByteRange>>,
    /// The region's flat block.
    persistent: FlatBlock,
    /// The activation stack: the frames of the running calls, each on top of its caller's
    /// (specification 34 MEM-12; C1-11). Reserved at construction for the deepest call chain.
    activation: FlatBlock,
    /// The frames on the activation stack, bottom first, reserved for the call levels.
    frames: Vec<FrameBlock>,
    /// The staged results of the initializers evaluated in place, each above the frame it
    /// initializes, bottom first, reserved for the call levels (C1-13).
    results: Vec<ResultBlock>,
    /// The function-block and class instances that frames on the activation stack own, each above
    /// the frame that created it, in creation order, reserved for the call levels (C1-11c).
    call_instances: Vec<(InstanceId, PersistentInstance)>,
    /// The reference leaves and the `ValueRef` each non-null one holds, by byte offset (O9, R3,
    /// R5). Reserved at construction, one entry per reference leaf.
    references: ReferenceTable,
    /// The reference leaves of the frames and the instances on the activation stack and the
    /// `ValueRef` each non-null one holds, by byte offset (C1-19c4).
    activation_references: ReferenceTable,
    /// The persistent instances, one for each root with a template POU. Reserved at construction
    /// from that count and never grown (C1-9d2b-2, R5).
    instances: InstanceTable,
}

impl CellRegions {
    /// The byte range of the global declaration at STORAGE_LAYOUT index `index`, or `None` when
    /// that declaration owns no persistent bytes.
    #[must_use]
    pub fn global_range(&self, index: usize) -> Option<ByteRange> {
        self.globals.get(index).copied().flatten()
    }

    /// The byte range of the persistent root at CONSTRUCTION_ROOTS index `index`, or `None` when
    /// that root is not an instance.
    #[must_use]
    pub fn root_range(&self, index: usize) -> Option<ByteRange> {
        self.roots.get(index).copied().flatten()
    }

    /// The region's flat block.
    #[must_use]
    pub fn persistent(&self) -> &FlatBlock {
        &self.persistent
    }

    /// The activation stack's block (C1-11).
    #[must_use]
    pub fn activation(&self) -> &FlatBlock {
        &self.activation
    }

    /// The first free byte of the activation stack: the end of the topmost frame, of the instance a
    /// frame created last or of the latest staged result, whichever lies highest, or 0.
    pub(crate) fn activation_top(&self) -> u32 {
        let frame = self.frames.last().map_or(0, |frame| {
            frame.range.start.saturating_add(frame.range.bytes)
        });
        let instance = self.call_instances.last().map_or(0, |(_, instance)| {
            instance.range.start.saturating_add(instance.range.bytes)
        });
        let result = self
            .results
            .last()
            .map_or(0, |result| result_end(result.range));
        frame.max(instance).max(result)
    }

    /// The bytes a staged result of `bytes` bytes would take at the top of the activation stack, or
    /// `None` when they, rounded up to [`INSTANCE_ALIGN`], would leave the reserved block, or every
    /// reserved record is in use (C1-13).
    pub(crate) fn result_room(&self, bytes: u32) -> Option<ByteRange> {
        let range = ByteRange {
            region: LayoutRegion::Activation,
            start: self.activation_top(),
            bytes,
        };
        (result_end(range) as usize <= self.activation.reserved()
            && self.results.len() < self.results.capacity())
        .then_some(range)
    }

    /// Record `result` on top of the activation stack (C1-13). Refused, as a frame is, when its
    /// bytes leave the reserved block or the reserved records are all in use; neither grows.
    pub(crate) fn push_result(
        &mut self,
        result: ResultBlock,
        leaves: &[crate::memory::layout::ReferenceLeaf],
    ) -> Result<(), RuntimeError> {
        if result.range.region != LayoutRegion::Activation
            || result_end(result.range) as usize > self.activation.reserved()
            || self.results.len() == self.results.capacity()
            || !self.activation_references.push_leaves(
                leaves
                    .iter()
                    .map(|leaf| result.range.start.saturating_add(leaf.byte_offset)),
            )
        {
            return Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error());
        }
        self.results.push(result);
        Ok(())
    }

    /// The staged result of activation `id`, when it is on the activation stack (C1-13).
    pub(crate) fn result(&self, id: FrameId) -> Option<ResultBlock> {
        self.results
            .iter()
            .rev()
            .find(|result| result.id == id)
            .copied()
    }

    /// Take the staged result of activation `id` off the activation stack (C1-13).
    pub(crate) fn release_result(&mut self, id: FrameId) {
        if let Some(at) = self.results.iter().rposition(|result| result.id == id) {
            self.results.remove(at);
        }
        self.release_activation_references();
    }

    /// Record `frame` on top of the activation stack. Refused when its bytes leave the reserved block
    /// or the reserved frame records are all in use; neither grows (MEM-09).
    pub(crate) fn push_frame(
        &mut self,
        frame: FrameBlock,
        leaves: &[u32],
    ) -> Result<(), RuntimeError> {
        let end = frame
            .range
            .start
            .checked_add(frame.range.bytes)
            .ok_or(RuntimeError::Overflow)?;
        if frame.range.region != LayoutRegion::Activation
            || end as usize > self.activation.reserved()
            || self.frames.len() == self.frames.capacity()
            || !self.activation_references.push_leaves(
                leaves
                    .iter()
                    .map(|leaf| frame.range.start.saturating_add(*leaf)),
            )
        {
            return Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error());
        }
        self.frames.push(frame);
        Ok(())
    }

    /// The frame of activation `id`, when it is on the activation stack.
    pub(crate) fn frame(&self, id: FrameId) -> Option<FrameBlock> {
        self.frames
            .iter()
            .rev()
            .find(|frame| frame.id == id)
            .copied()
    }

    /// Take the frame of activation `id` off the activation stack, with the reference leaves above
    /// what the stack still holds.
    pub(crate) fn release_frame(&mut self, id: FrameId) {
        if let Some(at) = self.frames.iter().rposition(|frame| frame.id == id) {
            self.frames.remove(at);
        }
        self.release_activation_references();
    }

    /// Drop the reference leaves above the top of the activation stack, whose frames and instances
    /// have left it (C1-19c4).
    fn release_activation_references(&mut self) {
        let top = self.activation_top();
        self.activation_references.release_from(top);
    }

    /// The references that leaves leaving the activation stack held, for the engine to take their
    /// path buffers back (C1-19c4).
    pub(crate) fn drain_released_references(&mut self) -> impl Iterator<Item = ValueRef> + '_ {
        self.activation_references.drain_released()
    }

    /// The bytes of `range` on the activation stack, mutably (C1-11).
    pub(crate) fn activation_bytes_mut(&mut self, range: ByteRange) -> Option<&mut [u8]> {
        self.activation.bytes_mut(range)
    }

    /// The block that holds the bytes of `region`.
    #[must_use]
    pub fn block(&self, region: LayoutRegion) -> &FlatBlock {
        match region {
            LayoutRegion::Persistent => &self.persistent,
            LayoutRegion::Activation => &self.activation,
        }
    }

    /// The region's reference table.
    pub(crate) fn references(&self) -> &ReferenceTable {
        &self.references
    }

    /// The reference table of `region`: the persistent block's, or the activation stack's
    /// (C1-19c4).
    pub(crate) fn references_in(&self, region: LayoutRegion) -> &ReferenceTable {
        match region {
            LayoutRegion::Persistent => &self.references,
            LayoutRegion::Activation => &self.activation_references,
        }
    }

    /// The reference that the reference leaf at byte `offset` of `region` holds through the
    /// region's table, or `None` for a null leaf (C1-19c4).
    pub(crate) fn reference_in(&self, region: LayoutRegion, offset: u32) -> Option<&ValueRef> {
        self.references_in(region).get(offset)
    }

    /// The reference the last write into a reference leaf of either table replaced, for the engine
    /// to take its path buffers back (spec 34 ARCH-05).
    pub(crate) fn take_displaced(&mut self) -> Option<ValueRef> {
        self.references
            .take_displaced()
            .or_else(|| self.activation_references.take_displaced())
    }

    /// The instance `id` the block holds: a persistent instance, or one that a frame on the
    /// activation stack owns (C1-11c). `None` for an identity that is neither.
    pub(crate) fn instance(&self, id: InstanceId) -> Option<&PersistentInstance> {
        self.instances.get(id).or_else(|| {
            self.call_instances
                .iter()
                .rev()
                .find(|(candidate, _)| *candidate == id)
                .map(|(_, instance)| instance)
        })
    }

    /// Whether `id` is a persistent instance, which lives as long as its generation.
    pub(crate) fn is_persistent_instance(&self, id: InstanceId) -> bool {
        self.instances.get(id).is_some()
    }

    /// Record the instance `id` that a frame on the activation stack owns, placed at the top of the
    /// stack. Refused, as a frame is, when its bytes leave the stack or the reserved records are
    /// all in use; neither grows (MEM-09).
    pub(crate) fn push_call_instance(
        &mut self,
        id: InstanceId,
        instance: PersistentInstance,
        leaves: &[u32],
    ) -> Result<(), RuntimeError> {
        let end = instance
            .range
            .start
            .checked_add(instance.range.bytes)
            .ok_or(RuntimeError::Overflow)?;
        if end as usize > self.activation.reserved()
            || self.call_instances.len() == self.call_instances.capacity()
            || !self.activation_references.push_leaves(
                leaves
                    .iter()
                    .map(|leaf| instance.range.start.saturating_add(*leaf)),
            )
        {
            return Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error());
        }
        self.call_instances.push((id, instance));
        Ok(())
    }

    /// Release the instance `id` that a frame on the activation stack owned; `false` when it is not
    /// one.
    pub(crate) fn release_call_instance(&mut self, id: InstanceId) -> bool {
        match self
            .call_instances
            .iter()
            .rposition(|(candidate, _)| *candidate == id)
        {
            Some(at) => {
                self.call_instances.remove(at);
                self.release_activation_references();
                true
            }
            None => false,
        }
    }

    /// The number of instances that frames on the activation stack own.
    pub(crate) fn call_instance_count(&self) -> usize {
        self.call_instances.len()
    }

    /// Whether a frame is on the activation stack, that is, a call runs.
    pub(crate) fn holds_frames(&self) -> bool {
        !self.frames.is_empty()
    }

    /// The persistent instance whose own members hold the byte at `offset` (C1-11d).
    pub(crate) fn instance_holding(&self, offset: u32) -> Option<(InstanceId, PersistentInstance)> {
        self.instances.holding(offset)
    }

    /// The persistent instance at `position` in identity order.
    pub(crate) fn instance_at(&self, position: usize) -> Option<(InstanceId, PersistentInstance)> {
        self.instances.at(position)
    }

    /// The number of persistent instances recorded.
    pub(crate) fn instance_count(&self) -> usize {
        self.instances.len()
    }

    /// The identity of every instance the block holds: the persistent instances in identity order,
    /// then those that frames on the activation stack own, in creation order (C1-17).
    pub(crate) fn instance_ids(&self) -> impl Iterator<Item = InstanceId> + '_ {
        self.instances
            .ids()
            .chain(self.call_instances.iter().map(|(id, _)| *id))
    }

    /// Record the persistent instance `id` in the table reserved at construction.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] when the table is full (see [`InstanceTable::insert`]).
    pub(crate) fn register_instance(
        &mut self,
        id: InstanceId,
        instance: PersistentInstance,
    ) -> Result<(), RuntimeError> {
        self.instances.insert(id, instance)
    }

    /// Give the persistent instance `id` the parent `parent`. `false` when `id` is not one.
    pub(crate) fn set_instance_parent(
        &mut self,
        id: InstanceId,
        parent: Option<InstanceId>,
    ) -> bool {
        let instance = match self.instances.get_mut(id) {
            Some(instance) => Some(instance),
            None => self
                .call_instances
                .iter_mut()
                .rev()
                .find(|(candidate, _)| *candidate == id)
                .map(|(_, instance)| instance),
        };
        match instance {
            Some(instance) => {
                instance.parent = parent;
                true
            }
            None => false,
        }
    }

    /// The number of persistent instances the reserved table holds room for (test support).
    #[cfg(test)]
    pub(crate) fn instance_capacity(&self) -> usize {
        self.instances.capacity()
    }

    /// The persistent block and the activation stack, mutably at once (C1-19c4).
    pub(crate) fn blocks_mut(&mut self) -> (&mut FlatBlock, &mut FlatBlock) {
        (&mut self.persistent, &mut self.activation)
    }

    /// The flat block and the reference table, mutably at once (C1-9c).
    pub(crate) fn split_mut(&mut self) -> (&mut FlatBlock, &mut ReferenceTable) {
        (&mut self.persistent, &mut self.references)
    }

    /// The block that holds the bytes of `region`, mutably, with the reference table of that region
    /// (C1-19c4).
    pub(crate) fn block_mut(
        &mut self,
        region: LayoutRegion,
    ) -> (&mut FlatBlock, Option<&mut ReferenceTable>) {
        match region {
            LayoutRegion::Persistent => (&mut self.persistent, Some(&mut self.references)),
            LayoutRegion::Activation => {
                (&mut self.activation, Some(&mut self.activation_references))
            }
        }
    }

    /// Write every declared leaf's initial value into its bytes (R2): the declaration's constant
    /// when it has one, else the default of its type under `profile`, the date/time profile of the
    /// engine that constructs the store. A leaf whose value only construction can give (an
    /// instance, which the engine creates) keeps its zero bytes.
    fn write_initial_values(
        &mut self,
        prepared: &PreparedModule,
        profile: DateTimeProfile,
        initials: &[Initial],
    ) -> Result<(), RuntimeError> {
        let (persistent, references) = self.split_mut();
        let mut defaults = DefaultWriter {
            prepared,
            profile,
            references,
        };
        for initial in initials {
            let bytes = persistent
                .bytes_mut(initial.range)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            match initial.constant {
                Some(index) => {
                    let value = prepared
                        .vm
                        .consts
                        .get(index as usize)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    write_value_in(
                        bytes,
                        initial.type_id,
                        value,
                        prepared,
                        Some(&mut *defaults.references),
                        initial.range.start,
                    )?;
                }
                None => defaults.write(bytes, initial.type_id, initial.range.start, 0)?,
            }
        }
        Ok(())
    }
}

/// What the store of a prepared module reserves: the persistent extent, the activation stack's bytes,
/// its frame and staged-result records, the instances calls create on it and its reference leaves.
struct StoreShape {
    extent: PersistentExtent,
    activation_bytes: u32,
    levels: usize,
    result_count: usize,
    call_instance_count: usize,
    activation_leaves: usize,
}

impl StoreShape {
    fn of(prepared: &PreparedModule) -> Result<Self, RuntimeError> {
        // The stack and one frame record for each call level, when some POU has a frame (C1-11),
        // a record for each instance the frames on it can own (C1-11c) and, when some initializer
        // is evaluated in place, a staged-result record for each call level (C1-13); and the
        // activation stack's reference table, for the reference leaves its frames and their
        // instances hold at most (C1-19c4).
        let activation_bytes = prepared.activation_reservation()?;
        let (levels, result_count) = match activation_bytes {
            0 => (0, 0),
            _ => (
                prepared.reserved_call_levels(),
                prepared.result_block_reservation(),
            ),
        };
        Ok(Self {
            extent: PersistentExtent::of(prepared)?,
            activation_bytes,
            levels,
            result_count,
            call_instance_count: prepared.call_instance_reservation()?,
            activation_leaves: prepared.activation_reference_reservation()?,
        })
    }
}

impl CellStorage {
    /// The reference leaves of the persistent block of `prepared` (C1-19c3).
    pub(crate) fn persistent_reference_leaves_of(
        prepared: &PreparedModule,
    ) -> Result<usize, RuntimeError> {
        Ok(PersistentExtent::of(prepared)?.references.len())
    }

    /// The bytes of heap the store of `prepared` keeps once construction has reserved it
    /// (specification 34 MEM-12), as [`CellStorage::construct`] reserves them: the persistent
    /// region's (its range tables, its block, its reference table and its instance table) and the
    /// activation stack's (its block, its frame, staged-result and call-instance records and its
    /// reference table). The lists construction only walks are not kept.
    pub(crate) fn heap_bytes(prepared: &PreparedModule) -> Result<(usize, usize), RuntimeError> {
        let shape = StoreShape::of(prepared)?;
        let ranges = shape
            .extent
            .globals
            .len()
            .checked_add(shape.extent.roots.len())
            .and_then(|count| count.checked_mul(core::mem::size_of::<Option<ByteRange>>()));
        let records = shape
            .levels
            .checked_mul(core::mem::size_of::<FrameBlock>())
            .zip(
                shape
                    .result_count
                    .checked_mul(core::mem::size_of::<ResultBlock>()),
            )
            .and_then(|(frames, results)| frames.checked_add(results))
            .zip(InstanceTable::reservation_bytes(shape.call_instance_count))
            .and_then(|(records, instances)| records.checked_add(instances));
        let sum = |parts: &[Option<usize>]| {
            parts
                .iter()
                .try_fold(0usize, |sum, bytes| sum.checked_add((*bytes)?))
                .ok_or(RuntimeError::Overflow)
        };
        Ok((
            sum(&[
                ranges,
                usize::try_from(shape.extent.bytes).ok(),
                ReferenceTable::reservation_bytes(shape.extent.references.len()),
                InstanceTable::reservation_bytes(shape.extent.instance_roots()),
            ])?,
            sum(&[
                usize::try_from(shape.activation_bytes).ok(),
                records,
                ReferenceTable::room_bytes(shape.activation_leaves),
            ])?,
        ))
    }
}

/// The bounded value store (specification 34 §6.2, MEM-12): the regions, the activation half and
/// the identities. It is built only through [`CellStorage::construct`]; the engine constructs and
/// charges it when it builds the cell arm (O18; C1-9d2b-1), and switches to it by default in C1-9d3.
#[derive(Debug, Default)]
#[allow(
    dead_code,
    reason = "the engine reads the activation half from C1-9d3; until then the engine's Value half holds the activations"
)]
pub struct CellStorage {
    regions: CellRegions,
    activations: ActivationStore,
    identities: Identities,
}

impl CellStorage {
    /// Build the persistent region of `prepared` for `generation` (specification 34 §6.2; O18) and
    /// write its declared initial values (R2), with the default date/time profile.
    ///
    /// Before anything is reserved, `charge` receives the bytes the region takes: its flat block,
    /// its reference table's bytes, its `globals` and `roots` range tables, its instance table and
    /// the leaf lists the construction builds (R6; C1-9d2b-2, R5). A refused charge returns its
    /// error, and nothing is reserved.
    ///
    /// # Errors
    ///
    /// Returns the error `charge` refuses with, a preparation error when the layouts and the
    /// declarations disagree, or an allocation refusal.
    pub fn construct(
        prepared: &PreparedModule,
        generation: Generation,
        charge: impl FnOnce(usize) -> Result<(), RuntimeError>,
    ) -> Result<Self, RuntimeError> {
        Self::construct_with_profile(prepared, generation, DateTimeProfile::default(), charge)
    }

    /// [`CellStorage::construct`] with the date/time profile of the engine that constructs the store:
    /// the default of a `DATE` or `DATE_AND_TIME` leaf is the profile's epoch (specification 34 §6.2;
    /// R3).
    ///
    /// # Errors
    ///
    /// As [`CellStorage::construct`].
    pub(crate) fn construct_with_profile(
        prepared: &PreparedModule,
        generation: Generation,
        profile: DateTimeProfile,
        charge: impl FnOnce(usize) -> Result<(), RuntimeError>,
    ) -> Result<Self, RuntimeError> {
        let StoreShape {
            extent,
            activation_bytes,
            levels,
            result_count,
            call_instance_count,
            activation_leaves,
        } = StoreShape::of(prepared)?;
        let block_bytes = usize::try_from(extent.bytes).map_err(|_| RuntimeError::Overflow)?;
        let table_bytes = ReferenceTable::reservation_bytes(extent.references.len())
            .ok_or(RuntimeError::Overflow)?;
        let bookkeeping_bytes = extent.bookkeeping_bytes()?;
        let instance_count = extent.instance_roots();
        let activation_table_bytes =
            ReferenceTable::room_bytes(activation_leaves).ok_or(RuntimeError::Overflow)?;
        let frame_bytes = levels
            .checked_mul(FRAME_RECORD_BYTES)
            .and_then(|bytes| bytes.checked_add(result_count.checked_mul(RESULT_RECORD_BYTES)?))
            .and_then(|bytes| {
                bytes.checked_add(InstanceTable::reservation_bytes(call_instance_count)?)
            })
            .ok_or(RuntimeError::Overflow)?;
        let bytes = block_bytes
            .checked_add(table_bytes)
            .and_then(|bytes| bytes.checked_add(activation_table_bytes))
            .and_then(|bytes| bytes.checked_add(bookkeeping_bytes))
            .and_then(|bytes| bytes.checked_add(usize::try_from(activation_bytes).ok()?))
            .and_then(|bytes| bytes.checked_add(frame_bytes))
            .ok_or(RuntimeError::Overflow)?;
        charge(bytes)?;
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(levels)
            .map_err(|_| crate::vm::VmTrap::BudgetExceeded.into_runtime_error())?;
        let mut results = Vec::new();
        results
            .try_reserve_exact(result_count)
            .map_err(|_| crate::vm::VmTrap::BudgetExceeded.into_runtime_error())?;
        let mut call_instances = Vec::new();
        call_instances
            .try_reserve_exact(call_instance_count)
            .map_err(|_| crate::vm::VmTrap::BudgetExceeded.into_runtime_error())?;
        let mut regions = CellRegions {
            globals: extent.globals,
            roots: extent.roots,
            persistent: FlatBlock::reserve(extent.bytes)?,
            activation: FlatBlock::reserve_in(LayoutRegion::Activation, activation_bytes)?,
            frames,
            results,
            call_instances,
            references: ReferenceTable::reserve(extent.references)?,
            activation_references: ReferenceTable::with_room(activation_leaves)?,
            instances: InstanceTable::reserve(instance_count)?,
        };
        regions.write_initial_values(prepared, profile, &extent.initials)?;
        Ok(Self {
            regions,
            activations: ActivationStore::default(),
            identities: Identities {
                generation,
                ..Identities::default()
            },
        })
    }

    /// The persistent region and its ranges.
    #[must_use]
    pub fn regions(&self) -> &CellRegions {
        &self.regions
    }

    /// The identity the next instance takes from the one counter that both kinds of instance share, and
    /// the counter value after it (specification 34 §6.2; R4). Nothing is issued until
    /// [`CellStorage::issue_instance`] records it, so an instance that cannot be placed leaves the counter
    /// where it was, as the `Value` store does.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Overflow`] once the counter is exhausted.
    pub(crate) fn next_instance_id(&self) -> Result<(InstanceId, u32), RuntimeError> {
        self.identities.next_instance()
    }

    /// Give `storage` this store's identities: its instance counter, its entry floor and its
    /// generation, so that a copy of the store issues no identity the store holds (C1-17).
    pub(crate) fn lend_identities(&self, storage: &mut VariableStorage) {
        storage.identities = self.identities.clone();
    }

    /// Record that `id` was issued and the counter now holds `following` (see
    /// [`CellStorage::next_instance_id`]).
    pub(crate) fn issue_instance(&mut self, id: InstanceId, following: u32) {
        self.identities.issue(id, following);
    }

    /// Issue the next instance identity in one step (test support).
    #[cfg(test)]
    pub(crate) fn issue_instance_id(&mut self) -> Result<InstanceId, RuntimeError> {
        let (id, following) = self.next_instance_id()?;
        self.issue_instance(id, following);
        Ok(id)
    }

    /// Record that the per-call instance `id` is gone, so that the entry floor can reissue its
    /// identity (specification 34 §4.3.1, HOST-02).
    pub(crate) fn release_instance(&mut self, id: InstanceId) {
        self.identities.release_instance(id);
    }

    /// Record that `released` instances at or above the entry floor are gone.
    pub(crate) fn release_instances(&mut self, released: usize) {
        self.identities.release(released);
    }

    /// The instance identity the current top-level execution entry started from.
    pub(crate) fn entry_instance_floor(&self) -> u32 {
        self.identities.entry_instance_floor
    }

    /// Begin the instance half of a top-level execution entry (specification 34 §4.3.1, HOST-02): the
    /// identities issued since the previous entry began are issued again when none of their instances is
    /// still live, and the persistent instances that construction created never are. The floor is the
    /// one counter's own (R4).
    pub(crate) fn begin_execution_entry(&mut self) {
        // No activation of an earlier entry is live, so no frame of one is on the stack (C1-11),
        // and no instance such a frame owned (C1-11c).
        for (id, _) in self.regions.call_instances.drain(..) {
            self.identities.release_instance(id);
        }
        self.identities.begin_entry();
        self.regions.frames.clear();
        self.regions.results.clear();
    }

    /// Release every instance that a frame on the activation stack owns and `keep` refuses, and
    /// record each release in the counter that issued the identity (C1-11c). Answers how many were
    /// released.
    pub(crate) fn retain_call_instances(
        &mut self,
        mut keep: impl FnMut(InstanceId) -> bool,
    ) -> usize {
        let before = self.regions.call_instances.len();
        let identities = &mut self.identities;
        self.regions.call_instances.retain(|&(id, _)| {
            let kept = keep(id);
            if !kept {
                identities.release_instance(id);
            }
            kept
        });
        self.regions.release_activation_references();
        before - self.regions.call_instances.len()
    }

    /// Move the next instance identity, so that a test reaches the identity limit without issuing
    /// 2^32 identities.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn set_next_instance_id_for_test(&mut self, next: u32) {
        self.identities.next_instance_id = next;
    }

    /// Move the installation generation, so that a test reaches the last generation without 2^32
    /// installations.
    #[cfg(test)]
    pub(crate) fn set_generation_for_test(&mut self, generation: Generation) {
        self.identities.generation = generation;
    }
}

/// What the persistent region holds, read from the declarations and the prepared layouts
/// (specification 34 §6.2, MEM-12): the global and root ranges, the bytes and reference leaves they
/// need, and the initial value of each declared leaf.
pub(crate) struct PersistentExtent {
    globals: Vec<Option<ByteRange>>,
    roots: Vec<Option<ByteRange>>,
    /// The bytes of the whole block: the globals, then each root that has bytes at
    /// [`INSTANCE_ALIGN`].
    pub(crate) bytes: u32,
    /// The byte offset of each reference leaf, in offset order (O9).
    pub(crate) references: Vec<u32>,
    initials: Vec<Initial>,
}

impl PersistentExtent {
    /// Read the persistent region of `prepared`: the globals first, at the offsets preparation gave
    /// them, then each persistent root in CONSTRUCTION_ROOTS order, each that has bytes at
    /// [`INSTANCE_ALIGN`] (specification 34 §6.2, MEM-12, O6, O8). A root that is not an
    /// inheritance parent reserves its whole inheritance chain: its own variables, then its
    /// parent's, and so on up (R0, C1-9c). The inheritance parent of a derived root lies inside
    /// that root's range and reserves no bytes of its own, so no byte is reserved twice.
    pub(crate) fn of(prepared: &PreparedModule) -> Result<Self, RuntimeError> {
        let mut leaves = Leaves::default();
        let mut globals = alloc::vec![None; prepared.layout.entries.len()];
        let mut end = 0u32;
        for (slot, declaration) in prepared.layout.entries.iter().enumerate() {
            if declaration.owner != StorageOwner::Global || !declaration.owns_storage() {
                continue;
            }
            let Some(type_id) = declaration.type_id else {
                continue;
            };
            let index = u32::try_from(slot).map_err(|_| RuntimeError::Overflow)?;
            let range = prepared
                .global_range(index)
                .ok_or(RuntimeError::TypeMismatch)?;
            globals[slot] = Some(range);
            end = end.max(slot_end(prepared, index, range.start, range.bytes)?);
            leaves.add_value(prepared, type_id, range.start, 0)?;
            if declaration.role != StorageRole::Static {
                leaves.initials.push(Initial {
                    range,
                    type_id,
                    constant: declaration.default_const_idx,
                });
            }
        }

        let mut bytes = end;
        let mut roots: Vec<Option<ByteRange>> = alloc::vec![None; prepared.roots.entries.len()];
        for (index, root) in prepared.roots.entries.iter().enumerate() {
            let Some(pou) = root.template_pou_id else {
                continue;
            };
            let range = if root.is_inheritance_parent() {
                let child = root.parent_root_idx.ok_or(RuntimeError::TypeMismatch)?;
                inherited_range(prepared, &roots, child, pou)?
            } else {
                let chain = chain_of(prepared, pou)?;
                let total = chain_total(&chain)?;
                // A root with bytes starts at INSTANCE_ALIGN; one without needs no padding.
                let start = match total {
                    0 => bytes,
                    _ => align_up(bytes, INSTANCE_ALIGN)?,
                };
                let range = ByteRange {
                    region: LayoutRegion::Persistent,
                    start,
                    bytes: total,
                };
                let mut base = range.start;
                for &(member, own) in &chain {
                    if leaves.add_instance(prepared, member, base)? != own {
                        return Err(RuntimeError::TypeMismatch);
                    }
                    base = offset_of(base, own)?;
                }
                bytes = offset_of(start, total)?;
                range
            };
            roots[index] = Some(range);
        }
        leaves.references.sort_unstable();
        Ok(Self {
            globals,
            roots,
            bytes,
            references: leaves.references,
            initials: leaves.initials,
        })
    }

    /// The roots that hold an instance: the CONSTRUCTION_ROOTS records with a template POU. Each gets one
    /// persistent instance, so this is the room the instance table reserves (C1-9d2b-2, R5).
    fn instance_roots(&self) -> usize {
        self.roots.iter().filter(|range| range.is_some()).count()
    }

    /// The bytes of the tables the region keeps and of the lists construction builds while it walks the
    /// layouts (specification 34 §6.2; R6, O18): one range entry per STORAGE_LAYOUT declaration in the
    /// `globals` table, one per CONSTRUCTION_ROOTS record in the `roots` table and one per instance root in
    /// the instance table, which the region keeps, and the transient reference-leaf and
    /// initial-value lists. They are memory the construction holds, so the charge counts them with
    /// the block before anything is reserved. Each entry has the same size on every target, which
    /// the assertions state.
    fn bookkeeping_bytes(&self) -> Result<usize, RuntimeError> {
        let tables = self
            .globals
            .len()
            .checked_add(self.roots.len())
            .and_then(|entries| entries.checked_mul(RANGE_ENTRY_BYTES));
        let lists = self
            .references
            .len()
            .checked_mul(REFERENCE_LEAF_BYTES)
            .and_then(|leaves| {
                self.initials
                    .len()
                    .checked_mul(INITIAL_VALUE_BYTES)
                    .and_then(|initials| leaves.checked_add(initials))
            });
        tables
            .zip(lists)
            .and_then(|(tables, lists)| tables.checked_add(lists))
            .zip(InstanceTable::reservation_bytes(self.instance_roots()))
            .and_then(|(bytes, instances)| bytes.checked_add(instances))
            .ok_or(RuntimeError::Overflow)
    }
}

/// The bytes of one entry of the `globals` and `roots` range tables (R6).
const RANGE_ENTRY_BYTES: usize = 12;
/// The bytes of one entry of the transient reference-leaf list (R6): a byte offset, a `u32`.
const REFERENCE_LEAF_BYTES: usize = 4;
/// The bytes of one entry of the transient initial-value list (R6).
const INITIAL_VALUE_BYTES: usize = 24;
/// The bytes of one record of a frame on the activation stack (C1-11): its activation identity, its
/// byte range and its POU.
const FRAME_RECORD_BYTES: usize = 20;
/// The bytes of one record of a staged result on the activation stack (C1-13): its activation
/// identity, its byte range and its type.
const RESULT_RECORD_BYTES: usize = 20;

// The entries have the sizes the specification states on every target, since none holds a pointer.
const _: () = assert!(core::mem::size_of::<Option<ByteRange>>() == RANGE_ENTRY_BYTES);
const _: () = assert!(core::mem::size_of::<Initial>() == INITIAL_VALUE_BYTES);
const _: () = assert!(core::mem::size_of::<FrameBlock>() == FRAME_RECORD_BYTES);
const _: () = assert!(core::mem::size_of::<ResultBlock>() == RESULT_RECORD_BYTES);

/// A declared leaf whose initial value construction writes (R2): its bytes, its type and the
/// constant its declaration names, when it names one.
struct Initial {
    range: ByteRange,
    type_id: u32,
    constant: Option<u32>,
}

/// The reference leaves and the initial values found while walking the persistent region.
#[derive(Default)]
struct Leaves {
    references: Vec<u32>,
    initials: Vec<Initial>,
}

impl Leaves {
    /// Add the leaves of one persistent instance of template `pou` placed at byte `base`: its own
    /// declarations in order, each at the offset preparation gave it within the template. Answers
    /// the template's bytes, rounded up to [`INSTANCE_ALIGN`], which the template's own bytes must
    /// equal (specification 34 §6.2, MEM-12).
    fn add_instance(
        &mut self,
        prepared: &PreparedModule,
        pou: u32,
        base: u32,
    ) -> Result<u32, RuntimeError> {
        let mut end = 0u32;
        for &id in prepared.declarations(StorageOwner::Instance, Some(pou)) {
            let declaration = prepared
                .layout
                .entries
                .get(id as usize)
                .ok_or(RuntimeError::TypeMismatch)?;
            let Some(type_id) = declaration.type_id else {
                continue;
            };
            if !declaration.owns_storage() {
                continue;
            }
            let offset = prepared
                .declaration_offset(id)
                .ok_or(RuntimeError::TypeMismatch)?;
            let bytes = layout_of(prepared, type_id)?.shape.bytes;
            let start = offset_of(base, offset)?;
            self.add_value(prepared, type_id, start, 0)?;
            if declaration.role != StorageRole::NativeState {
                self.initials.push(Initial {
                    range: ByteRange {
                        region: LayoutRegion::Persistent,
                        start,
                        bytes,
                    },
                    type_id,
                    constant: declaration.default_const_idx,
                });
            }
            end = end.max(slot_end(prepared, id, offset, bytes)?);
        }
        align_up(end, INSTANCE_ALIGN)
    }

    /// Add the reference leaves of one value of `type_id` placed at byte `base`. Arrays expand per
    /// element and structures and unions per member, so each leaf occurrence is counted once (O10). A
    /// structure with a physical layout holds its references beside its backing (C1-15).
    fn add_value(
        &mut self,
        prepared: &PreparedModule,
        type_id: u32,
        base: u32,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let entry = prepared
            .vm
            .types
            .entries
            .get(type_id as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        let layout = layout_of(prepared, type_id)?;
        match &entry.data {
            TypeData::Reference { .. } => self.references.push(base),
            TypeData::Alias { target_type_id } => {
                self.add_value(prepared, *target_type_id, base, depth + 1)?;
            }
            TypeData::Array { elem_type_id, .. } => {
                if layout.reference_leaves.is_empty() {
                    return Ok(());
                }
                for offset in element_offsets(layout)? {
                    self.add_value(
                        prepared,
                        *elem_type_id,
                        offset_of(base, offset?)?,
                        depth + 1,
                    )?;
                }
            }
            TypeData::Struct { fields } | TypeData::Union { fields } => match &layout.overlay {
                // A structure with a physical layout holds its references beside its backing
                // (C1-15).
                Some(overlay) => {
                    for slot in &overlay.slots {
                        let start = offset_of(base, slot.byte_offset)?;
                        self.add_value(prepared, slot.type_id, start, depth + 1)?;
                    }
                }
                None => {
                    for (field, member) in fields.iter().zip(&layout.members) {
                        let start = offset_of(base, member.byte_offset)?;
                        self.add_value(prepared, field.type_id, start, depth + 1)?;
                    }
                }
            },
            _ => {}
        }
        Ok(())
    }
}

struct DefaultWriter<'a> {
    prepared: &'a PreparedModule,
    profile: DateTimeProfile,
    references: &'a mut ReferenceTable,
}

impl DefaultWriter<'_> {
    /// Write the default of the `TYPE_TABLE` type `type_id` into `bytes`, the bytes of one value of
    /// it that starts at byte `start` of its block (R2). A structure and an array are written leaf
    /// by leaf, so no whole default value is built. A type whose value only construction can give
    /// keeps its bytes as they are: an instance, a structure with a physical layout, whose backing
    /// construction encodes from its members (C1-15), and a union.
    fn write(
        &mut self,
        bytes: &mut [u8],
        type_id: u32,
        start: u32,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        let prepared = self.prepared;
        let entry = prepared
            .vm
            .types
            .entries
            .get(type_id as usize)
            .ok_or(RuntimeError::TypeMismatch)?;
        match &entry.data {
            TypeData::Alias { target_type_id } => {
                self.write(bytes, *target_type_id, start, depth + 1)
            }
            TypeData::Struct { fields } | TypeData::Union { fields }
                if layout_of(prepared, type_id)?.overlay.is_none() =>
            {
                let layout = layout_of(prepared, type_id)?;
                for (member, field) in layout.members.iter().zip(fields) {
                    let size = layout_of(prepared, field.type_id)?.shape.bytes;
                    self.write(
                        part(bytes, member.byte_offset, size)?,
                        field.type_id,
                        offset_of(start, member.byte_offset)?,
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            TypeData::Array { elem_type_id, .. } => {
                let layout = layout_of(prepared, type_id)?;
                let size = layout_of(prepared, *elem_type_id)?.shape.bytes;
                // An `ARRAY[*]` has no bytes of its own, so there is no element to write.
                for offset in element_offsets(layout)? {
                    let offset = offset?;
                    self.write(
                        part(bytes, offset, size)?,
                        *elem_type_id,
                        offset_of(start, offset)?,
                        depth + 1,
                    )?;
                }
                Ok(())
            }
            _ => match default_leaf(prepared, self.profile, entry)? {
                Some(value) => write_value_in(
                    bytes,
                    type_id,
                    &value,
                    prepared,
                    Some(&mut *self.references),
                    start,
                ),
                None => Ok(()),
            },
        }
    }
}

/// The byte offset of each element of an array of layout `layout`, in flat order: none for an
/// `ARRAY[*]`, which has no bytes of its own.
fn element_offsets(
    layout: &crate::memory::layout::TypeLayout,
) -> Result<impl Iterator<Item = Result<u32, RuntimeError>>, RuntimeError> {
    let stride = layout
        .array
        .as_ref()
        .map(|array| array.byte_stride)
        .ok_or(RuntimeError::TypeMismatch)?;
    let count = layout.shape.bytes.checked_div(stride).unwrap_or(0);
    Ok((0..count).map(move |element| element.checked_mul(stride).ok_or(RuntimeError::Overflow)))
}

/// The default value of a leaf type: a scalar, a text, an enumeration's first variant, a null
/// reference, or a subrange's lower bound (IEC 61131-3 defaults, as construction gives them). A `DATE`
/// or `DATE_AND_TIME` defaults to the epoch of `profile`. `None` for an instance and for a type
/// C1-15 places.
fn default_leaf(
    prepared: &PreparedModule,
    profile: DateTimeProfile,
    entry: &TypeEntry,
) -> Result<Option<Value>, RuntimeError> {
    Ok(match &entry.data {
        TypeData::Primitive { prim_id, .. } => Some(scalar::default(*prim_id, profile)?),
        TypeData::Subrange {
            base_type_id,
            lower,
            ..
        } => {
            // The lower bound is a number; its base type is the primitive it lies as, so it is
            // narrowed into that primitive as the value store's construction does.
            let (prim_id, max_length) =
                resolved_primitive_shape(prepared.type_table(), *base_type_id, 0)
                    .ok_or(RuntimeError::TypeMismatch)?;
            Some(scalar::coerce(prim_id, max_length, Value::LInt(*lower))?)
        }
        TypeData::Enum { variants, .. } => {
            let variant = variants.first().ok_or(RuntimeError::TypeMismatch)?;
            let name = string_of(prepared, variant.name_idx)?;
            Some(Value::Enum(alloc::boxed::Box::new(
                EnumValue::from_canonical_parts(type_name(prepared, entry), name, variant.value),
            )))
        }
        TypeData::Reference { .. } => Some(Value::Reference(None)),
        TypeData::Struct { .. }
        | TypeData::Union { .. }
        | TypeData::Pou { .. }
        | TypeData::Interface { .. }
        | TypeData::Array { .. }
        | TypeData::Alias { .. } => None,
    })
}

/// The bytes `offset` to `offset + len` of `bytes`, or a refusal when the layout does not fit them.
fn part(bytes: &mut [u8], offset: u32, len: u32) -> Result<&mut [u8], RuntimeError> {
    let start = usize::try_from(offset).map_err(|_| RuntimeError::Overflow)?;
    let end = start
        .checked_add(usize::try_from(len).map_err(|_| RuntimeError::Overflow)?)
        .ok_or(RuntimeError::Overflow)?;
    bytes
        .get_mut(start..end)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The `STRING_TABLE` name of a type entry, or the empty name of an anonymous type.
fn type_name(prepared: &PreparedModule, entry: &TypeEntry) -> SmolStr {
    entry
        .name_idx
        .and_then(|index| prepared.strings().get(index as usize))
        .map(SmolStr::new)
        .unwrap_or_default()
}

/// The `STRING_TABLE` record `name_idx` names.
fn string_of(prepared: &PreparedModule, name_idx: u32) -> Result<SmolStr, RuntimeError> {
    prepared
        .strings()
        .get(name_idx as usize)
        .map(SmolStr::new)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// One type's prepared layout.
fn layout_of(
    prepared: &PreparedModule,
    type_id: u32,
) -> Result<&crate::memory::layout::TypeLayout, RuntimeError> {
    prepared
        .type_layout(type_id)
        .ok_or(RuntimeError::InvalidExecutionState)
}

/// The byte `relative` bytes after `base`, or an overflow refusal.
fn offset_of(base: u32, relative: u32) -> Result<u32, RuntimeError> {
    base.checked_add(relative).ok_or(RuntimeError::Overflow)
}

/// The first byte after the slot of `declaration`, whose `bytes` of value lie at `start`: after
/// its presence byte when it keeps one (MEM-12).
fn slot_end(
    prepared: &PreparedModule,
    declaration: u32,
    start: u32,
    bytes: u32,
) -> Result<u32, RuntimeError> {
    match prepared.presence_offset(declaration, start) {
        Some(presence) => offset_of(presence, 1),
        None => offset_of(start, bytes),
    }
}

/// The own bytes of `pou`'s template: none for a POU without a template, which has no own variables
/// and no parent (O7).
pub(crate) fn own_bytes(prepared: &PreparedModule, pou: u32) -> u32 {
    prepared
        .instance_template(pou)
        .map_or(0, |template| template.range.bytes)
}

/// The POUs of `pou`'s inheritance chain, `pou` first and then each parent, each with the own bytes
/// of its template (O6).
fn chain_of(prepared: &PreparedModule, pou: u32) -> Result<Vec<(u32, u32)>, RuntimeError> {
    let mut chain = Vec::new();
    let mut current = Some(pou);
    for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
        let Some(member) = current else {
            return Ok(chain);
        };
        let parent = prepared
            .instance_template(member)
            .and_then(|template| template.parent);
        chain.push((member, own_bytes(prepared, member)));
        current = parent;
    }
    Err(RuntimeError::TypeMismatch)
}

/// The bytes of a whole inheritance chain: the sum of its own bytes.
fn chain_total(chain: &[(u32, u32)]) -> Result<u32, RuntimeError> {
    chain.iter().try_fold(0u32, |sum, &(_, own)| {
        sum.checked_add(own).ok_or(RuntimeError::Overflow)
    })
}

/// The range of the inheritance parent `pou` of the root `child` (R0): it starts where the child's
/// own bytes end, spans its own chain, and must lie inside the child's range.
fn inherited_range(
    prepared: &PreparedModule,
    roots: &[Option<ByteRange>],
    child: u32,
    pou: u32,
) -> Result<ByteRange, RuntimeError> {
    let child_range = roots
        .get(child as usize)
        .copied()
        .flatten()
        .ok_or(RuntimeError::TypeMismatch)?;
    let child_pou = prepared
        .roots
        .entries
        .get(child as usize)
        .and_then(|root| root.template_pou_id)
        .ok_or(RuntimeError::TypeMismatch)?;
    let start = offset_of(child_range.start, own_bytes(prepared, child_pou))?;
    let bytes = chain_total(&chain_of(prepared, pou)?)?;
    let end = offset_of(start, bytes)?;
    if end > offset_of(child_range.start, child_range.bytes)? {
        return Err(RuntimeError::TypeMismatch);
    }
    Ok(ByteRange {
        region: LayoutRegion::Persistent,
        start,
        bytes,
    })
}

impl CellStorage {
    /// The installation generation this store's references carry (specification 34 §11.3, MEM-07).
    #[must_use]
    pub fn generation(&self) -> Generation {
        self.identities.generation
    }
}

impl CellStorage {
    /// The regions, mutably: the cell arm's writes publish into them (C1-9c).
    pub(crate) fn regions_mut(&mut self) -> &mut CellRegions {
        &mut self.regions
    }
}
