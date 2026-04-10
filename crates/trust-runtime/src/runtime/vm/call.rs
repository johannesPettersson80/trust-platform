use smol_str::SmolStr;

use crate::bytecode::{
    NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_FUNCTION_BLOCK, NATIVE_CALL_KIND_METHOD,
    NATIVE_CALL_KIND_STDLIB,
};
use crate::error::RuntimeError;
use crate::memory::{FrameId, InstanceId, MemoryLocation};
use crate::stdlib::{conversions, fbs, time, StdParams};
use crate::value::{RefSegment, Value, ValueRef};

use super::errors::VmTrap;
use super::frames::{FrameStack, VmFrame};
use super::stack::OperandStack;
use super::{VmModule, VmNativeArgSpec, VmNativeSymbolSpec};

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

#[derive(Debug, Clone)]
struct VmNativeArg {
    name: Option<SmolStr>,
    value: VmNativeArgValue,
}

#[derive(Debug, Clone)]
enum VmNativeArgValue {
    Expr(Value),
    Target(ValueRef),
}

#[derive(Debug, Clone)]
struct VmOutBinding {
    slot: usize,
    target: ValueRef,
}

#[derive(Debug, Clone)]
struct VmFbOutBinding {
    source: ValueRef,
    target: ValueRef,
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
    let (target_name, arg_specs) = module.native_symbol_spec(symbol_idx)?;
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

    let mut payload = Vec::with_capacity(total);
    for _ in 0..total {
        payload.push(operand_stack.pop()?);
    }
    payload.reverse();

    let receiver_value = if receiver_count == 1 {
        Some(payload.remove(0))
    } else {
        None
    };

    let mut vm_args = Vec::with_capacity(arg_specs.len());
    for (spec, value) in arg_specs.iter().zip(payload) {
        let value = if spec.is_target {
            let Value::Reference(Some(reference)) = value else {
                return Err(VmTrap::InvalidNativeCall(
                    format!(
                        "target argument '{}' requires reference payload",
                        spec.name.as_deref().unwrap_or("<positional>")
                    )
                    .into(),
                ));
            };
            VmNativeArgValue::Target(reference)
        } else {
            VmNativeArgValue::Expr(value)
        };
        vm_args.push(VmNativeArg {
            name: spec.name.clone(),
            value,
        });
    }

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
        NATIVE_CALL_KIND_STDLIB => {
            dispatch_native_stdlib_call(runtime, frame, target_name, &vm_args)
        }
        NATIVE_CALL_KIND_FUNCTION | NATIVE_CALL_KIND_FUNCTION_BLOCK | NATIVE_CALL_KIND_METHOD => {
            dispatch_native_vm_call(
                runtime,
                module,
                frame,
                caller_depth,
                shared_budget,
                kind,
                target_name,
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
    receiver_value: Option<Value>,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    match kind {
        NATIVE_CALL_KIND_FUNCTION => {
            let key = SmolStr::new(target_name.to_ascii_uppercase());
            let pou_id = module.function_ids.get(&key).copied().ok_or_else(|| {
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
            let key = SmolStr::new(instance_type_name.to_ascii_uppercase());
            let pou_id = module
                .function_block_ids
                .get(&key)
                .copied()
                .ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::UndefinedFunctionBlock(
                        instance_type_name.clone(),
                    ))
                })?;
            if let Some(kind) = fbs::builtin_kind(instance_type_name.as_str()) {
                execute_native_builtin_function_block_call(
                    runtime,
                    frame,
                    instance_id,
                    &instance_type_name,
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
                .resolve_method_pou_id(owner_pou_id, target_name.as_str())
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
        let value = result.locals.get(binding.slot).cloned().ok_or_else(|| {
            VmTrap::InvalidNativeCall(
                format!("native call output slot {} out of bounds", binding.slot).into(),
            )
        })?;
        write_vm_reference(runtime, caller_frame, &binding.target, value)?;
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
        let value = runtime
            .storage
            .read_by_ref(binding.source.clone())
            .cloned()
            .ok_or(VmTrap::Runtime(RuntimeError::NullReference))?;
        write_vm_reference(runtime, caller_frame, &binding.target, value)?;
    }

    Ok(())
}

fn execute_native_builtin_function_block_call(
    runtime: &mut super::super::core::Runtime,
    caller_frame: &mut VmFrame,
    instance_id: InstanceId,
    fb_type_name: &SmolStr,
    kind: fbs::BuiltinFbKind,
    args: &[VmNativeArg],
) -> Result<(), VmTrap> {
    let out_bindings = bind_builtin_function_block_arguments(
        runtime,
        caller_frame,
        fb_type_name,
        instance_id,
        args,
    )?;
    let now = runtime.current_time();
    fbs::execute_builtin_in_storage(&mut runtime.storage, now, instance_id, kind)
        .map_err(VmTrap::Runtime)?;

    for binding in out_bindings {
        let value = runtime
            .storage
            .read_by_ref(binding.source.clone())
            .cloned()
            .ok_or(VmTrap::Runtime(RuntimeError::NullReference))?;
        write_vm_reference(runtime, caller_frame, &binding.target, value)?;
    }

    Ok(())
}

fn bind_builtin_function_block_arguments(
    runtime: &mut super::super::core::Runtime,
    caller_frame: &VmFrame,
    fb_type_name: &SmolStr,
    instance_id: InstanceId,
    args: &[VmNativeArg],
) -> Result<Vec<VmFbOutBinding>, VmTrap> {
    let key = SmolStr::new(fb_type_name.to_ascii_uppercase());
    let params = runtime
        .function_blocks()
        .get(&key)
        .ok_or_else(|| VmTrap::Runtime(RuntimeError::UndefinedFunctionBlock(fb_type_name.clone())))?
        .params
        .clone();
    let positional = args.iter().all(|arg| arg.name.is_none());
    let mut positional_index = 0usize;
    let mut consumed = vec![false; args.len()];
    let mut out_bindings = Vec::new();

    for param in &params {
        let arg_index = if positional {
            let next = (positional_index < args.len()).then_some(positional_index);
            if next.is_some() {
                positional_index = positional_index.saturating_add(1);
            }
            next
        } else {
            args.iter().position(|arg| {
                arg.name
                    .as_ref()
                    .map(|name| name.eq_ignore_ascii_case(param.name.as_str()))
                    .unwrap_or(false)
            })
        };
        if let Some(index) = arg_index {
            consumed[index] = true;
        }
        let arg = arg_index.and_then(|index| args.get(index));
        let field_ref = runtime
            .storage
            .ref_for_instance_recursive(instance_id, param.name.as_str())
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::UndefinedField(param.name.clone())))?;

        match param.direction {
            trust_hir::symbols::ParamDirection::In => {
                let value = match arg {
                    Some(arg) => resolve_vm_arg_value(runtime, caller_frame, arg)?,
                    None => runtime
                        .storage
                        .read_by_ref(field_ref.clone())
                        .map(default_like_value)
                        .unwrap_or(Value::Null),
                };
                if !runtime.storage.write_by_ref(field_ref.clone(), value) {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
            }
            trust_hir::symbols::ParamDirection::Out => {
                if let Some(arg) = arg {
                    out_bindings.push(VmFbOutBinding {
                        source: field_ref.clone(),
                        target: require_output_target(arg)?,
                    });
                }
            }
            trust_hir::symbols::ParamDirection::InOut => {
                let Some(arg) = arg else {
                    continue;
                };
                let target = require_output_target(arg)?;
                let value = read_vm_reference(runtime, caller_frame, &target)?;
                if !runtime.storage.write_by_ref(field_ref.clone(), value) {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
                out_bindings.push(VmFbOutBinding {
                    source: field_ref.clone(),
                    target,
                });
            }
        }
    }

    if positional {
        if positional_index < args.len() {
            return Err(VmTrap::InvalidNativeCall(
                format!(
                    "too many positional arguments: expected at most {}, got {}",
                    params.len(),
                    args.len()
                )
                .into(),
            ));
        }
    } else {
        for (index, consumed) in consumed.iter().enumerate() {
            if !consumed {
                let name = args[index]
                    .name
                    .as_deref()
                    .unwrap_or("<positional>")
                    .to_owned();
                return Err(VmTrap::InvalidNativeCall(
                    format!("unexpected named argument '{name}'").into(),
                ));
            }
        }
    }

    Ok(out_bindings)
}

fn bind_vm_function_block_arguments(
    runtime: &mut super::super::core::Runtime,
    module: &VmModule,
    caller_frame: &VmFrame,
    pou_id: u32,
    instance_id: InstanceId,
    args: &[VmNativeArg],
) -> Result<Vec<VmFbOutBinding>, VmTrap> {
    let params = module.pou_params(pou_id).ok_or_else(|| {
        VmTrap::InvalidNativeCall(format!("missing parameter metadata for pou id {pou_id}").into())
    })?;
    let positional = args.iter().all(|arg| arg.name.is_none());
    let mut positional_index = 0usize;
    let mut consumed = vec![false; args.len()];
    let mut out_bindings = Vec::new();

    for param in params {
        let arg_index = if positional {
            let next = (positional_index < args.len()).then_some(positional_index);
            if next.is_some() {
                positional_index = positional_index.saturating_add(1);
            }
            next
        } else {
            args.iter().position(|arg| {
                arg.name
                    .as_ref()
                    .map(|name| name.eq_ignore_ascii_case(param.name.as_str()))
                    .unwrap_or(false)
            })
        };
        if let Some(index) = arg_index {
            consumed[index] = true;
        }
        let arg = arg_index.and_then(|index| args.get(index));
        let field_ref = runtime
            .storage
            .ref_for_instance_recursive(instance_id, param.name.as_str())
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::UndefinedField(param.name.clone())))?;

        match param.direction {
            0 => {
                let value = match arg {
                    Some(arg) => resolve_vm_arg_value(runtime, caller_frame, arg)?,
                    None => {
                        if let Some(default_const_idx) = param.default_const_idx {
                            module
                                .consts
                                .get(default_const_idx as usize)
                                .cloned()
                                .ok_or(VmTrap::InvalidConstIndex(default_const_idx))?
                        } else {
                            runtime
                                .storage
                                .read_by_ref(field_ref.clone())
                                .map(default_like_value)
                                .unwrap_or(Value::Null)
                        }
                    }
                };
                if !runtime.storage.write_by_ref(field_ref.clone(), value) {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
            }
            1 => {
                if let Some(arg) = arg {
                    out_bindings.push(VmFbOutBinding {
                        source: field_ref.clone(),
                        target: require_output_target(arg)?,
                    });
                }
            }
            2 => {
                let Some(arg) = arg else {
                    continue;
                };
                let target = require_output_target(arg)?;
                let value = read_vm_reference(runtime, caller_frame, &target)?;
                if !runtime.storage.write_by_ref(field_ref.clone(), value) {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
                out_bindings.push(VmFbOutBinding {
                    source: field_ref.clone(),
                    target,
                });
            }
            other => {
                return Err(VmTrap::InvalidNativeCall(
                    format!("invalid parameter direction {other}").into(),
                ));
            }
        }
    }

    if positional {
        if positional_index < args.len() {
            return Err(VmTrap::InvalidNativeCall(
                format!(
                    "too many positional arguments: expected at most {}, got {}",
                    params.len(),
                    args.len()
                )
                .into(),
            ));
        }
    } else {
        for (index, consumed) in consumed.iter().enumerate() {
            if !consumed {
                let name = args[index]
                    .name
                    .as_deref()
                    .unwrap_or("<positional>")
                    .to_owned();
                return Err(VmTrap::InvalidNativeCall(
                    format!("unexpected named argument '{name}'").into(),
                ));
            }
        }
    }

    Ok(out_bindings)
}

fn default_like_value(value: &Value) -> Value {
    match value {
        Value::Bool(_) => Value::Bool(false),
        Value::SInt(_) => Value::SInt(0),
        Value::Int(_) => Value::Int(0),
        Value::DInt(_) => Value::DInt(0),
        Value::LInt(_) => Value::LInt(0),
        Value::USInt(_) => Value::USInt(0),
        Value::UInt(_) => Value::UInt(0),
        Value::UDInt(_) => Value::UDInt(0),
        Value::ULInt(_) => Value::ULInt(0),
        Value::Real(_) => Value::Real(0.0),
        Value::LReal(_) => Value::LReal(0.0),
        Value::Byte(_) => Value::Byte(0),
        Value::Word(_) => Value::Word(0),
        Value::DWord(_) => Value::DWord(0),
        Value::LWord(_) => Value::LWord(0),
        Value::String(_) => Value::String(SmolStr::new("")),
        Value::WString(_) => Value::WString(String::new()),
        Value::Char(_) => Value::Char(0),
        Value::WChar(_) => Value::WChar(0),
        _ => Value::Null,
    }
}

fn bind_vm_call_arguments(
    runtime: &super::super::core::Runtime,
    module: &VmModule,
    caller_frame: &VmFrame,
    pou_id: u32,
    args: &[VmNativeArg],
) -> Result<(Vec<Value>, Vec<VmOutBinding>), VmTrap> {
    let pou = module.pou(pou_id).ok_or(VmTrap::MissingPou(pou_id))?;
    let params = module.pou_params(pou_id).ok_or_else(|| {
        VmTrap::InvalidNativeCall(format!("missing parameter metadata for pou id {pou_id}").into())
    })?;
    let mut locals = vec![Value::Null; pou.local_ref_count as usize];
    let mut out_bindings = Vec::new();
    let return_slots = usize::from(module.pou_has_return_slot(pou_id));
    let positional = args.iter().all(|arg| arg.name.is_none());
    let mut positional_index = 0usize;
    let mut consumed = vec![false; args.len()];

    for (index, param) in params.iter().enumerate() {
        let slot = return_slots + index;
        if slot >= locals.len() {
            return Err(VmTrap::InvalidNativeCall(
                format!(
                    "parameter slot overflow for pou id {pou_id}: slot={slot} locals={}",
                    locals.len()
                )
                .into(),
            ));
        }
        let arg_index = if positional {
            let next = (positional_index < args.len()).then_some(positional_index);
            if next.is_some() {
                positional_index = positional_index.saturating_add(1);
            }
            next
        } else {
            args.iter().position(|arg| {
                arg.name
                    .as_ref()
                    .map(|name| name.eq_ignore_ascii_case(param.name.as_str()))
                    .unwrap_or(false)
            })
        };
        if let Some(arg_index) = arg_index {
            consumed[arg_index] = true;
        }
        let arg = arg_index.and_then(|arg_index| args.get(arg_index));

        match param.direction {
            0 => {
                let value = match arg {
                    Some(VmNativeArg {
                        value: VmNativeArgValue::Expr(value),
                        ..
                    }) => value.clone(),
                    Some(VmNativeArg {
                        value: VmNativeArgValue::Target(reference),
                        ..
                    }) => read_vm_reference(runtime, caller_frame, reference)?,
                    None => {
                        if let Some(default_const_idx) = param.default_const_idx {
                            module
                                .consts
                                .get(default_const_idx as usize)
                                .cloned()
                                .ok_or(VmTrap::InvalidConstIndex(default_const_idx))?
                        } else {
                            Value::Null
                        }
                    }
                };
                locals[slot] = value;
            }
            1 => {
                locals[slot] = Value::Null;
                if let Some(arg) = arg {
                    let VmNativeArgValue::Target(reference) = &arg.value else {
                        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
                    };
                    out_bindings.push(VmOutBinding {
                        slot,
                        target: reference.clone(),
                    });
                }
            }
            2 => {
                let Some(arg) = arg else {
                    return Err(VmTrap::InvalidNativeCall(
                        format!("missing IN_OUT argument '{}'", param.name).into(),
                    ));
                };
                let VmNativeArgValue::Target(reference) = &arg.value else {
                    return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
                };
                locals[slot] = read_vm_reference(runtime, caller_frame, reference)?;
                out_bindings.push(VmOutBinding {
                    slot,
                    target: reference.clone(),
                });
            }
            other => {
                return Err(VmTrap::InvalidNativeCall(
                    format!("invalid parameter direction {other}").into(),
                ));
            }
        }
    }

    if positional {
        if positional_index < args.len() {
            return Err(VmTrap::InvalidNativeCall(
                format!(
                    "too many positional arguments: expected at most {}, got {}",
                    params.len(),
                    args.len()
                )
                .into(),
            ));
        }
    } else {
        for (index, consumed) in consumed.iter().enumerate() {
            if !consumed {
                let name = args[index]
                    .name
                    .as_deref()
                    .unwrap_or("<positional>")
                    .to_owned();
                return Err(VmTrap::InvalidNativeCall(
                    format!("unexpected named argument '{name}'").into(),
                ));
            }
        }
    }

    Ok((locals, out_bindings))
}

fn dispatch_native_stdlib_call(
    runtime: &mut super::super::core::Runtime,
    frame: &mut VmFrame,
    target_name: &SmolStr,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    let key = SmolStr::new(target_name.to_ascii_uppercase());
    if time::is_split_name(key.as_str()) {
        return dispatch_native_split_call(runtime, frame, key.as_str(), args);
    }
    let stdlib = runtime.stdlib();
    if let Some(entry) = stdlib.get(key.as_str()) {
        let values = bind_stdlib_values(runtime, frame, &entry.params, args)?;
        return (entry.func)(&values).map_err(VmTrap::Runtime);
    }
    if conversions::is_conversion_name(key.as_str()) {
        let params = StdParams::Fixed(vec![SmolStr::new("IN")]);
        let values = bind_stdlib_values(runtime, frame, &params, args)?;
        return stdlib.call(key.as_str(), &values).map_err(VmTrap::Runtime);
    }
    Err(VmTrap::Runtime(RuntimeError::UndefinedFunction(
        target_name.clone(),
    )))
}

fn dispatch_native_split_call(
    runtime: &mut super::super::core::Runtime,
    frame: &mut VmFrame,
    name: &str,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    let params: &[&str] = match name {
        "SPLIT_DATE" => &["IN", "YEAR", "MONTH", "DAY"],
        "SPLIT_TOD" | "SPLIT_LTOD" => &["IN", "HOUR", "MINUTE", "SECOND", "MILLISECOND"],
        "SPLIT_DT" | "SPLIT_LDT" => &[
            "IN",
            "YEAR",
            "MONTH",
            "DAY",
            "HOUR",
            "MINUTE",
            "SECOND",
            "MILLISECOND",
        ],
        _ => {
            return Err(VmTrap::Runtime(RuntimeError::UndefinedFunction(
                name.into(),
            )))
        }
    };

    let (input, outputs) = bind_split_vm_args(runtime, frame, params, args)?;
    match name {
        "SPLIT_DATE" => {
            let (year, month, day) = time::split_date(&input, runtime.profile)?;
            write_output_int(runtime, frame, &outputs[0], year)?;
            write_output_int(runtime, frame, &outputs[1], month)?;
            write_output_int(runtime, frame, &outputs[2], day)?;
        }
        "SPLIT_TOD" => {
            let (hour, minute, second, millis) = time::split_tod(&input, runtime.profile)?;
            write_output_int(runtime, frame, &outputs[0], hour)?;
            write_output_int(runtime, frame, &outputs[1], minute)?;
            write_output_int(runtime, frame, &outputs[2], second)?;
            write_output_int(runtime, frame, &outputs[3], millis)?;
        }
        "SPLIT_LTOD" => {
            let (hour, minute, second, millis) = time::split_ltod(&input)?;
            write_output_int(runtime, frame, &outputs[0], hour)?;
            write_output_int(runtime, frame, &outputs[1], minute)?;
            write_output_int(runtime, frame, &outputs[2], second)?;
            write_output_int(runtime, frame, &outputs[3], millis)?;
        }
        "SPLIT_DT" => {
            let (year, month, day, hour, minute, second, millis) =
                time::split_dt(&input, runtime.profile)?;
            write_output_int(runtime, frame, &outputs[0], year)?;
            write_output_int(runtime, frame, &outputs[1], month)?;
            write_output_int(runtime, frame, &outputs[2], day)?;
            write_output_int(runtime, frame, &outputs[3], hour)?;
            write_output_int(runtime, frame, &outputs[4], minute)?;
            write_output_int(runtime, frame, &outputs[5], second)?;
            write_output_int(runtime, frame, &outputs[6], millis)?;
        }
        "SPLIT_LDT" => {
            let (year, month, day, hour, minute, second, millis) = time::split_ldt(&input)?;
            write_output_int(runtime, frame, &outputs[0], year)?;
            write_output_int(runtime, frame, &outputs[1], month)?;
            write_output_int(runtime, frame, &outputs[2], day)?;
            write_output_int(runtime, frame, &outputs[3], hour)?;
            write_output_int(runtime, frame, &outputs[4], minute)?;
            write_output_int(runtime, frame, &outputs[5], second)?;
            write_output_int(runtime, frame, &outputs[6], millis)?;
        }
        _ => {}
    }
    Ok(Value::Null)
}

fn bind_split_vm_args(
    runtime: &super::super::core::Runtime,
    frame: &VmFrame,
    params: &[&str],
    args: &[VmNativeArg],
) -> Result<(Value, Vec<ValueRef>), VmTrap> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    if positional {
        if args.len() != params.len() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            }));
        }
        let mut outputs = Vec::with_capacity(params.len().saturating_sub(1));
        let input = resolve_vm_arg_value(runtime, frame, &args[0])?;
        for arg in &args[1..] {
            outputs.push(require_output_target(arg)?);
        }
        return Ok((input, outputs));
    }

    if args.iter().any(|arg| arg.name.is_none()) {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
            "<unnamed>".into(),
        )));
    }
    if args.len() != params.len() {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        }));
    }

    let mut assigned: Vec<Option<&VmNativeArg>> = vec![None; params.len()];
    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                "<unnamed>".into(),
            )));
        };
        let key = name.to_ascii_uppercase();
        let position = params
            .iter()
            .position(|param| param.eq_ignore_ascii_case(&key))
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::InvalidArgumentName(name.clone())))?;
        if assigned[position].is_some() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                name.clone(),
            )));
        }
        assigned[position] = Some(arg);
    }

    let input = assigned[0]
        .ok_or({
            VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            })
        })
        .and_then(|arg| resolve_vm_arg_value(runtime, frame, arg))?;
    let mut outputs = Vec::with_capacity(params.len().saturating_sub(1));
    for arg in assigned.into_iter().skip(1) {
        let arg = arg.ok_or({
            VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            })
        })?;
        outputs.push(require_output_target(arg)?);
    }
    Ok((input, outputs))
}

fn require_output_target(arg: &VmNativeArg) -> Result<ValueRef, VmTrap> {
    match &arg.value {
        VmNativeArgValue::Target(reference) => Ok(reference.clone()),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

fn write_output_int(
    runtime: &mut super::super::core::Runtime,
    frame: &mut VmFrame,
    reference: &ValueRef,
    value: i64,
) -> Result<(), VmTrap> {
    let current = read_vm_reference(runtime, frame, reference)?;
    let converted = match current {
        Value::SInt(_) => Value::SInt(i8::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::Int(_) => Value::Int(i16::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::DInt(_) => Value::DInt(i32::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::LInt(_) => Value::LInt(value),
        Value::USInt(_) => {
            if value < 0 {
                return Err(VmTrap::Runtime(RuntimeError::Overflow));
            }
            Value::USInt(u8::try_from(value).map_err(|_| RuntimeError::Overflow)?)
        }
        Value::UInt(_) => {
            if value < 0 {
                return Err(VmTrap::Runtime(RuntimeError::Overflow));
            }
            Value::UInt(u16::try_from(value).map_err(|_| RuntimeError::Overflow)?)
        }
        Value::UDInt(_) => {
            if value < 0 {
                return Err(VmTrap::Runtime(RuntimeError::Overflow));
            }
            Value::UDInt(u32::try_from(value).map_err(|_| RuntimeError::Overflow)?)
        }
        Value::ULInt(_) => {
            if value < 0 {
                return Err(VmTrap::Runtime(RuntimeError::Overflow));
            }
            Value::ULInt(value as u64)
        }
        _ => return Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    };
    write_vm_reference(runtime, frame, reference, converted)
}

fn resolve_vm_arg_value(
    runtime: &super::super::core::Runtime,
    frame: &VmFrame,
    arg: &VmNativeArg,
) -> Result<Value, VmTrap> {
    match &arg.value {
        VmNativeArgValue::Expr(value) => Ok(value.clone()),
        VmNativeArgValue::Target(reference) => read_vm_reference(runtime, frame, reference),
    }
}

fn bind_stdlib_values(
    runtime: &super::super::core::Runtime,
    frame: &VmFrame,
    params: &StdParams,
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    if positional {
        return bind_stdlib_positional_values(runtime, frame, params, args);
    }
    if args.iter().any(|arg| arg.name.is_none()) {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
            "<unnamed>".into(),
        )));
    }
    bind_stdlib_named_values(runtime, frame, params, args)
}

fn bind_stdlib_positional_values(
    runtime: &super::super::core::Runtime,
    frame: &VmFrame,
    params: &StdParams,
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    match params {
        StdParams::Fixed(expected) => {
            if args.len() != expected.len() {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                    expected: expected.len(),
                    got: args.len(),
                }));
            }
        }
        StdParams::Variadic { fixed, min, .. } => {
            let expected = fixed.len() + *min;
            if args.len() < expected {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                    expected,
                    got: args.len(),
                }));
            }
        }
    }
    let mut values = Vec::with_capacity(args.len());
    for arg in args {
        values.push(resolve_vm_arg_value(runtime, frame, arg)?);
    }
    Ok(values)
}

fn bind_stdlib_named_values(
    runtime: &super::super::core::Runtime,
    frame: &VmFrame,
    params: &StdParams,
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    match params {
        StdParams::Fixed(params) => bind_stdlib_named_values_fixed(runtime, frame, params, args),
        StdParams::Variadic {
            fixed,
            prefix,
            start,
            min,
        } => bind_stdlib_named_values_variadic(runtime, frame, fixed, prefix, *start, *min, args),
    }
}

fn bind_stdlib_named_values_fixed(
    runtime: &super::super::core::Runtime,
    frame: &VmFrame,
    params: &[SmolStr],
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    if args.len() != params.len() {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: params.len(),
            got: args.len(),
        }));
    }

    let mut values: Vec<Option<Value>> = vec![None; params.len()];
    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                "<unnamed>".into(),
            )));
        };
        let key = name.to_ascii_uppercase();
        let position = params
            .iter()
            .position(|param| param.as_str() == key)
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::InvalidArgumentName(name.clone())))?;
        if values[position].is_some() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                name.clone(),
            )));
        }
        values[position] = Some(resolve_vm_arg_value(runtime, frame, arg)?);
    }

    let mut resolved = Vec::with_capacity(values.len());
    for value in values {
        let Some(value) = value else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: params.len(),
                got: args.len(),
            }));
        };
        resolved.push(value);
    }
    Ok(resolved)
}

fn bind_stdlib_named_values_variadic(
    runtime: &super::super::core::Runtime,
    frame: &VmFrame,
    fixed: &[SmolStr],
    prefix: &SmolStr,
    start: usize,
    min: usize,
    args: &[VmNativeArg],
) -> Result<Vec<Value>, VmTrap> {
    let mut fixed_values: Vec<Option<Value>> = vec![None; fixed.len()];
    let mut variadic_values: Vec<Option<Value>> = Vec::new();
    let mut max_index: Option<usize> = None;

    for arg in args {
        let Some(name) = arg.name.as_ref() else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                "<unnamed>".into(),
            )));
        };
        let key = name.to_ascii_uppercase();
        if let Some(position) = fixed.iter().position(|param| param.as_str() == key) {
            if fixed_values[position].is_some() {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                    name.clone(),
                )));
            }
            fixed_values[position] = Some(resolve_vm_arg_value(runtime, frame, arg)?);
            continue;
        }

        let prefix_str = prefix.as_str();
        if let Some(suffix) = key.strip_prefix(prefix_str) {
            if suffix.is_empty() {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                    name.clone(),
                )));
            }
            let index = suffix
                .parse::<usize>()
                .map_err(|_| VmTrap::Runtime(RuntimeError::InvalidArgumentName(name.clone())))?;
            if index < start {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                    name.clone(),
                )));
            }
            let offset = index - start;
            if variadic_values.len() <= offset {
                variadic_values.resize(offset + 1, None);
            }
            if variadic_values[offset].is_some() {
                return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
                    name.clone(),
                )));
            }
            variadic_values[offset] = Some(resolve_vm_arg_value(runtime, frame, arg)?);
            max_index = Some(max_index.map_or(offset, |max| max.max(offset)));
            continue;
        }

        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentName(
            name.clone(),
        )));
    }

    for value in &fixed_values {
        if value.is_none() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + min,
                got: args.len(),
            }));
        }
    }

    let count = max_index.map(|idx| idx + 1).unwrap_or(0);
    if count < min {
        return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
            expected: fixed.len() + min,
            got: args.len(),
        }));
    }
    for idx in 0..count {
        if variadic_values
            .get(idx)
            .and_then(|value| value.as_ref())
            .is_none()
        {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + count,
                got: args.len(),
            }));
        }
    }

    let mut resolved = Vec::with_capacity(fixed.len() + count);
    for value in fixed_values {
        let Some(value) = value else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + count,
                got: args.len(),
            }));
        };
        resolved.push(value);
    }
    for value in variadic_values.into_iter().take(count) {
        let Some(value) = value else {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: fixed.len() + count,
                got: args.len(),
            }));
        };
        resolved.push(value);
    }
    Ok(resolved)
}

fn native_receiver_count(kind: u32) -> Result<usize, VmTrap> {
    match kind {
        NATIVE_CALL_KIND_FUNCTION | NATIVE_CALL_KIND_STDLIB => Ok(0),
        NATIVE_CALL_KIND_FUNCTION_BLOCK | NATIVE_CALL_KIND_METHOD => Ok(1),
        _ => Err(VmTrap::InvalidNativeCallKind(kind)),
    }
}

pub(super) fn preparse_native_symbol_spec(symbol: &SmolStr) -> VmNativeSymbolSpec {
    match parse_native_symbol(symbol) {
        Ok((target_name, arg_specs)) => VmNativeSymbolSpec::Parsed {
            target_name,
            arg_specs,
        },
        Err(err) => VmNativeSymbolSpec::ParseError(err),
    }
}

fn parse_native_symbol(symbol: &SmolStr) -> Result<(SmolStr, Vec<VmNativeArgSpec>), SmolStr> {
    let mut parts = symbol.split('|');
    let target = SmolStr::new(parts.next().unwrap_or_default());
    let mut args = Vec::new();
    for raw in parts {
        if raw.is_empty() {
            return Err("empty CALL_NATIVE arg token".into());
        }
        let (is_target, suffix) = if let Some(rest) = raw.strip_prefix('E') {
            (false, rest)
        } else if let Some(rest) = raw.strip_prefix('T') {
            (true, rest)
        } else {
            return Err("CALL_NATIVE arg token must start with E/T".into());
        };
        let name = if suffix.is_empty() {
            None
        } else if let Some(named) = suffix.strip_prefix(':') {
            if named.is_empty() {
                return Err("CALL_NATIVE named token missing argument name".into());
            }
            Some(SmolStr::new(named))
        } else {
            return Err("CALL_NATIVE arg token suffix must be ':NAME'".into());
        };
        args.push(VmNativeArgSpec { name, is_target });
    }
    Ok((target, args))
}

fn is_vm_local_sentinel(reference: &ValueRef) -> bool {
    matches!(
        reference.location,
        MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID))
    )
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use smol_str::SmolStr;

    use super::super::VmNativeSymbolSpec;
    use super::preparse_native_symbol_spec;

    #[test]
    fn preparse_native_symbol_spec_parses_named_and_target_args() {
        let entry = preparse_native_symbol_spec(&SmolStr::new("Add|E:a|T:out"));
        match entry {
            VmNativeSymbolSpec::Parsed {
                target_name,
                arg_specs,
            } => {
                assert_eq!(target_name, SmolStr::new("Add"));
                assert_eq!(arg_specs.len(), 2);
                assert_eq!(arg_specs[0].name.as_deref(), Some("a"));
                assert!(!arg_specs[0].is_target);
                assert_eq!(arg_specs[1].name.as_deref(), Some("out"));
                assert!(arg_specs[1].is_target);
            }
            VmNativeSymbolSpec::ParseError(err) => {
                panic!("unexpected parse error: {err}");
            }
        }
    }

    #[test]
    fn preparse_native_symbol_spec_preserves_parse_error_message() {
        let entry = preparse_native_symbol_spec(&SmolStr::new("Add|Q:oops"));
        match entry {
            VmNativeSymbolSpec::ParseError(err) => {
                assert!(err.contains("must start with E/T"));
            }
            VmNativeSymbolSpec::Parsed { .. } => {
                panic!("expected parse error");
            }
        }
    }
}

fn read_vm_reference(
    runtime: &super::super::core::Runtime,
    caller_frame: &VmFrame,
    reference: &ValueRef,
) -> Result<Value, VmTrap> {
    if is_vm_local_sentinel(reference) {
        let root = caller_frame.locals.get(reference.offset).ok_or_else(|| {
            VmTrap::InvalidNativeCall(
                format!(
                    "local reference offset {} out of range for VM frame (locals={})",
                    reference.offset,
                    caller_frame.locals.len()
                )
                .into(),
            )
        })?;
        return read_by_ref_path(root, &reference.path)
            .cloned()
            .ok_or(VmTrap::Runtime(RuntimeError::NullReference));
    }
    runtime
        .storage
        .read_by_ref(reference.clone())
        .cloned()
        .ok_or(VmTrap::Runtime(RuntimeError::NullReference))
}

fn write_vm_reference(
    runtime: &mut super::super::core::Runtime,
    caller_frame: &mut VmFrame,
    reference: &ValueRef,
    value: Value,
) -> Result<(), VmTrap> {
    if is_vm_local_sentinel(reference) {
        let local_count = caller_frame.locals.len();
        let Some(slot) = caller_frame.locals.get_mut(reference.offset) else {
            return Err(VmTrap::InvalidNativeCall(
                format!(
                    "local reference offset {} out of range for VM frame (locals={local_count})",
                    reference.offset,
                )
                .into(),
            ));
        };
        if write_by_ref_path(slot, &reference.path, value) {
            return Ok(());
        }
        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
    }
    if runtime.storage.write_by_ref(reference.clone(), value) {
        Ok(())
    } else {
        Err(VmTrap::Runtime(RuntimeError::NullReference))
    }
}

fn read_by_ref_path<'a>(value: &'a Value, path: &[RefSegment]) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }
    match &path[0] {
        RefSegment::Field(name) => match value {
            Value::Struct(struct_value) => struct_value
                .fields
                .get(name.as_str())
                .and_then(|field| read_by_ref_path(field, &path[1..])),
            _ => None,
        },
        RefSegment::Index(indices) => match value {
            Value::Array(array) => {
                let offset = array_offset_i64(&array.dimensions, indices)?;
                array
                    .elements
                    .get(offset)
                    .and_then(|element| read_by_ref_path(element, &path[1..]))
            }
            _ => None,
        },
    }
}

fn write_by_ref_path(target: &mut Value, path: &[RefSegment], value: Value) -> bool {
    if path.is_empty() {
        *target = value;
        return true;
    }
    match &path[0] {
        RefSegment::Field(name) => match target {
            Value::Struct(struct_value) => struct_value
                .fields
                .get_mut(name.as_str())
                .map(|field| write_by_ref_path(field, &path[1..], value))
                .unwrap_or(false),
            _ => false,
        },
        RefSegment::Index(indices) => match target {
            Value::Array(array) => {
                let offset = match array_offset_i64(&array.dimensions, indices) {
                    Some(offset) => offset,
                    None => return false,
                };
                array
                    .elements
                    .get_mut(offset)
                    .map(|element| write_by_ref_path(element, &path[1..], value))
                    .unwrap_or(false)
            }
            _ => false,
        },
    }
}

fn array_offset_i64(dimensions: &[(i64, i64)], indices: &[i64]) -> Option<usize> {
    if dimensions.len() != indices.len() {
        return None;
    }
    let mut offset: i128 = 0;
    let mut stride: i128 = 1;
    for ((lower, upper), index) in dimensions.iter().zip(indices).rev() {
        if index < lower || index > upper {
            return None;
        }
        let len = (*upper - *lower + 1) as i128;
        offset += (index - *lower) as i128 * stride;
        stride *= len;
    }
    usize::try_from(offset).ok()
}
