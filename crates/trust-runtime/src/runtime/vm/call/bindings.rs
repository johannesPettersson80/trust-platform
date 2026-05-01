use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::{FrameId, InstanceId, MemoryLocation};
use crate::value::{
    materialize_value_path, read_value_path_borrowed, write_value_path, Value, ValueRef,
};

use super::super::errors::VmTrap;
use super::super::frames::VmFrame;
use super::super::register_ir::{RegisterCallOpKind, RegisterValueOpKind};
use super::super::stack::OperandStack;
use super::super::{materialize_borrowed_value, VmModule, VmNativeArgSpec};
use super::VM_LOCAL_SENTINEL_FRAME_ID;

#[derive(Debug, Clone)]
pub(super) struct VmNativeArg {
    pub(super) name: Option<SmolStr>,
    pub(super) value: VmNativeArgValue,
}

#[derive(Debug, Clone)]
pub(super) enum VmNativeArgValue {
    Expr(Value),
    Target(ValueRef),
}

#[derive(Debug, Clone)]
pub(super) struct VmOutBinding {
    pub(super) slot: usize,
    pub(super) target: VmWriteTarget,
}

#[derive(Debug, Clone)]
pub(super) struct VmFbOutBinding {
    pub(super) source: VmFbOutSource,
    pub(super) target: VmWriteTarget,
}

#[derive(Debug, Clone)]
pub(super) enum VmFbOutSource {
    Direct {
        instance_id: InstanceId,
        offset: usize,
    },
    Reference(ValueRef),
}

impl VmFbOutSource {
    pub(super) fn read<'a>(
        &self,
        runtime: &'a super::super::super::core::Runtime,
    ) -> Option<&'a Value> {
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
pub(super) enum VmWriteTarget {
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
    pub(super) fn from_reference(reference: &ValueRef) -> Self {
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

    pub(super) fn peek<'a>(
        &self,
        runtime: &'a super::super::super::core::Runtime,
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

    pub(super) fn read(
        &self,
        runtime: &mut super::super::super::core::Runtime,
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

    pub(super) fn write(
        &self,
        runtime: &mut super::super::super::core::Runtime,
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
pub(super) enum VmFbFieldBinding {
    Direct {
        instance_id: InstanceId,
        offset: usize,
    },
    Reference(ValueRef),
}

impl VmFbFieldBinding {
    pub(super) fn resolve(
        runtime: &super::super::super::core::Runtime,
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

    pub(super) fn read<'a>(
        &self,
        runtime: &'a super::super::super::core::Runtime,
    ) -> Option<&'a Value> {
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

    pub(super) fn write(
        &self,
        runtime: &mut super::super::super::core::Runtime,
        value: Value,
    ) -> bool {
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

    pub(super) fn out_source(&self) -> VmFbOutSource {
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

pub(super) fn clone_value_with_profile(
    runtime: &mut super::super::super::core::Runtime,
    value: &Value,
    kind: RegisterValueOpKind,
) -> Value {
    let (value, cloned) = materialize_borrowed_value(value);
    if cloned {
        runtime.vm_register_profile.record_value_op(kind);
    }
    value
}

pub(super) fn unpack_native_call_payload(
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

pub(super) fn resolve_named_arg_index(
    args: &[VmNativeArg],
    consumed: &[bool],
    param_name: &SmolStr,
    ordered_named_index: &mut usize,
) -> Option<usize> {
    *ordered_named_index = consumed
        .iter()
        .enumerate()
        .skip(*ordered_named_index)
        .find_map(|(index, consumed)| (!*consumed).then_some(index))
        .unwrap_or(args.len());

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

pub(super) fn bind_builtin_function_block_arguments(
    runtime: &mut super::super::super::core::Runtime,
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

pub(super) fn bind_vm_function_block_arguments(
    runtime: &mut super::super::super::core::Runtime,
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

pub(super) fn bind_vm_call_arguments(
    runtime: &mut super::super::super::core::Runtime,
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

pub(super) fn require_output_target(arg: &VmNativeArg) -> Result<VmWriteTarget, VmTrap> {
    match &arg.value {
        VmNativeArgValue::Target(reference) => Ok(VmWriteTarget::from_reference(reference)),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

pub(super) fn write_output_int(
    runtime: &mut super::super::super::core::Runtime,
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
        Value::USInt(_) => Value::USInt(u8::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::UInt(_) => Value::UInt(u16::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::UDInt(_) => Value::UDInt(u32::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        Value::ULInt(_) => Value::ULInt(u64::try_from(value).map_err(|_| RuntimeError::Overflow)?),
        _ => return Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    };
    target.write(runtime, frame, converted)
}

pub(super) fn read_vm_target_value(
    runtime: &mut super::super::super::core::Runtime,
    frame: &VmFrame,
    reference: &ValueRef,
) -> Result<Value, VmTrap> {
    VmWriteTarget::from_reference(reference).read(runtime, frame)
}

pub(super) fn resolve_vm_arg_value(
    runtime: &mut super::super::super::core::Runtime,
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

fn is_vm_local_sentinel(reference: &ValueRef) -> bool {
    matches!(
        reference.location,
        MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID))
    )
}

pub(super) fn peek_vm_reference<'a>(
    runtime: &'a super::super::super::core::Runtime,
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

pub(super) fn read_vm_reference(
    runtime: &mut super::super::super::core::Runtime,
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

pub(super) fn write_vm_reference(
    runtime: &mut super::super::super::core::Runtime,
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
