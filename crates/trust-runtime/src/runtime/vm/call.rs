use smol_str::SmolStr;

use crate::bytecode::{
    NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_FUNCTION_BLOCK, NATIVE_CALL_KIND_METHOD,
    NATIVE_CALL_KIND_STDLIB,
};
use crate::error::RuntimeError;
use crate::memory::InstanceId;
use crate::stdlib::fbs;
use crate::value::Value;

use super::errors::VmTrap;
use super::frames::{FrameStack, VmFrame};
use super::register_ir::{RegisterCallOpKind, RegisterValueOpKind};
use super::stack::OperandStack;
use super::{materialize_borrowed_value, VmModule, VmNativeSymbolSpec};

mod bindings;
mod stdlib;
mod symbols;

#[cfg(test)]
mod tests;

use self::bindings::{
    bind_builtin_function_block_arguments, bind_vm_call_arguments,
    bind_vm_function_block_arguments, clone_value_with_profile, unpack_native_call_payload,
    VmNativeArg,
};
use self::stdlib::dispatch_native_stdlib_call;
pub(super) use self::symbols::{preparse_native_symbol_spec, resolve_native_symbol_specs};

#[cfg(test)]
use self::bindings::{
    read_vm_reference, read_vm_target_value, resolve_named_arg_index, write_output_int,
    VmFbFieldBinding, VmFbOutSource, VmNativeArgValue, VmWriteTarget,
};
#[cfg(test)]
use self::stdlib::{
    bind_conversion_value, bind_stdlib_named_values, bind_stdlib_positional_values,
};

pub(super) const VM_LOCAL_SENTINEL_FRAME_ID: u32 = u32::MAX;

pub(super) fn push_call_frame(
    frame_stack: &mut FrameStack,
    module: &VmModule,
    pou_id: u32,
    return_pc: usize,
    runtime_instance: Option<InstanceId>,
) -> Result<usize, VmTrap> {
    let pou = module.pou(pou_id).ok_or(VmTrap::MissingPou(pou_id))?;
    let local_count = pou.local_ref_count as usize;
    let frame = VmFrame {
        pou_id,
        return_pc,
        code_start: pou.code_start,
        code_end: pou.code_end,
        local_ref_start: pou.local_ref_start,
        local_ref_count: pou.local_ref_count,
        locals: vec![Value::Null; local_count],
        runtime_instance,
        instance_owner: pou.primary_instance_owner,
    };
    let entry_pc = frame.code_start;
    frame_stack.push(frame)?;
    Ok(entry_pc)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_native_call(
    runtime: &mut super::super::core::Runtime,
    module: &VmModule,
    frame: &mut VmFrame,
    operand_stack: &mut OperandStack,
    caller_depth: u32,
    shared_budget: &mut usize,
    kind: u32,
    symbol_idx: u32,
    arg_count: u32,
) -> Result<Value, VmTrap> {
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

    match kind {
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
                shared_budget,
                kind,
                target_name,
                normalized_target_name,
                resolved_function_pou_id,
                receiver_value,
                &vm_args,
            )
        }
        _ => Err(VmTrap::InvalidNativeCallKind(kind)),
    }
}

#[allow(clippy::too_many_arguments)]
fn dispatch_native_vm_call(
    runtime: &mut super::super::core::Runtime,
    module: &VmModule,
    frame: &mut VmFrame,
    caller_depth: u32,
    shared_budget: &mut usize,
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
            execute_native_vm_pou_call(
                runtime,
                module,
                frame,
                pou_id,
                None,
                caller_depth,
                shared_budget,
                args,
            )
        }
        NATIVE_CALL_KIND_FUNCTION_BLOCK => {
            let Some(Value::Instance(instance_id)) = receiver_value else {
                return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
            };
            let instance_type_name = runtime
                .storage
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
                    shared_budget,
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
                .storage
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
                shared_budget,
                args,
            )
        }
        _ => Err(VmTrap::InvalidNativeCallKind(kind)),
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_native_vm_pou_call(
    runtime: &mut super::super::core::Runtime,
    module: &VmModule,
    caller_frame: &mut VmFrame,
    pou_id: u32,
    entry_instance: Option<InstanceId>,
    caller_depth: u32,
    shared_budget: &mut usize,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    let (initial_locals, out_bindings) =
        bind_vm_call_arguments(runtime, module, caller_frame, pou_id, args)?;
    let capture_return = module.pou_has_return_slot(pou_id);
    let result = if let Some(result) =
        super::register_ir::try_execute_pou_with_register_ir_with_locals(
            runtime,
            module,
            pou_id,
            entry_instance,
            Some(initial_locals.as_slice()),
            capture_return,
            caller_depth.saturating_add(1),
            Some(shared_budget),
        )
        .map_err(VmTrap::from)?
    {
        super::dispatch::VmPouStackResult {
            return_value: result.return_value,
            locals: result.locals,
        }
    } else {
        super::dispatch::execute_pou_stack_with_locals(
            runtime,
            module,
            pou_id,
            entry_instance,
            Some(initial_locals.as_slice()),
            capture_return,
            caller_depth.saturating_add(1),
            Some(shared_budget),
        )
        .map_err(VmTrap::from)?
    };

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
            })?;
        binding.target.write(runtime, caller_frame, value)?;
    }

    Ok(result.return_value.unwrap_or(Value::Null))
}

#[allow(clippy::too_many_arguments)]
fn execute_native_vm_function_block_call(
    runtime: &mut super::super::core::Runtime,
    module: &VmModule,
    caller_frame: &mut VmFrame,
    pou_id: u32,
    instance_id: InstanceId,
    caller_depth: u32,
    shared_budget: &mut usize,
    args: &[VmNativeArg],
) -> Result<(), VmTrap> {
    runtime
        .vm_register_profile
        .record_call_op(RegisterCallOpKind::FunctionBlockCallEntry);
    let out_bindings =
        bind_vm_function_block_arguments(runtime, module, caller_frame, pou_id, instance_id, args)?;
    if super::register_ir::try_execute_pou_with_register_ir_with_locals(
        runtime,
        module,
        pou_id,
        Some(instance_id),
        None,
        false,
        caller_depth.saturating_add(1),
        Some(shared_budget),
    )
    .map_err(VmTrap::from)?
    .is_none()
    {
        super::dispatch::execute_pou_stack_with_locals(
            runtime,
            module,
            pou_id,
            Some(instance_id),
            None,
            false,
            caller_depth.saturating_add(1),
            Some(shared_budget),
        )
        .map_err(VmTrap::from)?;
    }

    for binding in out_bindings {
        runtime
            .vm_register_profile
            .record_call_op(RegisterCallOpKind::OutputCopyBack);
        let value = {
            let value = binding
                .source
                .read(runtime)
                .ok_or(VmTrap::Runtime(RuntimeError::NullReference))?;
            let (value, cloned) = materialize_borrowed_value(value);
            if cloned {
                runtime
                    .vm_register_profile
                    .record_value_op(RegisterValueOpKind::OutputValueClone);
            }
            value
        };
        binding.target.write(runtime, caller_frame, value)?;
    }

    Ok(())
}

fn execute_native_builtin_function_block_call(
    runtime: &mut super::super::core::Runtime,
    caller_frame: &mut VmFrame,
    instance_id: InstanceId,
    fb_type_name: &SmolStr,
    fb_type_key: &str,
    kind: fbs::BuiltinFbKind,
    args: &[VmNativeArg],
) -> Result<(), VmTrap> {
    runtime
        .vm_register_profile
        .record_call_op(RegisterCallOpKind::FunctionBlockCallEntry);
    let out_bindings = bind_builtin_function_block_arguments(
        runtime,
        caller_frame,
        fb_type_name,
        fb_type_key,
        instance_id,
        args,
    )?;
    let now = runtime.current_time();
    fbs::execute_builtin_in_storage(&mut runtime.storage, now, instance_id, kind)
        .map_err(VmTrap::Runtime)?;

    for binding in out_bindings {
        runtime
            .vm_register_profile
            .record_call_op(RegisterCallOpKind::OutputCopyBack);
        let value = {
            let value = binding
                .source
                .read(runtime)
                .ok_or(VmTrap::Runtime(RuntimeError::NullReference))?;
            let (value, cloned) = materialize_borrowed_value(value);
            if cloned {
                runtime
                    .vm_register_profile
                    .record_value_op(RegisterValueOpKind::OutputValueClone);
            }
            value
        };
        binding.target.write(runtime, caller_frame, value)?;
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
