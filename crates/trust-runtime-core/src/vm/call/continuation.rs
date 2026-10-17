//! Owned user-call inputs and completion: shared by iterative and hosted tier entries.
use super::bindings::{
    bind_vm_call_arguments_owned, bind_vm_call_arguments_owned_planned,
    bind_vm_function_block_arguments_planned, VmFbOutBinding, VmOutBinding,
};
use super::*;
use crate::vm::prepared::call_plans::CallArguments;
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
    /// A completed or abandoned call whose output bindings have gone back to the call scratch.
    Empty,
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
        args: &mut [VmNativeArg],
        arguments: Option<&CallArguments>,
    ) -> Result<Self, VmTrap> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing =
            crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::ArgumentBind);
        let (bound, capture_return) = match arguments {
            Some(arguments) => bind_vm_call_arguments_owned_planned(
                runtime, module, caller, pou_id, args, arguments,
            )?,
            None => bind_vm_call_arguments_owned(runtime, module, caller, pou_id, args)?,
        };
        Ok(Self {
            pou_id,
            instance,
            locals: Some(bound.locals),
            present: Some(bound.present),
            capture_return,
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
        arguments: Option<&CallArguments>,
    ) -> Result<Self, VmTrap> {
        runtime.record_call_op(RegisterCallOpKind::FunctionBlockCallEntry);
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing =
            crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::ArgumentBind);
        let outputs = match arguments {
            Some(arguments) => bind_vm_function_block_arguments_planned(
                runtime, module, caller, pou_id, instance, args, arguments,
            )?,
            None => {
                bind_vm_function_block_arguments(runtime, module, caller, pou_id, instance, args)?
            }
        };
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
    /// Restore this call's edge inputs and return the reserved call scratch it still holds. It
    /// runs on completion and on every path that abandons the call — a trap, a refusal or an
    /// abort — so a scratch is never dropped (spec 34 MEM-09; census C1, C3, C4, C5, C7). The
    /// completion path empties `present` and `outputs` first, so their copy-back still sees the
    /// bindings and returns them itself.
    pub(in crate::vm) fn restore_edges(&mut self, runtime: &mut impl ExecutionContext) {
        if let Some(edge) = self.edge.take() {
            edge.restore(runtime);
        }
        if let Some(present) = self.present.take() {
            runtime.recycle_call_flags(present);
        }
        match core::mem::replace(&mut self.outputs, Outputs::Empty) {
            Outputs::Function(outputs) => runtime.recycle_call_function_outputs(outputs),
            Outputs::FunctionBlock(outputs) => runtime.recycle_call_fb_outputs(outputs),
            Outputs::Empty => {}
        }
    }
    #[cfg(any(feature = "hir", feature = "register-ir"))]
    pub(in crate::vm) fn execute_sync(
        mut self,
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
            &mut self.locals,
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
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing =
            crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::CallComplete);
        // Empty `present` and `outputs` before restoring the edges, so this completion keeps the
        // bindings for its copy-back group and returns them itself. An abandoned call reaches
        // `restore_edges` with them still held, and it returns them there (spec 34 MEM-09; census
        // C5).
        let present = self.present.take();
        let outputs = core::mem::replace(&mut self.outputs, Outputs::Empty);
        self.restore_edges(runtime);
        if let Some(present) = present {
            runtime.recycle_call_flags(present);
        }
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                recycle_outputs(runtime, outputs);
                return Err(error);
            }
        };
        let pou_id = self.pou_id;
        let VmPouStackResult {
            return_value,
            mut locals,
        } = result;
        let completed = match outputs {
            Outputs::Function(out_bindings) => complete_function(
                runtime,
                module,
                caller_frame,
                return_value,
                &mut locals,
                out_bindings,
            ),
            Outputs::FunctionBlock(out_bindings) => {
                complete_function_block(runtime, module, caller_frame, out_bindings)
            }
            Outputs::Empty => {
                let recycled = runtime
                    .recycle_optimized_result_locals(pou_id, locals)
                    .map_err(VmTrap::Runtime);
                return recycled.map(|()| return_value.unwrap_or(Value::Null));
            }
        };
        let recycled = runtime
            .recycle_optimized_result_locals(pou_id, locals)
            .map_err(VmTrap::Runtime);
        match completed {
            Err(error) => Err(error),
            Ok(value) => recycled.map(|()| value),
        }
    }
}

/// Return a completed call's output bindings to the reserved scratch (spec 34 MEM-09; census
/// C3, C4, C5).
fn recycle_outputs(runtime: &mut impl ExecutionContext, outputs: Outputs) {
    match outputs {
        Outputs::Function(outputs) => runtime.recycle_call_function_outputs(outputs),
        Outputs::FunctionBlock(outputs) => runtime.recycle_call_fb_outputs(outputs),
        Outputs::Empty => {}
    }
}

fn complete_function(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    caller_frame: &mut VmFrame,
    return_value: Option<Value>,
    locals: &mut [Value],
    out_bindings: Vec<VmOutBinding>,
) -> Result<Value, VmTrap> {
    // The output bindings and the copy-back group come from the call's reserved scratch and are
    // returned on every path (spec 34 MEM-09; census C5, C7).
    let mut out_bindings = out_bindings;
    let mut prepared_outputs = runtime.take_call_prepared_outputs();
    let result = prepare_function_outputs(
        runtime,
        module,
        caller_frame,
        locals,
        &mut out_bindings,
        &mut prepared_outputs,
    );
    runtime.recycle_call_function_outputs(out_bindings);
    runtime.recycle_call_prepared_outputs(prepared_outputs);
    result?;

    Ok(return_value.unwrap_or(Value::Null))
}

/// Normalize a function call's outputs and commit them as one copy-back group (spec 34 MEM-09;
/// census C7). The caller owns the scratch vectors and returns them.
fn prepare_function_outputs(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    caller_frame: &mut VmFrame,
    locals: &mut [Value],
    out_bindings: &mut Vec<VmOutBinding>,
    prepared_outputs: &mut Vec<(VmWriteTarget, Value)>,
) -> Result<(), VmTrap> {
    for binding in out_bindings.drain(..) {
        // Binding assigns one distinct local slot per parameter. Caller targets
        // may alias, but the completed callee payload is private and is recycled
        // after this group. Transfer it while retaining ordered preflight/commit.
        let slot = locals.get_mut(binding.slot).ok_or_else(|| {
            VmTrap::InvalidNativeCall(
                format!("native call output slot {} out of bounds", binding.slot).into(),
            )
        })?;
        let value = core::mem::replace(slot, Value::Null);
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
            for (target, value) in prepared_outputs.drain(..) {
                target.write(runtime, caller_frame, value)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn complete_function_block(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    caller_frame: &mut VmFrame,
    out_bindings: Vec<VmFbOutBinding>,
) -> Result<Value, VmTrap> {
    let mut out_bindings = out_bindings;
    let mut prepared_outputs = runtime.take_call_prepared_outputs();
    let result = prepare_function_block_outputs(
        runtime,
        module,
        caller_frame,
        &mut out_bindings,
        &mut prepared_outputs,
    );
    runtime.recycle_call_fb_outputs(out_bindings);
    runtime.recycle_call_prepared_outputs(prepared_outputs);
    result?;

    Ok(Value::Null)
}

/// Normalize a function block's outputs and commit them as one copy-back group (spec 34
/// MEM-09; census C7). The caller owns the scratch vectors and returns them.
fn prepare_function_block_outputs(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    caller_frame: &mut VmFrame,
    out_bindings: &mut Vec<VmFbOutBinding>,
    prepared_outputs: &mut Vec<(VmWriteTarget, Value)>,
) -> Result<(), VmTrap> {
    for binding in out_bindings.drain(..) {
        runtime.record_call_op(RegisterCallOpKind::OutputCopyBack);
        let value = {
            let value = binding
                .source
                .read_checked(runtime)?
                .ok_or_else(|| VmTrap::Runtime(RuntimeError::NullReference))?;
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
            for (target, value) in prepared_outputs.drain(..) {
                target.write(runtime, caller_frame, value)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}
