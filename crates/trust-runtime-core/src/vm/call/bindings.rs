#[cfg(all(feature = "std", feature = "test-support"))]
use crate::vm::diagnostic_timing::attribution::{self, CopyConsumerKind, CopyReadKind};
use alloc::{borrow::ToOwned, format, vec::Vec};
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::{FrameId, InstanceId, MemoryLocation};
use crate::value::{materialize_value_path, read_value_path_borrowed, RefSegment, Value, ValueRef};

use super::super::dispatch_refs::{charge_materialize_path_work, dynamic_ref_type};
use super::super::errors::VmTrap;
use super::super::frames::VmFrame;
use super::super::materialize_borrowed_value;
use super::super::module::{VmModule, VmNativeArgSpec, VmParamMeta};
use super::super::stack::OperandStack;
use super::super::type_policy::{vm_string_primitive_for_type, vm_string_shape_for_type};
use super::context::{
    BuiltinParam, BuiltinParamDirection, CallContext, RegisterCallOpKind, RegisterValueOpKind,
};
use super::VM_LOCAL_SENTINEL_FRAME_ID;
use crate::vm::prepared::call_plans::CallArguments;

mod indices;
mod targets;
pub use targets::{VmFbFieldBinding, VmFbOutSource, VmWriteTarget};

use indices::resolve_vm_arg_indices;

/// Evaluated native argument with an optional source name.
#[derive(Debug, Clone)]
pub struct VmNativeArg {
    /// Original parameter or symbol name.
    pub name: Option<SmolStr>,
    /// Evaluated expression value or target descriptor.
    pub value: VmNativeArgValue,
}

/// An evaluated expression or an output-capable target reference.
#[derive(Debug, Clone)]
pub enum VmNativeArgValue {
    /// An already evaluated input expression.
    Expr(Value),
    /// A reference used as an output or in-out argument.
    Target(ValueRef),
}

/// Function-local output slot and its caller destination.
#[derive(Debug, Clone)]
pub struct VmOutBinding {
    /// Local output slot copied back after the call.
    pub slot: usize,
    /// Caller destination for output copy-back.
    pub target: VmWriteTarget,
    /// Optional declared destination type used for normalization.
    pub target_type_idx: Option<u32>,
}

/// Function-block output source and its caller destination.
#[derive(Debug, Clone)]
pub struct VmFbOutBinding {
    /// Function-block field supplying the output value.
    pub source: VmFbOutSource,
    /// Caller destination for output copy-back.
    pub target: VmWriteTarget,
    /// Optional declared destination type used for normalization.
    pub target_type_idx: Option<u32>,
}

/// Charge before copying a value and record the requested clone profile event.
pub fn clone_value_with_profile(
    runtime: &mut impl CallContext,
    value: &Value,
    kind: RegisterValueOpKind,
) -> Result<Value, VmTrap> {
    runtime.before_value_clone(value).map_err(VmTrap::Runtime)?;
    let (value, cloned) = materialize_borrowed_value(value);
    if cloned {
        runtime.record_value_op(kind);
    }
    Ok(value)
}

/// Decode argument descriptors from the native-call operand payload.
pub fn unpack_native_call_payload(
    operand_stack: &mut OperandStack,
    arg_specs: &[VmNativeArgSpec],
    receiver_count: usize,
) -> Result<(Option<Value>, Vec<VmNativeArg>), VmTrap> {
    let mut args = Vec::new();
    let receiver =
        unpack_native_call_payload_into(&mut args, operand_stack, arg_specs, receiver_count)?;
    Ok((receiver, args))
}

/// Decode argument descriptors from the native-call operand payload into `args`, the argument
/// vector of the call's reserved scratch (spec 34 MEM-09; census C1).
pub fn unpack_native_call_payload_into(
    args: &mut Vec<VmNativeArg>,
    operand_stack: &mut OperandStack,
    arg_specs: &[VmNativeArgSpec],
    receiver_count: usize,
) -> Result<Option<Value>, VmTrap> {
    let total = arg_specs.len().saturating_add(receiver_count);
    let mut payload = operand_stack.drain_top(total)?;

    let receiver_value = if receiver_count == 1 {
        Some(payload.next().ok_or_else(|| {
            VmTrap::InvalidNativeCall("missing function-block/method receiver payload".into())
        })?)
    } else {
        None
    };

    args.clear();
    for spec in arg_specs {
        let value = payload.next().ok_or_else(|| {
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
        args.push(VmNativeArg {
            name: spec.name.clone(),
            value,
        });
    }

    Ok(receiver_value)
}

#[cfg(test)]
#[path = "bindings_payload_tests.rs"]
mod payload_tests;

/// Find an unconsumed named argument while retaining source argument order.
pub fn resolve_named_arg_index(
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

    if let Some(arg) = args.get(*ordered_named_index)
        && arg
            .name
            .as_ref()
            .map(|name| name.eq_ignore_ascii_case(param_name.as_str()))
            .unwrap_or(false)
    {
        let index = *ordered_named_index;
        *ordered_named_index += 1;
        return Some(index);
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

/// Bind input and output arguments for a native function block. The consumption flags and the
/// output bindings come from the call's reserved scratch and are returned on every path (spec 34
/// MEM-09; census C3).
pub fn bind_builtin_function_block_arguments(
    runtime: &mut impl CallContext,
    caller_frame: &VmFrame,
    fb_type_name: &SmolStr,
    fb_type_key: &str,
    instance_id: InstanceId,
    args: &[VmNativeArg],
) -> Result<Vec<VmFbOutBinding>, VmTrap> {
    let params = runtime.builtin_params(fb_type_key).ok_or_else(|| {
        VmTrap::Runtime(RuntimeError::UndefinedFunctionBlock(fb_type_name.clone()))
    })?;
    let mut consumed = runtime.take_call_flags(args.len());
    let mut out_bindings = runtime.take_call_fb_outputs();
    let result = bind_builtin_function_block_arguments_inner(
        runtime,
        caller_frame,
        &params,
        instance_id,
        args,
        &mut consumed,
        &mut out_bindings,
    );
    runtime.recycle_call_flags(consumed);
    match result {
        Ok(()) => Ok(out_bindings),
        Err(error) => {
            runtime.recycle_call_fb_outputs(out_bindings);
            Err(error)
        }
    }
}

/// The binding loop of [`bind_builtin_function_block_arguments`], writing the reserved vectors.
fn bind_builtin_function_block_arguments_inner(
    runtime: &mut impl CallContext,
    caller_frame: &VmFrame,
    params: &[BuiltinParam],
    instance_id: InstanceId,
    args: &[VmNativeArg],
    consumed: &mut [bool],
    out_bindings: &mut Vec<VmFbOutBinding>,
) -> Result<(), VmTrap> {
    let positional = args.iter().all(|arg| arg.name.is_none());
    let mut positional_index = 0usize;
    let mut ordered_named_index = 0usize;

    #[cfg(all(feature = "std", feature = "test-support"))]
    let mut diagnostic_formals = 0..;
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _consumer = attribution::consumer(CopyConsumerKind::BuiltinFunctionBlock, None);
    for param in params {
        runtime.record_call_op(RegisterCallOpKind::ParameterBinding);
        let arg_index = if positional {
            let next = (positional_index < args.len()).then_some(positional_index);
            if next.is_some() {
                positional_index = positional_index.saturating_add(1);
            }
            next
        } else {
            resolve_named_arg_index(args, consumed, &param.name, &mut ordered_named_index)
        };
        if let Some(index) = arg_index {
            consumed[index] = true;
        }
        let arg = arg_index.and_then(|index| args.get(index));
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _parameter = attribution::parameter(
            diagnostic_formals.next().unwrap_or(usize::MAX),
            match param.direction {
                BuiltinParamDirection::In => 0,
                BuiltinParamDirection::Out => 1,
                BuiltinParamDirection::InOut => 2,
            },
            arg.is_none(),
        );
        if matches!(
            param.direction,
            BuiltinParamDirection::Out | BuiltinParamDirection::InOut
        ) && arg.is_none()
        {
            continue;
        }
        let field_binding = VmFbFieldBinding::resolve(runtime, instance_id, &param.name)?;

        match param.direction {
            BuiltinParamDirection::In => {
                let value = match arg {
                    Some(arg) => resolve_vm_arg_value(runtime, caller_frame, arg)?,
                    None => {
                        if let Some(value) = field_binding.read_checked(runtime)? {
                            #[cfg(all(feature = "std", feature = "test-support"))]
                            let _read = attribution::read(CopyReadKind::RetainedField);
                            runtime.before_value_clone(value).map_err(VmTrap::Runtime)?;
                            let (value, cloned) = materialize_borrowed_value(value);
                            if cloned {
                                runtime.record_value_op(RegisterValueOpKind::ReadReference);
                            }
                            value
                        } else {
                            Value::Null
                        }
                    }
                };
                field_binding.check_write(runtime, &value)?;
                if !field_binding.write_checked(runtime, value)? {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
            }
            BuiltinParamDirection::Out => {
                if let Some(arg) = arg {
                    out_bindings.push(VmFbOutBinding {
                        source: field_binding.out_source(),
                        target: require_output_target(arg)?,
                        target_type_idx: None,
                    });
                }
            }
            BuiltinParamDirection::InOut => {
                let Some(arg) = arg else {
                    continue;
                };
                let target = require_output_target(arg)?;
                let value = target.snapshot(runtime, caller_frame)?;
                field_binding.check_write(runtime, &value)?;
                if !field_binding.write_checked(runtime, value)? {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
                out_bindings.push(VmFbOutBinding {
                    source: field_binding.out_source(),
                    target,
                    target_type_idx: None,
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

    Ok(())
}

/// Bind input and output arguments for a native function block from its prepared plan
/// (spec 34 MEM-09). The plan resolved each formal once, so no argument names are compared.
/// The output bindings come from the call's reserved scratch and are returned on refusal.
pub(super) fn bind_builtin_function_block_arguments_planned(
    runtime: &mut impl CallContext,
    caller_frame: &VmFrame,
    params: &[VmParamMeta],
    instance_id: InstanceId,
    args: &[VmNativeArg],
    arguments: &CallArguments,
) -> Result<Vec<VmFbOutBinding>, VmTrap> {
    let mut out_bindings = runtime.take_call_fb_outputs();
    let result = bind_builtin_function_block_arguments_planned_inner(
        runtime,
        caller_frame,
        params,
        instance_id,
        args,
        arguments,
        &mut out_bindings,
    );
    match result {
        Ok(()) => Ok(out_bindings),
        Err(error) => {
            runtime.recycle_call_fb_outputs(out_bindings);
            Err(error)
        }
    }
}

/// The binding loop of [`bind_builtin_function_block_arguments_planned`].
fn bind_builtin_function_block_arguments_planned_inner(
    runtime: &mut impl CallContext,
    caller_frame: &VmFrame,
    params: &[VmParamMeta],
    instance_id: InstanceId,
    args: &[VmNativeArg],
    arguments: &CallArguments,
    out_bindings: &mut Vec<VmFbOutBinding>,
) -> Result<(), VmTrap> {
    #[cfg(all(feature = "std", feature = "test-support"))]
    let mut diagnostic_formals = 0..;
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _consumer = attribution::consumer(CopyConsumerKind::BuiltinFunctionBlock, None);
    for (index, param) in params.iter().enumerate() {
        runtime.record_call_op(RegisterCallOpKind::ParameterBinding);
        let arg = arguments
            .actual(index)
            .and_then(|arg_index| args.get(arg_index));
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _parameter = attribution::parameter(
            diagnostic_formals.next().unwrap_or(usize::MAX),
            param.direction,
            arg.is_none(),
        );
        if matches!(param.direction, 1 | 2) && arg.is_none() {
            continue;
        }
        let field_binding = VmFbFieldBinding::resolve(runtime, instance_id, &param.name)?;
        match param.direction {
            0 => {
                let value = match arg {
                    Some(arg) => resolve_vm_arg_value(runtime, caller_frame, arg)?,
                    None => {
                        if let Some(value) = field_binding.read_checked(runtime)? {
                            #[cfg(all(feature = "std", feature = "test-support"))]
                            let _read = attribution::read(CopyReadKind::RetainedField);
                            runtime.before_value_clone(value).map_err(VmTrap::Runtime)?;
                            let (value, cloned) = materialize_borrowed_value(value);
                            if cloned {
                                runtime.record_value_op(RegisterValueOpKind::ReadReference);
                            }
                            value
                        } else {
                            Value::Null
                        }
                    }
                };
                field_binding.check_write(runtime, &value)?;
                if !field_binding.write_checked(runtime, value)? {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
            }
            1 => {
                if let Some(arg) = arg {
                    out_bindings.push(VmFbOutBinding {
                        source: field_binding.out_source(),
                        target: require_output_target(arg)?,
                        target_type_idx: None,
                    });
                }
            }
            2 => {
                let Some(arg) = arg else {
                    continue;
                };
                let target = require_output_target(arg)?;
                let value = target.snapshot(runtime, caller_frame)?;
                field_binding.check_write(runtime, &value)?;
                if !field_binding.write_checked(runtime, value)? {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
                out_bindings.push(VmFbOutBinding {
                    source: field_binding.out_source(),
                    target,
                    target_type_idx: None,
                });
            }
            other => {
                return Err(VmTrap::InvalidNativeCall(
                    format!("invalid parameter direction {other}").into(),
                ));
            }
        }
    }
    Ok(())
}

/// Resolved formal-to-actual indices: from the prepared plan or resolved at the call.
enum CallArgIndices<'a> {
    Resolved(Vec<Option<usize>>),
    Planned(&'a CallArguments),
}

impl CallArgIndices<'_> {
    fn get(&self, formal: usize) -> Option<usize> {
        match self {
            Self::Resolved(indices) => indices.get(formal).copied().flatten(),
            Self::Planned(arguments) => arguments.actual(formal),
        }
    }
}

/// Bind a bytecode function block against its declared parameter metadata.
pub fn bind_vm_function_block_arguments(
    runtime: &mut impl CallContext,
    module: &VmModule,
    caller_frame: &VmFrame,
    pou_id: u32,
    instance_id: InstanceId,
    args: &[VmNativeArg],
) -> Result<Vec<VmFbOutBinding>, VmTrap> {
    bind_vm_function_block_arguments_with(
        runtime,
        module,
        caller_frame,
        pou_id,
        instance_id,
        args,
        None,
    )
}

/// Bind a bytecode function block from its prepared plan.
pub(super) fn bind_vm_function_block_arguments_planned(
    runtime: &mut impl CallContext,
    module: &VmModule,
    caller_frame: &VmFrame,
    pou_id: u32,
    instance_id: InstanceId,
    args: &[VmNativeArg],
    arguments: &CallArguments,
) -> Result<Vec<VmFbOutBinding>, VmTrap> {
    bind_vm_function_block_arguments_with(
        runtime,
        module,
        caller_frame,
        pou_id,
        instance_id,
        args,
        Some(arguments),
    )
}

fn bind_vm_function_block_arguments_with(
    runtime: &mut impl CallContext,
    module: &VmModule,
    caller_frame: &VmFrame,
    pou_id: u32,
    instance_id: InstanceId,
    args: &[VmNativeArg],
    arguments: Option<&CallArguments>,
) -> Result<Vec<VmFbOutBinding>, VmTrap> {
    let mut prepared_targets = runtime.take_call_prepared_targets();
    let mut out_bindings = runtime.take_call_fb_outputs();
    let result = bind_vm_function_block_arguments_inner(
        runtime,
        module,
        caller_frame,
        pou_id,
        instance_id,
        args,
        arguments,
        &mut prepared_targets,
        &mut out_bindings,
    );
    runtime.recycle_call_prepared_targets(prepared_targets);
    match result {
        Ok(()) => Ok(out_bindings),
        Err(error) => {
            runtime.recycle_call_fb_outputs(out_bindings);
            Err(error)
        }
    }
}

/// The binding of [`bind_vm_function_block_arguments_with`], writing the reserved vectors
/// (spec 34 MEM-09; census C4).
#[allow(
    clippy::too_many_arguments,
    reason = "one binder per call kind keeps the reserved scratch explicit"
)]
fn bind_vm_function_block_arguments_inner(
    runtime: &mut impl CallContext,
    module: &VmModule,
    caller_frame: &VmFrame,
    pou_id: u32,
    instance_id: InstanceId,
    args: &[VmNativeArg],
    arguments: Option<&CallArguments>,
    prepared_targets: &mut Vec<Option<(VmWriteTarget, Option<u32>)>>,
    out_bindings: &mut Vec<VmFbOutBinding>,
) -> Result<(), VmTrap> {
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _consumer = attribution::consumer(CopyConsumerKind::FunctionBlock, Some(pou_id));
    let params = module.pou_params(pou_id).ok_or_else(|| {
        VmTrap::InvalidNativeCall(format!("missing parameter metadata for pou id {pou_id}").into())
    })?;
    let arg_indices = match arguments {
        Some(arguments) => CallArgIndices::Planned(arguments),
        None => CallArgIndices::Resolved(resolve_vm_arg_indices(params, args)?),
    };
    prepared_targets.clear();
    prepared_targets.resize(params.len(), None);
    for (param_index, param) in params.iter().enumerate() {
        let arg = arg_indices
            .get(param_index)
            .and_then(|index| args.get(index));
        let prepared = if matches!(param.direction, 1 | 2) {
            match arg {
                Some(arg) => {
                    let binding = bind_output_target(runtime, module, caller_frame, arg)?;
                    if param.direction == 2 {
                        require_in_out_type_compatibility(module, param.type_id, binding.1)?;
                    }
                    Some(binding)
                }
                None => None,
            }
        } else {
            None
        };
        prepared_targets[param_index] = prepared;
    }

    for (param_index, param) in params.iter().enumerate() {
        runtime.record_call_op(RegisterCallOpKind::ParameterBinding);
        let arg = arg_indices
            .get(param_index)
            .and_then(|index| args.get(index));
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _parameter = attribution::parameter(param_index, param.direction, arg.is_none());
        if matches!(param.direction, 1 | 2) && arg.is_none() {
            continue;
        }
        let field_binding = VmFbFieldBinding::resolve(runtime, instance_id, &param.name)?;

        match param.direction {
            0 => {
                let value = match arg {
                    Some(arg) => resolve_vm_arg_value(runtime, caller_frame, arg)?,
                    None => {
                        if let Some(value) = field_binding.read_checked(runtime)? {
                            #[cfg(all(feature = "std", feature = "test-support"))]
                            let _read = attribution::read(CopyReadKind::RetainedField);
                            runtime.before_value_clone(value).map_err(VmTrap::Runtime)?;
                            let (value, cloned) = materialize_borrowed_value(value);
                            if cloned {
                                runtime.record_value_op(RegisterValueOpKind::ReadReference);
                            }
                            value
                        } else if let Some(default_const_idx) = param.default_const_idx {
                            let value = module
                                .consts
                                .get(default_const_idx as usize)
                                .ok_or_else(|| VmTrap::InvalidConstIndex(default_const_idx))?;
                            runtime.before_value_clone(value).map_err(VmTrap::Runtime)?;
                            let (value, cloned) = materialize_borrowed_value(value);
                            if cloned {
                                runtime.record_value_op(RegisterValueOpKind::LoadConstant);
                            }
                            value
                        } else {
                            Value::Null
                        }
                    }
                };
                let value = runtime
                    .normalize_assignment(module, param.type_id, value)
                    .map_err(VmTrap::Runtime)?;
                field_binding.check_write(runtime, &value)?;
                if !field_binding.write_checked(runtime, value)? {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
            }
            1 => {
                if arg.is_some() {
                    let (target, target_type_idx) = prepared_targets[param_index]
                        .take()
                        .expect("output target preflight must produce a binding");
                    out_bindings.push(VmFbOutBinding {
                        source: field_binding.out_source(),
                        target,
                        target_type_idx,
                    });
                }
            }
            2 => {
                if arg.is_none() {
                    continue;
                }
                let (target, target_type_idx) = prepared_targets[param_index]
                    .take()
                    .expect("IN_OUT target preflight must produce a binding");
                let value = target.snapshot(runtime, caller_frame)?;
                let value = runtime
                    .normalize_assignment(module, param.type_id, value)
                    .map_err(VmTrap::Runtime)?;
                field_binding.check_write(runtime, &value)?;
                if !field_binding.write_checked(runtime, value)? {
                    return Err(VmTrap::Runtime(RuntimeError::NullReference));
                }
                out_bindings.push(VmFbOutBinding {
                    source: field_binding.out_source(),
                    target,
                    target_type_idx,
                });
            }
            other => {
                return Err(VmTrap::InvalidNativeCall(
                    format!("invalid parameter direction {other}").into(),
                ));
            }
        }
    }

    Ok(())
}

/// Prepared locals, output destinations and explicit parameter presence.
#[derive(Debug)]
pub struct BoundVmCall {
    /// Final or initially bound local values in slot order.
    pub locals: Vec<Value>,
    /// Ordered output destinations for copy-back.
    pub out_bindings: Vec<VmOutBinding>,
    /// Explicit argument-presence bits; a supplied NULL is present.
    pub present: Vec<bool>,
}

trait VmCallArgumentSource {
    fn arguments(&self) -> &[VmNativeArg];

    fn bind_input(
        &mut self,
        runtime: &mut impl CallContext,
        caller_frame: &VmFrame,
        arg_index: usize,
    ) -> Result<Value, VmTrap>;
}

#[cfg(any(feature = "hir", test))]
struct BorrowedVmCallArguments<'a>(&'a [VmNativeArg]);

#[cfg(any(feature = "hir", test))]
impl VmCallArgumentSource for BorrowedVmCallArguments<'_> {
    fn arguments(&self) -> &[VmNativeArg] {
        self.0
    }

    fn bind_input(
        &mut self,
        runtime: &mut impl CallContext,
        caller_frame: &VmFrame,
        arg_index: usize,
    ) -> Result<Value, VmTrap> {
        let arg = self.0.get(arg_index).ok_or_else(|| {
            VmTrap::InvalidNativeCall(SmolStr::new_static(
                "resolved argument index is out of range",
            ))
        })?;
        resolve_vm_arg_value(runtime, caller_frame, arg)
    }
}

struct OwnedVmCallArguments<'a>(&'a mut [VmNativeArg]);

impl VmCallArgumentSource for OwnedVmCallArguments<'_> {
    fn arguments(&self) -> &[VmNativeArg] {
        self.0
    }

    fn bind_input(
        &mut self,
        runtime: &mut impl CallContext,
        caller_frame: &VmFrame,
        arg_index: usize,
    ) -> Result<Value, VmTrap> {
        let arg = self.0.get_mut(arg_index).ok_or_else(|| {
            VmTrap::InvalidNativeCall(SmolStr::new_static(
                "resolved argument index is out of range",
            ))
        })?;
        match &mut arg.value {
            VmNativeArgValue::Expr(value) => Ok(core::mem::replace(value, Value::Null)),
            VmNativeArgValue::Target(reference) => {
                read_vm_target_value(runtime, caller_frame, reference)
            }
        }
    }
}

/// Construct a function call frame and output copy-back plan.
#[cfg(any(feature = "hir", test))]
pub fn bind_vm_call_arguments(
    runtime: &mut impl CallContext,
    module: &VmModule,
    caller_frame: &VmFrame,
    pou_id: u32,
    args: &[VmNativeArg],
) -> Result<BoundVmCall, VmTrap> {
    bind_vm_call_arguments_from_source(
        runtime,
        module,
        caller_frame,
        pou_id,
        &mut BorrowedVmCallArguments(args),
        None,
    )
    .map(|(bound, _)| bound)
}

/// Consume private native-call expressions after unique parameter binding.
pub(super) fn bind_vm_call_arguments_owned(
    runtime: &mut impl CallContext,
    module: &VmModule,
    caller_frame: &VmFrame,
    pou_id: u32,
    args: &mut [VmNativeArg],
) -> Result<(BoundVmCall, bool), VmTrap> {
    bind_vm_call_arguments_from_source(
        runtime,
        module,
        caller_frame,
        pou_id,
        &mut OwnedVmCallArguments(args),
        None,
    )
}

/// Consume private native-call expressions from the call's prepared plan.
pub(super) fn bind_vm_call_arguments_owned_planned(
    runtime: &mut impl CallContext,
    module: &VmModule,
    caller_frame: &VmFrame,
    pou_id: u32,
    args: &mut [VmNativeArg],
    arguments: &CallArguments,
) -> Result<(BoundVmCall, bool), VmTrap> {
    bind_vm_call_arguments_from_source(
        runtime,
        module,
        caller_frame,
        pou_id,
        &mut OwnedVmCallArguments(args),
        Some(arguments),
    )
}

fn bind_vm_call_arguments_from_source(
    runtime: &mut impl CallContext,
    module: &VmModule,
    caller_frame: &VmFrame,
    pou_id: u32,
    args: &mut impl VmCallArgumentSource,
    arguments: Option<&CallArguments>,
) -> Result<(BoundVmCall, bool), VmTrap> {
    // The POU identity is checked before any parameter metadata, exactly as before.
    let pou = module
        .pou(pou_id)
        .ok_or_else(|| VmTrap::MissingPou(pou_id))?;
    let mut locals = runtime.take_optimized_call_locals(pou.local_ref_count as usize);
    // The output bindings and the presence bits come from the call's reserved scratch and are
    // returned on a refusal (spec 34 MEM-09; census C5).
    let mut out_bindings = runtime.take_call_function_outputs();
    let mut present = runtime.take_call_flags(locals.len());
    let capture_return = module.pou_has_return_slot(pou_id);
    let result = bind_vm_call_arguments_inner(
        runtime,
        module,
        caller_frame,
        pou_id,
        args,
        arguments,
        &mut locals,
        &mut out_bindings,
        &mut present,
    );
    match result {
        Ok(()) => Ok((
            BoundVmCall {
                locals,
                out_bindings,
                present,
            },
            capture_return,
        )),
        Err(error) => {
            runtime.recycle_call_function_outputs(out_bindings);
            runtime.recycle_call_flags(present);
            Err(error)
        }
    }
}

/// The binding of [`bind_vm_call_arguments_from_source`], writing the reserved vectors.
#[allow(
    clippy::too_many_arguments,
    reason = "one binder per call kind keeps the reserved scratch explicit"
)]
fn bind_vm_call_arguments_inner(
    runtime: &mut impl CallContext,
    module: &VmModule,
    caller_frame: &VmFrame,
    pou_id: u32,
    args: &mut impl VmCallArgumentSource,
    arguments: Option<&CallArguments>,
    locals: &mut [Value],
    out_bindings: &mut Vec<VmOutBinding>,
    present: &mut [bool],
) -> Result<(), VmTrap> {
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _consumer = attribution::consumer(CopyConsumerKind::UserCall, Some(pou_id));
    let params = module.pou_params(pou_id).ok_or_else(|| {
        VmTrap::InvalidNativeCall(format!("missing parameter metadata for pou id {pou_id}").into())
    })?;
    let return_slots = usize::from(module.pou_has_return_slot(pou_id));
    let arg_indices = match arguments {
        Some(arguments) => CallArgIndices::Planned(arguments),
        None => CallArgIndices::Resolved(resolve_vm_arg_indices(params, args.arguments())?),
    };

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
        let arg_index = arg_indices.get(index);
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _parameter = attribution::parameter(index, param.direction, arg_index.is_none());

        present[slot] = match arguments {
            Some(arguments) => arguments.present()[index],
            None => {
                param.default_const_idx.is_some() || (param.direction != 1 && arg_index.is_some())
            }
        };
        match param.direction {
            0 => {
                let value = match arg_index {
                    Some(arg_index) => args.bind_input(runtime, caller_frame, arg_index)?,
                    None => {
                        if let Some(default_const_idx) = param.default_const_idx {
                            module
                                .consts
                                .get(default_const_idx as usize)
                                .map(|value| {
                                    clone_value_with_profile(
                                        runtime,
                                        value,
                                        RegisterValueOpKind::LoadConstant,
                                    )
                                })
                                .ok_or_else(|| VmTrap::InvalidConstIndex(default_const_idx))??
                        } else {
                            Value::Null
                        }
                    }
                };
                locals[slot] = if module.version.is_source_free() && !present[slot] {
                    value
                } else {
                    runtime
                        .normalize_assignment(module, param.type_id, value)
                        .map_err(VmTrap::Runtime)?
                };
            }
            1 => {
                locals[slot] = if let Some(default_const_idx) = param.default_const_idx {
                    module
                        .consts
                        .get(default_const_idx as usize)
                        .map(|value| {
                            clone_value_with_profile(
                                runtime,
                                value,
                                RegisterValueOpKind::LoadConstant,
                            )
                        })
                        .ok_or_else(|| VmTrap::InvalidConstIndex(default_const_idx))??
                } else {
                    Value::Null
                };
                if let Some(arg_index) = arg_index {
                    let arg = args.arguments().get(arg_index).ok_or_else(|| {
                        VmTrap::InvalidNativeCall(SmolStr::new_static(
                            "resolved argument index is out of range",
                        ))
                    })?;
                    let (target, target_type_idx) =
                        bind_output_target(runtime, module, caller_frame, arg)?;
                    out_bindings.push(VmOutBinding {
                        slot,
                        target,
                        target_type_idx,
                    });
                }
            }
            2 => {
                let Some(arg_index) = arg_index else {
                    return Err(VmTrap::InvalidNativeCall(
                        format!("missing IN_OUT argument '{}'", param.name).into(),
                    ));
                };
                let arg = args.arguments().get(arg_index).ok_or_else(|| {
                    VmTrap::InvalidNativeCall(SmolStr::new_static(
                        "resolved argument index is out of range",
                    ))
                })?;
                let (target, target_type_idx) =
                    bind_output_target(runtime, module, caller_frame, arg)?;
                require_in_out_type_compatibility(module, param.type_id, target_type_idx)?;
                let value = target.snapshot(runtime, caller_frame)?;
                locals[slot] = runtime
                    .normalize_assignment(module, param.type_id, value)
                    .map_err(VmTrap::Runtime)?;
                out_bindings.push(VmOutBinding {
                    slot,
                    target,
                    target_type_idx,
                });
            }
            other => {
                return Err(VmTrap::InvalidNativeCall(
                    format!("invalid parameter direction {other}").into(),
                ));
            }
        }
    }

    Ok(())
}

/// Require an argument to designate writable storage.
pub fn require_output_target(arg: &VmNativeArg) -> Result<VmWriteTarget, VmTrap> {
    match &arg.value {
        VmNativeArgValue::Target(reference) => Ok(VmWriteTarget::from_reference(reference)),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch)),
    }
}

fn bind_output_target(
    runtime: &impl CallContext,
    module: &VmModule,
    caller_frame: &VmFrame,
    arg: &VmNativeArg,
) -> Result<(VmWriteTarget, Option<u32>), VmTrap> {
    let VmNativeArgValue::Target(reference) = &arg.value else {
        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
    };
    let target = VmWriteTarget::from_reference(reference);
    let target_type_idx = dynamic_ref_type(runtime, module, caller_frame, reference)?;
    if target_type_idx.is_none()
        && matches!(
            target.peek(runtime, caller_frame)?,
            Value::String(_) | Value::WString(_)
        )
    {
        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
    }
    Ok((target, target_type_idx))
}

fn require_in_out_type_compatibility(
    module: &VmModule,
    formal_type_idx: u32,
    target_type_idx: Option<u32>,
) -> Result<(), VmTrap> {
    let formal = vm_string_shape_for_type(module, formal_type_idx);
    let actual = target_type_idx.and_then(|type_idx| vm_string_shape_for_type(module, type_idx));
    if (formal.is_some() || actual.is_some()) && formal != actual {
        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
    }
    Ok(())
}

/// Normalize an output using its declared destination type and current target shape.
pub fn normalize_output_copyback_value(
    runtime: &mut impl CallContext,
    module: &VmModule,
    caller_frame: &VmFrame,
    target: &VmWriteTarget,
    target_type_idx: Option<u32>,
    value: Value,
) -> Result<Value, VmTrap> {
    let shape = |value: &Value| {
        (
            match value {
                Value::String(_) => Some(24),
                Value::WString(_) => Some(25),
                _ => None,
            },
            matches!(value, Value::Null),
        )
    };
    let (current_string_primitive, current_is_null) = match target.peek(runtime, caller_frame) {
        Ok(current) => shape(current),
        Err(VmTrap::Runtime(RuntimeError::NullReference)) => {
            // A valid STRING/WSTRING element synthesizes CHAR/WCHAR and cannot
            // lend a Value. The checked read path charges the materialization and
            // still rejects nonexistent/out-of-bounds or forbidden references.
            shape(&target.read(runtime, caller_frame)?)
        }
        Err(error) => return Err(error),
    };
    let value_string_primitive = match &value {
        Value::String(_) => Some(24),
        Value::WString(_) => Some(25),
        _ => None,
    };
    let declared_string_primitive =
        target_type_idx.and_then(|type_idx| vm_string_primitive_for_type(module, type_idx));
    if let Some(expected) = declared_string_primitive {
        let current_matches = current_string_primitive == Some(expected) || current_is_null;
        let value_matches =
            value_string_primitive == Some(expected) || matches!(value, Value::Null);
        if !current_matches || !value_matches {
            return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
        }
    } else if current_string_primitive.is_some() || value_string_primitive.is_some() {
        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
    }
    match target_type_idx {
        Some(type_idx) => runtime
            .normalize_assignment(module, type_idx, value)
            .map_err(VmTrap::Runtime),
        None => Ok(value),
    }
}

/// Store a native integer output in the destination integer representation.
pub fn write_output_int(
    runtime: &mut impl CallContext,
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

/// Materialize an output target through the checked shared read path.
pub fn read_vm_target_value(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    reference: &ValueRef,
) -> Result<Value, VmTrap> {
    VmWriteTarget::from_reference(reference).read(runtime, frame)
}

/// Read an expression or target argument using the shared call context.
pub fn resolve_vm_arg_value(
    runtime: &mut impl CallContext,
    frame: &VmFrame,
    arg: &VmNativeArg,
) -> Result<Value, VmTrap> {
    match &arg.value {
        VmNativeArgValue::Expr(value) => {
            clone_value_with_profile(runtime, value, RegisterValueOpKind::BindExpression)
        }
        VmNativeArgValue::Target(reference) => read_vm_target_value(runtime, frame, reference),
    }
}

fn is_vm_local_sentinel(reference: &ValueRef) -> bool {
    matches!(
        reference.location,
        MemoryLocation::Local(FrameId(VM_LOCAL_SENTINEL_FRAME_ID))
    )
}

fn is_current_location(location: MemoryLocation, frame: &VmFrame) -> bool {
    location == MemoryLocation::Local(frame.reference_frame_id())
}

/// Borrow a referenced value without allocating a reference path.
pub fn peek_vm_reference<'a>(
    runtime: &'a impl CallContext,
    caller_frame: &'a VmFrame,
    reference: &ValueRef,
) -> Result<&'a Value, VmTrap> {
    peek_vm_reference_path(runtime, caller_frame, reference, &reference.path)
}

fn peek_vm_reference_path<'a>(
    runtime: &'a impl CallContext,
    caller_frame: &'a VmFrame,
    reference: &ValueRef,
    path: &[RefSegment],
) -> Result<&'a Value, VmTrap> {
    runtime
        .check_reference_read(reference.as_view())
        .map_err(VmTrap::Runtime)?;
    if is_current_location(reference.location, caller_frame) {
        let root = caller_frame
            .locals
            .get(reference.offset as usize)
            .ok_or_else(|| {
                VmTrap::InvalidNativeCall(
                    format!(
                        "local reference offset {} out of range for VM frame (locals={})",
                        reference.offset,
                        caller_frame.locals.len()
                    )
                    .into(),
                )
            })?;
        return read_value_path_borrowed(root, path)
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::NullReference));
    }
    runtime
        .storage()
        .read_by_ref_parts(reference.location, reference.offset as usize, path)
        .ok_or_else(|| VmTrap::Runtime(RuntimeError::NullReference))
}

/// Materialize a referenced value after read-policy and copy-budget checks.
pub fn read_vm_reference(
    runtime: &mut impl CallContext,
    caller_frame: &VmFrame,
    reference: &ValueRef,
) -> Result<Value, VmTrap> {
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _timing =
        crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::ReferenceRead);
    match peek_vm_reference(runtime, caller_frame, reference) {
        Ok(value) => {
            runtime.before_value_clone(value).map_err(VmTrap::Runtime)?;
            let (value, cloned) = materialize_borrowed_value(value);
            if cloned {
                runtime.record_value_op(RegisterValueOpKind::ReadReference);
            }
            Ok(value)
        }
        Err(VmTrap::Runtime(RuntimeError::NullReference)) => {
            // String elements synthesize a scalar rather than borrowing a Value.
            let (last, prefix) = reference
                .path
                .split_last()
                .ok_or_else(|| VmTrap::Runtime(RuntimeError::NullReference))?;
            let parent = peek_vm_reference_path(runtime, caller_frame, reference, prefix)?;
            runtime
                .before_value_clone(parent)
                .map_err(VmTrap::Runtime)?;
            charge_materialize_path_work(runtime, parent, core::slice::from_ref(last))
                .map_err(VmTrap::Runtime)?;
            materialize_value_path(parent, core::slice::from_ref(last))
                .ok_or_else(|| VmTrap::Runtime(RuntimeError::NullReference))
        }
        Err(error) => Err(error),
    }
}

/// Write through a reference using frame-aware type and policy gates.
pub fn write_vm_reference(
    runtime: &mut impl CallContext,
    caller_frame: &mut VmFrame,
    reference: &ValueRef,
    value: Value,
) -> Result<(), VmTrap> {
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _timing =
        crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::ReferenceWrite);
    runtime
        .check_native_write(Some(caller_frame), reference, &value)
        .map_err(VmTrap::Runtime)?;
    let capacity = runtime.reference_string_capacity(reference.as_view());
    if is_current_location(reference.location, caller_frame) {
        let local_count = caller_frame.locals.len();
        let Some(slot) = caller_frame.locals.get_mut(reference.offset as usize) else {
            return Err(VmTrap::InvalidNativeCall(
                format!(
                    "local reference offset {} out of range for VM frame (locals={local_count})",
                    reference.offset,
                )
                .into(),
            ));
        };
        runtime
            .before_path_write(slot, &reference.path, &value)
            .map_err(VmTrap::Runtime)?;
        runtime.before_write_commit().map_err(VmTrap::Runtime)?;
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _storage_timing =
            crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::StorageWrite);
        if runtime
            .write_typed_local_checked(slot, &reference.path, value, capacity)
            .map_err(VmTrap::Runtime)?
        {
            return Ok(());
        }
        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch));
    }
    {
        let root = runtime
            .storage()
            .read_direct_slot_by_location(reference.location, reference.offset as usize)
            .ok_or_else(|| VmTrap::Runtime(RuntimeError::NullReference))?;
        runtime
            .before_path_write(root, &reference.path, &value)
            .map_err(VmTrap::Runtime)?;
    }
    #[cfg(all(feature = "std", feature = "test-support"))]
    let _storage_timing =
        crate::vm::diagnostic_timing::begin(crate::vm::diagnostic_timing::Phase::StorageWrite);
    let prepared = runtime
        .storage()
        .prepare_by_ref_parts_write(
            reference.location,
            reference.offset as usize,
            &reference.path,
            &value,
        )
        .map_err(VmTrap::Runtime)?
        .ok_or_else(|| VmTrap::Runtime(RuntimeError::NullReference))?;
    runtime.before_write_commit().map_err(VmTrap::Runtime)?;
    if runtime
        .storage_mut()
        .commit_prepared_write(&reference.path, value, prepared, capacity)
    {
        Ok(())
    } else {
        Err(VmTrap::Runtime(RuntimeError::NullReference))
    }
}
