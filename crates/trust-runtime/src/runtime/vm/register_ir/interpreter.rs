use super::*;

pub(super) fn build_register_pou_result(
    frame: super::super::frames::VmFrame,
    capture_return: bool,
) -> RegisterPouExecutionResult {
    let return_value = if capture_return {
        frame.locals.first().cloned()
    } else {
        None
    };
    RegisterPouExecutionResult {
        return_value,
        locals: frame.locals,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RegisterBlockExecutionOutcome {
    Continue(Option<BlockTarget>),
    ReturnFromPou,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tier1BlockExecutionOutcome {
    Executed(RegisterBlockExecutionOutcome),
}

pub(super) enum BorrowedBinaryEval {
    GuardHit(Value),
    Materialized {
        left: Value,
        left_cloned: bool,
        right: Value,
        right_cloned: bool,
    },
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_register_block_interpreted(
    runtime: &mut Runtime,
    module: &VmModule,
    program: &RegisterProgram,
    frames: &mut FrameStack,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    native_call_stack: &mut OperandStack,
    block: &RegisterBlock,
    budget: &mut usize,
    depth_offset: u32,
) -> Result<RegisterBlockExecutionOutcome, RuntimeError> {
    let mut control_target = None;
    for (instruction_index, instruction) in block.instructions.iter().enumerate() {
        if should_check_register_deadline(instruction_index)
            && deadline_exceeded(runtime.execution_deadline)
        {
            return Err(VmTrap::DeadlineExceeded.into_runtime_error());
        }

        match instruction {
            RegisterInstr::Nop => {}
            RegisterInstr::LoadConst { dest, const_idx } => {
                let value = module
                    .consts
                    .get(*const_idx as usize)
                    .map(|value| {
                        let (value, cloned) = materialize_borrowed_value(value);
                        if cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ConstLoadClone);
                        }
                        value
                    })
                    .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, value)?;
            }
            RegisterInstr::LoadNull { dest } => {
                write_register(registers, *dest, Value::Null)?;
            }
            RegisterInstr::LoadSelf { dest } => {
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let self_instance = frame.runtime_instance.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                write_register(registers, *dest, Value::Instance(self_instance))?;
            }
            RegisterInstr::LoadSuper { dest } => {
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let self_instance = frame.runtime_instance.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                let instance = runtime
                    .storage
                    .get_instance(self_instance)
                    .ok_or_else(|| VmTrap::NullReference.into_runtime_error())?;
                let super_instance = instance.parent.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                write_register(registers, *dest, Value::Instance(super_instance))?;
            }
            RegisterInstr::LoadSelfFieldDynamic { field_idx, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefField);
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadDynamic);
                let field = register_field_name(module, *field_idx)?;
                let self_instance = current_self_instance(frames)?;
                let value = load_instance_field_dynamic(runtime, frames, self_instance, field)?;
                write_register(registers, *dest, value)?;
            }
            RegisterInstr::StoreSelfFieldDynamic { field_idx, value } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefField);
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::StoreDynamic);
                let field = register_field_name(module, *field_idx)?;
                let self_instance = current_self_instance(frames)?;
                let reference = resolve_instance_field_ref(runtime, self_instance, field)?;
                let value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *value,
                )?;
                dynamic_store_ref(runtime, frames, &reference, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::Move { src, dest } => {
                let value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                write_register(registers, *dest, value)?;
            }
            RegisterInstr::LoadRef { dest, ref_idx } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadRef);
                let value = {
                    let value = peek_ref(runtime, module, frames, *ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let (value, cloned) = materialize_borrowed_value(value);
                    if cloned {
                        runtime
                            .vm_register_profile
                            .record_value_op(RegisterValueOpKind::ReadValueClone);
                    }
                    value
                };
                write_register(registers, *dest, value)?;
            }
            RegisterInstr::LoadRefAddr { dest, ref_idx } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadRefAddr);
                let reference =
                    load_ref_addr(module, frames, *ref_idx).map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, Value::Reference(Some(reference)))?;
            }
            RegisterInstr::StoreRef { ref_idx, src } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::StoreRef);
                let value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                store_ref(runtime, module, frames, *ref_idx, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::CallNative {
                kind,
                symbol_idx,
                args,
                dest,
            } => {
                native_call_stack.clear();
                for arg in args {
                    let value = read_register_with_counts(
                        &mut runtime.vm_register_profile,
                        registers,
                        remaining_register_reads,
                        *arg,
                    )?;
                    native_call_stack
                        .push(value)
                        .map_err(VmTrap::into_runtime_error)?;
                }
                let arg_count = u32::try_from(args.len())
                    .map_err(|_| invalid_bytecode("register-ir executor arg_count overflow"))?;
                let caller_depth =
                    depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                let frame = frames
                    .current_mut()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let result = execute_native_call(
                    runtime,
                    module,
                    frame,
                    native_call_stack,
                    caller_depth,
                    budget,
                    *kind,
                    *symbol_idx,
                    arg_count,
                )
                .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, result)?;
            }
            RegisterInstr::SizeOfType { type_idx, dest } => {
                let size = sizeof_type_from_table(&module.types, *type_idx)
                    .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                let size = i32::try_from(size)
                    .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow).into_runtime_error())?;
                write_register(registers, *dest, Value::DInt(size))?;
            }
            RegisterInstr::SizeOfValue { src, dest } => {
                let value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                let size = size_of_value(runtime.registry(), &value)
                    .map_err(sizeof_error_to_runtime)
                    .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                let size = i32::try_from(size)
                    .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow).into_runtime_error())?;
                write_register(registers, *dest, Value::DInt(size))?;
            }
            RegisterInstr::RefField {
                base,
                field_idx,
                dest,
            } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefField);
                let field = module
                    .strings
                    .get(*field_idx as usize)
                    .cloned()
                    .ok_or_else(|| {
                        VmTrap::BytecodeDecode(
                            format!("invalid index {field_idx} for string").into(),
                        )
                        .into_runtime_error()
                    })?;
                let base_value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *base,
                )?;
                let next = match base_value {
                    Value::Reference(Some(reference)) => {
                        dynamic_ref_field(runtime, frames, reference, field.clone())
                            .map_err(VmTrap::into_runtime_error)?
                    }
                    Value::Reference(None) => {
                        return Err(VmTrap::NullReference.into_runtime_error());
                    }
                    Value::Instance(instance_id) => {
                        runtime
                            .vm_register_profile
                            .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                        let next = runtime
                            .storage
                            .resolved_instance_field_ref(instance_id, field.as_str())
                            .ok_or_else(|| {
                                VmTrap::Runtime(RuntimeError::UndefinedField(field))
                                    .into_runtime_error()
                            })?;
                        next
                    }
                    _ => {
                        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error())
                    }
                };
                write_register(registers, *dest, Value::Reference(Some(next)))?;
            }
            RegisterInstr::RefIndex { base, index, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefIndex);
                let index_value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *index,
                )?;
                let index = index_to_i64(index_value).map_err(VmTrap::into_runtime_error)?;
                let reference = read_reference_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *base,
                )?;
                let next = dynamic_ref_index(runtime, frames, reference, index)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, Value::Reference(Some(next)))?;
            }
            RegisterInstr::LoadDynamic { reference, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadDynamic);
                let reference = read_reference_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *reference,
                )?;
                let value = dynamic_load_ref(runtime, frames, &reference)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, value)?;
            }
            RegisterInstr::StoreDynamic { reference, value } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::StoreDynamic);
                let reference = read_reference_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *reference,
                )?;
                let value = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *value,
                )?;
                dynamic_store_ref(runtime, frames, &reference, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::Unary { op, src, dest } => {
                let source = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                let result = apply_unary(*op, source)?;
                write_register(registers, *dest, result)?;
            }
            RegisterInstr::Binary {
                op,
                left,
                right,
                dest,
            } => {
                let left = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *left,
                )?;
                let right = read_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *right,
                )?;
                let result = apply_binary(*op, left, right, &runtime.profile)?;
                write_register(registers, *dest, result)?;
            }
            RegisterInstr::BinaryRefToRef {
                op,
                left_ref_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                let eval = {
                    let left = peek_ref(runtime, module, frames, *left_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = peek_ref(runtime, module, frames, *right_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::BinaryRefConstToRef {
                op,
                left_ref_idx,
                const_idx,
                dest_ref_idx,
            } => {
                let eval = {
                    let left = peek_ref(runtime, module, frames, *left_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = module
                        .consts
                        .get(*const_idx as usize)
                        .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ConstLoadClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::BinaryConstRefToRef {
                op,
                const_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                let eval = {
                    let left = module
                        .consts
                        .get(*const_idx as usize)
                        .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = peek_ref(runtime, module, frames, *right_ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ConstLoadClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::CmpRefConstJumpIf {
                op,
                ref_idx,
                const_idx,
                jump_if_true,
                target,
            } => {
                let eval = {
                    let left = peek_ref(runtime, module, frames, *ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    let right = module
                        .consts
                        .get(*const_idx as usize)
                        .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                        .map_err(VmTrap::into_runtime_error)?;
                    prepare_borrowed_binary_eval(*op, left, right)?
                };
                let result = match eval {
                    BorrowedBinaryEval::GuardHit(result) => result,
                    BorrowedBinaryEval::Materialized {
                        left,
                        left_cloned,
                        right,
                        right_cloned,
                    } => {
                        if left_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ReadValueClone);
                        }
                        if right_cloned {
                            runtime
                                .vm_register_profile
                                .record_value_op(RegisterValueOpKind::ConstLoadClone);
                        }
                        apply_binary(*op, left, right, &runtime.profile)?
                    }
                };
                let condition = match result {
                    Value::Bool(value) => value,
                    _ => return Err(VmTrap::ConditionNotBool.into_runtime_error()),
                };
                if condition == *jump_if_true {
                    consume_loop_budget_for_block_target(program, block, *target, budget)?;
                    control_target = Some(*target);
                    break;
                }
            }
            RegisterInstr::Jump { target } => {
                consume_loop_budget_for_block_target(program, block, *target, budget)?;
                control_target = Some(*target);
                break;
            }
            RegisterInstr::JumpIf {
                cond,
                jump_if_true,
                target,
            } => {
                let condition = read_bool_register_with_counts(
                    &mut runtime.vm_register_profile,
                    registers,
                    remaining_register_reads,
                    *cond,
                )?;
                if condition == *jump_if_true {
                    consume_loop_budget_for_block_target(program, block, *target, budget)?;
                    control_target = Some(*target);
                    break;
                }
            }
            RegisterInstr::Return => return Ok(RegisterBlockExecutionOutcome::ReturnFromPou),
            RegisterInstr::VmFallback { opcode, .. } => {
                return Err(invalid_bytecode(format!(
                    "register-ir executor encountered fallback opcode 0x{opcode:02X}",
                )));
            }
        }
    }
    Ok(RegisterBlockExecutionOutcome::Continue(control_target))
}
