//! Explicit hosted POU entries reuse the admitted engine without another dispatcher.

use super::*;
use crate::bytecode::StorageRole;

impl EngineState<'_> {
    pub(super) fn execute_named_program(&mut self, name: &str) -> Result<(), RuntimeError> {
        let prepared = self.prepared;
        let (_, declaration) = prepared
            .global(name)
            .filter(|(_, declaration)| declaration.role == StorageRole::ProgramRoot)
            .ok_or_else(|| RuntimeError::UndefinedProgram(name.into()))?;
        let pou = declaration
            .owner_pou_id
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let instance = match self
            .data
            .storage
            .read_global_slot_by_offset(declaration.slot as usize)
        {
            Some(Value::Instance(instance)) => *instance,
            _ => return Err(RuntimeError::InvalidExecutionState),
        };
        self.execute_pou(pou, Some(instance))?;
        self.check_entry_deadline()
    }

    pub(super) fn execute_bound_function_block(
        &mut self,
        reference: &ValueRef,
    ) -> Result<(), RuntimeError> {
        let instance = match self.read_checked_reference(reference)? {
            Value::Instance(instance) => instance,
            _ => return Err(RuntimeError::TypeMismatch),
        };
        let pou = self
            .data
            .construction
            .instance_templates
            .get(&instance)
            .copied()
            .ok_or(RuntimeError::ReferenceLifetime)?;
        self.execute_pou(pou, Some(instance))?;
        self.check_entry_deadline()
    }
    /// Call an input-only method of a live function-block or class instance between owner
    /// operations, binding the named inputs with the same normalization as an ST call. The
    /// instance's body does not run. `work` bounds the call's allowance; the result is the
    /// method's value and the work the call used.
    #[cfg(any(feature = "hir", feature = "register-ir"))]
    pub(super) fn execute_instance_method(
        &mut self,
        instance: InstanceId,
        method: &str,
        inputs: &[(smol_str::SmolStr, Value)],
        work: usize,
    ) -> Result<(Value, usize), RuntimeError> {
        use crate::vm::call::bindings::{VmNativeArg, VmNativeArgValue};
        if self.data.storage.execution_activation_count() != 0 {
            return Err(RuntimeError::InvalidExecutionState);
        }
        let prepared = self.prepared;
        let module = &prepared.vm;
        let type_name = self
            .data
            .storage
            .get_instance(instance)
            .ok_or(RuntimeError::ReferenceLifetime)?
            .type_name
            .clone();
        let key = smol_str::SmolStr::new(type_name.to_ascii_uppercase());
        let owner = module
            .function_block_ids
            .get(&key)
            .or_else(|| module.class_ids.get(&key))
            .copied()
            .ok_or_else(|| RuntimeError::UndefinedField(type_name.clone()))?;
        let pou = module
            .resolve_method_pou_id_uppercase(owner, &method.to_ascii_uppercase())
            .ok_or_else(|| RuntimeError::UndefinedField(method.into()))?;
        let params = module
            .pou_params(pou)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if params.iter().any(|param| param.direction != 0) {
            return Err(RuntimeError::InvalidConfig(
                "a host method call accepts only input parameters".into(),
            ));
        }
        let mut args = Vec::new();
        args.try_reserve_exact(inputs.len())
            .map_err(|_| RuntimeError::Overflow)?;
        for (name, value) in inputs {
            self.charge_allocation_bytes(self.value_clone_charge(value, 0)?)?;
            args.push(VmNativeArg {
                name: Some(name.clone()),
                value: VmNativeArgValue::Expr(value.clone()),
            });
        }
        let allowance = work.min(prepared.limits.max_work);
        self.data.resources.work_budget.reset(allowance);
        // Input values never resolve references through their caller, so the method's own
        // frame template serves as a detached caller for argument binding.
        let mut frames = crate::vm::frames::FrameStack::default();
        crate::vm::call::push_call_frame(&mut frames, module, pou, usize::MAX, Some(instance))
            .map_err(super::super::VmTrap::into_runtime_error)?;
        let caller = frames
            .pop()
            .map_err(super::super::VmTrap::into_runtime_error)?;
        let mut call = crate::vm::call::continuation::PreparedCall::function(
            self,
            module,
            &caller,
            pou,
            Some(instance),
            &mut args,
        )
        .map_err(super::super::VmTrap::into_runtime_error)?;
        let present = call.present.take();
        let result = crate::vm::dispatch::execute_pou_stack_with_owned_locals(
            self,
            module,
            pou,
            Some(instance),
            &mut call.locals,
            present.as_deref(),
            call.capture_return,
            0,
            crate::vm::budget::ExecutionEntry::Nested,
        )?;
        self.check_entry_deadline()?;
        let used = allowance.saturating_sub(self.data.resources.work_budget.remaining());
        Ok((result.return_value.unwrap_or(Value::Null), used))
    }

    pub(super) fn execute_temporary_function_block(
        &mut self,
        name: &str,
    ) -> Result<(), RuntimeError> {
        // Public explicit entries occur between owner operations. A lifetime
        // scope is not a PLC/debug frame and has no fabricated local variables.
        if self.data.storage.execution_activation_count() != 0
            || !self.data.construction.initializers.is_empty()
        {
            return Err(RuntimeError::InvalidExecutionState);
        }
        self.charge_allocation_bytes(name.len())?;
        let pou = self
            .prepared
            .vm
            .function_block_ids
            .get(name.to_ascii_uppercase().as_str())
            .copied()
            .ok_or_else(|| RuntimeError::UndefinedFunctionBlock(name.into()))?;
        if self.data.storage.execution_activation_count()
            == self.data.storage.execution_frame_capacity()
        {
            return Err(super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        let scope = self.data.storage.reserve_execution_frame()?;
        self.data.storage.activate_execution_frame(scope)?;
        let result = (|| {
            let instance = self.construct_typed_instance(pou, false)?;
            self.execute_pou(pou, Some(instance))?;
            self.check_entry_deadline()
        })();
        // Frame retirement is bookkeeping under TIME-08 and draws no work, so cleanup
        // is never refused by the allowance. The original body fault is preserved.
        let instances = self.remove_owned_instances(scope);
        self.data.storage.release_execution_frame(scope);
        result.and(instances)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_fb_entry_does_not_claim_persistent_roots_or_leave_lifetime_metadata() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let instances = data.storage.instances().len();
        let roots = data.construction.roots.clone();
        let claimed = data.construction.claimed_roots.clone();
        let templates = data.construction.instance_templates.len();
        for _ in 0..3 {
            let mut engine =
                EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
            engine
                .engineering_operation(|engine| engine.execute_temporary_function_block("Counter"))
                .unwrap();
            assert_eq!(engine.data.storage.instances().len(), instances);
            assert_eq!(engine.data.construction.roots, roots);
            assert_eq!(engine.data.construction.claimed_roots, claimed);
            assert_eq!(engine.data.construction.instance_templates.len(), templates);
            assert_eq!(engine.data.storage.execution_activation_count(), 0);
            assert_eq!(engine.data.storage.execution_frame_owner_count(), 0);
            assert_eq!(engine.data.storage.execution_frame_count(), 0);
        }
    }
}
