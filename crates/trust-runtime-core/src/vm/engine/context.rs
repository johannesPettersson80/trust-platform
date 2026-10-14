//! Artifact-backed services for the shared dispatcher and native call path.
use super::super::call::context::{
    BuiltinParam, BuiltinParamDirection, CallContext, RegisterCallOpKind, RegisterValueOpKind,
    VmEdgeInput,
};
use super::super::context::{ExecutionContext, ReferenceContext};
use super::super::{OperandStack, VmTrap};
use super::*;
use crate::memory::MemoryLocation;
use crate::stdlib::StandardLibrary;
use crate::value::{write_value_path, RefSegment};

#[cfg(test)]
#[path = "context/owner_tests.rs"]
mod owner_tests;

impl ReferenceContext for EngineState<'_> {
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

    fn charge_text_result(&self, bytes: usize) -> Result<(), RuntimeError> {
        // TIME-08 TEXT: a new text value costs one unit and its Value plus its bytes.
        self.charge_work_units(1)?;
        let allocation = core::mem::size_of::<Value>()
            .checked_add(bytes)
            .ok_or(RuntimeError::Overflow)?;
        self.charge_allocation_bytes(allocation)
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
        let bytes = self.reference_path_clone_charge(reference.path)?;
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
                    .data
                    .storage
                    .get_instance(instance)
                    .and_then(|value| value.parent)
                    .ok_or_else(|| VmTrap::NullReference)?;
            }
        }
        root.and_then(|(index, _)| self.data.construction.roots[index])
            .ok_or_else(|| VmTrap::NullReference)
    }

    fn storage(&self) -> &VariableStorage {
        &self.data.storage
    }
    fn storage_mut(&mut self) -> &mut VariableStorage {
        &mut self.data.storage
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
    ) -> Result<bool, RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing =
            crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::StorageWrite);
        self.charge_storage_path_write(&self.data.storage, location, offset, path, &value)?;
        if path.is_empty() && !matches!(value, Value::Struct(_)) {
            return Ok(self
                .data
                .storage
                .write_direct_slot_by_location(location, offset, value));
        }
        let Some(prepared) = self
            .data
            .storage
            .prepare_by_ref_parts_write(location, offset, path, &value)?
        else {
            return Ok(false);
        };
        if prepared.requires_final_deadline_check() {
            self.check_entry_deadline()?;
        }
        Ok(self
            .data
            .storage
            .commit_prepared_write(path, value, prepared))
    }
    fn write_typed_local(&self, root: &mut Value, path: &[RefSegment], value: Value) -> bool {
        write_value_path(root, path, value)
    }
    fn write_typed_local_checked(
        &self,
        root: &mut Value,
        path: &[RefSegment],
        value: Value,
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
            root, path, value, prepared,
        ))
    }
}

impl CallContext for EngineState<'_> {
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
        let pou = self.prepared.vm.function_block_ids.get(key)?;
        self.prepared
            .vm
            .pou_params(*pou)?
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
        let Some(pou) = self
            .prepared
            .vm
            .function_block_ids
            .get(owner.to_ascii_uppercase().as_str())
            .or_else(|| {
                self.prepared
                    .vm
                    .program_ids
                    .get(owner.to_ascii_uppercase().as_str())
            })
        else {
            return Vec::new();
        };
        self.prepared.edge_inputs(*pou).to_vec()
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
}

impl ExecutionContext for EngineState<'_> {
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
        if depth as usize >= self.prepared.limits.max_call_depth {
            return Err(VmTrap::CallStackOverflow.into_runtime_error());
        }
        let id = self.data.storage.reserve_execution_frame()?;
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
        self.initialize_frame_declarations(module, frame, depth)
    }
    fn retire_frame(&mut self, frame: &VmFrame) -> Result<(), RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing =
            crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::FrameRetire);
        if let Some(id) = frame.activation {
            let owned_work = self.remove_owned_instances(id);
            self.data.storage.release_execution_frame(id);
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
            self.services
                .on_statement(pou, pc, depth, &self.data.storage, self.data.now);
        }
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
            return self.services.on_statement_with_frame_checked(
                pou,
                pc,
                depth,
                &self.data.storage,
                self.data.now,
                frame.with_call_stack(&self.data.storage),
            );
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
    fn take_execution_buffers(&mut self) -> ExecutionBuffers {
        self.data.resources.buffers.pop().unwrap_or_default()
    }
    fn recycle_execution_buffers(&mut self, buffers: ExecutionBuffers) {
        self.data.resources.buffers.push(buffers);
    }
}

#[cfg(test)]
mod edge_input_tests;
