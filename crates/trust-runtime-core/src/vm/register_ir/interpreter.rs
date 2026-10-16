use super::*;

#[cfg(feature = "test-support")]
pub fn build_register_pou_result(
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
pub enum RegisterBlockExecutionOutcome {
    Continue(Option<BlockTarget>),
    ReturnFromPou,
}

pub(in crate::vm) enum RegisterBlockStep {
    Continue(Option<BlockTarget>),
    ReturnFromPou,
    Call {
        call: super::super::call::continuation::PreparedCall,
        destination: RegisterId,
        next_instruction: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier1BlockExecutionOutcome {
    Executed(RegisterBlockExecutionOutcome),
}

pub enum BorrowedBinaryEval {
    GuardHit(Value),
    Materialized {
        left: Value,
        left_cloned: bool,
        right: Value,
        right_cloned: bool,
    },
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_register_block_interpreted_slice(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    _program: &RegisterProgram,
    frames: &mut FrameStack,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    native_call_stack: &mut OperandStack,
    block: &RegisterBlock,
    depth_offset: u32,
    start_instruction: usize,
) -> Result<RegisterBlockStep, RuntimeError> {
    let mut control_target = None;
    for (instruction_index, instruction) in block
        .instructions
        .iter()
        .enumerate()
        .skip(start_instruction)
    {
        let instruction_cost_steps = block
            .instruction_cost_steps
            .get(instruction_index)
            .ok_or_else(|| invalid_bytecode("register-ir instruction cost steps missing"))?;
        let instruction_cost_steps = instruction_cost_steps.as_slice();
        if instruction_cost_steps.len() == 1 {
            charge_instruction_cost(runtime, instruction_cost_steps[0])?;
        }

        match instruction {
            RegisterInstr::Nop => {}
            RegisterInstr::LoadConst { dest, const_idx } => {
                let value = module
                    .consts()
                    .get(*const_idx as usize)
                    .map(|value| -> Result<Value, RuntimeError> {
                        runtime.before_value_clone(value)?;
                        let (value, cloned) = materialize_borrowed_value(value);
                        if cloned {
                            runtime
                                .register_profile_mut()
                                .record_value_op(RegisterValueOpKind::LoadConstant);
                        }
                        Ok(value)
                    })
                    .transpose()?
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
                    .storage()
                    .get_instance(self_instance)
                    .ok_or_else(|| VmTrap::NullReference.into_runtime_error())?;
                let super_instance = instance.parent.ok_or_else(|| {
                    VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()
                })?;
                write_register(registers, *dest, Value::Instance(super_instance))?;
            }
            RegisterInstr::LoadSelfFieldDynamic { field_idx, dest } => {
                charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let self_instance = current_self_instance(frames)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let field = register_field_name(module, *field_idx)?;
                let reference = resolve_instance_field_ref(runtime, self_instance, field)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::RefField);
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::LoadDynamic);
                let value = dynamic_load_ref(runtime, frames, &reference)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, value)?;
            }
            RegisterInstr::StoreSelfFieldDynamic { field_idx, value } => {
                charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let self_instance = current_self_instance(frames)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let field = register_field_name(module, *field_idx)?;
                let reference = resolve_instance_field_ref(runtime, self_instance, field)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::RefField);
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::StoreDynamic);
                let value = checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *value,
                )?;
                dynamic_store_ref(runtime, module, frames, &reference, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::Move { src, dest } => {
                let value = checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                write_register(registers, *dest, value)?;
            }
            RegisterInstr::LoadRef { dest, ref_idx } => {
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
            RegisterInstr::LoadRefAddr { dest, ref_idx } => {
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::LoadRefAddr);
                let reference = load_ref_addr(runtime, module, frames, *ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, *dest, Value::Reference(Some(reference)))?;
            }
            RegisterInstr::StoreRef { ref_idx, src } => {
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
            RegisterInstr::CallNative {
                kind,
                symbol_idx,
                args,
                dest,
            } => {
                native_call_stack.clear();
                for arg in args {
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
                    .map_err(|_| invalid_bytecode("register-ir executor arg_count overflow"))?;
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
                            return Ok(RegisterBlockStep::Call {
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
            RegisterInstr::SizeOfType { type_idx, dest } => {
                let size = crate::vm::sizeof_type_from_table_with_abi(
                    module.types(),
                    *type_idx,
                    module.storage_abi(),
                    &mut |units| runtime.charge_execution_work(units),
                )
                .map_err(|err| VmTrap::Runtime(err).into_runtime_error())?;
                let size = i32::try_from(size)
                    .map_err(|_| VmTrap::Runtime(RuntimeError::Overflow).into_runtime_error())?;
                write_register(registers, *dest, Value::DInt(size))?;
            }
            RegisterInstr::SizeOfValue { src, dest } => {
                let value = checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *src,
                )?;
                let size = runtime.sizeof_value(&value)?;
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
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::RefField);
                let field = module
                    .strings()
                    .get(*field_idx as usize)
                    .cloned()
                    .ok_or_else(|| {
                        VmTrap::BytecodeDecode(
                            format!("invalid index {field_idx} for string").into(),
                        )
                        .into_runtime_error()
                    })?;
                let base_value = checked_read_register_with_counts(
                    runtime,
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
                            .register_profile_mut()
                            .record_ref_op(RegisterRefOpKind::InstanceFieldLookup);
                        runtime
                            .storage()
                            .resolved_instance_field_ref(instance_id, field.as_str())
                            .ok_or_else(|| {
                                VmTrap::Runtime(RuntimeError::UndefinedField(field))
                                    .into_runtime_error()
                            })?
                    }
                    _ => {
                        return Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error())
                    }
                };
                write_register(registers, *dest, Value::Reference(Some(next)))?;
            }
            RegisterInstr::RefIndex { base, index, dest } => {
                runtime
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::RefIndex);
                let index_value = checked_read_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *index,
                )?;
                let index = index_to_i64(index_value).map_err(VmTrap::into_runtime_error)?;
                let reference = checked_read_reference_register_with_counts(
                    runtime,
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
                    .register_profile_mut()
                    .record_ref_op(RegisterRefOpKind::LoadDynamic);
                let reference = checked_read_reference_register_with_counts(
                    runtime,
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
            RegisterInstr::Unary { op, src, dest } => {
                let source = checked_read_register_with_counts(
                    runtime,
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
                let result = apply_binary(*op, left, right, runtime.profile())?;
                write_register(registers, *dest, result)?;
            }
            RegisterInstr::BinaryRefToRef {
                op,
                left_ref_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let left = register_ref_operand(runtime, module, frames, *left_ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                let left = charge_borrowed_binary_operand(runtime, left)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let right = register_ref_operand(runtime, module, frames, *right_ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                let right = charge_borrowed_binary_operand(runtime, right)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
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
                charge_instruction_cost_step(runtime, instruction_cost_steps, 3)?;
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::BinaryRefConstToRef {
                op,
                left_ref_idx,
                const_idx,
                dest_ref_idx,
            } => {
                charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let left = register_ref_operand(runtime, module, frames, *left_ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                let left = charge_borrowed_binary_operand(runtime, left)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let right = module
                    .consts()
                    .get(*const_idx as usize)
                    .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.before_value_clone(right)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
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
                charge_instruction_cost_step(runtime, instruction_cost_steps, 3)?;
                store_ref(runtime, module, frames, *dest_ref_idx, result)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            RegisterInstr::BinaryConstRefToRef {
                op,
                const_idx,
                right_ref_idx,
                dest_ref_idx,
            } => {
                charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let left = module
                    .consts()
                    .get(*const_idx as usize)
                    .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.before_value_clone(left)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let right = register_ref_operand(runtime, module, frames, *right_ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                let right = charge_borrowed_binary_operand(runtime, right)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
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
                charge_instruction_cost_step(runtime, instruction_cost_steps, 3)?;
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
                charge_instruction_cost_step(runtime, instruction_cost_steps, 0)?;
                let left = register_ref_operand(runtime, module, frames, *ref_idx)
                    .map_err(VmTrap::into_runtime_error)?;
                let left = charge_borrowed_binary_operand(runtime, left)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 1)?;
                let right = module
                    .consts()
                    .get(*const_idx as usize)
                    .ok_or(VmTrap::InvalidConstIndex(*const_idx))
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.before_value_clone(right)?;
                charge_instruction_cost_step(runtime, instruction_cost_steps, 2)?;
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
                charge_instruction_cost_step(runtime, instruction_cost_steps, 3)?;
                if condition == *jump_if_true {
                    control_target = Some(*target);
                    break;
                }
            }
            RegisterInstr::Jump { target } => {
                control_target = Some(*target);
                break;
            }
            RegisterInstr::JumpIf {
                cond,
                jump_if_true,
                target,
            } => {
                let condition = checked_read_bool_register_with_counts(
                    runtime,
                    registers,
                    remaining_register_reads,
                    *cond,
                )?;
                if condition == *jump_if_true {
                    control_target = Some(*target);
                    break;
                }
            }
            RegisterInstr::Return => return Ok(RegisterBlockStep::ReturnFromPou),
            RegisterInstr::ForStepZero => {
                return Err(VmTrap::ForStepZero.into_runtime_error());
            }
            RegisterInstr::VmFallback { opcode, .. } => {
                return Err(invalid_bytecode(format!(
                    "register-ir executor encountered fallback opcode 0x{opcode:02X}",
                )));
            }
        }
    }
    Ok(RegisterBlockStep::Continue(control_target))
}

#[allow(clippy::too_many_arguments)]
pub fn execute_register_block_interpreted(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    program: &RegisterProgram,
    frames: &mut FrameStack,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    native_call_stack: &mut OperandStack,
    block: &RegisterBlock,
    depth_offset: u32,
) -> Result<RegisterBlockExecutionOutcome, RuntimeError> {
    let mut instruction_index = 0;
    loop {
        match execute_register_block_interpreted_slice(
            runtime,
            module,
            program,
            frames,
            registers,
            remaining_register_reads,
            native_call_stack,
            block,
            depth_offset,
            instruction_index,
        )? {
            RegisterBlockStep::Continue(target) => {
                return Ok(RegisterBlockExecutionOutcome::Continue(target));
            }
            RegisterBlockStep::ReturnFromPou => {
                return Ok(RegisterBlockExecutionOutcome::ReturnFromPou);
            }
            RegisterBlockStep::Call {
                call,
                destination,
                next_instruction,
            } => {
                let caller_depth =
                    depth_offset.saturating_add(frames.len().saturating_sub(1) as u32);
                let frame = frames
                    .current_mut()
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let result = call
                    .execute_sync(runtime, module, frame, caller_depth.saturating_add(1))
                    .map_err(VmTrap::into_runtime_error)?;
                write_register(registers, destination, result)?;
                instruction_index = next_instruction;
            }
        }
    }
}
