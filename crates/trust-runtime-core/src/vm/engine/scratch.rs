//! Execution scratch that construction reserves and every scan reuses (spec 34 MEM-03,
//! MEM-09): the stack executor's execution buffer, the local slots of its activations, the
//! initializer stacks, the output copy-back journal and the process-image staging.
use super::io::staging::StagingDemand;
use super::output_transaction::SavedDestinations;
use super::*;
use crate::value::{ArrayValue, RefIndices, RefPath, RefSegment, StructValue};
use crate::vm::construction::values::ValueOperation;
use crate::vm::module::VmNativeSymbolSpec;
use crate::vm::VmTrap;
use alloc::sync::Arc;

/// Initializer, construction-frame and lifetime-check bookkeeping that construction reserves and
/// every scan reuses (spec 34 MEM-03, MEM-09, census N1-N4 and I10): one staged result slot, one
/// pending-identity list, one seed visited set, one seed backup list and one owned-instance
/// snapshot per callback level, plus the lifetime check's visited set. Every buffer is taken from
/// its list and returned, so no evaluation or check allocates it again; growth is still charged at
/// the call site against the capacity a buffer growing on demand would have (spec 34 TIME-08).
#[derive(Default)]
pub(super) struct BookkeepingScratch {
    slots: Vec<Vec<Value>>,
    pending: Vec<Vec<u32>>,
    visited: Vec<InstanceSet>,
    backups: Vec<Vec<(InstanceId, crate::memory::InstanceData)>>,
    snapshots: Vec<Vec<InstanceId>>,
    /// The lifetime check's visited set, reserved for the live instance count and cleared per
    /// check (spec 34 MEM-09, census I10).
    pub(super) lifetime_visited: core::cell::RefCell<super::policy::LifetimeVisited>,
    /// The number of call levels reserved. A buffer returned above it is dropped.
    levels: usize,
}

impl BookkeepingScratch {
    /// The bytes this scratch reserves for `levels` callback levels whose calls create at most
    /// `instances` instances each.
    fn reservation_bytes(levels: usize, instances: usize) -> Option<usize> {
        let identities = instances.checked_mul(
            core::mem::size_of::<u32>()
                + core::mem::size_of::<(u32, ())>()
                + core::mem::size_of::<InstanceId>(),
        )?;
        let per_level = core::mem::size_of::<Vec<Value>>()
            .checked_add(core::mem::size_of::<Value>())?
            .checked_add(core::mem::size_of::<Vec<u32>>())?
            .checked_add(core::mem::size_of::<InstanceSet>())?
            .checked_add(core::mem::size_of::<
                Vec<(InstanceId, crate::memory::InstanceData)>,
            >())?
            .checked_add(core::mem::size_of::<Vec<InstanceId>>())?
            .checked_add(identities)?;
        levels
            .checked_mul(per_level)?
            .checked_add(core::mem::size_of::<Self>())
    }

    /// Reserve the per-level lists and the fixed one-element result slots (spec 34 MEM-09), the
    /// pending identities, the visited set and the owned-instance snapshot of each level with room
    /// for the `instances` a call creates at most, so that the first call that creates instances
    /// allocates none of them either (C1-19c).
    fn reserve(&mut self, levels: usize, instances: usize) -> Result<(), RuntimeError> {
        let refused = |_| VmTrap::BudgetExceeded.into_runtime_error();
        self.levels = levels;
        self.slots.try_reserve_exact(levels).map_err(refused)?;
        self.pending.try_reserve_exact(levels).map_err(refused)?;
        self.visited.try_reserve_exact(levels).map_err(refused)?;
        self.backups.try_reserve_exact(levels).map_err(refused)?;
        self.snapshots.try_reserve_exact(levels).map_err(refused)?;
        for _ in 0..levels {
            let mut slot = Vec::new();
            slot.try_reserve_exact(1).map_err(refused)?;
            self.slots.push(slot);
            let mut pending = Vec::new();
            pending.try_reserve_exact(instances).map_err(refused)?;
            self.pending.push(pending);
            let mut visited = InstanceSet::default();
            visited.reserve_capacity(instances)?;
            self.visited.push(visited);
            self.backups.push(Vec::new());
            let mut snapshot = Vec::new();
            snapshot.try_reserve_exact(instances).map_err(refused)?;
            self.snapshots.push(snapshot);
        }
        Ok(())
    }

    /// Reserve the lifetime check's visited set for `instances` live identities. Called when the
    /// live instance count grows, never during a scan of an unchanged program (spec 34 MEM-09).
    pub(super) fn reserve_lifetime_visited(&self, instances: usize) -> Result<(), RuntimeError> {
        self.lifetime_visited.borrow_mut().reserve(instances)
    }

    pub(super) fn take_slot(&mut self) -> Vec<Value> {
        self.slots.pop().unwrap_or_default()
    }
    pub(super) fn return_slot(&mut self, mut slot: Vec<Value>) {
        slot.clear();
        // A slot whose buffer a suspended evaluation took has no room to keep; the buffer comes
        // back when the evaluation finishes (C1-13).
        if slot.capacity() > 0 && self.slots.len() < self.levels {
            self.slots.push(slot);
        }
    }
    pub(super) fn take_pending(&mut self) -> Vec<u32> {
        self.pending.pop().unwrap_or_default()
    }
    pub(super) fn return_pending(&mut self, mut pending: Vec<u32>) {
        pending.clear();
        if self.pending.len() < self.levels {
            self.pending.push(pending);
        }
    }
    pub(super) fn take_visited(&mut self) -> InstanceSet {
        self.visited.pop().unwrap_or_default()
    }
    pub(super) fn return_visited(&mut self, mut visited: InstanceSet) {
        // A set comes back empty, so that no later initializer may write the instances of the
        // one that used it before, nor skip their backups.
        visited.clear();
        if self.visited.len() < self.levels {
            self.visited.push(visited);
        }
    }
    pub(super) fn take_backups(&mut self) -> Vec<(InstanceId, crate::memory::InstanceData)> {
        self.backups.pop().unwrap_or_default()
    }
    pub(super) fn return_backups(
        &mut self,
        mut backups: Vec<(InstanceId, crate::memory::InstanceData)>,
    ) {
        backups.clear();
        if self.backups.len() < self.levels {
            self.backups.push(backups);
        }
    }
    pub(super) fn take_snapshot(&mut self) -> Vec<InstanceId> {
        self.snapshots.pop().unwrap_or_default()
    }
    pub(super) fn return_snapshot(&mut self, mut snapshot: Vec<InstanceId>) {
        snapshot.clear();
        if self.snapshots.len() < self.levels {
            self.snapshots.push(snapshot);
        }
    }

    /// Whether every reserved buffer has been returned to its list (spec 34 MEM-09; test support).
    #[cfg(test)]
    pub(super) fn all_buffers_returned(&self) -> bool {
        self.slots.len() == self.levels
            && self.pending.len() == self.levels
            && self.visited.len() == self.levels
            && self.backups.len() == self.levels
            && self.snapshots.len() == self.levels
    }
}

/// Local-slot vectors for stack activations. Each has room for the largest POU's locals, and
/// construction reserves one for every call level, so handing one out never allocates, the first
/// call at a depth included (spec 34 MEM-09). Each activation's parameter-presence bits come from
/// the same store, so a call with presence bits allocates none either (census A4).
#[derive(Debug, Default)]
pub(super) struct FrameLocals {
    spares: Vec<Vec<Value>>,
    presence: Vec<Vec<bool>>,
    slots: usize,
}

impl FrameLocals {
    /// Locals for an activation with `count` slots, all NULL.
    pub(super) fn take(&mut self, count: usize) -> Vec<Value> {
        let mut locals = self.spares.pop().unwrap_or_else(|| {
            // Without the register tier every finished call returns its locals here, so a
            // new vector gets the payload size and is kept. The register tier keeps or drops
            // call results itself, so its vectors are as large as the call needs.
            #[cfg(not(feature = "register-ir"))]
            let capacity = self.slots.max(count);
            #[cfg(feature = "register-ir")]
            let capacity = count;
            Vec::with_capacity(capacity)
        });
        locals.resize(count, Value::Null);
        locals
    }

    /// Keep a finished activation's locals for the next activation, emptied, when they have
    /// the payload size and the reserved room holds them; otherwise they are dropped.
    pub(super) fn recycle(&mut self, mut locals: Vec<Value>) {
        if locals.capacity() >= self.slots && self.spares.len() < self.spares.capacity() {
            locals.clear();
            self.spares.push(locals);
        }
    }

    /// Parameter-presence bits for a new activation, none set; the activation writes its own.
    pub(super) fn take_presence(&mut self) -> Vec<bool> {
        self.presence
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(self.slots))
    }

    /// Keep a finished activation's presence bits for the next activation, emptied, as
    /// [`FrameLocals::recycle`] keeps its locals.
    pub(super) fn recycle_presence(&mut self, mut present: Vec<bool>) {
        if present.capacity() >= self.slots && self.presence.len() < self.presence.capacity() {
            present.clear();
            self.presence.push(present);
        }
    }

    /// A vector of `slots` locals and one of `slots` presence bits for each of `levels` call
    /// levels, the activations a chain of calls holds at once.
    fn reserve(&mut self, levels: usize, slots: usize) -> Result<(), RuntimeError> {
        let refused = |_| VmTrap::BudgetExceeded.into_runtime_error();
        self.slots = slots;
        self.spares.try_reserve_exact(levels).map_err(refused)?;
        self.presence.try_reserve_exact(levels).map_err(refused)?;
        for _ in 0..levels {
            let mut locals = Vec::new();
            locals.try_reserve_exact(slots).map_err(refused)?;
            self.spares.push(locals);
            let mut present = Vec::new();
            present.try_reserve_exact(slots).map_err(refused)?;
            self.presence.push(present);
        }
        Ok(())
    }

    /// The bytes [`FrameLocals::reserve`] takes for `levels` levels of `slots` slots.
    fn reservation_bytes(levels: usize, slots: usize) -> Option<usize> {
        let locals = core::mem::size_of::<Vec<Value>>()
            .checked_add(slots.checked_mul(core::mem::size_of::<Value>())?)?;
        let presence = core::mem::size_of::<Vec<bool>>()
            .checked_add(slots.checked_mul(core::mem::size_of::<bool>())?)?;
        levels.checked_mul(locals.checked_add(presence)?)
    }
}

impl EngineState<'_> {
    /// Give back the path buffers of the references a finished activation's `locals` hold, before
    /// the locals are kept for the next activation (spec 34 ARCH-05): a call-time reference
    /// initializer then takes the same buffers on every call (C1-13).
    pub(super) fn recycle_local_references(&self, locals: &mut [Value]) {
        for local in locals {
            if let Value::Reference(Some(_)) = local
                && let Value::Reference(Some(reference)) = core::mem::replace(local, Value::Null)
            {
                self.data.resources.references.recycle(reference.path);
            }
        }
    }
}

/// The path and index buffers of the references a scan builds (spec 34 ARCH-05). A reference
/// whose path grows takes a kept buffer, and one the execution has used gives its buffers back,
/// so after the first scan a program's references allocate nothing. The charges stay those of an
/// owned path (TIME-08): a buffer's capacity changes no charge. The references borrow the engine
/// immutably while they grow, so the kept buffers are behind a `RefCell`.
#[derive(Debug, Default)]
pub(super) struct ReferenceBuffers {
    paths: core::cell::RefCell<Vec<RefPath>>,
    indices: core::cell::RefCell<Vec<RefIndices>>,
    /// The room of a reserved path and index buffer, the longest path and the most subscripts of
    /// the module's types (C1-19c3).
    path_room: usize,
    index_room: usize,
}

/// The most buffers of each kind kept; the references a program holds beyond them allocate.
const KEPT_REFERENCE_BUFFERS: usize = 64;

/// The room of a new path buffer and of a new index buffer: the segments and the dimensions of
/// the deepest paths and arrays programs commonly build. A buffer that has to grow is kept grown.
const REFERENCE_BUFFER_ROOM: usize = 4;

impl ReferenceBuffers {
    /// Keep `paths` more path buffers with room for `path_room` segments and `indices` more index
    /// buffers with room for `index_room` subscripts, as construction reserves them for the
    /// references a scan can hold at once (spec 34 MEM-09; C1-19c3).
    pub(super) fn reserve(
        &mut self,
        (paths, path_room): (usize, usize),
        (indices, index_room): (usize, usize),
    ) -> Result<(), RuntimeError> {
        let refused = |_| VmTrap::BudgetExceeded.into_runtime_error();
        self.path_room = self.path_room.max(path_room);
        self.index_room = self.index_room.max(index_room);
        let kept = self.paths.get_mut();
        kept.try_reserve_exact(paths).map_err(refused)?;
        for _ in 0..paths {
            let mut path = RefPath::new();
            path.try_reserve_exact(self.path_room).map_err(refused)?;
            kept.push(path);
        }
        let kept = self.indices.get_mut();
        kept.try_reserve_exact(indices).map_err(refused)?;
        for _ in 0..indices {
            let mut index = RefIndices::new();
            index.try_reserve_exact(self.index_room).map_err(refused)?;
            kept.push(index);
        }
        Ok(())
    }

    /// The bytes [`ReferenceBuffers::reserve`] takes.
    pub(super) fn reservation_bytes(
        (paths, path_room): (usize, usize),
        (indices, index_room): (usize, usize),
    ) -> Option<usize> {
        let path = path_room
            .checked_mul(core::mem::size_of::<RefSegment>())?
            .checked_add(core::mem::size_of::<RefPath>())?;
        let index = index_room
            .checked_mul(core::mem::size_of::<i64>())?
            .checked_add(core::mem::size_of::<RefIndices>())?;
        paths
            .checked_mul(path)?
            .checked_add(indices.checked_mul(index)?)
    }

    /// The path buffers and the index buffers kept (test support).
    #[cfg(test)]
    pub(super) fn kept(&self) -> (usize, usize) {
        (self.paths.borrow().len(), self.indices.borrow().len())
    }

    /// An empty path buffer: a kept one, else a new one.
    pub(super) fn take_path(&self) -> RefPath {
        self.paths
            .borrow_mut()
            .pop()
            .unwrap_or_else(|| RefPath::with_capacity(self.path_room.max(REFERENCE_BUFFER_ROOM)))
    }

    /// An empty index buffer: a kept one, else a new one.
    pub(super) fn take_indices(&self) -> RefIndices {
        self.indices.borrow_mut().pop().unwrap_or_else(|| {
            RefIndices::with_capacity(self.index_room.max(REFERENCE_BUFFER_ROOM))
        })
    }

    /// Keep the buffers of a path the execution has used: the path's and each index segment's.
    pub(super) fn recycle(&self, mut path: RefPath) {
        for segment in path.drain(..) {
            if let RefSegment::Index(mut indices) = segment {
                indices.clear();
                keep(&self.indices, indices);
            }
        }
        keep(&self.paths, path);
    }
}

/// Keep an emptied buffer with room in it, unless as many as were reserved, and at least
/// [`KEPT_REFERENCE_BUFFERS`], are kept already.
fn keep<T>(spares: &core::cell::RefCell<Vec<Vec<T>>>, buffer: Vec<T>) {
    let mut spares = spares.borrow_mut();
    if buffer.capacity() > 0 && spares.len() < spares.capacity().max(KEPT_REFERENCE_BUFFERS) {
        spares.push(buffer);
    }
}

/// The containers of the raw arrays and structures that initializer bodies build (`ARRAY_NEW`,
/// `STRUCT_NEW`), kept once a construction has consumed them (spec 34 ARCH-05; C1-13). A body then
/// builds its raws again in kept containers, so after the first scan an aggregate initializer's raw
/// allocates nothing. The charges stay those of a new container (TIME-08): a kept container's room
/// changes no charge.
#[derive(Debug, Default)]
pub(super) struct RawShells {
    #[allow(
        clippy::vec_box,
        reason = "a raw array is a `Value::Array`, whose box is the allocation kept"
    )]
    arrays: Vec<Box<ArrayValue>>,
    structures: Vec<Arc<StructValue>>,
}

/// The most containers of each kind kept; the raws a body holds beyond them allocate.
const KEPT_RAW_SHELLS: usize = 16;

impl RawShells {
    /// A raw array of `count` null elements whose one dimension runs from 0 to `count - 1`: in a
    /// kept container, else in a new one.
    pub(super) fn array(&mut self, count: u32) -> Result<Box<ArrayValue>, RuntimeError> {
        let mut array = self
            .arrays
            .pop()
            .unwrap_or_else(|| Box::new(ArrayValue::from_canonical_parts(Vec::new(), Vec::new())));
        let length = count as usize;
        array.elements.clear();
        array
            .elements
            .try_reserve_exact(length)
            .map_err(|_| RuntimeError::Overflow)?;
        array.elements.resize(length, Value::Null);
        array.dimensions.clear();
        array.dimensions.push((0, i64::from(count) - 1));
        Ok(array)
    }

    /// A raw structure with no name and no fields: a kept container, else a new one.
    pub(super) fn structure(&mut self) -> Arc<StructValue> {
        self.structures.pop().unwrap_or_else(|| {
            Arc::new(StructValue::from_canonical_parts(
                SmolStr::default(),
                crate::collections::OrderedMap::default(),
            ))
        })
    }

    /// Keep the container of an array a construction consumed, its elements gone.
    pub(super) fn keep_array(&mut self, mut array: Box<ArrayValue>) {
        if self.arrays.len() < KEPT_RAW_SHELLS {
            array.elements.clear();
            self.arrays.push(array);
        }
    }

    /// Keep the container of a structure a construction consumed, emptied, when nothing else
    /// holds it.
    pub(super) fn keep_structure(&mut self, mut structure: Arc<StructValue>) {
        if self.structures.len() < KEPT_RAW_SHELLS
            && let Some(value) = Arc::get_mut(&mut structure)
        {
            value.type_name = SmolStr::default();
            value.fields.clear();
            value.physical = None;
            self.structures.push(structure);
        }
    }
}

/// The scratch sizes of one prepared module under its admitted limits.
struct ScratchPlan {
    /// Call levels of the stack executor: the module's static call depth capped at the
    /// admitted ceiling, and at least one for the entry activation (spec 34 MEM-02, MEM-09).
    levels: usize,
    /// Operand values across the levels: the deepest body, plus for every further level the
    /// most operands a body keeps below the call that started it.
    operands: usize,
    /// The largest local-slot count of any POU.
    slots: usize,
    /// Nested initializer evaluations and construction frames.
    initializers: usize,
    /// The execution buffers of nested initializer bodies, one for each nested evaluation when
    /// the module has an initializer body, and the operands each body needs (spec 34 MEM-09).
    initializer_buffers: usize,
    initializer_operands: usize,
    /// The most instances on the activation stack that one call creates (C1-11c): each owner of
    /// instances and each bookkeeping list a call takes has room for them (C1-19c).
    call_instances: usize,
    /// The bytes of the temporary area: of the scan's operand stack, the deepest body's
    /// temporaries and, for every further call level, those a body keeps below a call; of each
    /// initializer body's (C1-19c2).
    temporaries: usize,
    initializer_temporaries: usize,
    /// The text-call buffers: the longest result, the most bytes of the texts one call reads, and
    /// the most arguments of one call.
    text_result: usize,
    text_arguments: usize,
    text_count: usize,
    /// The path buffers the references a scan holds at once take, those its operands hold on
    /// every call level and those its frames hold, with one more for a path that moves into a
    /// larger buffer, and their index buffers (C1-19c3).
    reference_paths: usize,
    path_shape: crate::vm::prepared::operand_bounds::PathShape,
    /// Nested typed value operations.
    types: usize,
    /// Destinations of one output copy-back group: the most parameters of any POU, or the most
    /// arguments of any call site, whose outputs a native call's group writes (C1-19c).
    journal: usize,
    /// The most edge inputs of any POU: one edge-input list and one restore list.
    edges: usize,
    /// Reusable per-depth call-binding scratch (spec 34 MEM-09; census C1, C3, C4, C5, C7).
    call_scratch: crate::vm::call::scratch::CallScratchSizes,
    staging: StagingDemand,
}

impl ScratchPlan {
    fn of(prepared: &PreparedModule) -> Result<Self, RuntimeError> {
        Self::with_levels(prepared, prepared.reserved_call_levels())
    }

    fn with_levels(prepared: &PreparedModule, levels: usize) -> Result<Self, RuntimeError> {
        let residue = prepared.max_call_operand_residue.unwrap_or(0);
        let operands = levels
            .saturating_sub(1)
            .checked_mul(residue)
            .and_then(|below| below.checked_add(prepared.max_operand_depth))
            .ok_or(RuntimeError::Overflow)?;
        let slots = prepared
            .vm
            .pou_by_id
            .values()
            .map(|pou| pou.local_ref_count as usize)
            .max()
            .unwrap_or(0);
        let journal = prepared
            .vm
            .pou_params
            .values()
            .map(Vec::len)
            .max()
            .unwrap_or(0);
        // The most actual arguments of any call site sizes the argument and flag vectors
        // (spec 34 MEM-09; census C1, C3).
        let call_args = prepared
            .vm
            .native_symbol_specs
            .iter()
            .map(|spec| match spec {
                VmNativeSymbolSpec::Parsed { arg_specs, .. } => arg_specs.len(),
                VmNativeSymbolSpec::ParseError(_) => 0,
            })
            .max()
            .unwrap_or(0);
        // `PouMap::keys` exists only with the register feature, so iterate the identity and
        // payload pairs and ignore the payloads; clippy's `keys()` suggestion does not
        // compile on the chips.
        #[allow(
            clippy::iter_kv_map,
            reason = "PouMap::keys exists only with register-ir"
        )]
        let edges = prepared
            .vm
            .pou_by_id
            .iter()
            .map(|(pou, _)| prepared.edge_inputs(*pou).len())
            .max()
            .unwrap_or(0);
        // An initializer body runs in execution buffers of its own (C1-13); construction reserves
        // them, so the first call that initializes a frame variable allocates none.
        let bodies = prepared
            .initializers
            .entries
            .iter()
            .any(|entry| entry.code_length != 0);
        Ok(Self {
            levels,
            operands,
            slots,
            initializers: levels,
            initializer_buffers: if bodies { levels } else { 0 },
            initializer_operands: prepared.max_initializer_operand_depth,
            call_instances: prepared.largest_call_instances()?,
            temporaries: levels
                .saturating_sub(1)
                .checked_mul(prepared.operand_bounds.temporaries_below_call)
                .and_then(|below| below.checked_add(prepared.operand_bounds.temporaries))
                .ok_or(RuntimeError::Overflow)?,
            initializer_temporaries: prepared.operand_bounds.initializer_temporaries,
            text_result: prepared.operand_bounds.text_result,
            text_arguments: prepared.operand_bounds.text_arguments,
            text_count: prepared.operand_bounds.text_count,
            reference_paths: levels
                .saturating_sub(1)
                .checked_mul(prepared.operand_bounds.references_below_call)
                .and_then(|below| below.checked_add(prepared.operand_bounds.references))
                .and_then(|operands| {
                    levels
                        .checked_mul(crate::vm::prepared::operand_bounds::frame_references(
                            prepared,
                        ))?
                        .checked_add(operands)?
                        .checked_add(1)
                })
                .ok_or(RuntimeError::Overflow)?,
            path_shape: crate::vm::prepared::operand_bounds::PathShape::of(prepared),
            types: crate::bytecode::BYTECODE_MAX_TYPE_NESTING + 1,
            journal: journal.max(call_args),
            call_scratch: crate::vm::call::scratch::CallScratchSizes {
                args: call_args,
                params: journal,
                slots,
            },
            edges,
            staging: StagingDemand::of(&prepared.io_bindings).ok_or(RuntimeError::Overflow)?,
        })
    }

    /// The owners of instances a chain of calls holds at once: a frame on each level and an
    /// initializer evaluated on each.
    fn owner_lists(&self) -> usize {
        if self.call_instances == 0 {
            0
        } else {
            self.levels.saturating_add(self.initializers)
        }
    }

    /// Every byte the plan reserves.
    fn bytes(&self) -> Option<usize> {
        let buffer = ExecutionBuffers::reservation_bytes(self.operands, self.levels)?
            .checked_add(core::mem::size_of::<ExecutionBuffers>())?;
        // Each initializer body's buffers: its operands, its temporaries and the one frame it runs in.
        let initializer_buffers = self.initializer_buffers.checked_mul(
            ExecutionBuffers::reservation_bytes(self.initializer_operands, 1)?
                .checked_add(core::mem::size_of::<ExecutionBuffers>())?
                .checked_add(self.initializer_temporaries)?,
        )?;
        let references = ReferenceBuffers::reservation_bytes(
            (self.reference_paths, self.path_shape.segments),
            (
                self.reference_paths.checked_mul(self.path_shape.indexes)?,
                self.path_shape.subscripts,
            ),
        )?;
        let temporaries =
            self.temporaries
                .checked_add(crate::vm::TextBuffers::reservation_bytes(
                    self.text_result,
                    self.text_arguments,
                    self.text_count,
                )?)?;
        let frame_locals = FrameLocals::reservation_bytes(self.levels, self.slots)?;
        // One frame payload per call depth, `levels` local slots and presence bits wide.
        let frame_payloads =
            crate::vm::frames::payloads::FramePayloads::reservation_bytes(self.levels, self.slots)?;
        // One edge-input list and one restore list, the most edge inputs of any POU.
        let edge_input = core::mem::size_of::<crate::vm::call::context::VmEdgeInput>();
        let edge_restore =
            core::mem::size_of::<smol_str::SmolStr>().checked_add(core::mem::size_of::<Value>())?;
        let edges = self
            .edges
            .checked_mul(edge_input.checked_add(edge_restore)?)?;
        let initializers = self.initializers.checked_mul(
            core::mem::size_of::<ActiveInitializer>() + core::mem::size_of::<VmFrame>(),
        )?;
        let types = self
            .types
            .checked_mul(core::mem::size_of::<(u32, ValueOperation)>())?;
        let journal = SavedDestinations::allocation_bytes(self.journal).ok()?;
        let bookkeeping =
            BookkeepingScratch::reservation_bytes(self.initializers, self.call_instances)?
                .checked_add(OwnedInstances::list_reservation_bytes(
                    self.owner_lists(),
                    self.call_instances,
                )?)?;
        let call_scratch = crate::vm::call::scratch::CallScratch::reservation_bytes(
            self.levels,
            self.call_scratch,
        )?;
        buffer
            .checked_add(initializer_buffers)?
            .checked_add(temporaries)?
            .checked_add(references)?
            .checked_add(frame_locals)?
            .checked_add(frame_payloads)?
            .checked_add(edges)?
            .checked_add(initializers)?
            .checked_add(types)?
            .checked_add(journal)?
            .checked_add(bookkeeping)?
            .checked_add(call_scratch)?
            .checked_add(self.staging.reservation_bytes()?)
    }
}

impl EngineState<'_> {
    /// Reserve a path buffer, and its index buffers, for each reference leaf of the persistent block,
    /// whose reference may keep a path (C1-19c3), charged as construction memory.
    pub(super) fn reserve_stored_references(&mut self) -> Result<(), RuntimeError> {
        let EngineStore::Cells(arm) = &self.data.store else {
            return Ok(());
        };
        let leaves = arm.persistent_reference_leaves();
        if leaves == 0 {
            return Ok(());
        }
        let shape = crate::vm::prepared::operand_bounds::PathShape::of(self.prepared);
        let paths = (leaves, shape.segments);
        let indices = (
            leaves
                .checked_mul(shape.indexes)
                .ok_or(RuntimeError::Overflow)?,
            shape.subscripts,
        );
        self.charge_allocation_bytes(
            ReferenceBuffers::reservation_bytes(paths, indices).ok_or(RuntimeError::Overflow)?,
        )?;
        self.data.resources.references.reserve(paths, indices)
    }

    /// Reserve the lifetime check's visited set for the persistent instances construction made and
    /// the most per-call instances the activation stack holds, charged as construction memory, so
    /// that a call that creates instances does not grow it (spec 34 MEM-09; C1-19c).
    pub(super) fn reserve_call_lifetimes(&mut self) -> Result<(), RuntimeError> {
        let calls = self.prepared.call_instance_reservation()?;
        self.charge_allocation_bytes(
            calls
                .checked_mul(core::mem::size_of::<u32>())
                .ok_or(RuntimeError::Overflow)?,
        )?;
        let live = self
            .live_instance_count()
            .checked_add(calls)
            .ok_or(RuntimeError::Overflow)?;
        self.data
            .resources
            .bookkeeping
            .reserve_lifetime_visited(live)
    }

    /// Reserve the scratch that every scan reuses, charged in full against the construction
    /// allowance before any of it is reserved (spec 34 MEM-09). Each later activation and
    /// staging is still charged as before (TIME-08); only the allocation goes.
    pub(super) fn reserve_execution_scratch(&mut self) -> Result<(), RuntimeError> {
        let plan = ScratchPlan::of(self.prepared)?;
        self.charge_allocation_bytes(plan.bytes().ok_or(RuntimeError::Overflow)?)?;
        let refused = |_| VmTrap::BudgetExceeded.into_runtime_error();
        let mut buffers = ExecutionBuffers::default();
        buffers
            .reserve(plan.operands, plan.levels)
            .map_err(VmTrap::into_runtime_error)?;
        // One frame payload per call depth, so a stack activation takes one instead of
        // allocating (spec 34 MEM-03; census A3, A4).
        buffers
            .frames
            .reserve_frame_payloads(plan.levels, plan.slots)?;
        let resources = &mut self.data.resources;
        resources
            .buffers
            .try_reserve_exact(1 + plan.initializer_buffers)
            .map_err(refused)?;
        // An entry takes the last buffers of the pool: the scan's own go on top, below them those
        // of the initializer bodies nested in it.
        for _ in 0..plan.initializer_buffers {
            let mut nested = ExecutionBuffers::default();
            nested
                .reserve(plan.initializer_operands, 1)
                .map_err(VmTrap::into_runtime_error)?;
            nested
                .operand_stack
                .reserve_temporaries(plan.initializer_temporaries)
                .map_err(VmTrap::into_runtime_error)?;
            resources.buffers.push(nested);
        }
        buffers
            .operand_stack
            .reserve_temporaries(plan.temporaries)
            .map_err(VmTrap::into_runtime_error)?;
        resources.buffers.push(buffers);
        resources
            .text_call
            .reserve(plan.text_result, plan.text_arguments, plan.text_count)?;
        let shape = plan.path_shape;
        resources.references.reserve(
            (plan.reference_paths, shape.segments),
            (
                plan.reference_paths
                    .checked_mul(shape.indexes)
                    .ok_or(RuntimeError::Overflow)?,
                shape.subscripts,
            ),
        )?;
        resources.frame_locals.reserve(plan.levels, plan.slots)?;
        // One edge-input list and one restore list, reused by every edge-affected call
        // (spec 34 MEM-09; census A5).
        resources
            .edge_inputs
            .try_reserve_exact(plan.edges)
            .map_err(refused)?;
        resources
            .edge_restores
            .try_reserve_exact(plan.edges)
            .map_err(refused)?;
        resources.spare_journal.reserve_storage(plan.journal)?;
        // One reusable call-binding vector of each kind per call depth, so a call takes one
        // instead of allocating (spec 34 MEM-09; census C1, C3, C4, C5, C7).
        resources
            .call_scratch
            .reserve(plan.levels, plan.call_scratch)?;
        resources
            .staging
            .reserve(plan.staging, &self.prepared.io_bindings)?;
        let construction = &mut self.data.construction;
        construction
            .initializers
            .try_reserve_exact(plan.initializers)
            .map_err(refused)?;
        construction
            .frames
            .try_reserve_exact(plan.initializers)
            .map_err(refused)?;
        construction
            .types
            .try_reserve_exact(plan.types)
            .map_err(refused)?;
        // The instances a call creates are listed under the frame that owns them and, while an
        // initializer constructs them, under the initializer's result (C1-11c, C1-13).
        self.data
            .lifetimes
            .owned_instances
            .reserve_lists(plan.owner_lists(), plan.call_instances)?;
        self.data
            .resources
            .bookkeeping
            .reserve(plan.initializers, plan.call_instances)
    }
}

#[cfg(test)]
#[path = "scratch/tests.rs"]
mod tests;
