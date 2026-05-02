use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::runtime::vm::register_ir) fn execute_tier1_compiled_block(
    runtime: &mut Runtime,
    module: &VmModule,
    program: &RegisterProgram,
    source_block: &RegisterBlock,
    frames: &mut FrameStack,
    registers: &mut [Value],
    native_call_stack: &mut OperandStack,
    block: &Tier1CompiledBlock,
    budget: &mut usize,
    depth_offset: u32,
) -> Result<Tier1BlockExecutionOutcome, RuntimeError> {
    let mut control_target = None;
    for (instruction_index, instruction) in block.instructions.iter().enumerate() {
        if should_check_register_deadline(instruction_index)
            && deadline_exceeded(runtime.execution_deadline)
        {
            return Err(VmTrap::DeadlineExceeded.into_runtime_error());
        }

        match instruction {
            Tier1CompiledInstr::Nop => {}
            Tier1CompiledInstr::LoadConst { dest, value } => {
                let (value, cloned) = materialize_borrowed_value(value);
                if cloned {
                    runtime
                        .vm_register_profile
                        .record_value_op(RegisterValueOpKind::ConstLoadClone);
                }
                write_register(registers, *dest, value)?;
            }
            Tier1CompiledInstr::LoadNull { dest } => {
                write_register(registers, *dest, Value::Null)?;
            }
            Tier1CompiledInstr::LoadSelf { dest } => {
                let frame = frames
                    .current()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let self_instance = frame.runtime_instance.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                write_register(registers, *dest, Value::Instance(self_instance))?;
            }
            Tier1CompiledInstr::LoadSuper { dest } => {
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
            Tier1CompiledInstr::LoadSelfFieldDynamic { field, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefField);
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadDynamic);
                let self_instance = current_self_instance(frames)?;
                let value = load_instance_field_dynamic(runtime, frames, self_instance, field)?;
                write_register(registers, *dest, value)?;
            }
            Tier1CompiledInstr::StoreSelfFieldDynamic { field, value } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefField);
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::StoreDynamic);
                let self_instance = current_self_instance(frames)?;
                let reference = resolve_instance_field_ref(runtime, self_instance, field)?;
                let value = read_register(registers, *value)?;
                dynamic_store_ref(runtime, frames, &reference, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::Move { src, dest } => {
                let value = read_register(registers, *src)?;
                write_register(registers, *dest, value)?;
            }
            Tier1CompiledInstr::CallNative {
                kind,
                symbol_idx,
                args,
                dest,
            } => {
                native_call_stack.clear();
                for arg in args.iter() {
                    let value = read_register(registers, *arg)?;
                    native_call_stack
                        .push(value)
                        .map_err(VmTrap::into_runtime_error)?;
                }
                let arg_count = u32::try_from(args.len())
                    .map_err(|_| invalid_bytecode("tier-1 call-native arg_count overflow"))?;
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
            Tier1CompiledInstr::LoadRef { dest, ref_idx } => {
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
            Tier1CompiledInstr::LoadRefAddr { dest, ref_idx } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadRefAddr);
                let reference =
                    load_ref_addr(module, frames, *ref_idx).map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, Value::Reference(Some(reference)))?;
            }
            Tier1CompiledInstr::StoreRef { ref_idx, src } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::StoreRef);
                let value = read_register(registers, *src)?;
                store_ref(runtime, module, frames, *ref_idx, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::RefField { base, field, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefField);
                let next = match read_register_ref(registers, *base)? {
                    Value::Reference(Some(reference)) => {
                        dynamic_ref_field_borrowed(runtime, frames, reference, field.clone())
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
                            .resolved_instance_field_ref(*instance_id, field.as_str())
                            .ok_or_else(|| {
                                VmTrap::Runtime(RuntimeError::UndefinedField(field.clone()))
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
            Tier1CompiledInstr::RefIndex { base, index, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::RefIndex);
                let index = index_to_i64(read_register(registers, *index)?)
                    .map_err(VmTrap::into_runtime_error)?;
                let reference = read_reference_register(registers, *base)?;
                let next = dynamic_ref_index(runtime, frames, reference, index)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, Value::Reference(Some(next)))?;
            }
            Tier1CompiledInstr::LoadDynamic { reference, dest } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::LoadDynamic);
                let reference = read_reference_register(registers, *reference)?;
                let value = dynamic_load_ref(runtime, frames, &reference)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, value)?;
            }
            Tier1CompiledInstr::StoreDynamic { reference, value } => {
                runtime
                    .vm_register_profile
                    .record_ref_op(RegisterRefOpKind::StoreDynamic);
                let reference = read_reference_register(registers, *reference)?;
                let value = read_register(registers, *value)?;
                dynamic_store_ref(runtime, frames, &reference, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::Unary { op, src, dest } => {
                let source = read_register(registers, *src)?;
                let result = apply_unary(*op, source)?;
                write_register(registers, *dest, result)?;
            }
            Tier1CompiledInstr::BinaryDIntGuard {
                op,
                left,
                right,
                dest,
            } => {
                let left = read_register(registers, *left)?;
                let right = read_register(registers, *right)?;
                let result =
                    if let Some(result) = apply_dint_binary_guard_borrowed(*op, &left, &right)? {
                        result
                    } else {
                        apply_binary(*op, left, right, &runtime.profile)?
                    };
                write_register(registers, *dest, result)?;
            }
            Tier1CompiledInstr::BinaryRefToRefDIntGuard {
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
            Tier1CompiledInstr::BinaryRefConstToRefDIntGuard {
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
            Tier1CompiledInstr::BinaryConstRefToRefDIntGuard {
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
            Tier1CompiledInstr::CmpRefConstJumpIfDIntGuard {
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
                    consume_loop_budget_for_block_target(program, source_block, *target, budget)?;
                    control_target = Some(*target);
                    break;
                }
            }
            Tier1CompiledInstr::Jump { target } => {
                consume_loop_budget_for_block_target(program, source_block, *target, budget)?;
                control_target = Some(*target);
                break;
            }
            Tier1CompiledInstr::JumpIf {
                cond,
                jump_if_true,
                target,
            } => {
                let condition = read_bool_register(registers, *cond)?;
                if condition == *jump_if_true {
                    consume_loop_budget_for_block_target(program, source_block, *target, budget)?;
                    control_target = Some(*target);
                    break;
                }
            }
            Tier1CompiledInstr::Return => {
                return Ok(Tier1BlockExecutionOutcome::Executed(
                    RegisterBlockExecutionOutcome::ReturnFromPou,
                ));
            }
        }
    }
    Ok(Tier1BlockExecutionOutcome::Executed(
        RegisterBlockExecutionOutcome::Continue(control_target),
    ))
}
