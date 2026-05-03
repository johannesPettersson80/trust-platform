use super::*;

pub(super) fn read_register_ref(
    registers: &[Value],
    register: RegisterId,
) -> Result<&Value, RuntimeError> {
    registers.get(register.index() as usize).ok_or_else(|| {
        invalid_bytecode(format!(
            "register-ir executor read out-of-bounds register {}",
            register.index()
        ))
    })
}

pub(super) fn read_register(
    registers: &[Value],
    register: RegisterId,
) -> Result<Value, RuntimeError> {
    read_register_ref(registers, register).cloned()
}

pub(super) fn register_read_counts_by_block(program: &RegisterProgram) -> Vec<Vec<u32>> {
    program
        .blocks
        .iter()
        .map(|block| register_read_counts_for_block(program.max_registers, block))
        .collect()
}

fn register_read_counts_for_block(max_registers: u32, block: &RegisterBlock) -> Vec<u32> {
    let mut counts = vec![0_u32; max_registers as usize];
    for instruction in &block.instructions {
        match instruction {
            RegisterInstr::CallNative { args, .. } => {
                for arg in args {
                    increment_register_read_count(&mut counts, *arg);
                }
            }
            RegisterInstr::SizeOfValue { src, .. } => {
                increment_register_read_count(&mut counts, *src);
            }
            RegisterInstr::RefField { base, .. } => {
                increment_register_read_count(&mut counts, *base);
            }
            RegisterInstr::RefIndex { base, index, .. } => {
                increment_register_read_count(&mut counts, *base);
                increment_register_read_count(&mut counts, *index);
            }
            RegisterInstr::LoadDynamic { reference, .. } => {
                increment_register_read_count(&mut counts, *reference);
            }
            RegisterInstr::StoreDynamic { reference, value } => {
                increment_register_read_count(&mut counts, *reference);
                increment_register_read_count(&mut counts, *value);
            }
            RegisterInstr::StoreSelfFieldDynamic { value, .. } => {
                increment_register_read_count(&mut counts, *value);
            }
            RegisterInstr::Unary { src, .. } => {
                increment_register_read_count(&mut counts, *src);
            }
            RegisterInstr::Binary { left, right, .. } => {
                increment_register_read_count(&mut counts, *left);
                increment_register_read_count(&mut counts, *right);
            }
            RegisterInstr::StoreRef { src, .. } => {
                increment_register_read_count(&mut counts, *src);
            }
            RegisterInstr::Move { src, .. } => {
                increment_register_read_count(&mut counts, *src);
            }
            RegisterInstr::JumpIf { cond, .. } => {
                increment_register_read_count(&mut counts, *cond);
            }
            RegisterInstr::Nop
            | RegisterInstr::LoadConst { .. }
            | RegisterInstr::LoadRef { .. }
            | RegisterInstr::LoadRefAddr { .. }
            | RegisterInstr::LoadNull { .. }
            | RegisterInstr::LoadSelf { .. }
            | RegisterInstr::LoadSuper { .. }
            | RegisterInstr::LoadSelfFieldDynamic { .. }
            | RegisterInstr::SizeOfType { .. }
            | RegisterInstr::BinaryRefToRef { .. }
            | RegisterInstr::BinaryRefConstToRef { .. }
            | RegisterInstr::BinaryConstRefToRef { .. }
            | RegisterInstr::CmpRefConstJumpIf { .. }
            | RegisterInstr::Jump { .. }
            | RegisterInstr::Return
            | RegisterInstr::VmFallback { .. } => {}
        }
    }
    for slot in 0..block.entry_stack_depth {
        if let Some(count) = counts.get_mut(slot as usize) {
            *count = count.saturating_add(1);
        }
    }
    counts
}

fn increment_register_read_count(counts: &mut [u32], register: RegisterId) {
    if let Some(slot) = counts.get_mut(register.index() as usize) {
        *slot = slot.saturating_add(1);
    }
}

pub(super) fn read_register_with_counts(
    profile: &mut RegisterProfileState,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    register: RegisterId,
) -> Result<Value, RuntimeError> {
    let register_index = register.index() as usize;
    let consume = remaining_register_reads
        .get_mut(register_index)
        .ok_or_else(|| {
            invalid_bytecode(format!(
                "register-ir executor read-count out-of-bounds register {}",
                register.index()
            ))
        })
        .map(|slot| {
            if *slot == 0 {
                false
            } else {
                *slot = slot.saturating_sub(1);
                *slot == 0
            }
        })?;
    let slot = registers.get_mut(register_index).ok_or_else(|| {
        invalid_bytecode(format!(
            "register-ir executor read out-of-bounds register {}",
            register.index()
        ))
    })?;
    if consume {
        profile.record_value_op(RegisterValueOpKind::RegisterReadMove);
        Ok(std::mem::replace(slot, Value::Null))
    } else {
        profile.record_value_op(RegisterValueOpKind::RegisterReadClone);
        Ok(slot.clone())
    }
}

pub(super) fn register_field_name(
    module: &VmModule,
    field_idx: u32,
) -> Result<&smol_str::SmolStr, RuntimeError> {
    module.strings.get(field_idx as usize).ok_or_else(|| {
        VmTrap::BytecodeDecode(format!("invalid index {field_idx} for string").into())
            .into_runtime_error()
    })
}

pub(super) fn current_self_instance(frames: &FrameStack) -> Result<InstanceId, RuntimeError> {
    let frame = frames
        .current()
        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
    frame
        .runtime_instance
        .ok_or_else(|| VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error())
}

pub(super) fn resolve_instance_field_ref(
    runtime: &Runtime,
    instance_id: InstanceId,
    field: &smol_str::SmolStr,
) -> Result<crate::value::ValueRef, RuntimeError> {
    runtime
        .storage
        .resolved_instance_field_ref(instance_id, field.as_str())
        .ok_or_else(|| {
            VmTrap::Runtime(RuntimeError::UndefinedField(field.clone())).into_runtime_error()
        })
}

pub(super) fn load_instance_field_dynamic(
    runtime: &Runtime,
    frames: &FrameStack,
    instance_id: InstanceId,
    field: &smol_str::SmolStr,
) -> Result<Value, RuntimeError> {
    let reference = resolve_instance_field_ref(runtime, instance_id, field)?;
    dynamic_load_ref(runtime, frames, &reference).map_err(VmTrap::into_runtime_error)
}

pub(super) fn read_bool_register_with_counts(
    profile: &mut RegisterProfileState,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    register: RegisterId,
) -> Result<bool, RuntimeError> {
    match read_register_with_counts(profile, registers, remaining_register_reads, register)? {
        Value::Bool(value) => Ok(value),
        _ => Err(VmTrap::ConditionNotBool.into_runtime_error()),
    }
}

pub(super) fn read_reference_register_with_counts(
    profile: &mut RegisterProfileState,
    registers: &mut [Value],
    remaining_register_reads: &mut [u32],
    register: RegisterId,
) -> Result<crate::value::ValueRef, RuntimeError> {
    match read_register_with_counts(profile, registers, remaining_register_reads, register)? {
        Value::Reference(Some(reference)) => Ok(reference),
        Value::Reference(None) => Err(VmTrap::NullReference.into_runtime_error()),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()),
    }
}

pub(super) fn read_bool_register(
    registers: &[Value],
    register: RegisterId,
) -> Result<bool, RuntimeError> {
    match read_register_ref(registers, register)? {
        Value::Bool(value) => Ok(*value),
        _ => Err(VmTrap::ConditionNotBool.into_runtime_error()),
    }
}

pub(super) fn read_reference_register(
    registers: &[Value],
    register: RegisterId,
) -> Result<crate::value::ValueRef, RuntimeError> {
    match read_register_ref(registers, register)? {
        Value::Reference(Some(reference)) => Ok(reference.clone()),
        Value::Reference(None) => Err(VmTrap::NullReference.into_runtime_error()),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()),
    }
}

pub(super) fn write_register(
    registers: &mut [Value],
    register: RegisterId,
    value: Value,
) -> Result<(), RuntimeError> {
    let slot = registers
        .get_mut(register.index() as usize)
        .ok_or_else(|| {
            invalid_bytecode(format!(
                "register-ir executor write out-of-bounds register {}",
                register.index()
            ))
        })?;
    *slot = value;
    Ok(())
}
