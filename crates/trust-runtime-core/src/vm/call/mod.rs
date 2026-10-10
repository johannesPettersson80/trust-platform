use alloc::{format, vec, vec::Vec};
use smol_str::SmolStr;

use crate::bytecode::{
    NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_FUNCTION_BLOCK, NATIVE_CALL_KIND_METHOD,
    NATIVE_CALL_KIND_STDLIB,
};
use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::stdlib::fbs;
use crate::value::Value;

use self::context::{RegisterCallOpKind, RegisterValueOpKind};
use super::context::ExecutionContext;
use super::errors::VmTrap;
use super::frames::{FrameStack, VmFrame};
use super::materialize_borrowed_value;
use super::module::{VmModule, VmNativeSymbolSpec};
use super::stack::OperandStack;

pub(super) mod bindings;
pub(super) mod context;
pub(super) mod stdlib;
#[cfg(test)]
mod tests;

use self::bindings::{
    bind_builtin_function_block_arguments, bind_vm_call_arguments,
    bind_vm_function_block_arguments, clone_value_with_profile, normalize_output_copyback_value,
    unpack_native_call_payload, VmNativeArg,
};
use self::stdlib::dispatch_native_stdlib_call;

pub use crate::vm::context::VM_LOCAL_SENTINEL_FRAME_ID;

/// Push a frame with the declared local shape and caller return address.
pub fn push_call_frame(
    frame_stack: &mut FrameStack,
    module: &VmModule,
    pou_id: u32,
    return_pc: usize,
    runtime_instance: Option<InstanceId>,
) -> Result<usize, VmTrap> {
    let pou = module.pou(pou_id).ok_or(VmTrap::MissingPou(pou_id))?;
    let local_count = super::materialization_limits::checked_local_count(pou.local_ref_count)?;
    let frame = VmFrame {
        pou_id: Some(pou_id),
        activation: None,
        return_pc,
        code_start: pou.code_start,
        code_end: pou.code_end,
        local_ref_start: pou.local_ref_start,
        local_ref_count: pou.local_ref_count,
        locals: vec![Value::Null; local_count],
        parameter_values_present: Vec::new(),
        runtime_instance,
        instance_owner: pou.primary_instance_owner,
    };
    let entry_pc = frame.code_start;
    frame_stack.push(frame)?;
    Ok(entry_pc)
}

/// Dispatch an encoded call through shared binding, lifecycle and native policy hooks.
#[allow(clippy::too_many_arguments)]
pub fn execute_native_call(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    frame: &mut VmFrame,
    operand_stack: &mut OperandStack,
    caller_depth: u32,
    kind: u32,
    symbol_idx: u32,
    arg_count: u32,
) -> Result<Value, VmTrap> {
    if runtime.deadline_exceeded() {
        return Err(VmTrap::DeadlineExceeded);
    }
    let spec = module.native_symbol_spec(symbol_idx)?;
    let (target_name, normalized_target_name, resolved_function_pou_id, conversion_spec, arg_specs) =
        match spec {
            VmNativeSymbolSpec::Parsed {
                target_name,
                normalized_target_name,
                resolved_function_pou_id,
                conversion_spec,
                arg_specs,
            } => (
                target_name,
                normalized_target_name,
                *resolved_function_pou_id,
                *conversion_spec,
                arg_specs.as_slice(),
            ),
            VmNativeSymbolSpec::ParseError(message) => {
                return Err(VmTrap::InvalidNativeCall(message.clone()));
            }
        };
    let receiver_count = native_receiver_count(kind)?;
    let total = usize::try_from(arg_count)
        .map_err(|_| VmTrap::InvalidNativeCall("arg_count overflow".into()))?;
    if total < receiver_count {
        return Err(VmTrap::InvalidNativeCall(
            "arg_count smaller than native receiver arity".into(),
        ));
    }
    if arg_specs.len() + receiver_count != total {
        return Err(VmTrap::InvalidNativeCall(
            format!(
                "symbol arg metadata mismatch: expected {} payload(s), got {total}",
                arg_specs.len() + receiver_count
            )
            .into(),
        ));
    }

    let (receiver_value, vm_args) =
        unpack_native_call_payload(operand_stack, arg_specs, receiver_count)?;

    match kind {
        NATIVE_CALL_KIND_FUNCTION | NATIVE_CALL_KIND_STDLIB => {
            if target_name.is_empty() {
                return Err(VmTrap::InvalidNativeCall(
                    "missing native function target".into(),
                ));
            }
        }
        NATIVE_CALL_KIND_FUNCTION_BLOCK => {
            receiver_value.as_ref().ok_or_else(|| {
                VmTrap::InvalidNativeCall("missing function-block receiver payload".into())
            })?;
        }
        NATIVE_CALL_KIND_METHOD => {
            if target_name.is_empty() {
                return Err(VmTrap::InvalidNativeCall("missing method name".into()));
            }
            receiver_value.as_ref().ok_or_else(|| {
                VmTrap::InvalidNativeCall("missing method receiver payload".into())
            })?;
        }
        _ => return Err(VmTrap::InvalidNativeCallKind(kind)),
    }

    let result = match kind {
        NATIVE_CALL_KIND_STDLIB => dispatch_native_stdlib_call(
            runtime,
            frame,
            target_name,
            normalized_target_name,
            conversion_spec,
            &vm_args,
        ),
        NATIVE_CALL_KIND_FUNCTION | NATIVE_CALL_KIND_FUNCTION_BLOCK | NATIVE_CALL_KIND_METHOD => {
            dispatch_native_vm_call(
                runtime,
                module,
                frame,
                caller_depth,
                kind,
                target_name,
                normalized_target_name,
                resolved_function_pou_id,
                receiver_value,
                &vm_args,
            )
        }
        _ => Err(VmTrap::InvalidNativeCallKind(kind)),
    };
    if result.is_ok() && runtime.deadline_exceeded() {
        return Err(VmTrap::DeadlineExceeded);
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn dispatch_native_vm_call(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    frame: &mut VmFrame,
    caller_depth: u32,
    kind: u32,
    target_name: &SmolStr,
    normalized_target_name: &SmolStr,
    resolved_function_pou_id: Option<u32>,
    receiver_value: Option<Value>,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    match kind {
        NATIVE_CALL_KIND_FUNCTION => {
            let pou_id = resolved_function_pou_id.ok_or_else(|| {
                VmTrap::Runtime(RuntimeError::UndefinedFunction(target_name.clone()))
            })?;
            execute_native_vm_pou_call(runtime, module, frame, pou_id, None, caller_depth, args)
        }
        NATIVE_CALL_KIND_FUNCTION_BLOCK => {
            let Some(Value::Instance(instance_id)) = receiver_value else {
                return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
            };
            let instance_type_name = runtime
                .storage()
                .get_instance(instance_id)
                .ok_or(VmTrap::Runtime(RuntimeError::NullReference))?
                .type_name
                .clone();
            let type_key = SmolStr::new(instance_type_name.to_ascii_uppercase());
            let pou_id = module
                .function_block_ids
                .get(&type_key)
                .copied()
                .ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::UndefinedFunctionBlock(
                        instance_type_name.clone(),
                    ))
                })?;
            if let Some(kind) = fbs::builtin_kind_uppercase(type_key.as_str()) {
                execute_native_builtin_function_block_call(
                    runtime,
                    frame,
                    instance_id,
                    &instance_type_name,
                    type_key.as_str(),
                    kind,
                    args,
                )?;
            } else {
                execute_native_vm_function_block_call(
                    runtime,
                    module,
                    frame,
                    pou_id,
                    instance_id,
                    caller_depth,
                    args,
                )?;
            }
            Ok(Value::Null)
        }
        NATIVE_CALL_KIND_METHOD => {
            let Some(Value::Instance(instance_id)) = receiver_value else {
                return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
            };
            let instance = runtime
                .storage()
                .get_instance(instance_id)
                .ok_or(VmTrap::Runtime(RuntimeError::NullReference))?;
            let type_key = SmolStr::new(instance.type_name.to_ascii_uppercase());
            let owner_pou_id = module
                .function_block_ids
                .get(&type_key)
                .copied()
                .or_else(|| module.class_ids.get(&type_key).copied())
                .ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::UndefinedField(target_name.clone()))
                })?;
            let pou_id = module
                .resolve_method_pou_id_uppercase(owner_pou_id, normalized_target_name.as_str())
                .ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::UndefinedField(target_name.clone()))
                })?;
            execute_native_vm_pou_call(
                runtime,
                module,
                frame,
                pou_id,
                Some(instance_id),
                caller_depth,
                args,
            )
        }
        _ => Err(VmTrap::InvalidNativeCallKind(kind)),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_native_vm_pou_call(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    caller_frame: &mut VmFrame,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    caller_depth: u32,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    let bindings::BoundVmCall {
        locals: initial_locals,
        out_bindings,
        present,
    } = bind_vm_call_arguments(runtime, module, caller_frame, pou_id, args)?;
    let capture_return = module.pou_has_return_slot(pou_id);
    let result = execute_vm_target(
        runtime,
        module,
        caller_frame,
        pou_id,
        entry_instance,
        Some(initial_locals.as_slice()),
        Some(&present),
        capture_return,
        caller_depth.saturating_add(1),
    )?;

    let mut prepared_outputs = Vec::with_capacity(out_bindings.len());
    for binding in out_bindings {
        let value = result
            .locals
            .get(binding.slot)
            .map(|value| {
                clone_value_with_profile(runtime, value, RegisterValueOpKind::OutputValueClone)
            })
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

#[allow(clippy::too_many_arguments)]
fn execute_native_vm_function_block_call(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    caller_frame: &mut VmFrame,
    pou_id: u32,
    instance_id: InstanceId,
    caller_depth: u32,
    args: &[VmNativeArg],
) -> Result<(), VmTrap> {
    runtime.record_call_op(RegisterCallOpKind::FunctionBlockCallEntry);
    let out_bindings =
        bind_vm_function_block_arguments(runtime, module, caller_frame, pou_id, instance_id, args)?;
    let edge_transaction =
        super::edge::EdgeInputTransaction::begin(runtime, module, pou_id, Some(instance_id))
            .map_err(VmTrap::Runtime)?;
    let execution_result = execute_vm_target(
        runtime,
        module,
        caller_frame,
        pou_id,
        Some(instance_id),
        None,
        None,
        false,
        caller_depth.saturating_add(1),
    );
    if let Some(edge_transaction) = edge_transaction {
        edge_transaction.restore(runtime);
    }
    execution_result?;

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
                runtime.record_value_op(RegisterValueOpKind::OutputValueClone);
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

    Ok(())
}

fn execute_native_builtin_function_block_call(
    runtime: &mut impl ExecutionContext,
    caller_frame: &mut VmFrame,
    instance_id: InstanceId,
    fb_type_name: &SmolStr,
    fb_type_key: &str,
    kind: fbs::BuiltinFbKind,
    args: &[VmNativeArg],
) -> Result<(), VmTrap> {
    runtime.record_call_op(RegisterCallOpKind::FunctionBlockCallEntry);
    let out_bindings = bind_builtin_function_block_arguments(
        runtime,
        caller_frame,
        fb_type_name,
        fb_type_key,
        instance_id,
        args,
    )?;
    runtime
        .check_builtin_call(instance_id)
        .map_err(VmTrap::Runtime)?;
    let now = runtime.current_time();
    fbs::execute_builtin_in_storage(runtime.storage_mut(), now, instance_id, kind)
        .map_err(VmTrap::Runtime)?;

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
                runtime.record_value_op(RegisterValueOpKind::OutputValueClone);
            }
            value
        };
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

    Ok(())
}

fn native_receiver_count(kind: u32) -> Result<usize, VmTrap> {
    match kind {
        NATIVE_CALL_KIND_FUNCTION | NATIVE_CALL_KIND_STDLIB => Ok(0),
        NATIVE_CALL_KIND_FUNCTION_BLOCK | NATIVE_CALL_KIND_METHOD => Ok(1),
        _ => Err(VmTrap::InvalidNativeCallKind(kind)),
    }
}

/// Preserve caller activation identity while the shared dispatcher runs a callee.
#[allow(clippy::too_many_arguments)]
fn execute_vm_target(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    caller_frame: &mut VmFrame,
    pou_id: u32,
    instance: Option<InstanceId>,
    initial_locals: Option<&[Value]>,
    parameter_values_present: Option<&[bool]>,
    capture_return: bool,
    depth: u32,
) -> Result<super::dispatch::VmPouStackResult, VmTrap> {
    runtime
        .suspend_frame(caller_frame)
        .map_err(VmTrap::Runtime)?;
    let result = (|| {
        if let Some(result) = runtime.try_execute_optimized(
            module,
            pou_id,
            instance,
            initial_locals,
            capture_return,
            depth,
            super::budget::ExecutionEntry::Nested,
        )? {
            Ok(result)
        } else {
            super::dispatch::execute_pou_stack_with_parameter_presence(
                runtime,
                module,
                pou_id,
                instance,
                initial_locals,
                parameter_values_present,
                capture_return,
                depth,
                super::budget::ExecutionEntry::Nested,
            )
        }
    })();
    // Resume even when execution failed, so caller storage is never left parked.
    runtime
        .resume_frame(caller_frame)
        .map_err(VmTrap::Runtime)?;
    result.map_err(VmTrap::from)
}
