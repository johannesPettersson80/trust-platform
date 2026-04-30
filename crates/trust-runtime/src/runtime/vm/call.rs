use std::collections::HashMap;

use smol_str::SmolStr;

use crate::bytecode::{
    NATIVE_CALL_KIND_FUNCTION, NATIVE_CALL_KIND_FUNCTION_BLOCK, NATIVE_CALL_KIND_METHOD,
    NATIVE_CALL_KIND_STDLIB,
};
use crate::error::RuntimeError;
use crate::memory::{FrameId, InstanceId, MemoryLocation};
use crate::stdlib::{conversions, fbs, time, StdParams};
use crate::value::{
    materialize_value_path, read_value_path_borrowed, write_value_path, Value, ValueRef,
};

use super::errors::VmTrap;
use super::frames::{FrameStack, VmFrame};
use super::register_ir::{RegisterCallOpKind, RegisterValueOpKind};
use super::stack::OperandStack;
use super::{materialize_borrowed_value, VmModule, VmNativeArgSpec, VmNativeSymbolSpec};

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
    target: VmWriteTarget,
}

#[derive(Debug, Clone)]
struct VmFbOutBinding {
    source: VmFbOutSource,
    target: VmWriteTarget,
}

#[derive(Debug, Clone)]
enum VmFbOutSource {
    Direct {
        instance_id: InstanceId,
        offset: usize,
    },
    Reference(ValueRef),
}

impl VmFbOutSource {
    fn read<'a>(&self, runtime: &'a super::super::core::Runtime) -> Option<&'a Value> {
        match self {
            Self::Direct {
                instance_id,
                offset,
            } => runtime
                .storage
                .read_instance_field_by_offset(*instance_id, *offset),
            Self::Reference(reference) => runtime.storage.read_by_ref_ref(reference),
        }
    }
}

#[derive(Debug, Clone)]
enum VmWriteTarget {
    CallerLocalDirect {
        offset: usize,
    },
    DirectStorage {
        location: MemoryLocation,
        offset: usize,
    },
    Reference(ValueRef),
}

impl VmWriteTarget {
    fn from_reference(reference: &ValueRef) -> Self {
        if is_vm_local_sentinel(reference) && reference.path.is_empty() {
            return Self::CallerLocalDirect {
                offset: reference.offset,
            };
        }
        if reference.path.is_empty() {
            match reference.location {
                MemoryLocation::Global | MemoryLocation::Local(_) | MemoryLocation::Instance(_) => {
                    return Self::DirectStorage {
                        location: reference.location,
                        offset: reference.offset,
                    };
                }
                MemoryLocation::Io(_) | MemoryLocation::Retain => {}
            }
        }
        Self::Reference(reference.clone())
    }

    fn peek<'a>(
        &self,
        runtime: &'a super::super::core::Runtime,
        caller_frame: &'a VmFrame,
    ) -> Result<&'a Value, VmTrap> {
        match self {
            Self::CallerLocalDirect { offset } => {
                caller_frame.locals.get(*offset).ok_or_else(|| {
                    VmTrap::InvalidNativeCall(
                        format!(
                            "local reference offset {} out of range for VM frame (locals={})",
                            offset,
                            caller_frame.locals.len()
                        )
                        .into(),
                    )
                })
            }
            Self::DirectStorage { location, offset } => runtime
                .storage
                .read_direct_slot_by_location(*location, *offset)
                .ok_or(VmTrap::Runtime(RuntimeError::NullReference)),
            Self::Reference(reference) => peek_vm_reference(runtime, caller_frame, reference),
        }
    }

    fn read(
        &self,
        runtime: &mut super::super::core::Runtime,
        caller_frame: &VmFrame,
    ) -> Result<Value, VmTrap> {
        match self {
            Self::Reference(reference) => read_vm_reference(runtime, caller_frame, reference),
            Self::CallerLocalDirect { .. } | Self::DirectStorage { .. } => {
                let value = {
                    let value = self.peek(runtime, caller_frame)?;
                    let (value, cloned) = materialize_borrowed_value(value);
                    if cloned {
                        runtime
                            .vm_register_profile
                            .record_value_op(RegisterValueOpKind::ReadValueClone);
                    }
                    value
                };
                Ok(value)
            }
        }
    }

    fn write(
        &self,
        runtime: &mut super::super::core::Runtime,
        caller_frame: &mut VmFrame,
        value: Value,
    ) -> Result<(), VmTrap> {
        match self {
            Self::CallerLocalDirect { offset } => {
                let local_count = caller_frame.locals.len();
                let Some(slot) = caller_frame.locals.get_mut(*offset) else {
                    return Err(VmTrap::InvalidNativeCall(
                        format!(
                            "local reference offset {} out of range for VM frame (locals={local_count})",
                            offset,
                        )
                        .into(),
                    ));
                };
                *slot = value;
                Ok(())
            }
            Self::DirectStorage { location, offset } => runtime
                .storage
                .write_direct_slot_by_location(*location, *offset, value)
                .then_some(())
                .ok_or(VmTrap::Runtime(RuntimeError::NullReference)),
            Self::Reference(reference) => {
                write_vm_reference(runtime, caller_frame, reference, value)
            }
        }
    }
}

#[derive(Debug, Clone)]
enum VmFbFieldBinding {
    Direct {
        instance_id: InstanceId,
        offset: usize,
    },
    Reference(ValueRef),
}

impl VmFbFieldBinding {
    fn resolve(
        runtime: &super::super::core::Runtime,
        instance_id: InstanceId,
        field_name: &SmolStr,
    ) -> Result<Self, VmTrap> {
        if let Some(offset) = runtime
            .storage
            .declared_instance_field_offset(instance_id, field_name.as_str())
        {
            return Ok(Self::Direct {
                instance_id,
                offset,
            });
        }

        runtime
            .storage
            .ref_for_instance_recursive(instance_id, field_name.as_str())
            .map(Self::Reference)
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::UndefinedField(field_name.clone())))
    }

    fn read<'a>(&self, runtime: &'a super::super::core::Runtime) -> Option<&'a Value> {
        match self {
            Self::Direct {
                instance_id,
                offset,
            } => runtime
                .storage
                .read_instance_field_by_offset(*instance_id, *offset),
            Self::Reference(reference) => runtime.storage.read_by_ref_ref(reference),
        }
    }

    fn write(&self, runtime: &mut super::super::core::Runtime, value: Value) -> bool {
        match self {
            Self::Direct {
                instance_id,
                offset,
            } => runtime
                .storage
                .write_instance_field_by_offset(*instance_id, *offset, value),
            Self::Reference(reference) => runtime.storage.write_by_ref_ref(reference, value),
        }
    }

    fn out_source(&self) -> VmFbOutSource {
        match self {
            Self::Direct {
                instance_id,
                offset,
            } => VmFbOutSource::Direct {
                instance_id: *instance_id,
                offset: *offset,
            },
            Self::Reference(reference) => VmFbOutSource::Reference(reference.clone()),
        }
    }
}

fn clone_value_with_profile(
    runtime: &mut super::super::core::Runtime,
    value: &Value,
    kind: RegisterValueOpKind,
) -> Value {
    let (value, cloned) = materialize_borrowed_value(value);
    if cloned {
        runtime.vm_register_profile.record_value_op(kind);
    }
    value
}

fn unpack_native_call_payload(
    operand_stack: &mut OperandStack,
    arg_specs: &[VmNativeArgSpec],
    receiver_count: usize,
) -> Result<(Option<Value>, Vec<VmNativeArg>), VmTrap> {
    let total = arg_specs.len().saturating_add(receiver_count);
    let mut payload = Vec::with_capacity(total);
    for _ in 0..total {
        payload.push(operand_stack.pop()?);
    }

    let receiver_value = if receiver_count == 1 {
        Some(payload.pop().ok_or_else(|| {
            VmTrap::InvalidNativeCall("missing function-block/method receiver payload".into())
        })?)
    } else {
        None
    };

    let mut vm_args = Vec::with_capacity(arg_specs.len());
    for spec in arg_specs {
        let value = payload.pop().ok_or_else(|| {
            VmTrap::InvalidNativeCall("missing native call payload while decoding args".into())
        })?;
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

    Ok((receiver_value, vm_args))
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
    let (target_name, normalized_target_name, resolved_function_pou_id, arg_specs) = match spec {
        VmNativeSymbolSpec::Parsed {
            target_name,
            normalized_target_name,
            resolved_function_pou_id,
            arg_specs,
        } => (
            target_name,
            normalized_target_name,
            *resolved_function_pou_id,
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

fn resolve_named_arg_index(
    args: &[VmNativeArg],
    consumed: &[bool],
    param_name: &SmolStr,
    ordered_named_index: &mut usize,
) -> Option<usize> {
    while *ordered_named_index < args.len() && consumed[*ordered_named_index] {
        *ordered_named_index += 1;
    }

    if let Some(arg) = args.get(*ordered_named_index) {
        if arg
            .name
            .as_ref()
            .map(|name| name.eq_ignore_ascii_case(param_name.as_str()))
            .unwrap_or(false)
        {
            let index = *ordered_named_index;
            *ordered_named_index += 1;
            return Some(index);
        }
    }

    args.iter().enumerate().find_map(|(index, arg)| {
        (!consumed[index]
            && arg
                .name
                .as_ref()
                .map(|name| name.eq_ignore_ascii_case(param_name.as_str()))
                .unwrap_or(false))
        .then_some(index)
    })
}

fn bind_builtin_function_block_arguments(
    runtime: &mut super::super::core::Runtime,
    caller_frame: &VmFrame,
    fb_type_name: &SmolStr,
    fb_type_key: &str,
    instance_id: InstanceId,
    args: &[VmNativeArg],
) -> Result<Vec<VmFbOutBinding>, VmTrap> {
    let params = runtime
        .function_blocks()
        .get(fb_type_key)
        .ok_or_else(|| VmTrap::Runtime(RuntimeError::UndefinedFunctionBlock(fb_type_name.clone())))?
        .params
        .clone();
    let positional = args.iter().all(|arg| arg.name.is_none());
    let mut positional_index = 0usize;
    let mut ordered_named_index = 0usize;
    let mut consumed = vec![false; args.len()];
    let mut out_bindings = Vec::new();

    for param in &params {
        runtime
            .vm_register_profile
            .record_call_op(RegisterCallOpKind::ParameterBinding);
        let arg_index = if positional {
            let next = (positional_index < args.len()).then_some(positional_index);
            if next.is_some() {
                positional_index = positional_index.saturating_add(1);
            }
            next
        } else {
            resolve_named_arg_index(args, &consumed, &param.name, &mut ordered_named_index)
        };
        if let Some(index) = arg_index {
            consumed[index] = true;
        }
        let arg = arg_index.and_then(|index| args.get(index));
        if matches!(
            param.direction,
            trust_hir::symbols::ParamDirection::Out | trust_hir::symbols::ParamDirection::InOut
        ) && arg.is_none()
        {
            continue;
        }
        let field_binding = VmFbFieldBinding::resolve(runtime, instance_id, &param.name)?;

        match param.direction {
            trust_hir::symbols::ParamDirection::In => {
                let value = match arg {
                    Some(arg) => resolve_vm_arg_value(runtime, caller_frame, arg)?,
                    None => {
                        if let Some(value) = field_binding.read(runtime) {
                            let (value, cloned) = materialize_borrowed_value(value);
                            if cloned {
                                runtime
                                    .vm_register_profile
                                    .record_value_op(RegisterValueOpKind::ReadValueClone);
                            }
                            value
                        } else {
                            Value::Null
                        }
                    }
                };
                if !field_binding.write(runtime, value) {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
            }
            trust_hir::symbols::ParamDirection::Out => {
                if let Some(arg) = arg {
                    out_bindings.push(VmFbOutBinding {
                        source: field_binding.out_source(),
                        target: require_output_target(arg)?,
                    });
                }
            }
            trust_hir::symbols::ParamDirection::InOut => {
                let Some(arg) = arg else {
                    continue;
                };
                let target = require_output_target(arg)?;
                let value = target.read(runtime, caller_frame)?;
                if !field_binding.write(runtime, value) {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
                out_bindings.push(VmFbOutBinding {
                    source: field_binding.out_source(),
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
    let mut ordered_named_index = 0usize;
    let mut consumed = vec![false; args.len()];
    let mut out_bindings = Vec::new();

    for param in params {
        runtime
            .vm_register_profile
            .record_call_op(RegisterCallOpKind::ParameterBinding);
        let arg_index = if positional {
            let next = (positional_index < args.len()).then_some(positional_index);
            if next.is_some() {
                positional_index = positional_index.saturating_add(1);
            }
            next
        } else {
            resolve_named_arg_index(args, &consumed, &param.name, &mut ordered_named_index)
        };
        if let Some(index) = arg_index {
            consumed[index] = true;
        }
        let arg = arg_index.and_then(|index| args.get(index));
        if matches!(param.direction, 1 | 2) && arg.is_none() {
            continue;
        }
        let field_binding = VmFbFieldBinding::resolve(runtime, instance_id, &param.name)?;

        match param.direction {
            0 => {
                let value = match arg {
                    Some(arg) => resolve_vm_arg_value(runtime, caller_frame, arg)?,
                    None => {
                        if let Some(value) = field_binding.read(runtime) {
                            let (value, cloned) = materialize_borrowed_value(value);
                            if cloned {
                                runtime
                                    .vm_register_profile
                                    .record_value_op(RegisterValueOpKind::ReadValueClone);
                            }
                            value
                        } else if let Some(default_const_idx) = param.default_const_idx {
                            let value = module
                                .consts
                                .get(default_const_idx as usize)
                                .ok_or(VmTrap::InvalidConstIndex(default_const_idx))?;
                            let (value, cloned) = materialize_borrowed_value(value);
                            if cloned {
                                runtime
                                    .vm_register_profile
                                    .record_value_op(RegisterValueOpKind::ConstLoadClone);
                            }
                            value
                        } else {
                            Value::Null
                        }
                    }
                };
                if !field_binding.write(runtime, value) {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
            }
            1 => {
                if let Some(arg) = arg {
                    out_bindings.push(VmFbOutBinding {
                        source: field_binding.out_source(),
                        target: require_output_target(arg)?,
                    });
                }
            }
            2 => {
                let Some(arg) = arg else {
                    continue;
                };
                let target = require_output_target(arg)?;
                let value = target.read(runtime, caller_frame)?;
                if !field_binding.write(runtime, value) {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
                out_bindings.push(VmFbOutBinding {
                    source: field_binding.out_source(),
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

fn bind_vm_call_arguments(
    runtime: &mut super::super::core::Runtime,
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
    let mut ordered_named_index = 0usize;
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
            resolve_named_arg_index(args, &consumed, &param.name, &mut ordered_named_index)
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
                    }) => clone_value_with_profile(
                        runtime,
                        value,
                        RegisterValueOpKind::BindingExprClone,
                    ),
                    Some(VmNativeArg {
                        value: VmNativeArgValue::Target(reference),
                        ..
                    }) => read_vm_target_value(runtime, caller_frame, reference)?,
                    None => {
                        if let Some(default_const_idx) = param.default_const_idx {
                            module
                                .consts
                                .get(default_const_idx as usize)
                                .map(|value| {
                                    clone_value_with_profile(
                                        runtime,
                                        value,
                                        RegisterValueOpKind::ConstLoadClone,
                                    )
                                })
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
                        target: VmWriteTarget::from_reference(reference),
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
                let target = VmWriteTarget::from_reference(reference);
                locals[slot] = target.read(runtime, caller_frame)?;
                out_bindings.push(VmOutBinding { slot, target });
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
    normalized_target_name: &SmolStr,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    if time::is_runtime_clock_name(normalized_target_name.as_str()) {
        if !args.is_empty() {
            return Err(VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: 0,
                got: args.len(),
            }));
        }
        return Ok(Value::Time(runtime.current_time()));
    }
    if time::is_split_name(normalized_target_name.as_str()) {
        return dispatch_native_split_call(runtime, frame, normalized_target_name.as_str(), args);
    }
    if let Some(entry) = runtime.stdlib().get(normalized_target_name.as_str()) {
        let params = entry.params.clone();
        let func = entry.func;
        let values = bind_stdlib_values(runtime, frame, &params, args)?;
        return func(&values).map_err(VmTrap::Runtime);
    }
    if conversions::is_conversion_name(normalized_target_name.as_str()) {
        let params = StdParams::Fixed(vec![SmolStr::new("IN")]);
        let values = bind_stdlib_values(runtime, frame, &params, args)?;
        return runtime
            .stdlib()
            .call(normalized_target_name.as_str(), &values)
            .map_err(VmTrap::Runtime);
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
    runtime: &mut super::super::core::Runtime,
    frame: &VmFrame,
    params: &[&str],
    args: &[VmNativeArg],
) -> Result<(Value, Vec<VmWriteTarget>), VmTrap> {
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

fn require_output_target(arg: &VmNativeArg) -> Result<VmWriteTarget, VmTrap> {
    match &arg.value {
        VmNativeArgValue::Target(reference) => Ok(VmWriteTarget::from_reference(reference)),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

fn write_output_int(
    runtime: &mut super::super::core::Runtime,
    frame: &mut VmFrame,
    target: &VmWriteTarget,
    value: i64,
) -> Result<(), VmTrap> {
    let current = target.peek(runtime, frame)?;
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
    target.write(runtime, frame, converted)
}

fn read_vm_target_value(
    runtime: &mut super::super::core::Runtime,
    frame: &VmFrame,
    reference: &ValueRef,
) -> Result<Value, VmTrap> {
    VmWriteTarget::from_reference(reference).read(runtime, frame)
}

fn resolve_vm_arg_value(
    runtime: &mut super::super::core::Runtime,
    frame: &VmFrame,
    arg: &VmNativeArg,
) -> Result<Value, VmTrap> {
    match &arg.value {
        VmNativeArgValue::Expr(value) => Ok(clone_value_with_profile(
            runtime,
            value,
            RegisterValueOpKind::BindingExprClone,
        )),
        VmNativeArgValue::Target(reference) => read_vm_target_value(runtime, frame, reference),
    }
}

fn bind_stdlib_values(
    runtime: &mut super::super::core::Runtime,
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
    runtime: &mut super::super::core::Runtime,
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
    runtime: &mut super::super::core::Runtime,
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
    runtime: &mut super::super::core::Runtime,
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
    runtime: &mut super::super::core::Runtime,
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
            normalized_target_name: SmolStr::new(target_name.to_ascii_uppercase()),
            resolved_function_pou_id: None,
            target_name,
            arg_specs,
        },
        Err(err) => VmNativeSymbolSpec::ParseError(err),
    }
}

pub(super) fn resolve_native_symbol_specs(
    specs: &mut [VmNativeSymbolSpec],
    function_ids: &HashMap<SmolStr, u32>,
) {
    for spec in specs {
        if let VmNativeSymbolSpec::Parsed {
            normalized_target_name,
            resolved_function_pou_id,
            ..
        } = spec
        {
            *resolved_function_pou_id = function_ids.get(normalized_target_name).copied();
        }
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
    use std::collections::{HashMap, HashSet};

    use indexmap::IndexMap;
    use smol_str::SmolStr;

    use crate::bytecode::TypeTable;
    use crate::error::RuntimeError;
    use crate::memory::{FrameId, MemoryLocation};
    use crate::value::{RefPath, RefSegment, StructValue, Value, ValueRef};
    use crate::Runtime;

    use super::super::frames::VmFrame;
    use super::super::stack::OperandStack;
    use super::super::{VmModule, VmNativeArgSpec, VmNativeSymbolSpec, VmParamMeta, VmPouEntry};
    use super::{
        preparse_native_symbol_spec, VmFbOutSource, VmWriteTarget, VM_LOCAL_SENTINEL_FRAME_ID,
    };

    fn manual_vm_function_block_module(params: Vec<VmParamMeta>) -> (VmModule, u32) {
        let pou_id = 1_u32;
        let mut pou_by_id = HashMap::new();
        pou_by_id.insert(
            pou_id,
            VmPouEntry {
                name: SmolStr::new("FB"),
                code_start: 0,
                code_end: 0,
                local_ref_start: 0,
                local_ref_count: 0,
                primary_instance_owner: None,
            },
        );
        let mut function_block_ids = HashMap::new();
        function_block_ids.insert(SmolStr::new("FB"), pou_id);
        let mut pou_params = HashMap::new();
        pou_params.insert(pou_id, params);
        (
            VmModule {
                code: Vec::new(),
                strings: Vec::new(),
                types: TypeTable::default(),
                refs: Vec::new(),
                consts: Vec::new(),
                pou_by_id,
                program_ids: HashMap::new(),
                function_ids: HashMap::new(),
                function_block_ids,
                class_ids: HashMap::new(),
                native_symbol_specs: Vec::new(),
                pou_params,
                pou_has_return_slot: HashSet::new(),
                method_table_by_owner: HashMap::new(),
                debug_map: super::super::debug_map::VmDebugMap::default(),
                instruction_budget: super::super::DEFAULT_INSTRUCTION_BUDGET,
            },
            pou_id,
        )
    }

    fn manual_vm_function_module(
        name: &str,
        params: Vec<VmParamMeta>,
        has_return_slot: bool,
    ) -> (VmModule, u32) {
        let pou_id = 1_u32;
        let mut pou_by_id = HashMap::new();
        pou_by_id.insert(
            pou_id,
            VmPouEntry {
                name: SmolStr::new(name),
                code_start: 0,
                code_end: 0,
                local_ref_start: 0,
                local_ref_count: params.len() as u32 + u32::from(has_return_slot),
                primary_instance_owner: None,
            },
        );
        let mut function_ids = HashMap::new();
        function_ids.insert(SmolStr::new(name.to_ascii_uppercase()), pou_id);
        let mut pou_params = HashMap::new();
        pou_params.insert(pou_id, params);
        let mut pou_has_return_slot = HashSet::new();
        if has_return_slot {
            pou_has_return_slot.insert(pou_id);
        }
        (
            VmModule {
                code: Vec::new(),
                strings: Vec::new(),
                types: TypeTable::default(),
                refs: Vec::new(),
                consts: Vec::new(),
                pou_by_id,
                program_ids: HashMap::new(),
                function_ids,
                function_block_ids: HashMap::new(),
                class_ids: HashMap::new(),
                native_symbol_specs: Vec::new(),
                pou_params,
                pou_has_return_slot,
                method_table_by_owner: HashMap::new(),
                debug_map: super::super::debug_map::VmDebugMap::default(),
                instruction_budget: super::super::DEFAULT_INSTRUCTION_BUDGET,
            },
            pou_id,
        )
    }

    fn expr_arg(name: Option<&str>, value: Value) -> super::VmNativeArg {
        super::VmNativeArg {
            name: name.map(SmolStr::new),
            value: super::VmNativeArgValue::Expr(value),
        }
    }

    fn target_arg(name: Option<&str>, reference: ValueRef) -> super::VmNativeArg {
        super::VmNativeArg {
            name: name.map(SmolStr::new),
            value: super::VmNativeArgValue::Target(reference),
        }
    }

    fn empty_caller_frame() -> VmFrame {
        VmFrame {
            pou_id: 0,
            return_pc: 0,
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            locals: vec![],
            runtime_instance: None,
            instance_owner: None,
        }
    }

    #[test]
    fn bind_vm_function_block_arguments_skips_omitted_out_without_field_resolution() {
        let mut runtime = Runtime::new();
        let instance = runtime.storage.create_instance("FB");
        let (module, pou_id) = manual_vm_function_block_module(vec![VmParamMeta {
            name: SmolStr::new("OUT"),
            direction: 1,
            default_const_idx: None,
        }]);
        let caller_frame = empty_caller_frame();

        let bindings = super::bind_vm_function_block_arguments(
            &mut runtime,
            &module,
            &caller_frame,
            pou_id,
            instance,
            &[],
        )
        .expect("omitted OUT should skip field binding resolution");

        assert!(bindings.is_empty());
    }

    #[test]
    fn bind_vm_function_block_arguments_skips_omitted_inout_without_field_resolution() {
        let mut runtime = Runtime::new();
        let instance = runtime.storage.create_instance("FB");
        let (module, pou_id) = manual_vm_function_block_module(vec![VmParamMeta {
            name: SmolStr::new("ACC"),
            direction: 2,
            default_const_idx: None,
        }]);
        let caller_frame = empty_caller_frame();

        let bindings = super::bind_vm_function_block_arguments(
            &mut runtime,
            &module,
            &caller_frame,
            pou_id,
            instance,
            &[],
        )
        .expect("omitted IN_OUT should skip field binding resolution");

        assert!(bindings.is_empty());
    }

    #[test]
    fn resolve_named_arg_index_prefers_in_order_next_argument() {
        let args = vec![
            super::VmNativeArg {
                name: Some(SmolStr::new("ENABLE")),
                value: super::VmNativeArgValue::Expr(Value::Bool(true)),
            },
            super::VmNativeArg {
                name: Some(SmolStr::new("VALUE")),
                value: super::VmNativeArgValue::Expr(Value::DInt(1)),
            },
        ];
        let consumed = vec![false, false];
        let mut ordered_named_index = 0usize;

        let first = super::resolve_named_arg_index(
            &args,
            &consumed,
            &SmolStr::new("Enable"),
            &mut ordered_named_index,
        );
        let second = super::resolve_named_arg_index(
            &args,
            &consumed,
            &SmolStr::new("Value"),
            &mut ordered_named_index,
        );

        assert_eq!(first, Some(0));
        assert_eq!(second, Some(1));
    }

    #[test]
    fn resolve_named_arg_index_handles_omitted_middle_parameter() {
        let args = vec![
            super::VmNativeArg {
                name: Some(SmolStr::new("ENABLE")),
                value: super::VmNativeArgValue::Expr(Value::Bool(true)),
            },
            super::VmNativeArg {
                name: Some(SmolStr::new("VALUE")),
                value: super::VmNativeArgValue::Expr(Value::DInt(1)),
            },
        ];
        let consumed = vec![false, false];
        let mut ordered_named_index = 0usize;

        let first = super::resolve_named_arg_index(
            &args,
            &consumed,
            &SmolStr::new("Enable"),
            &mut ordered_named_index,
        );
        let missing = super::resolve_named_arg_index(
            &args,
            &consumed,
            &SmolStr::new("Increment"),
            &mut ordered_named_index,
        );
        let second = super::resolve_named_arg_index(
            &args,
            &consumed,
            &SmolStr::new("Value"),
            &mut ordered_named_index,
        );

        assert_eq!(first, Some(0));
        assert_eq!(missing, None);
        assert_eq!(second, Some(1));
    }

    #[test]
    fn resolve_named_arg_index_falls_back_for_out_of_order_named_arguments() {
        let args = vec![
            super::VmNativeArg {
                name: Some(SmolStr::new("VALUE")),
                value: super::VmNativeArgValue::Expr(Value::DInt(1)),
            },
            super::VmNativeArg {
                name: Some(SmolStr::new("ENABLE")),
                value: super::VmNativeArgValue::Expr(Value::Bool(true)),
            },
        ];
        let mut consumed = vec![false, false];
        let mut ordered_named_index = 0usize;

        let enable = super::resolve_named_arg_index(
            &args,
            &consumed,
            &SmolStr::new("Enable"),
            &mut ordered_named_index,
        );
        consumed[enable.expect("enable index")] = true;
        let value = super::resolve_named_arg_index(
            &args,
            &consumed,
            &SmolStr::new("Value"),
            &mut ordered_named_index,
        );

        assert_eq!(enable, Some(1));
        assert_eq!(value, Some(0));
    }

    #[test]
    fn unpack_native_call_payload_preserves_receiver_and_argument_order() {
        let mut operand_stack = OperandStack::default();
        let target_ref = ValueRef {
            location: MemoryLocation::Global,
            offset: 3,
            path: RefPath::new(),
        };
        operand_stack
            .push(Value::Instance(crate::memory::InstanceId(7)))
            .expect("push receiver");
        operand_stack.push(Value::DInt(11)).expect("push expr arg");
        operand_stack
            .push(Value::Reference(Some(target_ref.clone())))
            .expect("push target arg");

        let (receiver, args) = super::unpack_native_call_payload(
            &mut operand_stack,
            &[
                VmNativeArgSpec {
                    name: Some(SmolStr::new("lhs")),
                    is_target: false,
                },
                VmNativeArgSpec {
                    name: Some(SmolStr::new("out")),
                    is_target: true,
                },
            ],
            1,
        )
        .expect("decode payload");

        assert_eq!(
            receiver,
            Some(Value::Instance(crate::memory::InstanceId(7)))
        );
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name.as_deref(), Some("lhs"));
        assert!(matches!(
            args[0].value,
            super::VmNativeArgValue::Expr(Value::DInt(11))
        ));
        assert_eq!(args[1].name.as_deref(), Some("out"));
        match &args[1].value {
            super::VmNativeArgValue::Target(reference) => assert_eq!(reference, &target_ref),
            super::VmNativeArgValue::Expr(_) => panic!("expected target arg"),
        }
    }

    #[test]
    fn preparse_native_symbol_spec_parses_named_and_target_args() {
        let entry = preparse_native_symbol_spec(&SmolStr::new("Add|E:a|T:out"));
        match entry {
            VmNativeSymbolSpec::Parsed {
                target_name,
                normalized_target_name,
                resolved_function_pou_id,
                arg_specs,
            } => {
                assert_eq!(target_name, SmolStr::new("Add"));
                assert_eq!(normalized_target_name, SmolStr::new("ADD"));
                assert_eq!(resolved_function_pou_id, None);
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
    fn resolve_native_symbol_specs_caches_resolved_function_id() {
        let mut specs = vec![
            preparse_native_symbol_spec(&SmolStr::new("Add|E:a")),
            preparse_native_symbol_spec(&SmolStr::new("Len|E:in")),
        ];
        let mut function_ids = HashMap::new();
        function_ids.insert(SmolStr::new("ADD"), 7);

        super::resolve_native_symbol_specs(&mut specs, &function_ids);

        match &specs[0] {
            VmNativeSymbolSpec::Parsed {
                resolved_function_pou_id,
                ..
            } => assert_eq!(*resolved_function_pou_id, Some(7)),
            VmNativeSymbolSpec::ParseError(err) => panic!("unexpected parse error: {err}"),
        }
        match &specs[1] {
            VmNativeSymbolSpec::Parsed {
                resolved_function_pou_id,
                ..
            } => assert_eq!(*resolved_function_pou_id, None),
            VmNativeSymbolSpec::ParseError(err) => panic!("unexpected parse error: {err}"),
        }
    }

    #[test]
    fn bind_vm_call_arguments_rejects_too_many_positional_arguments() {
        let mut runtime = Runtime::new();
        let (module, pou_id) = manual_vm_function_module(
            "DoWork",
            vec![VmParamMeta {
                name: SmolStr::new("IN"),
                direction: 0,
                default_const_idx: None,
            }],
            false,
        );
        let err = super::bind_vm_call_arguments(
            &mut runtime,
            &module,
            &empty_caller_frame(),
            pou_id,
            &[
                expr_arg(None, Value::DInt(1)),
                expr_arg(None, Value::DInt(2)),
            ],
        )
        .expect_err("extra positional argument should fail");

        match err {
            super::VmTrap::InvalidNativeCall(message) => {
                assert!(message.contains("too many positional arguments"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn bind_vm_call_arguments_keeps_omitted_middle_named_input_as_null() {
        let mut runtime = Runtime::new();
        let (module, pou_id) = manual_vm_function_module(
            "DoWork",
            vec![
                VmParamMeta {
                    name: SmolStr::new("A"),
                    direction: 0,
                    default_const_idx: None,
                },
                VmParamMeta {
                    name: SmolStr::new("B"),
                    direction: 0,
                    default_const_idx: None,
                },
                VmParamMeta {
                    name: SmolStr::new("C"),
                    direction: 0,
                    default_const_idx: None,
                },
            ],
            false,
        );
        let (locals, out_bindings) = super::bind_vm_call_arguments(
            &mut runtime,
            &module,
            &empty_caller_frame(),
            pou_id,
            &[
                expr_arg(Some("A"), Value::DInt(1)),
                expr_arg(Some("C"), Value::DInt(3)),
            ],
        )
        .expect("named omission should bind remaining params");

        assert!(out_bindings.is_empty());
        assert_eq!(locals, vec![Value::DInt(1), Value::Null, Value::DInt(3)]);
    }

    #[test]
    fn bind_stdlib_named_values_rejects_duplicate_named_argument() {
        let mut runtime = Runtime::new();
        let params = crate::stdlib::StdParams::Fixed(vec![SmolStr::new("IN"), SmolStr::new("N")]);
        let err = super::bind_stdlib_named_values(
            &mut runtime,
            &empty_caller_frame(),
            &params,
            &[
                expr_arg(Some("IN"), Value::DInt(1)),
                expr_arg(Some("IN"), Value::DInt(2)),
            ],
        )
        .expect_err("duplicate stdlib named arg should fail");

        assert!(matches!(
            err,
            super::VmTrap::Runtime(RuntimeError::InvalidArgumentName(name))
                if name == SmolStr::new("IN")
        ));
    }

    #[test]
    fn bind_stdlib_named_values_variadic_rejects_hole() {
        let mut runtime = Runtime::new();
        let params = crate::stdlib::StdParams::Variadic {
            fixed: vec![SmolStr::new("IN")],
            prefix: SmolStr::new("IN"),
            start: 2,
            min: 2,
        };
        let err = super::bind_stdlib_named_values(
            &mut runtime,
            &empty_caller_frame(),
            &params,
            &[
                expr_arg(Some("IN"), Value::DInt(1)),
                expr_arg(Some("IN2"), Value::DInt(2)),
                expr_arg(Some("IN4"), Value::DInt(4)),
            ],
        )
        .expect_err("variadic hole should fail");

        assert!(matches!(
            err,
            super::VmTrap::Runtime(RuntimeError::InvalidArgumentCount {
                expected: 4,
                got: 3,
            })
        ));
    }

    #[test]
    fn bind_vm_function_block_arguments_supports_mixed_out_and_inout_rebinding() {
        let mut runtime = Runtime::new();
        let instance = runtime.storage.create_instance("FB");
        assert!(runtime
            .storage
            .set_instance_var(instance, "OUT", crate::value::Value::DInt(0)));
        assert!(runtime
            .storage
            .set_instance_var(instance, "ACC", crate::value::Value::DInt(11)));
        runtime.storage.set_global("OUT", Value::DInt(0));
        runtime.storage.set_global("ACC_SRC", Value::DInt(5));
        let out_ref = runtime
            .storage
            .ref_for_global("OUT")
            .expect("out global ref");
        let acc_ref = runtime
            .storage
            .ref_for_global("ACC_SRC")
            .expect("acc source ref");
        let (module, pou_id) = manual_vm_function_block_module(vec![
            VmParamMeta {
                name: SmolStr::new("OUT"),
                direction: 1,
                default_const_idx: None,
            },
            VmParamMeta {
                name: SmolStr::new("ACC"),
                direction: 2,
                default_const_idx: None,
            },
        ]);

        let bindings = super::bind_vm_function_block_arguments(
            &mut runtime,
            &module,
            &empty_caller_frame(),
            pou_id,
            instance,
            &[
                target_arg(Some("OUT"), out_ref.clone()),
                target_arg(Some("ACC"), acc_ref.clone()),
            ],
        )
        .expect("mixed OUT and IN_OUT binding should succeed");

        assert_eq!(bindings.len(), 2);
    }

    #[test]
    fn vm_fb_out_source_reads_direct_instance_field() {
        let mut runtime = Runtime::new();
        let instance = runtime.storage.create_instance("FB");
        assert!(runtime
            .storage
            .set_instance_var(instance, "OUT", crate::value::Value::DInt(41)));

        let offset = runtime
            .storage
            .declared_instance_field_offset(instance, "OUT")
            .expect("declared OUT offset");
        let source = VmFbOutSource::Direct {
            instance_id: instance,
            offset,
        };

        assert!(matches!(
            source.read(&runtime).expect("direct out source read"),
            crate::value::Value::DInt(41)
        ));
    }

    #[test]
    fn vm_fb_out_source_reads_reference_field() {
        let mut runtime = Runtime::new();
        let instance = runtime.storage.create_instance("FB");
        assert!(runtime
            .storage
            .set_instance_var(instance, "OUT", crate::value::Value::DInt(17)));

        let reference = runtime
            .storage
            .ref_for_instance_recursive(instance, "OUT")
            .expect("reference OUT field");
        let source = VmFbOutSource::Reference(reference.clone());
        assert_eq!(reference.location, MemoryLocation::Instance(instance));
        assert!(matches!(
            source.read(&runtime).expect("reference out source read"),
            crate::value::Value::DInt(17)
        ));
    }

    #[test]
    fn vm_fb_field_binding_out_source_uses_direct_for_declared_fields() {
        let mut runtime = Runtime::new();
        let instance = runtime.storage.create_instance("FB");
        assert!(runtime
            .storage
            .set_instance_var(instance, "OUT", crate::value::Value::DInt(23)));

        let binding = super::VmFbFieldBinding::resolve(&runtime, instance, &SmolStr::new("OUT"))
            .expect("declared field binding");
        match binding.out_source() {
            VmFbOutSource::Direct {
                instance_id,
                offset,
            } => {
                assert_eq!(instance_id, instance);
                assert_eq!(offset, 0);
            }
            VmFbOutSource::Reference(_) => panic!("expected direct output source"),
        }
    }

    #[test]
    fn vm_fb_field_binding_out_source_falls_back_to_reference_for_inherited_fields() {
        let mut runtime = Runtime::new();
        let base = runtime.storage.create_instance("BASE");
        let derived = runtime.storage.create_instance("DERIVED");
        runtime
            .storage
            .get_instance_mut(derived)
            .expect("derived instance")
            .parent = Some(base);
        assert!(runtime
            .storage
            .set_instance_var(base, "OUT", crate::value::Value::DInt(29)));

        let binding = super::VmFbFieldBinding::resolve(&runtime, derived, &SmolStr::new("OUT"))
            .expect("inherited field binding");
        match binding.out_source() {
            VmFbOutSource::Reference(reference) => {
                assert_eq!(reference.location, MemoryLocation::Instance(base));
                assert_eq!(reference.offset, 0);
            }
            VmFbOutSource::Direct { .. } => panic!("expected inherited fallback reference"),
        }
    }

    #[test]
    fn vm_write_target_uses_direct_storage_for_empty_path_instance_refs() {
        let mut runtime = Runtime::new();
        let instance = runtime.storage.create_instance("FB");
        assert!(runtime
            .storage
            .set_instance_var(instance, "OUT", Value::DInt(31)));
        let reference = runtime
            .storage
            .ref_for_instance(instance, "OUT")
            .expect("instance output ref");
        let target = VmWriteTarget::from_reference(&reference);
        assert!(matches!(
            target.clone(),
            VmWriteTarget::DirectStorage {
                location: MemoryLocation::Instance(id),
                offset: 0
            } if id == instance
        ));

        let caller_frame = VmFrame {
            pou_id: 0,
            return_pc: 0,
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            locals: vec![],
            runtime_instance: None,
            instance_owner: None,
        };
        assert!(matches!(
            target
                .read(&mut runtime, &caller_frame)
                .expect("direct instance read"),
            Value::DInt(31)
        ));
    }

    #[test]
    fn vm_write_target_uses_direct_storage_for_empty_path_global_refs() {
        let mut runtime = Runtime::new();
        runtime.storage.set_global("OUT", Value::DInt(17));
        let reference = runtime
            .storage
            .ref_for_global("OUT")
            .expect("global output ref");
        let target = VmWriteTarget::from_reference(&reference);
        assert!(matches!(
            target.clone(),
            VmWriteTarget::DirectStorage {
                location: MemoryLocation::Global,
                offset: 0
            }
        ));

        let caller_frame = VmFrame {
            pou_id: 0,
            return_pc: 0,
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            locals: vec![],
            runtime_instance: None,
            instance_owner: None,
        };
        assert!(matches!(
            target
                .read(&mut runtime, &caller_frame)
                .expect("direct target read"),
            Value::DInt(17)
        ));
    }

    #[test]
    fn vm_write_target_uses_caller_local_direct_for_empty_path_vm_locals() {
        let mut runtime = Runtime::new();
        let reference = ValueRef {
            location: MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID)),
            offset: 0,
            path: RefPath::new(),
        };
        let target = VmWriteTarget::from_reference(&reference);
        assert!(matches!(
            target.clone(),
            VmWriteTarget::CallerLocalDirect { offset: 0 }
        ));

        let mut caller_frame = VmFrame {
            pou_id: 0,
            return_pc: 0,
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 1,
            locals: vec![Value::DInt(21)],
            runtime_instance: None,
            instance_owner: None,
        };
        assert!(matches!(
            target
                .read(&mut runtime, &caller_frame)
                .expect("local direct read"),
            Value::DInt(21)
        ));
        target
            .write(&mut runtime, &mut caller_frame, Value::DInt(42))
            .expect("local direct write");
        assert!(matches!(
            caller_frame.locals.first(),
            Some(&Value::DInt(42))
        ));
    }

    #[test]
    fn vm_write_target_keeps_nested_path_targets_on_reference_fallback() {
        let reference = ValueRef {
            location: MemoryLocation::Global,
            offset: 0,
            path: [RefSegment::Field(SmolStr::new("VALUE"))]
                .into_iter()
                .collect(),
        };
        let target = VmWriteTarget::from_reference(&reference);
        assert!(matches!(target.clone(), VmWriteTarget::Reference(_)));
    }

    #[test]
    fn read_vm_target_value_matches_generic_reference_path_across_reference_shapes() {
        let mut runtime = Runtime::new();
        runtime.storage.set_global("GLOBAL", Value::DInt(7));
        let mut struct_fields = IndexMap::new();
        struct_fields.insert(SmolStr::new("VALUE"), Value::DInt(8));
        runtime.storage.set_global(
            "STRUCT",
            Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
                SmolStr::new("TEST_STRUCT"),
                struct_fields,
            ))),
        );
        let instance = runtime.storage.create_instance("FB");
        assert!(runtime
            .storage
            .set_instance_var(instance, "ACC", Value::DInt(9)));

        let global_ref = runtime
            .storage
            .ref_for_global("GLOBAL")
            .expect("global ref");
        let struct_ref = runtime
            .storage
            .ref_for_global("STRUCT")
            .expect("struct ref");
        let nested_ref = ValueRef {
            location: struct_ref.location,
            offset: struct_ref.offset,
            path: [RefSegment::Field(SmolStr::new("VALUE"))]
                .into_iter()
                .collect(),
        };
        let instance_ref = runtime
            .storage
            .ref_for_instance(instance, "ACC")
            .expect("instance ref");

        let caller_frame = VmFrame {
            pou_id: 0,
            return_pc: 0,
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 1,
            locals: vec![Value::DInt(10)],
            runtime_instance: None,
            instance_owner: None,
        };
        let local_ref = ValueRef {
            location: MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID)),
            offset: 0,
            path: RefPath::new(),
        };

        for reference in [global_ref, nested_ref, instance_ref, local_ref] {
            let direct = super::read_vm_target_value(&mut runtime, &caller_frame, &reference)
                .expect("direct target value");
            let generic = super::read_vm_reference(&mut runtime, &caller_frame, &reference)
                .expect("generic target value");
            assert_eq!(direct, generic);
        }
    }

    #[test]
    fn write_output_int_inspects_target_type_without_read_clone() {
        let mut runtime = Runtime::new();
        runtime.storage.set_global("OUT", Value::DInt(0));
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let reference = runtime
            .storage
            .ref_for_global("OUT")
            .expect("global output ref");
        let target = super::VmWriteTarget::from_reference(&reference);
        let mut caller_frame = empty_caller_frame();

        super::write_output_int(&mut runtime, &mut caller_frame, &target, 17)
            .expect("write output int");

        assert_eq!(runtime.storage.get_global("OUT"), Some(&Value::DInt(17)));
        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(profile.value_ops.read_value_clones, 0);
    }

    #[test]
    fn read_vm_target_value_avoids_clone_counter_for_scalar_direct_target() {
        let mut runtime = Runtime::new();
        runtime.storage.set_global("OUT", Value::DInt(23));
        runtime.set_vm_register_profile_enabled(true);
        runtime.reset_vm_register_profile();

        let reference = runtime
            .storage
            .ref_for_global("OUT")
            .expect("global output ref");
        let caller_frame = empty_caller_frame();

        let value = super::read_vm_target_value(&mut runtime, &caller_frame, &reference)
            .expect("read target value");

        assert_eq!(value, Value::DInt(23));
        let profile = runtime.vm_register_profile_snapshot();
        assert_eq!(profile.value_ops.read_value_clones, 0);
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

fn peek_vm_reference<'a>(
    runtime: &'a super::super::core::Runtime,
    caller_frame: &'a VmFrame,
    reference: &ValueRef,
) -> Result<&'a Value, VmTrap> {
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
        return read_value_path_borrowed(root, &reference.path)
            .ok_or(VmTrap::Runtime(RuntimeError::NullReference));
    }
    runtime
        .storage
        .read_by_ref_ref(reference)
        .ok_or(VmTrap::Runtime(RuntimeError::NullReference))
}

fn read_vm_reference(
    runtime: &mut super::super::core::Runtime,
    caller_frame: &VmFrame,
    reference: &ValueRef,
) -> Result<Value, VmTrap> {
    let value = if is_vm_local_sentinel(reference) {
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
        if let Some(value) = read_value_path_borrowed(root, &reference.path) {
            let (value, cloned) = materialize_borrowed_value(value);
            if cloned {
                runtime
                    .vm_register_profile
                    .record_value_op(RegisterValueOpKind::ReadValueClone);
            }
            value
        } else {
            materialize_value_path(root, &reference.path)
                .ok_or(VmTrap::Runtime(RuntimeError::NullReference))?
        }
    } else if let Some(value) = runtime.storage.read_by_ref_ref(reference) {
        let (value, cloned) = materialize_borrowed_value(value);
        if cloned {
            runtime
                .vm_register_profile
                .record_value_op(RegisterValueOpKind::ReadValueClone);
        }
        value
    } else {
        runtime
            .storage
            .materialize_by_ref_ref(reference)
            .ok_or(VmTrap::Runtime(RuntimeError::NullReference))?
    };
    Ok(value)
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
        if write_value_path(slot, &reference.path, value) {
            return Ok(());
        }
        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
    }
    if runtime.storage.write_by_ref_ref(reference, value) {
        Ok(())
    } else {
        Err(VmTrap::Runtime(RuntimeError::NullReference))
    }
}
