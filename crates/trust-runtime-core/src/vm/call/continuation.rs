//! Owned user-call inputs and completion: shared by iterative and hosted tier entries.
use super::bindings::{VmFbOutBinding, VmOutBinding};
use super::*;
use crate::vm::{dispatch::VmPouStackResult, edge::EdgeInputTransaction};

#[derive(Debug)]
pub(in crate::vm) enum NativeCall {
    Immediate(Value),
    User(PreparedCall),
}

#[derive(Debug)]
enum Outputs {
    Function(Vec<VmOutBinding>),
    FunctionBlock(Vec<VmFbOutBinding>),
}

#[derive(Debug)]
pub(in crate::vm) struct PreparedCall {
    pub(in crate::vm) pou_id: u32,
    pub(in crate::vm) instance: Option<InstanceId>,
    pub(in crate::vm) locals: Option<Vec<Value>>,
    pub(in crate::vm) present: Option<Vec<bool>>,
    pub(in crate::vm) capture_return: bool,
    outputs: Outputs,
    edge: Option<EdgeInputTransaction>,
}

impl PreparedCall {
    pub(in crate::vm) fn function(
        runtime: &mut impl ExecutionContext,
        module: &VmModule,
        caller: &VmFrame,
        pou_id: u32,
        instance: Option<InstanceId>,
        args: &[VmNativeArg],
    ) -> Result<Self, VmTrap> {
        let bound = bind_vm_call_arguments(runtime, module, caller, pou_id, args)?;
        Ok(Self {
            pou_id,
            instance,
            locals: Some(bound.locals),
            present: Some(bound.present),
            capture_return: module.pou_has_return_slot(pou_id),
            outputs: Outputs::Function(bound.out_bindings),
            edge: None,
        })
    }
    pub(in crate::vm) fn function_block(
        runtime: &mut impl ExecutionContext,
        module: &VmModule,
        caller: &VmFrame,
        pou_id: u32,
        instance: InstanceId,
        args: &[VmNativeArg],
    ) -> Result<Self, VmTrap> {
        runtime.record_call_op(RegisterCallOpKind::FunctionBlockCallEntry);
        let outputs =
            bind_vm_function_block_arguments(runtime, module, caller, pou_id, instance, args)?;
        let edge = EdgeInputTransaction::begin(runtime, module, pou_id, Some(instance))
            .map_err(VmTrap::Runtime)?;
        Ok(Self {
            pou_id,
            instance: Some(instance),
            locals: None,
            present: None,
            capture_return: false,
            outputs: Outputs::FunctionBlock(outputs),
            edge,
        })
    }
    pub(in crate::vm) fn restore_edges(&mut self, runtime: &mut impl ExecutionContext) {
        if let Some(edge) = self.edge.take() {
            edge.restore(runtime);
        }
    }
    #[cfg(feature = "hir")]
    pub(in crate::vm) fn execute_sync(
        self,
        runtime: &mut impl ExecutionContext,
        module: &VmModule,
        caller: &mut VmFrame,
        depth: u32,
    ) -> Result<Value, VmTrap> {
        let result = execute_vm_target(
            runtime,
            module,
            caller,
            self.pou_id,
            self.instance,
            self.locals.as_deref(),
            self.present.as_deref(),
            self.capture_return,
            depth,
        );
        self.complete(runtime, module, caller, result)
    }
    pub(in crate::vm) fn complete(
        mut self,
        runtime: &mut impl ExecutionContext,
        module: &VmModule,
        caller_frame: &mut VmFrame,
        result: Result<VmPouStackResult, VmTrap>,
    ) -> Result<Value, VmTrap> {
        self.restore_edges(runtime);
        let result = result?;
        match self.outputs {
            Outputs::Function(out_bindings) => {
                complete_function(runtime, module, caller_frame, result, out_bindings)
            }
            Outputs::FunctionBlock(out_bindings) => {
                drop(result);
                complete_function_block(runtime, module, caller_frame, out_bindings)
            }
        }
    }
}

fn complete_function(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    caller_frame: &mut VmFrame,
    result: VmPouStackResult,
    out_bindings: Vec<VmOutBinding>,
) -> Result<Value, VmTrap> {
    let mut prepared_outputs = Vec::with_capacity(out_bindings.len());
    for binding in out_bindings {
        let value = result
            .locals
            .get(binding.slot)
            .map(|value| clone_value_with_profile(runtime, value, RegisterValueOpKind::CopyOutput))
            .ok_or_else(|| {
                VmTrap::InvalidNativeCall(
                    format!("native call output slot {} out of bounds", binding.slot).into(),
                )
            })??;
        let value = normalize_output_copyback_value(
            runtime,
            module,
            caller_frame,
            &binding.target,
            binding.target_type_idx,
            value,
        )?;
        binding.target.check_write(runtime, caller_frame, &value)?;
        prepared_outputs.push((binding.target, value));
    }
    if !prepared_outputs.is_empty() {
        runtime.with_output_transaction(caller_frame, |runtime, caller_frame| {
            for (target, value) in prepared_outputs {
                target.write(runtime, caller_frame, value)?;
            }
            Ok(())
        })?;
    }

    Ok(result.return_value.unwrap_or(Value::Null))
}

fn complete_function_block(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    caller_frame: &mut VmFrame,
    out_bindings: Vec<VmFbOutBinding>,
) -> Result<Value, VmTrap> {
    let mut prepared_outputs = Vec::with_capacity(out_bindings.len());
    for binding in out_bindings {
        runtime.record_call_op(RegisterCallOpKind::OutputCopyBack);
        let value = {
            let value = binding
                .source
                .read_checked(runtime)?
                .ok_or(VmTrap::Runtime(RuntimeError::NullReference))?;
            runtime.before_value_clone(value).map_err(VmTrap::Runtime)?;
            let (value, cloned) = materialize_borrowed_value(value);
            if cloned {
                runtime.record_value_op(RegisterValueOpKind::CopyOutput);
            }
            value
        };
        let value = normalize_output_copyback_value(
            runtime,
            module,
            caller_frame,
            &binding.target,
            binding.target_type_idx,
            value,
        )?;
        binding.target.check_write(runtime, caller_frame, &value)?;
        prepared_outputs.push((binding.target, value));
    }
    if !prepared_outputs.is_empty() {
        runtime.with_output_transaction(caller_frame, |runtime, caller_frame| {
            for (target, value) in prepared_outputs {
                target.write(runtime, caller_frame, value)?;
            }
            Ok(())
        })?;
    }

    Ok(Value::Null)
}
