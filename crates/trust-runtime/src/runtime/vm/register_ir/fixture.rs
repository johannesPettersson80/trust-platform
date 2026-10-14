//! Explicit low-level legacy verifier fixtures. This adapter is test-only and
//! never installs a controller or supplies production admission evidence.
use super::*;
use crate::{
    error::RuntimeError,
    memory::{InstanceId, MemoryLocation, VariableStorage},
    value::{DateTimeProfile, DateTimeValue, Duration, RefSegment, Value, ValueRef, ValueRefView},
};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};
use trust_runtime_core::vm::hosted::{
    budget::{ExecutionBudget, ExecutionEntry},
    call::context::{
        BuiltinParam, CallContext, RegisterCallOpKind, RegisterValueOpKind, VmEdgeInput,
    },
    context::{ExecutionContext, ReferenceContext},
    dispatch::VmPouStackResult,
    OperandStack, VmFrame,
};
use trust_runtime_core::vm::register_test_support::{
    self as register_test_support, PreparedRegisterPlans, PreparedRegisterProgram, RegisterContext,
    RegisterExecutionBuffers, RegisterProfileState, RegisterTier1SpecializedExecutorState,
};

pub(crate) struct LegacyRegisterFixtureContext<'a> {
    runtime: &'a mut Runtime,
    plans: PreparedRegisterPlans,
    scratch: Vec<(u32, Option<RegisterExecutionBuffers>)>,
}
impl<'a> LegacyRegisterFixtureContext<'a> {
    pub(crate) fn new(runtime: &'a mut Runtime, module: &VmModule) -> Result<Self, RuntimeError> {
        let limits = trust_runtime_core::vm::PreparationLimits::default();
        let mut bytes = 0usize;
        let mut work = 0usize;
        let plans = PreparedRegisterPlans::prepare(module, |amount, units| {
            bytes = bytes.checked_add(amount).ok_or(RuntimeError::Overflow)?;
            work = work.checked_add(units).ok_or(RuntimeError::Overflow)?;
            if bytes > limits.max_preparation_bytes || work > limits.max_preparation_work {
                return Err(RuntimeError::PreparationLimit);
            }
            Ok(())
        })?;
        let scratch = plans.reserve_execution_buffers(module, |amount, units| {
            bytes = bytes.checked_add(amount).ok_or(RuntimeError::Overflow)?;
            work = work.checked_add(units).ok_or(RuntimeError::Overflow)?;
            if bytes > limits.max_preparation_bytes || work > limits.max_preparation_work {
                return Err(RuntimeError::PreparationLimit);
            }
            Ok(())
        })?;
        runtime.vm_register_profile.prepare(&plans);
        runtime.vm_register_lowering_cache.prepare(&plans);
        runtime.vm_tier1_specialized_executor.prepare(&plans)?;
        Ok(Self {
            runtime,
            plans,
            scratch,
        })
    }
}
impl Deref for LegacyRegisterFixtureContext<'_> {
    type Target = Runtime;
    fn deref(&self) -> &Runtime {
        self.runtime
    }
}
impl DerefMut for LegacyRegisterFixtureContext<'_> {
    fn deref_mut(&mut self) -> &mut Runtime {
        self.runtime
    }
}
impl ReferenceContext for LegacyRegisterFixtureContext<'_> {
    fn normalize_assignment(
        &self,
        module: &VmModule,
        ty: u32,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        ReferenceContext::normalize_assignment(self.runtime, module, ty, value)
    }
    fn before_path_write(
        &self,
        root: &Value,
        path: &[RefSegment],
        input: &Value,
    ) -> Result<(), RuntimeError> {
        ReferenceContext::before_path_write(self.runtime, root, path, input)
    }
    fn before_output_write(
        &mut self,
        frame: &VmFrame,
        reference: ValueRefView<'_>,
    ) -> Result<(), RuntimeError> {
        ReferenceContext::before_output_write(self.runtime, frame, reference)
    }
    fn before_value_clone(&self, value: &Value) -> Result<(), RuntimeError> {
        ReferenceContext::before_value_clone(self.runtime, value)
    }
    fn check_reference_write(
        &self,
        reference: ValueRefView<'_>,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        ReferenceContext::check_reference_write(self.runtime, reference, value)
    }
    fn check_reference_read(&self, reference: ValueRefView<'_>) -> Result<(), RuntimeError> {
        ReferenceContext::check_reference_read(self.runtime, reference)
    }
    fn initializer_reference<'a>(
        &self,
        id: u32,
        path: &'a [RefSegment],
    ) -> Result<ValueRefView<'a>, trust_runtime_core::vm::VmTrap> {
        ReferenceContext::initializer_reference(self.runtime, id, path)
    }
    fn resolve_instance_owner(
        &self,
        id: u32,
        frame: &VmFrame,
    ) -> Result<InstanceId, trust_runtime_core::vm::VmTrap> {
        ReferenceContext::resolve_instance_owner(self.runtime, id, frame)
    }
    fn storage(&self) -> &VariableStorage {
        ReferenceContext::storage(self.runtime)
    }
    fn storage_mut(&mut self) -> &mut VariableStorage {
        ReferenceContext::storage_mut(self.runtime)
    }
    fn write_typed_storage(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> Result<bool, RuntimeError> {
        ReferenceContext::write_typed_storage(self.runtime, location, offset, path, value)
    }
    fn write_typed_local(&self, root: &mut Value, path: &[RefSegment], value: Value) -> bool {
        ReferenceContext::write_typed_local(self.runtime, root, path, value)
    }
    fn write_typed_local_checked(
        &self,
        root: &mut Value,
        path: &[RefSegment],
        value: Value,
    ) -> Result<bool, RuntimeError> {
        ReferenceContext::write_typed_local_checked(self.runtime, root, path, value)
    }
}
impl CallContext for LegacyRegisterFixtureContext<'_> {
    fn recycle_optimized_result_locals(
        &mut self,
        pou_id: u32,
        locals: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        let Ok(index) = self.scratch.binary_search_by_key(&pou_id, |(id, _)| *id) else {
            return Ok(());
        };
        let Some(scratch) = self.scratch[index].1.as_mut() else {
            return Ok(());
        };
        register_test_support::recycle_result_locals(scratch, locals)
    }

    fn profile(&self) -> &DateTimeProfile {
        CallContext::profile(self.runtime)
    }
    fn current_time(&self) -> Duration {
        CallContext::current_time(self.runtime)
    }
    fn current_dt(&mut self) -> Result<DateTimeValue, RuntimeError> {
        CallContext::current_dt(self.runtime)
    }
    fn stdlib(&self) -> &crate::stdlib::StandardLibrary {
        CallContext::stdlib(self.runtime)
    }
    fn builtin_params(&self, key: &str) -> Option<Vec<BuiltinParam>> {
        CallContext::builtin_params(self.runtime, key)
    }
    fn edge_inputs(&self, key: &str) -> Vec<VmEdgeInput> {
        CallContext::edge_inputs(self.runtime, key)
    }
    fn suspend_frame(&mut self, frame: &mut VmFrame) -> Result<(), RuntimeError> {
        CallContext::suspend_frame(self.runtime, frame)
    }
    fn resume_frame(&mut self, frame: &mut VmFrame) -> Result<(), RuntimeError> {
        CallContext::resume_frame(self.runtime, frame)
    }
    fn record_call_op(&mut self, kind: RegisterCallOpKind) {
        CallContext::record_call_op(self.runtime, kind);
    }
    fn record_value_op(&mut self, kind: RegisterValueOpKind) {
        CallContext::record_value_op(self.runtime, kind);
    }
    fn check_native_write(
        &self,
        frame: Option<&VmFrame>,
        reference: &ValueRef,
        value: &Value,
    ) -> Result<(), RuntimeError> {
        CallContext::check_native_write(self.runtime, frame, reference, value)
    }
    fn check_builtin_call(&self, id: InstanceId) -> Result<(), RuntimeError> {
        CallContext::check_builtin_call(self.runtime, id)
    }
    fn try_execute_optimized(
        &mut self,
        module: &VmModule,
        id: u32,
        instance: Option<InstanceId>,
        locals: Option<&[Value]>,
        capture: bool,
        depth: u32,
        entry: ExecutionEntry,
    ) -> Result<Option<VmPouStackResult>, RuntimeError> {
        self.try_execute_optimized_with_parameter_presence(
            module, id, instance, locals, None, capture, depth, entry,
        )
    }
    fn try_execute_optimized_with_parameter_presence(
        &mut self,
        module: &VmModule,
        id: u32,
        instance: Option<InstanceId>,
        locals: Option<&[Value]>,
        presence: Option<&[bool]>,
        capture: bool,
        depth: u32,
        entry: ExecutionEntry,
    ) -> Result<Option<VmPouStackResult>, RuntimeError> {
        trust_runtime_core::vm::register_test_support::try_execute_pou_with_register_ir_with_parameter_presence(self,module,id,instance,locals,presence,capture,depth,entry).map(|result|result.map(|result|VmPouStackResult{return_value:result.return_value,locals:result.locals}))
    }
}
impl ExecutionContext for LegacyRegisterFixtureContext<'_> {
    fn initialize_frame(
        &mut self,
        module: &VmModule,
        frame: &mut VmFrame,
        depth: u32,
    ) -> Result<(), RuntimeError> {
        ExecutionContext::initialize_frame(self.runtime, module, frame, depth)
    }
    fn execution_budget(&self) -> &ExecutionBudget {
        ExecutionContext::execution_budget(self.runtime)
    }
    fn retire_frame(&mut self, frame: &VmFrame) -> Result<(), RuntimeError> {
        ExecutionContext::retire_frame(self.runtime, frame)
    }
    fn check_frame_return(&self, frame: &VmFrame, value: &Value) -> Result<(), RuntimeError> {
        ExecutionContext::check_frame_return(self.runtime, frame, value)
    }
    fn execute_initialization_opcode(
        &mut self,
        module: &VmModule,
        frame: &mut VmFrame,
        opcode: u8,
        operand: u32,
        stack: &mut OperandStack,
        depth: u32,
    ) -> Result<(), RuntimeError> {
        ExecutionContext::execute_initialization_opcode(
            self.runtime,
            module,
            frame,
            opcode,
            operand,
            stack,
            depth,
        )
    }
    fn deadline_exceeded(&self) -> bool {
        ExecutionContext::deadline_exceeded(self.runtime)
    }
    fn on_statement(&mut self, module: &VmModule, id: u32, pc: usize, depth: u32) {
        ExecutionContext::on_statement(self.runtime, module, id, pc, depth);
    }
    fn sizeof_value(&self, value: &Value) -> Result<u64, RuntimeError> {
        ExecutionContext::sizeof_value(self.runtime, value)
    }
}
impl RegisterContext for LegacyRegisterFixtureContext<'_> {
    fn register_profile(&self) -> &RegisterProfileState {
        &self.runtime.vm_register_profile
    }
    fn register_profile_mut(&mut self) -> &mut RegisterProfileState {
        &mut self.runtime.vm_register_profile
    }
    fn tier1_state(&self) -> &RegisterTier1SpecializedExecutorState {
        &self.runtime.vm_tier1_specialized_executor
    }
    fn tier1_state_mut(&mut self) -> &mut RegisterTier1SpecializedExecutorState {
        &mut self.runtime.vm_tier1_specialized_executor
    }
    fn prepared_register_plan(&self, id: u32) -> Option<Arc<PreparedRegisterProgram>> {
        self.plans.get(id)
    }
    fn debug_active(&self) -> bool {
        self.runtime.debug.is_some()
    }
    fn register_buffers_available(&self, pou_id: u32) -> bool {
        self.scratch
            .binary_search_by_key(&pou_id, |(id, _)| *id)
            .ok()
            .and_then(|index| self.scratch.get(index))
            .is_some_and(|(_, scratch)| {
                scratch
                    .as_ref()
                    .is_some_and(register_test_support::can_enter_with_frame_payload)
            })
    }
    fn record_register_plan_lookup(&mut self, found: bool) {
        self.runtime.vm_register_lowering_cache.record_lookup(found);
    }
    fn take_register_buffers(
        &mut self,
        pou_id: u32,
        count: usize,
    ) -> Result<RegisterExecutionBuffers, RuntimeError> {
        let index = self
            .scratch
            .binary_search_by_key(&pou_id, |(id, _)| *id)
            .map_err(|_| RuntimeError::InvalidExecutionState)?;
        let slot = &mut self.scratch[index].1;
        let mut scratch = slot.take().ok_or(RuntimeError::InvalidExecutionState)?;
        if let Err(error) = scratch.select(count) {
            *slot = Some(scratch);
            return Err(error);
        }
        Ok(scratch)
    }
    fn recycle_register_buffers(
        &mut self,
        pou_id: u32,
        mut scratch: RegisterExecutionBuffers,
    ) -> Result<(), RuntimeError> {
        let index = self
            .scratch
            .binary_search_by_key(&pou_id, |(id, _)| *id)
            .map_err(|_| RuntimeError::InvalidExecutionState)?;
        let slot = &mut self.scratch[index].1;
        if slot.is_some() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        scratch.clear();
        *slot = Some(scratch);
        Ok(())
    }
}
