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

impl ReferenceContext for EngineState<'_> {
    fn before_path_write(&self, root: &Value, path: &[RefSegment]) -> Result<(), RuntimeError> {
        self.charge_path_write(root, path)
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
        if self.resources.output_journal.is_none() {
            return Ok(());
        }
        let saved = self.snapshot_destinations(Some(frame), core::iter::once(reference))?;
        if let Some(journal) = self.resources.output_journal.as_mut() {
            journal.extend(saved);
        }
        Ok(())
    }
    fn before_value_clone(&self, value: &Value) -> Result<(), RuntimeError> {
        let bytes = self.value_clone_charge(value, 0)?;
        self.charge_allocation_bytes(bytes)
    }

    fn resolve_instance_owner(&self, encoded: u32, frame: &VmFrame) -> Result<InstanceId, VmTrap> {
        self.charge_work_units(self.prepared.lookup_work())
            .map_err(VmTrap::Runtime)?;
        let root = self.prepared.root_for_instance_owner(encoded);
        if frame.instance_owner == Some(encoded) {
            if let Some(mut instance) = frame.runtime_instance {
                let template = root.and_then(|(_, root)| root.template_pou_id).or_else(|| {
                    frame
                        .pou_id
                        .and_then(|pou| self.prepared.method_owners.get(&pou).copied())
                });
                for _ in 0..=self.prepared.vm.pou_by_id.len() {
                    if template.is_none()
                        || self.construction.instance_templates.get(&instance).copied() == template
                    {
                        return Ok(instance);
                    }
                    instance = self
                        .storage
                        .get_instance(instance)
                        .and_then(|value| value.parent)
                        .ok_or(VmTrap::NullReference)?;
                }
            }
        }
        root.and_then(|(index, _)| self.construction.roots[index])
            .ok_or(VmTrap::NullReference)
    }

    fn storage(&self) -> &VariableStorage {
        &self.storage
    }
    fn storage_mut(&mut self) -> &mut VariableStorage {
        &mut self.storage
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
            .construction
            .initializers
            .last()
            .filter(|active| active.id == initializer_id)
            .ok_or(VmTrap::NullReference)?;
        Ok(crate::value::ValueRefView {
            location: MemoryLocation::Local(active.result),
            offset: 0,
            path,
        })
    }
    fn write_typed_storage(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> Result<bool, RuntimeError> {
        self.charge_storage_path_write(&self.storage, location, offset, path)?;
        Ok(self
            .storage
            .write_by_ref_parts(location, offset, path, value))
    }
    fn write_typed_local(&self, root: &mut Value, path: &[RefSegment], value: Value) -> bool {
        write_value_path(root, path, value)
    }
}

impl CallContext for EngineState<'_> {
    fn with_output_transaction<T>(
        &mut self,
        frame: &mut VmFrame,
        action: impl FnOnce(&mut Self, &mut VmFrame) -> Result<T, VmTrap>,
    ) -> Result<T, VmTrap> {
        self.commit_output_transaction(frame, action)
    }
    fn profile(&self) -> &DateTimeProfile {
        &self.profile
    }
    fn current_time(&self) -> Duration {
        self.now
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

    fn suspend_frame(&mut self, frame: &mut VmFrame) -> Result<(), RuntimeError> {
        self.storage.suspend_execution_frame(
            frame
                .activation
                .ok_or(RuntimeError::InvalidExecutionState)?,
            &mut frame.locals,
        )
    }
    fn resume_frame(&mut self, frame: &mut VmFrame) -> Result<(), RuntimeError> {
        self.storage.resume_execution_frame(
            frame
                .activation
                .ok_or(RuntimeError::InvalidExecutionState)?,
            &mut frame.locals,
        )
    }
    fn record_call_op(&mut self, _kind: RegisterCallOpKind) {}
    fn record_value_op(&mut self, _kind: RegisterValueOpKind) {}
    fn check_native_write(
        &self,
        _frame: Option<&VmFrame>,
        reference: &ValueRef,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        self.check_write(reference.as_view(), value)
    }
    fn check_builtin_call(&self, _instance_id: InstanceId) -> Result<(), RuntimeError> {
        if self.construction.initializers.is_empty() {
            Ok(())
        } else {
            Err(RuntimeError::StagingViolation)
        }
    }
}

impl ExecutionContext for EngineState<'_> {
    fn execution_budget(&self) -> &crate::vm::budget::ExecutionBudget {
        &self.resources.work_budget
    }
    fn initialize_frame(
        &mut self,
        module: &super::super::module::VmModule,
        frame: &mut VmFrame,
        depth: u32,
    ) -> Result<(), RuntimeError> {
        if depth as usize >= self.prepared.limits.max_call_depth {
            return Err(VmTrap::CallStackOverflow.into_runtime_error());
        }
        let id = self.storage.reserve_execution_frame()?;
        frame.activation = Some(id);
        self.lifetimes.live_activations.push(id);
        self.lifetimes.activation_pous.insert(
            id,
            frame
                .pou_id
                .ok_or_else(|| invalid_bytecode("ordinary frame has no POU owner"))?,
        );
        self.initialize_frame_declarations(module, frame, depth)
    }
    fn retire_frame(&mut self, frame: &VmFrame) {
        if let Some(id) = frame.activation {
            self.remove_owned_instances(id);
            self.storage.release_execution_frame(id);
            self.lifetimes
                .live_activations
                .retain(|candidate| *candidate != id);
            self.lifetimes.activation_pous.remove(&id);
        }
    }
    fn check_frame_return(&self, frame: &VmFrame, value: &Value) -> Result<(), RuntimeError> {
        let destination = frame
            .activation
            .and_then(|id| {
                self.lifetimes
                    .live_activations
                    .iter()
                    .position(|candidate| *candidate == id)
            })
            .and_then(|index| index.checked_sub(1))
            .map(|index| self.lifetimes.live_activations[index]);
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
        self.services.deadline_exceeded()
    }
    fn on_statement(
        &mut self,
        _module: &super::super::module::VmModule,
        _pou: u32,
        _pc: usize,
        _depth: u32,
    ) {
    }
    fn sizeof_value(&self, value: &Value) -> Result<u64, RuntimeError> {
        self.sizeof_runtime_value(value)
    }
    fn take_execution_buffers(&mut self) -> ExecutionBuffers {
        self.resources.buffers.pop().unwrap_or_default()
    }
    fn recycle_execution_buffers(&mut self, buffers: ExecutionBuffers) {
        self.resources.buffers.push(buffers);
    }
}
