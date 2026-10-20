//! Initializers evaluated in place on the flat store (specification 34 MEM-12, ARCH-05; C1-13).
//!
//! The initializer of a frame variable whose result is an array, a structure or a text that a store
//! copies as bytes ([`PreparedModule::in_place_result`]) stages its result on the activation stack,
//! above the frame it initializes, instead of in a `Value`: a default is constructed into those
//! bytes, the seed of an explicit initializer is copied there from the variable, and a body's
//! `DEFAULT_TYPED`, `COERCE_INIT_VALUE` or `APPLY_INIT_VALUE` whose value the next instruction stores
//! as the result writes it there. The commit copies the bytes into the variable's slot. Each step
//! charges and checks what the value path's step does, in the same order (TIME-08), so a result, a
//! charge and a refusal are the same on either path, and the evaluation allocates nothing. An
//! initializer whose result finds no room on the stack runs on the value path, decided before
//! anything is charged.
//!
//! The default of a global or of a variable of a persistent instance
//! ([`PreparedModule::persistent_in_place_result`]) is built in the variable's own bytes in the
//! persistent block, so that no value of it is built on the heap, however large it is (C1-22): its
//! staged result is those bytes, and its commit admits the markers the value path's commit admits
//! and writes nothing more.

use super::super::super::construction::values::{construct_in_place, ValueConstructionMode};
use super::*;
use crate::bytecode::opcodes::{APPLY_INIT_VALUE, COERCE_INIT_VALUE, DEFAULT_TYPED};
use crate::bytecode::InitializerEntry;
use crate::memory::layout::ByteRange;
use crate::memory::regions::ResultBlock;
use crate::vm::prepared::byte_types::{is_compound, same_type};
use crate::vm::{OperandStack, Place};

/// Where an initializer evaluated in place stages its result and whom it initializes.
#[derive(Clone, Copy)]
struct InPlace {
    /// The result's type.
    result_type: u32,
    /// The bytes the staged result takes: on the activation stack, or the variable's own bytes for a
    /// result built there (C1-22).
    range: ByteRange,
    /// The bytes of the variable's slot.
    slot: ByteRange,
}

impl EngineState<'_> {
    /// Run declaration action `id` of the frame variable `declaration` in place when its result
    /// can be staged on the activation stack, committing it with `once_marker` as the value path
    /// commits it (C1-13). `Ok(false)`, having charged nothing, when the value path must run it.
    pub(super) fn run_declaration_in_place(
        &mut self,
        id: u32,
        declaration: u32,
        instance: Option<InstanceId>,
        frame: &mut VmFrame,
        depth: u32,
        once_marker: Option<(u32, Option<InstanceId>)>,
    ) -> Result<bool, RuntimeError> {
        let Some(place) = self.in_place(id, declaration, frame) else {
            return Ok(false);
        };
        let mut staged = None;
        let committed = self
            .evaluate_in_place(id, instance, Some(frame), depth, place, &mut staged)
            .and_then(|()| self.commit_in_place(declaration, place, once_marker));
        // The staged result leaves the stack on every path, once its bytes are committed.
        if let Some(result) = staged {
            self.data.store.release_result(result);
        }
        committed.map(|()| true)
    }

    /// Run declaration action `id` of the global or persistent instance variable `declaration` by
    /// building its default in the variable's own bytes, when the prepared module builds it there,
    /// committing it with `once_marker` as the value path commits it (C1-22). `Ok(false)`, having
    /// charged nothing, when the value path must run it.
    pub(super) fn run_persistent_in_place(
        &mut self,
        id: u32,
        declaration: u32,
        instance: Option<InstanceId>,
        lexical: Option<&mut VmFrame>,
        depth: u32,
        once_marker: Option<(u32, Option<InstanceId>)>,
    ) -> Result<bool, RuntimeError> {
        let Some(place) = self.persistent_in_place(id, declaration, instance) else {
            return Ok(false);
        };
        let mut staged = None;
        let committed = self
            .evaluate_in_place(id, instance, lexical, depth, place, &mut staged)
            .and_then(|()| {
                self.commit_persistent_in_place(declaration, instance, place, once_marker)
            });
        // The record of the staged result is given back on every path; its bytes are the variable's.
        if let Some(result) = staged {
            self.data.store.release_result(result);
        }
        committed.map(|()| true)
    }

    /// Where initializer `id` of the global or persistent instance variable `declaration` builds its
    /// result: the variable's own bytes, when the prepared module builds it there, the variable has
    /// no presence byte and a staged-result record is free. None of this charges anything.
    fn persistent_in_place(
        &self,
        id: u32,
        declaration: u32,
        instance: Option<InstanceId>,
    ) -> Option<InPlace> {
        let result_type = self.prepared.persistent_in_place_result(id)?;
        let declared = self.prepared.layout.entries.get(declaration as usize)?;
        let arm = &*self.data.store;
        let location = match declared.owner {
            StorageOwner::Global => MemoryLocation::Global,
            // A per-call instance keeps its variables on the activation stack, written as values.
            StorageOwner::Instance => {
                let instance = instance.filter(|id| arm.is_root_instance(*id))?;
                MemoryLocation::Instance(instance)
            }
            StorageOwner::Frame => return None,
        };
        if !arm.has_result_record() {
            return None;
        }
        let reader = self.data.reader(self.prepared);
        let (range, _, presence) = reader.cells_target(location, declared.slot as usize, &[])?;
        if presence.is_some() || range.region != crate::memory::layout::LayoutRegion::Persistent {
            return None;
        }
        Some(InPlace {
            result_type,
            range,
            slot: range,
        })
    }

    /// Where initializer `id` of `declaration` stages its result in place for `frame`: a result the
    /// prepared module evaluates in place, the variable's slot on the activation stack, and room
    /// for the result above it. None of this charges anything.
    fn in_place(&self, id: u32, declaration: u32, frame: &VmFrame) -> Option<InPlace> {
        let (result_type, _) = self.prepared.in_place_result(id)?;
        let arm = &*self.data.store;
        let declared = self.prepared.layout.entries.get(declaration as usize)?;
        let slot = declared.slot as usize;
        let owner = self.frame_slot_activation(frame, slot)?;
        let reader = self.data.reader(self.prepared);
        let (slot, _, _) = reader.cells_target(MemoryLocation::Local(owner), slot, &[])?;
        let range = arm.result_room(self.prepared.type_layout(result_type)?.shape.bytes)?;
        Some(InPlace {
            result_type,
            range,
            slot,
        })
    }

    /// `evaluate_initializer`'s staged path for initializer `id`, with the result staged in
    /// `place` on the activation stack: the same steps, charges and checks in the same order.
    fn evaluate_in_place(
        &mut self,
        id: u32,
        instance: Option<InstanceId>,
        mut lexical: Option<&mut VmFrame>,
        depth: u32,
        place: InPlace,
        staged: &mut Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        if depth as usize >= self.prepared.limits.max_call_depth {
            return Err(super::super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        let entry: InitializerEntry = self
            .prepared
            .initializers
            .entries
            .get(id as usize)
            .cloned()
            .ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static("unknown initializer"))
            })?;
        let context = entry.context_initializer_idx.unwrap_or(id);
        let Some(code_end) = (entry.code_offset as usize).checked_add(entry.code_length as usize)
        else {
            return Err(RuntimeError::Overflow);
        };
        let staging::InitializerSeed {
            value: _,
            backups,
            writable_instances,
        } = self.initializer_seed(&entry, instance, lexical.as_deref(), true)?;
        let result = match self.reserve_activation_identity() {
            Ok(result) => result,
            Err(error) => return Err(self.abandon_seed(backups, writable_instances, error)),
        };
        // The staged slot holds no value: the result lies on the activation stack.
        let mut slot = self.data.resources.bookkeeping.take_slot();
        let suspended = self.data.storage.suspend_execution_frame(result, &mut slot);
        self.data.resources.bookkeeping.return_slot(slot);
        if let Err(error) = suspended {
            return Err(self.abandon_seed(backups, writable_instances, error));
        }
        if let Err(error) = self.data.storage.activate_execution_frame(result) {
            return Err(self.abandon_seed(backups, writable_instances, error));
        }
        if let Err(error) = self.stage_result(&entry, result, place) {
            self.data.storage.release_execution_frame(result);
            return Err(self.abandon_seed(backups, writable_instances, error));
        }
        *staged = Some(result);
        let lexical_frame = lexical.as_ref().and_then(|frame| frame.activation);
        self.data.construction.initializers.push(ActiveInitializer {
            id,
            context,
            entry: entry.clone(),
            result,
            lexical_frame,
            instance,
            depth,
            writable_instances,
        });
        let template = entry
            .owner_pou_id
            .and_then(|owner| self.prepared.vm.pou(owner));
        let mut frame = VmFrame {
            parameter_values_present: Vec::new(),
            activation: lexical_frame,
            pou_id: entry.owner_pou_id,
            return_pc: usize::MAX,
            code_start: entry.code_offset as usize,
            code_end,
            local_ref_start: template.map_or(0, |pou| pou.local_ref_start),
            local_ref_count: template.map_or(0, |pou| pou.local_ref_count),
            locals: lexical
                .as_mut()
                .map_or_else(Vec::new, |frame| core::mem::take(&mut frame.locals)),
            runtime_instance: instance,
            instance_owner: template.and_then(|pou| pou.primary_instance_owner),
        };
        let outcome = if entry.code_length == 0 {
            self.with_construction_frame(&mut frame, |context| {
                let prepared = context.prepared;
                construct_in_place(
                    &prepared.vm.types,
                    context,
                    place.range,
                    place.result_type,
                    ValueConstructionMode::Default,
                    true,
                )
            })
        } else {
            execute_initializer_body(self, &self.prepared.vm, &mut frame, depth)
        };
        if let Some(lexical) = lexical {
            lexical.locals = frame.locals;
        }
        self.finish_in_place(outcome, result, backups, place)
    }

    /// Put the staged result of `result` on the activation stack at `place`: zero bytes, then the
    /// seed of an explicit initializer of an array or a structure, copied from the variable, where
    /// the value path stages the seed's value.
    fn stage_result(
        &mut self,
        entry: &InitializerEntry,
        result: FrameId,
        place: InPlace,
    ) -> Result<(), RuntimeError> {
        let seeded = entry.stage == crate::bytecode::InitializationStage::Explicit
            && is_compound(self.prepared, place.result_type);
        let arm = &mut *self.data.store;
        let cleared = match place.range.region {
            crate::memory::layout::LayoutRegion::Activation => arm.clear_activation(place.range),
            // The variable's own bytes, which the value path's commit would write whole (C1-22).
            crate::memory::layout::LayoutRegion::Persistent => arm.clear_range(place.range),
        };
        if !cleared {
            return Err(RuntimeError::InvalidExecutionState);
        }
        // The result's reference leaves take entries in the activation stack's table (C1-19c4).
        let leaves = self
            .prepared
            .type_layout(place.result_type)
            .map_or(&[][..], |layout| layout.reference_leaves.as_slice());
        arm.push_result(
            ResultBlock {
                id: result,
                range: place.range,
                type_id: place.result_type,
            },
            leaves,
        )?;
        if seeded && place.range != place.slot && !arm.copy_place(place.slot, place.range, None) {
            arm.release_result(result);
            return Err(RuntimeError::InvalidExecutionState);
        }
        Ok(())
    }

    /// `finish_initializer` for a result staged in place: the same returns of the reserved
    /// bookkeeping and the same checks of the result, made of its bytes. The result holds no
    /// instance (a plain type), so the promotion walk would find and charge nothing.
    fn finish_in_place(
        &mut self,
        outcome: Result<(), RuntimeError>,
        result: FrameId,
        mut backups: Vec<(InstanceId, crate::memory::InstanceData)>,
        place: InPlace,
    ) -> Result<(), RuntimeError> {
        // The staged slot comes back empty, its buffer kept, as the value path's comes back with
        // the result.
        let mut values = Vec::new();
        let resumed = outcome.and_then(|()| {
            self.data
                .storage
                .resume_execution_frame(result, &mut values)
        });
        if resumed.is_err() {
            // A trap leaves the staged slot suspended; reclaim it for the next evaluation.
            let _ = self
                .data
                .storage
                .resume_execution_frame(result, &mut values);
        }
        self.data.resources.bookkeeping.return_slot(values);
        let mut snapshot = self.data.resources.bookkeeping.take_snapshot();
        let checked = resumed.and_then(|()| {
            self.snapshot_owned_instances(result, &mut snapshot)?;
            self.check_place_lifetime(place.result_type)
        });
        if checked.is_err() {
            self.restore_initializer_instances(&mut backups);
            for instance in &snapshot {
                if self
                    .data
                    .lifetimes
                    .instance_lifetimes
                    .contains_key(instance)
                {
                    self.data
                        .lifetimes
                        .instance_lifetimes
                        .restore(*instance, Some(result));
                }
            }
        }
        if let Some(active) = self.data.construction.initializers.pop() {
            self.data
                .resources
                .bookkeeping
                .return_visited(active.writable_instances);
        }
        self.data.storage.release_execution_frame(result);
        self.remove_owned_instances_unchecked(result);
        self.data.resources.bookkeeping.return_snapshot(snapshot);
        self.data.resources.bookkeeping.return_backups(backups);
        checked
    }

    /// `commit_declaration_with_once` for a frame variable whose result was staged in place: the
    /// lifetime check of the result, the once marker's admission, then a copy of the result's bytes
    /// into the variable's slot.
    fn commit_in_place(
        &mut self,
        declaration: u32,
        place: InPlace,
        once_marker: Option<(u32, Option<InstanceId>)>,
    ) -> Result<(), RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = crate::vm::diagnostic_timing::begin(
            crate::vm::diagnostic_timing::Phase::InitializerCommit,
        );
        let Some(declared) = self.prepared.layout.entries.get(declaration as usize) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if self
            .prepared
            .vm
            .strings
            .get(declared.name_idx as usize)
            .is_none()
        {
            return Err(RuntimeError::InvalidExecutionState);
        }
        self.check_place_lifetime(place.result_type)?;
        let prepared_once_marker = if let Some(marker) = once_marker {
            let plan = self.data.construction.once.insertion_plan(marker)?;
            self.charge_allocation_bytes(plan.demand().0)?;
            Some(self.data.construction.once.prepare_insert(plan)?)
        } else {
            None
        };
        let arm = &mut *self.data.store;
        if !arm.copy_place(place.range, place.slot, None) {
            return Err(RuntimeError::InvalidExecutionState);
        }
        if let Some(prepared_once_marker) = prepared_once_marker {
            prepared_once_marker.publish();
        }
        Ok(())
    }

    /// `commit_declaration_with_once` for a global or a persistent instance variable whose default
    /// was built in its own bytes (C1-22): the lifetime check of the result, the admission of the
    /// once marker and of the initialized marker, charged as the value path charges them, and their
    /// publication. The value path's write of the value is already done: its bytes are the result.
    fn commit_persistent_in_place(
        &mut self,
        declaration: u32,
        instance: Option<InstanceId>,
        place: InPlace,
        once_marker: Option<(u32, Option<InstanceId>)>,
    ) -> Result<(), RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = crate::vm::diagnostic_timing::begin(
            crate::vm::diagnostic_timing::Phase::InitializerCommit,
        );
        let Some(declared) = self.prepared.layout.entries.get(declaration as usize) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if self
            .prepared
            .vm
            .strings
            .get(declared.name_idx as usize)
            .is_none()
        {
            return Err(RuntimeError::InvalidExecutionState);
        }
        self.check_place_lifetime(place.result_type)?;
        let once_plan = once_marker
            .map(|marker| self.data.construction.once.insertion_plan(marker))
            .transpose()?;
        if let Some(plan) = once_plan {
            self.charge_allocation_bytes(plan.demand().0)?;
        }
        let initialized_plan = self
            .data
            .construction
            .initialized_declarations
            .insertion_plan((declaration, instance))?;
        self.charge_allocation_bytes(initialized_plan.demand().0)?;
        let prepared_once_marker = if let Some(plan) = once_plan {
            Some(self.data.construction.once.prepare_insert(plan)?)
        } else {
            None
        };
        let prepared_marker = self
            .data
            .construction
            .initialized_declarations
            .prepare_insert(initialized_plan)?;
        prepared_marker.publish();
        if let Some(prepared_once_marker) = prepared_once_marker {
            prepared_once_marker.publish();
        }
        Ok(())
    }

    /// The dispatcher's `DEFAULT_TYPED`, `COERCE_INIT_VALUE` or `APPLY_INIT_VALUE` (`opcode`) of
    /// type `operand`, built in the staged result of the active initializer `initializer` when that
    /// result lies on the activation stack and has the type, its place pushed for the store that
    /// follows (C1-13). `Ok(false)`, with nothing popped or charged, otherwise.
    pub(in crate::vm::engine) fn construct_staged_result(
        &mut self,
        frame: &mut VmFrame,
        opcode: u8,
        operand: u32,
        stack: &mut OperandStack,
        initializer: u32,
    ) -> Result<bool, RuntimeError> {
        let Some(result) = self
            .data
            .construction
            .initializers
            .last()
            .filter(|active| active.id == initializer)
            .map(|active| active.result)
        else {
            return Ok(false);
        };
        let arm = &*self.data.store;
        let Some(block) = arm.result(result) else {
            return Ok(false);
        };
        if !same_type(self.prepared, operand, block.type_id, 0) {
            return Ok(false);
        }
        // A structure that lies as its backing is built in place by its default only; a coerced or
        // applied value is built as a value and stored, as the value path stores it (C1-15b).
        if opcode != DEFAULT_TYPED && self.prepared.vm.physical_layout_ref(operand).is_some() {
            return Ok(false);
        }
        let mode = match opcode {
            COERCE_INIT_VALUE => ValueConstructionMode::Coerce(
                stack
                    .pop()
                    .map_err(super::super::super::VmTrap::into_runtime_error)?,
            ),
            APPLY_INIT_VALUE => ValueConstructionMode::Apply(
                stack
                    .pop()
                    .map_err(super::super::super::VmTrap::into_runtime_error)?,
            ),
            _ => ValueConstructionMode::Default,
        };
        self.with_construction_frame(frame, |context| {
            let prepared = context.prepared;
            construct_in_place(
                &prepared.vm.types,
                context,
                block.range,
                operand,
                mode,
                false,
            )
        })?;
        stack
            .push_place(Place {
                range: block.range,
                type_id: operand,
            })
            .map_err(super::super::super::VmTrap::into_runtime_error)?;
        Ok(true)
    }
}
