//! Artifact-backed services for the shared dispatcher and native call path.
use super::super::call::bindings::{
    VmFbOutBinding, VmNativeArg, VmNativeArgValue, VmOutBinding, VmWriteTarget,
};
use super::super::call::context::{
    BuiltinParam, BuiltinParamDirection, CallContext, RegisterCallOpKind, RegisterValueOpKind,
    VmEdgeInput,
};
use super::super::context::{ExecutionContext, ReferenceContext};
use super::super::{OperandStack, VmTrap};
use super::*;
use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::memory::MemoryLocation;
use crate::stdlib::StandardLibrary;
use crate::value::{write_value_path, RefSegment};

#[cfg(test)]
#[path = "context/owner_tests.rs"]
mod owner_tests;

impl<'e> ReferenceContext for EngineState<'e> {
    // The dispatcher reaches the variables through the state itself, which routes each read and write
    // to the flat store and the `Value` half it keeps (C1-9d2b-1; C1-19e).
    type Store = EngineState<'e>;

    fn dynamic_reference_candidates(
        &self,
        module: &super::super::module::VmModule,
        reference: crate::value::ValueRefView<'_>,
    ) -> Result<Option<&[u32]>, RuntimeError> {
        if !core::ptr::eq(module, &self.prepared.vm) {
            return Ok(None);
        }
        Ok(Some(self.prepared.dynamic_reference_candidates(
            reference.location,
            reference.offset,
        )))
    }

    fn charge_text_work(&self, units: usize) -> Result<(), RuntimeError> {
        self.charge_work_units(units)
    }

    fn charge_text_result(&self, result: &Value) -> Result<(), RuntimeError> {
        // TIME-08 TEXT: a new text value costs one unit and its copy allocation.
        self.charge_work_units(1)?;
        let bytes = super::allocation::copy_allocation(result, 0)?;
        self.charge_allocation_bytes(bytes)
    }

    fn charge_new_text(&self, text: &str, wide: bool) -> Result<(), RuntimeError> {
        // As a new text value: one unit and its copy allocation (TIME-08 TEXT; C1-12).
        self.charge_work_units(1)?;
        self.charge_allocation_bytes(super::allocation::text_copy_allocation(text, wide)?)
    }

    // A call-transaction snapshot is transient bookkeeping: it lives only until the
    // call commits or rolls back, bounded by call depth and the admitted parameter
    // type, so it charges neither work nor cumulative allocation (spec 34 TIME-08).
    fn before_transaction_snapshot(&self, _value: &Value) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn before_reference_path_clone(
        &self,
        reference: crate::value::ValueRefView<'_>,
    ) -> Result<(), RuntimeError> {
        // A flat reference is charged by the path it was taken through, as a copy of the reference
        // is (C1-11d), so the copy of its path costs what it costs on the value store (C1-18).
        let flat = reference.location.flat_elements();
        self.charge_work_units(flat)?;
        let bytes = self
            .reference_path_clone_charge(reference.path)?
            .checked_add(
                flat.checked_mul(crate::memory::layout::CELL_BYTES)
                    .ok_or(RuntimeError::Overflow)?,
            )
            .ok_or(RuntimeError::Overflow)?;
        self.charge_allocation_bytes(bytes)
    }

    fn before_reference_path_growth(&self, bytes: usize, work: usize) -> Result<(), RuntimeError> {
        self.charge_work_units(work)?;
        self.charge_allocation_bytes(bytes)
    }

    fn before_path_write(
        &self,
        root: &Value,
        path: &[RefSegment],
        input: &Value,
    ) -> Result<(), RuntimeError> {
        self.charge_path_write(root, path, input)
    }
    fn before_view_path_write(
        &self,
        root: &ValueView<'_>,
        path: &[RefSegment],
        input: &Value,
    ) -> Result<(), RuntimeError> {
        self.charge_path_write_view(root, path, input)
    }
    fn before_view_clone(&self, view: &ValueView<'_>) -> Result<(), RuntimeError> {
        match view {
            ValueView::Value(value) => self.before_value_clone(value),
            ValueView::Instance(instance) => self.before_value_clone(&Value::Instance(*instance)),
            ValueView::Cells { .. } | ValueView::Bits { .. } => {
                let bytes = self.view_clone_charge(view)?;
                self.charge_allocation_bytes(bytes)
            }
        }
    }
    fn flatten_reference(&self, reference: ValueRef) -> ValueRef {
        EngineState::flatten_reference(self, reference)
    }
    fn frame_slot_in_block(&self, frame: &VmFrame, slot: usize) -> bool {
        self.frame_slot_activation(frame, slot).is_some()
    }
    fn read_frame_slot(&self, frame: &VmFrame, slot: usize) -> Option<Result<Value, RuntimeError>> {
        EngineState::read_frame_slot(self, frame, slot)
    }
    fn frame_block(&self, frame: &VmFrame) -> Option<crate::memory::regions::FrameBlock> {
        EngineState::frame_block(self, frame)
    }
    fn read_finished_slot(
        &self,
        block: crate::memory::regions::FrameBlock,
        slot: usize,
    ) -> Option<Result<Value, RuntimeError>> {
        EngineState::read_finished_slot(self, block, slot)
    }
    fn place_of(&self, view: &ValueView<'_>) -> Option<crate::vm::Place> {
        EngineState::place_of(self, view)
    }
    fn place_view(&self, place: crate::vm::Place) -> Option<ValueView<'_>> {
        EngineState::place_view(self, place)
    }
    fn forget_place_references(&mut self, range: crate::memory::layout::ByteRange, type_id: u32) {
        EngineState::forget_place_references(self, range, type_id);
    }
    fn place_destination(
        &self,
        reference: crate::value::ValueRefView<'_>,
        place: crate::vm::Place,
        declared: Option<u32>,
    ) -> Option<crate::vm::PlaceDestination> {
        EngineState::place_destination(self, reference, place, declared)
    }
    fn check_place_write(
        &self,
        reference: crate::value::ValueRefView<'_>,
        place: crate::vm::Place,
    ) -> Result<(), RuntimeError> {
        EngineState::check_place_write(self, reference, place)
    }
    fn copy_place(
        &mut self,
        place: crate::vm::Place,
        destination: crate::vm::PlaceDestination,
    ) -> Result<bool, RuntimeError> {
        EngineState::copy_place(self, place, destination)
    }
    fn charge_place_normalization(&self, place: crate::vm::Place) -> Result<(), RuntimeError> {
        EngineState::charge_place_normalization(self, place)
    }
    fn place_copy(&self, place: crate::vm::Place) -> Option<&[u8]> {
        EngineState::place_copy(self, place)
    }
    fn charge_copy_normalization(
        &self,
        bytes: &[u8],
        place: crate::vm::Place,
    ) -> Result<(), RuntimeError> {
        EngineState::charge_copy_normalization(self, bytes, place)
    }
    fn copy_value(&self, bytes: &[u8], type_id: u32) -> Result<Value, RuntimeError> {
        crate::memory::cells::read_value(bytes, type_id, self.prepared)
    }
    fn write_copy(
        &mut self,
        bytes: &[u8],
        destination: crate::vm::PlaceDestination,
    ) -> Result<bool, RuntimeError> {
        EngineState::write_copy(self, bytes, destination)
    }
    fn store_text(
        &mut self,
        reference: crate::value::ValueRefView<'_>,
        text: &str,
        wide: bool,
        declared: Option<u32>,
        check_first: bool,
    ) -> Option<Result<bool, RuntimeError>> {
        EngineState::store_text(self, reference, text, wide, declared, check_first)
    }
    fn before_text_clone(&self, text: &str, wide: bool) -> Result<(), RuntimeError> {
        // A WSTRING's copy costs one unit and its stored form, a STRING's its bytes (TIME-08).
        if wide {
            self.charge_work_units(1)?;
        }
        self.charge_allocation_bytes(super::allocation::text_copy_allocation(text, wide)?)
    }
    fn enum_operand(
        &self,
        view: &crate::memory::view::ValueView<'_>,
    ) -> Option<crate::vm::EnumTemp> {
        EngineState::enum_operand(self, view)
    }
    fn enum_constant(&self, value: &Value) -> Option<crate::vm::EnumTemp> {
        EngineState::enum_constant(self, value)
    }
    fn enum_value(&self, temp: crate::vm::EnumTemp) -> Result<Value, RuntimeError> {
        EngineState::enum_value(self, temp)
    }
    fn enums_equal(&self, left: crate::vm::EnumTemp, right: crate::vm::EnumTemp) -> bool {
        EngineState::enums_equal(self, left, right)
    }
    fn before_enum_clone(&self, _temp: crate::vm::EnumTemp) -> Result<(), RuntimeError> {
        self.charge_enum_copy()
    }
    fn store_enum(
        &mut self,
        reference: crate::value::ValueRefView<'_>,
        temp: crate::vm::EnumTemp,
        declared: Option<u32>,
    ) -> Option<Result<bool, RuntimeError>> {
        EngineState::store_enum(self, reference, temp, declared)
    }
    fn normalize_assignment(
        &self,
        _module: &super::super::module::VmModule,
        ty: u32,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        self.normalize_assignment_value(ty, value)
    }
    fn before_output_write(
        &mut self,
        frame: &VmFrame,
        reference: crate::value::ValueRefView<'_>,
    ) -> Result<(), RuntimeError> {
        self.journal_output_destination(frame, reference)
    }
    fn runs_compact(&self) -> bool {
        self.accesses_in_place()
    }
    fn instance_bytes(
        &self,
        instance: crate::memory::InstanceId,
    ) -> Option<crate::memory::layout::ByteRange> {
        self.data.reader(self.prepared).instance_range(instance)
    }
    fn scalar_at(&self, range: crate::memory::layout::ByteRange, prim: u16) -> Option<Value> {
        self.data.reader(self.prepared).scalar_at(range, prim)
    }
    fn instance_in_leaf(
        &self,
        range: crate::memory::layout::ByteRange,
    ) -> Option<crate::memory::InstanceId> {
        self.data.reader(self.prepared).instance_in_leaf(range)
    }
    fn store_scalar_at(
        &mut self,
        range: crate::memory::layout::ByteRange,
        prim: u16,
        value: &Value,
    ) -> bool {
        self.write_scalar_leaf(range, prim, value)
    }
    fn cell_at(
        &self,
        range: crate::memory::layout::ByteRange,
        prim: u16,
    ) -> Option<crate::memory::cells::Cell> {
        self.data.reader(self.prepared).cell_at(range, prim)
    }
    fn store_cell_at(
        &mut self,
        range: crate::memory::layout::ByteRange,
        prim: u16,
        cell: crate::memory::cells::Cell,
    ) -> bool {
        self.write_scalar_cell(range, prim, cell)
    }
    #[inline]
    fn leaf_bytes(&self, range: crate::memory::layout::ByteRange) -> Option<&[u8]> {
        self.data.store.bytes(&range)
    }
    #[inline]
    fn write_leaf_bytes(&mut self, range: crate::memory::layout::ByteRange, bytes: &[u8]) -> bool {
        self.data.store.write_bytes(&range, bytes)
    }
    fn before_scalar_clone(&self) -> Result<(), RuntimeError> {
        // A scalar's copy charges no work and allocates nothing (TIME-08 COPY), so it is admitted
        // while the allocation allowance holds, as `before_value_clone` admits a scalar value.
        self.charge_allocation_bytes(0)
    }
    fn own_scalar(
        &self,
        instance: crate::memory::InstanceId,
        field_idx: u32,
        field: &str,
    ) -> Option<Value> {
        if !self.accesses_in_place() {
            return None;
        }
        self.data
            .reader(self.prepared)
            .own_scalar(instance, field_idx, field)
    }
    fn own_scalar_slot(
        &self,
        instance: crate::memory::InstanceId,
        field_idx: u32,
        field: &str,
        value: &Value,
    ) -> Option<usize> {
        if !self.accesses_in_place() {
            return None;
        }
        self.data
            .reader(self.prepared)
            .own_scalar_slot(instance, field_idx, field, value)
    }
    #[cfg_attr(all(feature = "std", feature = "test-support"), track_caller)]
    fn before_value_clone(&self, value: &Value) -> Result<(), RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = crate::vm::diagnostic_timing::begin(
            crate::vm::diagnostic_timing::Phase::CopyAccounting,
        );

        #[cfg(not(all(feature = "std", feature = "test-support")))]
        {
            let bytes = self.value_clone_charge(value, 0)?;
            self.charge_allocation_bytes(bytes)
        }

        #[cfg(all(feature = "std", feature = "test-support"))]
        {
            let caller = if crate::vm::diagnostic_timing::callsite_accounting_log_enabled() {
                Some(core::panic::Location::caller())
            } else {
                None
            };
            let mut estimated_bytes = 0;
            let result = self.value_clone_charge(value, 0).and_then(|bytes| {
                estimated_bytes = bytes;
                self.charge_allocation_bytes(bytes)
            });
            if let Some(caller) = caller {
                let outcome = match &result {
                    Ok(()) => crate::vm::diagnostic_timing::CopyAdmissionOutcome::Admitted,
                    Err(error) => crate::vm::diagnostic_timing::copy_outcome(error),
                };
                let admitted_bytes = if result.is_ok() { estimated_bytes } else { 0 };
                crate::vm::diagnostic_timing::record_copy_admission(
                    caller,
                    value,
                    estimated_bytes,
                    admitted_bytes,
                    outcome,
                );
            }
            result
        }
    }

    fn resolve_instance_owner(&self, encoded: u32, frame: &VmFrame) -> Result<InstanceId, VmTrap> {
        // Owner, root and ancestry resolution is bookkeeping under TIME-08 and charges nothing.
        #[cfg(all(feature = "std", feature = "test-support"))]
        let root = crate::vm::diagnostic_timing::observe_index_lookup_result(
            crate::vm::diagnostic_timing::IndexLookupOperation::InstanceRoot,
            || Ok::<_, RuntimeError>(self.prepared.root_for_instance_owner(encoded)),
        )
        .map_err(VmTrap::Runtime)?;
        #[cfg(not(all(feature = "std", feature = "test-support")))]
        let root = self.prepared.root_for_instance_owner(encoded);
        if frame.instance_owner == Some(encoded)
            && let Some(mut instance) = frame.runtime_instance
        {
            let template = if let Some(template) = root.and_then(|(_, root)| root.template_pou_id) {
                Some(template)
            } else if let Some(pou) = frame.pou_id {
                self.prepared.method_owners.get(&pou).copied()
            } else {
                None
            };
            for _ in 0..=self.prepared.vm.pou_by_id.len() {
                let Some(template) = template else {
                    return Ok(instance);
                };
                if self
                    .data
                    .construction
                    .instance_templates
                    .get(&instance)
                    .copied()
                    == Some(template)
                {
                    return Ok(instance);
                }
                instance = self
                    .instance_parent(instance)?
                    .ok_or_else(|| VmTrap::NullReference)?;
            }
        }
        root.and_then(|(index, _)| self.data.construction.roots[index])
            .ok_or_else(|| VmTrap::NullReference)
    }

    fn storage(&self) -> &Self::Store {
        self
    }
    fn storage_mut(&mut self) -> &mut Self::Store {
        self
    }
    fn take_reference_path(&self) -> crate::value::RefPath {
        self.data.resources.references.take_path()
    }
    fn take_reference_indices(&self) -> crate::value::RefIndices {
        self.data.resources.references.take_indices()
    }
    fn recycle_reference_path(&self, path: crate::value::RefPath) {
        self.data.resources.references.recycle(path);
    }
    fn check_reference_write(
        &self,
        reference: crate::value::ValueRefView<'_>,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        self.check_write(reference, value)
    }
    fn check_reference_read(
        &self,
        reference: crate::value::ValueRefView<'_>,
    ) -> Result<(), RuntimeError> {
        self.check_read(reference)
    }
    fn initializer_reference<'a>(
        &self,
        initializer_id: u32,
        path: &'a [RefSegment],
    ) -> Result<crate::value::ValueRefView<'a>, VmTrap> {
        let active = self
            .data
            .construction
            .initializers
            .last()
            .filter(|active| active.id == initializer_id)
            .ok_or_else(|| VmTrap::NullReference)?;
        Ok(crate::value::ValueRefView {
            location: MemoryLocation::Local(active.result),
            offset: 0,
            generation: StoreAccess::generation(self),
            path,
        })
    }
    fn before_write_commit(&self) -> Result<(), RuntimeError> {
        self.check_entry_deadline()
    }
    fn write_typed_storage(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
        capacity: usize,
    ) -> Result<bool, RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing =
            crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::StorageWrite);
        self.charge_storage_path_write(&*self, location, offset, path, &value)?;
        if path.is_empty() && !matches!(value, Value::Struct(_)) {
            return Ok(self.write_slot(location, offset, value));
        }
        let Some(prepared) = self.prepare_write(location, offset, path, &value)? else {
            return Ok(false);
        };
        if prepared.requires_final_deadline_check() {
            self.check_entry_deadline()?;
        }
        Ok(self.commit_write(path, value, prepared, capacity))
    }
    fn write_typed_local(&self, root: &mut Value, path: &[RefSegment], value: Value) -> bool {
        write_value_path(root, path, value)
    }
    fn write_typed_local_checked(
        &self,
        root: &mut Value,
        path: &[RefSegment],
        value: Value,
        capacity: usize,
    ) -> Result<bool, RuntimeError> {
        if path.is_empty() && !matches!(value, Value::Struct(_)) {
            *root = crate::value::normalize_assignment_for_target(root, value);
            return Ok(true);
        }
        let Some(prepared) = crate::value::prepare_value_path_write(root, path, &value)? else {
            return Ok(false);
        };
        if prepared.requires_final_deadline_check() {
            self.check_entry_deadline()?;
        }
        Ok(crate::value::commit_value_path_write(
            root, path, value, prepared, capacity,
        ))
    }
    fn reference_string_capacity(&self, reference: crate::value::ValueRefView<'_>) -> usize {
        self.declared_string_capacity(reference)
    }
}

impl CallContext for EngineState<'_> {
    // Without the register tier every call result is a stack activation's locals, which go
    // back to the payloads reserved at construction.
    #[cfg(not(feature = "register-ir"))]
    fn recycle_optimized_result_locals(
        &mut self,
        _pou_id: u32,
        mut locals: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        self.recycle_local_references(&mut locals);
        self.data.resources.frame_locals.recycle(locals);
        Ok(())
    }

    #[cfg(feature = "register-ir")]
    fn recycle_optimized_result_locals(
        &mut self,
        pou_id: u32,
        locals: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        self.data
            .resources
            .register
            .recycle_result_locals(pou_id, locals)
    }

    #[cfg(feature = "register-ir")]
    fn try_execute_optimized_with_parameter_presence(
        &mut self,
        module: &super::super::module::VmModule,
        pou_id: u32,
        instance: Option<InstanceId>,
        initial_locals: Option<&[Value]>,
        present: Option<&[bool]>,
        capture_return: bool,
        depth: u32,
        entry: crate::vm::budget::ExecutionEntry,
    ) -> Result<Option<crate::vm::dispatch::VmPouStackResult>, RuntimeError> {
        crate::vm::register_ir::try_execute_pou_with_register_ir_with_parameter_presence(
            self,
            module,
            pou_id,
            instance,
            initial_locals,
            present,
            capture_return,
            depth,
            entry,
        )
        .map(|result| {
            result.map(|result| crate::vm::dispatch::VmPouStackResult {
                return_value: result.return_value,
                locals: result.locals,
                block: result.block,
                return_in_block: false,
            })
        })
    }

    #[cfg(feature = "register-ir")]
    fn try_execute_optimized_with_parameter_presence_owned(
        &mut self,
        module: &super::super::module::VmModule,
        pou_id: u32,
        instance: Option<InstanceId>,
        initial_locals: &mut Option<Vec<Value>>,
        present: Option<&[bool]>,
        capture_return: bool,
        depth: u32,
        entry: crate::vm::budget::ExecutionEntry,
    ) -> Result<Option<crate::vm::dispatch::VmPouStackResult>, RuntimeError> {
        crate::vm::register_ir::try_execute_pou_with_register_ir_with_parameter_presence_owned(
            self,
            module,
            pou_id,
            instance,
            initial_locals,
            present,
            capture_return,
            depth,
            entry,
        )
        .map(|result| {
            result.map(|result| crate::vm::dispatch::VmPouStackResult {
                return_value: result.return_value,
                locals: result.locals,
                block: None,
                return_in_block: false,
            })
        })
    }

    fn with_output_transaction<T>(
        &mut self,
        frame: &mut VmFrame,
        action: impl FnOnce(&mut Self, &mut VmFrame) -> Result<T, VmTrap>,
    ) -> Result<T, VmTrap> {
        self.commit_output_transaction(frame, action)
    }
    fn profile(&self) -> &DateTimeProfile {
        &self.data.profile
    }
    fn current_time(&self) -> Duration {
        self.data.now
    }
    fn current_dt(&mut self) -> Result<crate::value::DateTimeValue, RuntimeError> {
        self.services.current_dt()
    }
    fn stdlib(&self) -> &StandardLibrary {
        &self.prepared.stdlib
    }
    fn builtin_params(&self, key: &str) -> Option<Vec<BuiltinParam>> {
        let vm = &self.prepared.vm;
        let pou = vm.function_block_ids.get(&vm.strings, key)?;
        self.prepared
            .vm
            .pou_params(pou)?
            .iter()
            .map(|param| {
                Some(BuiltinParam {
                    name: param.name.clone(),
                    direction: match param.direction {
                        0 => BuiltinParamDirection::In,
                        1 => BuiltinParamDirection::Out,
                        2 => BuiltinParamDirection::InOut,
                        _ => return None,
                    },
                })
            })
            .collect()
    }
    fn edge_inputs(&self, owner: &str) -> Vec<VmEdgeInput> {
        let vm = &self.prepared.vm;
        let Some(pou) = vm
            .function_block_ids
            .get(&vm.strings, owner)
            .or_else(|| vm.program_ids.get(&vm.strings, owner))
        else {
            return Vec::new();
        };
        self.prepared.edge_inputs(pou).to_vec()
    }
    fn edge_inputs_for_pou(
        &self,
        module: &super::super::module::VmModule,
        pou_id: u32,
        owner: &str,
    ) -> Vec<VmEdgeInput> {
        if core::ptr::eq(module, &self.prepared.vm) {
            self.prepared.edge_inputs(pou_id).to_vec()
        } else {
            self.edge_inputs(owner)
        }
    }

    fn take_edge_inputs_for_pou(
        &mut self,
        module: &super::super::module::VmModule,
        pou_id: u32,
        _owner: &str,
    ) -> Vec<VmEdgeInput> {
        // A scan reads the prepared module's own index and never falls back to an
        // uppercase owner name; a module without the index is refused at preparation
        // (spec 34 MEM-09; census A5, A6).
        let mut inputs = core::mem::take(&mut self.data.resources.edge_inputs);
        inputs.clear();
        if core::ptr::eq(module, &self.prepared.vm) {
            inputs.extend_from_slice(self.prepared.edge_inputs(pou_id));
        }
        inputs
    }

    fn recycle_edge_inputs(&mut self, inputs: Vec<VmEdgeInput>) {
        self.data.resources.edge_inputs = inputs;
    }

    fn take_edge_restores(&mut self) -> Vec<(usize, Value)> {
        core::mem::take(&mut self.data.resources.edge_restores)
    }

    fn recycle_edge_restores(&mut self, restores: Vec<(usize, Value)>) {
        self.data.resources.edge_restores = restores;
    }

    /// A user function or method call takes the locals of its call frame from the frame
    /// payloads reserved at construction (spec 34 MEM-09; census C5).
    fn take_optimized_call_locals(&mut self, count: usize) -> Vec<Value> {
        self.data.resources.frame_locals.take(count)
    }

    /// The evaluated argument list of a call comes from the call scratch reserved at
    /// construction, and every other binding vector does too (spec 34 MEM-09; census C1, C3,
    /// C4, C5, C7).
    fn take_call_args(&mut self) -> Vec<VmNativeArg> {
        self.data.resources.call_scratch.take_args()
    }

    fn recycle_call_args(&mut self, mut args: Vec<VmNativeArg>) {
        // A reference an argument holds gives its path back to the engine's buffers, as a reference
        // the engine discards does (C1-10): a reference into a backing keeps its path (C1-15).
        for arg in args.drain(..) {
            if let VmNativeArgValue::Target(reference)
            | VmNativeArgValue::Expr(Value::Reference(Some(reference))) = arg.value
            {
                self.data.resources.references.recycle(reference.path);
            }
        }
        self.data.resources.call_scratch.recycle_args(args);
    }

    fn take_call_flags(&mut self, count: usize) -> Vec<bool> {
        self.data.resources.call_scratch.take_flags(count)
    }

    fn recycle_call_flags(&mut self, flags: Vec<bool>) {
        self.data.resources.call_scratch.recycle_flags(flags);
    }

    fn take_call_inputs(&mut self) -> Vec<(usize, crate::vm::Place)> {
        self.data.resources.call_scratch.take_inputs()
    }

    fn recycle_call_inputs(&mut self, inputs: Vec<(usize, crate::vm::Place)>) {
        self.data.resources.call_scratch.recycle_inputs(inputs);
    }

    fn input_place(
        &self,
        pou: u32,
        slot: usize,
        parameter: u32,
        reference: &ValueRef,
    ) -> Option<Result<crate::vm::Place, RuntimeError>> {
        EngineState::input_place(self, pou, slot, parameter, reference)
    }

    fn take_call_function_outputs(&mut self) -> Vec<VmOutBinding> {
        self.data.resources.call_scratch.take_function_outputs()
    }

    fn recycle_call_function_outputs(&mut self, mut outputs: Vec<VmOutBinding>) {
        // A target left by a refused call gives its path back, as an argument's does (C1-19c4).
        for binding in outputs.drain(..) {
            binding.target.recycle(self);
        }
        self.data
            .resources
            .call_scratch
            .recycle_function_outputs(outputs);
    }

    fn take_call_fb_outputs(&mut self) -> Vec<VmFbOutBinding> {
        self.data.resources.call_scratch.take_fb_outputs()
    }

    fn recycle_call_fb_outputs(&mut self, mut outputs: Vec<VmFbOutBinding>) {
        for binding in outputs.drain(..) {
            binding.source.recycle(self);
            binding.target.recycle(self);
        }
        self.data.resources.call_scratch.recycle_fb_outputs(outputs);
    }

    fn take_call_prepared_targets(&mut self) -> Vec<Option<(VmWriteTarget, Option<u32>)>> {
        self.data.resources.call_scratch.take_prepared_targets()
    }

    fn recycle_call_prepared_targets(
        &mut self,
        mut targets: Vec<Option<(VmWriteTarget, Option<u32>)>>,
    ) {
        for (target, _) in targets.drain(..).flatten() {
            target.recycle(self);
        }
        self.data
            .resources
            .call_scratch
            .recycle_prepared_targets(targets);
    }

    fn take_call_prepared_outputs(&mut self) -> Vec<(VmWriteTarget, Value)> {
        self.data.resources.call_scratch.take_prepared_outputs()
    }

    fn recycle_call_prepared_outputs(&mut self, mut outputs: Vec<(VmWriteTarget, Value)>) {
        for (target, _) in outputs.drain(..) {
            target.recycle(self);
        }
        self.data
            .resources
            .call_scratch
            .recycle_prepared_outputs(outputs);
    }

    fn take_call_stdlib_values(&mut self) -> Vec<Value> {
        self.data.resources.call_scratch.take_stdlib_values()
    }

    fn recycle_call_stdlib_values(&mut self, values: Vec<Value>) {
        self.data
            .resources
            .call_scratch
            .recycle_stdlib_values(values);
    }
    fn call_plan(
        &self,
        code_offset: u32,
    ) -> Option<alloc::sync::Arc<crate::vm::prepared::CallPlan>> {
        self.prepared.call_plan(code_offset)
    }

    fn instance_pou(&self, instance: InstanceId) -> Option<u32> {
        self.data.reader(self.prepared).instance_pou(instance)
    }

    fn builtin_formals(&self, pou: u32) -> Option<&crate::stdlib::fbs::members::FormalPositions> {
        self.prepared.builtin_formals(pou)
    }

    fn suspend_frame(&mut self, frame: &mut VmFrame) -> Result<(), RuntimeError> {
        let Some(activation) = frame.activation else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        self.data
            .storage
            .suspend_execution_frame(activation, &mut frame.locals)
    }
    fn resume_frame(&mut self, frame: &mut VmFrame) -> Result<(), RuntimeError> {
        let Some(activation) = frame.activation else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        self.data
            .storage
            .resume_execution_frame(activation, &mut frame.locals)
    }
    fn record_call_op(&mut self, _kind: RegisterCallOpKind) {
        #[cfg(feature = "register-ir")]
        self.data.resources.register.record_call_op(_kind);
    }
    fn record_value_op(&mut self, _kind: RegisterValueOpKind) {
        #[cfg(feature = "register-ir")]
        self.data.resources.register.record_value_op(_kind);
    }
    fn check_native_write(
        &self,
        _frame: Option<&VmFrame>,
        reference: &ValueRef,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        self.check_write(reference.as_view(), value)
    }
    fn check_builtin_call(&self, _instance_id: InstanceId) -> Result<(), RuntimeError> {
        if self.data.construction.initializers.is_empty() {
            Ok(())
        } else {
            Err(RuntimeError::StagingViolation)
        }
    }
    fn execute_builtin_fb(
        &mut self,
        now: Duration,
        instance_id: InstanceId,
        kind: crate::stdlib::fbs::BuiltinFbKind,
    ) -> Result<(), RuntimeError> {
        // The block runs on the store the engine holds: a persistent instance's members are in the cells
        // of the cell arm, not in the `Value` half (C1-9d3a, R1), at the places preparation fixed
        // for its POU, in the instance's bytes found once for the call (checklist P1-4).
        let prepared = self.prepared;
        let places = self
            .data
            .reader(prepared)
            .instance_pou_and_range(instance_id)
            .and_then(|(pou, bytes)| {
                prepared
                    .builtin_places(pou)
                    .map(|members| crate::stdlib::fbs::members::BlockPlaces { bytes, members })
            });
        crate::stdlib::fbs::execute_builtin_in_store(self, now, instance_id, kind, places)
    }
}

impl ExecutionContext for EngineState<'_> {
    fn instance_pou_and_bytes(
        &self,
        instance: InstanceId,
    ) -> Option<(u32, crate::memory::layout::ByteRange)> {
        self.data
            .reader(self.prepared)
            .instance_pou_and_range(instance)
    }
    fn execute_builtin_fb_at(
        &mut self,
        now: Duration,
        instance: InstanceId,
        kind: crate::stdlib::fbs::BuiltinFbKind,
        pou: u32,
        bytes: crate::memory::layout::ByteRange,
    ) -> Result<(), RuntimeError> {
        // The caller found the instance's POU and bytes (`instance_pou_and_bytes`), so the block
        // reaches its members at the places preparation fixed for the POU without finding the
        // instance again (checklist P1-5).
        let prepared = self.prepared;
        let places = prepared
            .builtin_places(pou)
            .map(|members| crate::stdlib::fbs::members::BlockPlaces { bytes, members });
        crate::stdlib::fbs::execute_builtin_in_store(self, now, instance, kind, places)
    }
    fn take_text_buffers(&mut self) -> Option<crate::vm::TextBuffers> {
        // Every text call takes the engine's kept buffers (C1-12).
        Some(core::mem::take(&mut self.data.resources.text_call))
    }
    fn return_text_buffers(&mut self, buffers: crate::vm::TextBuffers) {
        self.data.resources.text_call = buffers;
    }
    fn keeps_texts(&self) -> bool {
        true
    }
    fn call_depth_limit(&self) -> usize {
        self.prepared.limits.max_call_depth
    }
    fn charge_call_storage(&self, bytes: usize, _work: usize) -> Result<(), RuntimeError> {
        // Frame-stack relocation work is bookkeeping under TIME-08; only the bytes are admitted.
        self.charge_allocation_bytes(bytes)
    }
    fn execution_budget(&self) -> &crate::vm::budget::ExecutionBudget {
        &self.data.resources.work_budget
    }
    fn initialize_frame(
        &mut self,
        module: &super::super::module::VmModule,
        frame: &mut VmFrame,
        depth: u32,
    ) -> Result<(), RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = crate::vm::diagnostic_timing::begin(
            crate::vm::diagnostic_timing::Phase::FrameInitialize,
        );
        self.check_frame_depth(depth)?;
        let id = self.reserve_activation_identity()?;
        frame.activation = Some(id);
        self.data.storage.activate_execution_frame(id)?;
        self.data.storage.set_execution_frame_owner(
            id,
            frame.pou_id.ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static(
                    "ordinary frame has no POU owner",
                ))
            })?,
            frame.runtime_instance,
        )?;
        // Admission fixes the return declaration at slot zero. Whether the caller supplied it is
        // read before the frame moves its values onto the activation stack.
        let return_supplied = frame
            .locals
            .first()
            .is_some_and(|value| !matches!(value, Value::Null));
        self.place_frame(frame, id)?;
        self.initialize_frame_declarations(module, frame, depth, return_supplied)
    }
    fn begin_frameless_invocation(&mut self, depth: u32) -> Result<(), RuntimeError> {
        self.check_frame_depth(depth)?;
        self.begin_entry_before_first_frame()
    }
    fn retire_frame(&mut self, frame: &VmFrame) -> Result<(), RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing =
            crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::FrameRetire);
        if let Some(id) = frame.activation {
            let owned_work = self.remove_owned_instances(id);
            self.data.storage.release_execution_frame(id);
            self.release_frame_block(id);
            owned_work?;
        }
        Ok(())
    }
    fn check_frame_return(&self, frame: &VmFrame, value: &Value) -> Result<(), RuntimeError> {
        let destination = frame
            .activation
            .and_then(|id| self.data.storage.previous_execution_activation(id));
        self.check_value_lifetime(value, destination)
    }
    fn copy_input_place(
        &mut self,
        frame: &VmFrame,
        slot: usize,
        place: crate::vm::Place,
    ) -> Result<(), RuntimeError> {
        EngineState::copy_input_place(self, frame, slot, place)
    }
    fn check_return_place(&self, frame: &VmFrame) -> Option<Result<(), RuntimeError>> {
        EngineState::check_return_place(self, frame)
    }
    fn push_return_copy(
        &self,
        block: crate::memory::regions::FrameBlock,
        stack: &mut OperandStack,
    ) -> Result<(), RuntimeError> {
        EngineState::push_return_copy(self, block, stack)
    }
    fn construct_result_in_place(
        &mut self,
        frame: &mut VmFrame,
        opcode: u8,
        operand: u32,
        stack: &mut OperandStack,
        initializer_id: u32,
    ) -> Result<bool, RuntimeError> {
        self.construct_staged_result(frame, opcode, operand, stack, initializer_id)
    }
    fn execute_initialization_opcode(
        &mut self,
        _module: &super::super::module::VmModule,
        frame: &mut VmFrame,
        opcode: u8,
        operand: u32,
        stack: &mut OperandStack,
        depth: u32,
    ) -> Result<(), RuntimeError> {
        self.initialization_opcode(frame, opcode, operand, stack, depth)
    }
    fn deadline_exceeded(&self) -> bool {
        self.data
            .resources
            .work_budget
            .observe_deadline(|| self.services.deadline_exceeded())
    }
    fn on_statement(
        &mut self,
        _module: &super::super::module::VmModule,
        pou: u32,
        pc: usize,
        depth: u32,
    ) {
        // Candidate construction must not publish debugger snapshots or pauses
        // into the installed controller's service state.
        if self.data.construction_complete {
            // While a debugger is attached, its work in the hook is the services' (specification
            // 34, "The allocation count").
            let before = self
                .services
                .debug_active()
                .then(|| self.services.allocator_operations())
                .flatten();
            self.services
                .on_statement(pou, pc, depth, &self.data.storage, self.data.now);
            self.count_service_operations(before);
        }
    }
    fn observes_statements(&self) -> bool {
        // Before construction completes the hook publishes nothing (below).
        self.data.construction_complete && self.services.observes_statements()
    }
    fn on_statement_with_frame_checked(
        &mut self,
        _module: &super::super::module::VmModule,
        pou: u32,
        pc: usize,
        depth: u32,
        frame: crate::vm::StatementFrame<'_>,
    ) -> Result<(), RuntimeError> {
        // Neither initial installation nor an isolated restart candidate may
        // publish frame snapshots into the installed controller's service state.
        if self.data.construction_complete {
            // A hook that acts only at statement starts is not called at the other instructions of
            // a module with a statement index, and nothing is counted for them (checklist P1-10).
            if self.services.hook_acts_at_statement_starts()
                && self.prepared.has_statement_index()
                && let Ok(offset) = u32::try_from(pc)
                && self.prepared.statement_start(pou, offset).is_none()
            {
                return Ok(());
            }
            let frame = frame.with_call_stack(&self.data.storage);
            // On the flat store the frames' typed locals, the globals and the instances are read
            // from the blocks (C1-17).
            let frame = frame.with_cells(self.data.reader(self.prepared));
            // While a debugger is attached, its work in the hook is the services' (specification
            // 34, "The allocation count").
            let before = self
                .services
                .debug_active()
                .then(|| self.services.allocator_operations())
                .flatten();
            let result = self.services.on_statement_with_frame_checked(
                pou,
                pc,
                depth,
                &self.data.storage,
                self.data.now,
                frame,
            );
            self.count_service_operations(before);
            return result;
        }
        Ok(())
    }
    fn sizeof_value(&self, value: &Value) -> Result<u64, RuntimeError> {
        self.sizeof_runtime_value(value)
    }
    #[cfg(feature = "register-ir")]
    fn start_register_activation(
        &mut self,
        module: &super::super::module::VmModule,
        pou_id: u32,
        entry_instance: Option<crate::memory::InstanceId>,
        initial_locals: Option<&[crate::value::Value]>,
        owned_initial_locals: Option<&mut Option<alloc::vec::Vec<crate::value::Value>>>,
        parameter_values_present: Option<&[bool]>,
        capture_return: bool,
        depth: u32,
        entry: crate::vm::budget::ExecutionEntry,
        capture_result_locals: bool,
    ) -> Result<Option<crate::vm::register_ir::RegisterActivation>, RuntimeError> {
        crate::vm::register_ir::start_register_activation(
            self,
            module,
            pou_id,
            entry_instance,
            initial_locals,
            owned_initial_locals,
            parameter_values_present,
            capture_return,
            depth,
            entry,
            capture_result_locals,
        )
    }
    #[cfg(feature = "register-ir")]
    fn has_register_activation_pump(&self) -> bool {
        true
    }
    #[cfg(feature = "register-ir")]
    fn step_register_activation(
        &mut self,
        module: &super::super::module::VmModule,
        activation: &mut crate::vm::register_ir::RegisterActivation,
    ) -> Result<crate::vm::register_ir::RegisterActivationStep, RuntimeError> {
        crate::vm::register_ir::step_register_activation(self, module, activation)
    }
    #[cfg(feature = "register-ir")]
    fn recycle_register_activation(
        &mut self,
        activation: crate::vm::register_ir::RegisterActivation,
    ) -> Result<(), RuntimeError> {
        crate::vm::register_ir::recycle_register_activation(self, activation)
    }
    #[cfg(feature = "register-ir")]
    fn abort_register_activation(
        &mut self,
        activation: crate::vm::register_ir::RegisterActivation,
    ) -> Result<(), RuntimeError> {
        crate::vm::register_ir::abort_register_activation(self, activation)
    }
    fn take_execution_buffers(&mut self) -> alloc::boxed::Box<ExecutionBuffers> {
        self.data.resources.buffers.pop().unwrap_or_default()
    }
    fn recycle_execution_buffers(&mut self, buffers: alloc::boxed::Box<ExecutionBuffers>) {
        self.data.resources.buffers.push(buffers);
    }
    fn take_frame_locals(&mut self, count: usize) -> Vec<Value> {
        self.data.resources.frame_locals.take(count)
    }
    fn recycle_frame_locals(&mut self, mut locals: Vec<Value>) {
        self.recycle_local_references(&mut locals);
        self.data.resources.frame_locals.recycle(locals);
    }
    fn take_frame_presence(&mut self) -> Vec<bool> {
        self.data.resources.frame_locals.take_presence()
    }
    fn recycle_frame_presence(&mut self, present: Vec<bool>) {
        self.data.resources.frame_locals.recycle_presence(present);
    }
}

#[cfg(test)]
mod edge_input_tests;

impl EngineState<'_> {
    /// Whether a scan reads and writes a POU's own scalar variables where they lie (checklist P1-3):
    /// after construction, while no initializer stages its result and no statement hook observes
    /// the instructions that an access in place runs as one.
    fn accesses_in_place(&self) -> bool {
        self.data.construction_complete
            && self.data.construction.initializers.is_empty()
            && !self.services.observes_statements()
    }
}
