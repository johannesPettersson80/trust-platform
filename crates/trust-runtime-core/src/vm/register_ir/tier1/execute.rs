use super::*;

#[allow(clippy::too_many_arguments)]
pub(in crate::vm::register_ir) fn execute_tier1_compiled_block_slice(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    _program: &RegisterProgram,
    source_block: &RegisterBlock,
    frames: &mut FrameStack,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    native_call_stack: &mut OperandStack,
    block: &Tier1CompiledBlock,
    depth_offset: u32,
    start_instruction: usize,
) -> Result<super::super::RegisterBlockStep, RuntimeError> {
    let mut control_target = None;
    for (instruction_index, instruction) in block
        .instructions
        .iter()
        .enumerate()
        .skip(start_instruction)
    {
        let instruction_cost_steps = source_block
            .instruction_cost_steps
            .get(instruction_index)
            .ok_or_else(|| invalid_bytecode("tier-1 instruction cost steps missing"))?;
        let instruction_cost_steps = instruction_cost_steps.as_slice();
        if instruction_cost_steps.len() == 1 {
            super::super::charge_instruction_cost(runtime, instruction_cost_steps[0])?;
        }

        match instruction {
            Tier1CompiledInstr::Nop => {}
            Tier1CompiledInstr::LoadConst { dest, value } => {
                runtime.before_value_clone(value)?;
                let (value, cloned) = materialize_borrowed_value(value);
                if cloned {
                    runtime
                        .register_profile_mut()
                        .record_value_op(RegisterValueOpKind::LoadConstant);
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
                    .storage()
                    .get_instance(self_instance)
                    .ok_or_else(|| VmTrap::NullReference.into_runtime_error())?;
                let super_instance = instance.parent.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                write_register(registers, *dest, Value::Instance(super_instance))?;
            }
            Tier1CompiledInstr::LoadSelfFieldDynamic { field, dest } => {
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let self_instance = current_self_instance(frames)?;
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let reference = resolve_instance_field_ref(runtime, self_instance, field)?;
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::RefField);
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::LoadDynamic);
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
                let value = dynamic_load_ref(runtime, frames, &reference)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, value)?;
            }
            Tier1CompiledInstr::StoreSelfFieldDynamic { field, value } => {
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let self_instance = current_self_instance(frames)?;
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let reference = resolve_instance_field_ref(runtime, self_instance, field)?;
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::RefField);
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::StoreDynamic);
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
                let value = checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *value,
                )?;
                dynamic_store_ref(runtime, module, frames, &reference, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::Move { src, dest } => {
                let value = checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
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
                    let value = checked_read_register_with_counts(
                        runtime,
                        registers,
                        remaining_register_reads,
                        *arg,
                    )?;
                    native_call_stack
                        .push(value)
                        .map_err(VmTrap::into_runtime_error)?;
                }
                let arg_count = u32::try_from(args.len())
                    .map_err(|_| invalid_bytecode("tier-1 call-native arg_count overflow"))?;
                let frame = frames
                    .current_mut()
                    .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
                let prepared = prepare_native_call(
                    runtime,
                    module,
                    frame,
                    native_call_stack,
                    *kind,
                    *symbol_idx,
                    arg_count,
                )
                .map_err(VmTrap::into_runtime_error)?;
                match prepared {
                    NativeCall::Immediate(value) => {
                        runtime.check_execution_deadline()?;
                        write_register(registers, *dest, value)?;
                    }
                    NativeCall::User(call) => {
                        if !crate::vm::dispatch::leaf_call_ready(runtime, call.pou_id) {
                            return Ok(super::super::RegisterBlockStep::Call {
                                call,
                                destination: *dest,
                                next_instruction: instruction_index + 1,
                            });
                        }
                        let value = crate::vm::dispatch::execute_leaf_call(
                            runtime,
                            module,
                            frames,
                            depth_offset,
                            call,
                        )?;
                        write_register(registers, *dest, value)?;
                    }
                }
            }
            Tier1CompiledInstr::LoadRef { dest, ref_idx } => {
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::LoadRef);
                let value = {
                    let value = register_ref_operand(runtime, module, frames, *ref_idx)
                        .map_err(VmTrap::into_runtime_error)?;
                    runtime.before_value_clone(&value)?;
                    let (value, cloned) = materialize_borrowed_value(&value);
                    if cloned {
                        runtime
                            .register_profile_mut()
                            .record_value_op(RegisterValueOpKind::ReadReference);
                    }
                    value
                };
                write_register(registers, *dest, value)?;
            }
            Tier1CompiledInstr::LoadRefAddr { dest, ref_idx } => {
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::LoadRefAddr);
                let reference = load_ref_addr(runtime, module, frames, *ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, Value::Reference(Some(reference)))?;
            }
            Tier1CompiledInstr::StoreRef { ref_idx, src } => {
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::StoreRef);
                let value = checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                store_ref(runtime, module, frames, *ref_idx, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::RefField { base, field, dest } => {
                runtime
                    .register_profile_mut()
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
                            .register_profile_mut()
                            .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                        runtime
                            .storage()
                            .resolved_instance_field_ref(*instance_id, field.as_str())
                            .ok_or_else(|| {
                                VmTrap::Runtime(RuntimeError::UndefinedField(field.clone()))
                                    .into_runtime_error()
                            })?
                    }
                    _ => {
                        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error())
                    }
                };
                write_register(registers, *dest, Value::Reference(Some(next)))?;
            }
            Tier1CompiledInstr::RefIndex { base, index, dest } => {
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::RefIndex);
                let index = index_to_i64(checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *index,
                )?)
                .map_err(VmTrap::into_runtime_error)?;
                let reference = read_reference_register_ref(registers, *base)?;
                runtime.before_reference_path_clone(reference.as_view())?;
                let next = dynamic_ref_index(runtime, frames, reference.clone(), index)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, Value::Reference(Some(next)))?;
            }
            Tier1CompiledInstr::LoadDynamic { reference, dest } => {
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::LoadDynamic);
                let reference = read_reference_register_ref(registers, *reference)?;
                let value = dynamic_load_ref(runtime, frames, reference)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, value)?;
            }
            Tier1CompiledInstr::StoreDynamic { reference, value } => {
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::StoreDynamic);
                let reference = checked_read_reference_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *reference,
                )?;
                let value = checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *value,
                )?;
                dynamic_store_ref(runtime, module, frames, &reference, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::Unary { op, src, dest } => {
                let source = checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                let result = apply_unary(*op, source)?;
                write_register(registers, *dest, result)?;
            }
            Tier1CompiledInstr::BinaryDIntGuard {
                op,
                left,
                right,
                dest,
            } => {
                let left = checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *left,
                )?;
                let right = checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *right,
                )?;
                let result =
                    if let Some(result) = apply_dint_binary_guard_borrowed(*op, &left, &right)? {
                        result
                    } else {
                        apply_binary(*op, left, right, runtime.profile())?
                    };
                write_register(registers, *dest, result)?;
            }
            Tier1CompiledInstr::BinaryRefToRefDIntGuard {
                op,
                left_ref_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let left = register_ref_operand(runtime, module, frames, *left_ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                let left = charge_borrowed_binary_operand(runtime, left)?;
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let right = register_ref_operand(runtime, module, frames, *right_ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                let right = charge_borrowed_binary_operand(runtime, right)?;
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
                let eval = prepare_borrowed_binary_eval(runtime, *op, &left, &right)?;
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
                                .register_profile_mut()
                                .record_value_op(RegisterValueOpKind::ReadReference);
                        }
                        if right_cloned {
                            runtime
                                .register_profile_mut()
                                .record_value_op(RegisterValueOpKind::ReadReference);
                        }
                        apply_binary(*op, left, right, runtime.profile())?
                    }
                };
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 3)?;
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::BinaryRefConstToRefDIntGuard {
                op,
                left_ref_idx,
                const_idx,
                dest_ref_idx,
            } => {
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let left = register_ref_operand(runtime, module, frames, *left_ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                let left = charge_borrowed_binary_operand(runtime, left)?;
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let right = module
                    .consts()
                    .get(*const_idx as usize)
                    .ok_or_else(|| VmTrap::InvalidConstIndex(*const_idx))
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.before_value_clone(right)?;
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
                let eval = prepare_borrowed_binary_eval(runtime, *op, &left, right)?;
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
                                .register_profile_mut()
                                .record_value_op(RegisterValueOpKind::ReadReference);
                        }
                        if right_cloned {
                            runtime
                                .register_profile_mut()
                                .record_value_op(RegisterValueOpKind::LoadConstant);
                        }
                        apply_binary(*op, left, right, runtime.profile())?
                    }
                };
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 3)?;
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            Tier1CompiledInstr::BinaryConstRefToRefDIntGuard {
                op,
                const_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let left = module
                    .consts()
                    .get(*const_idx as usize)
                    .ok_or_else(|| VmTrap::InvalidConstIndex(*const_idx))
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.before_value_clone(left)?;
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let right = register_ref_operand(runtime, module, frames, *right_ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                let right = charge_borrowed_binary_operand(runtime, right)?;
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
                let eval = prepare_borrowed_binary_eval(runtime, *op, left, &right)?;
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
                                .register_profile_mut()
                                .record_value_op(RegisterValueOpKind::LoadConstant);
                        }
                        if right_cloned {
                            runtime
                                .register_profile_mut()
                                .record_value_op(RegisterValueOpKind::ReadReference);
                        }
                        apply_binary(*op, left, right, runtime.profile())?
                    }
                };
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 3)?;
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
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let left = register_ref_operand(runtime, module, frames, *ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                let left = charge_borrowed_binary_operand(runtime, left)?;
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let right = module
                    .consts()
                    .get(*const_idx as usize)
                    .ok_or_else(|| VmTrap::InvalidConstIndex(*const_idx))
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.before_value_clone(right)?;
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
                let eval = prepare_borrowed_binary_eval(runtime, *op, &left, right)?;
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
                                .register_profile_mut()
                                .record_value_op(RegisterValueOpKind::ReadReference);
                        }
                        if right_cloned {
                            runtime
                                .register_profile_mut()
                                .record_value_op(RegisterValueOpKind::LoadConstant);
                        }
                        apply_binary(*op, left, right, runtime.profile())?
                    }
                };
                let condition = match result {
                    Value::Bool(value) => value,
                    _ => return Err(VmTrap::ConditionNotBool.into_runtime_error()),
                };
                super::super::charge_instruction_cost_step(runtime, instruction_cost_steps, 3)?;
                if condition == *jump_if_true {
                    control_target = Some(*target);
                    break;
                }
            }
            Tier1CompiledInstr::Jump { target } => {
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
                    control_target = Some(*target);
                    break;
                }
            }
            Tier1CompiledInstr::Return => {
                return Ok(super::super::RegisterBlockStep::ReturnFromPou);
            }
        }
    }
    Ok(super::super::RegisterBlockStep::Continue(control_target))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_tier1_compiled_block(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    program: &RegisterProgram,
    source_block: &RegisterBlock,
    frames: &mut FrameStack,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    native_call_stack: &mut OperandStack,
    block: &Tier1CompiledBlock,
    depth_offset: u32,
) -> Result<Tier1BlockExecutionOutcome, RuntimeError> {
    let mut instruction_index = 0;
    loop {
        match execute_tier1_compiled_block_slice(
            runtime,
            module,
            program,
            source_block,
            frames,
            registers,
            remaining_register_reads,
            native_call_stack,
            block,
            depth_offset,
            instruction_index,
        )? {
            super::super::RegisterBlockStep::Continue(target) => {
                return Ok(Tier1BlockExecutionOutcome::Executed(
                    RegisterBlockExecutionOutcome::Continue(target),
                ));
            }
            super::super::RegisterBlockStep::ReturnFromPou => {
                return Ok(Tier1BlockExecutionOutcome::Executed(
                    RegisterBlockExecutionOutcome::ReturnFromPou,
                ));
            }
            super::super::RegisterBlockStep::Call {
                call,
                destination,
                next_instruction,
            } => {
                let caller_depth =
                    depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                let frame = frames
                    .current_mut()
                    .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
                let result = call
                    .execute_sync(runtime, module, frame, caller_depth.saturating_add(1))
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, destination, result)?;
                instruction_index = next_instruction;
            }
        }
    }
}
