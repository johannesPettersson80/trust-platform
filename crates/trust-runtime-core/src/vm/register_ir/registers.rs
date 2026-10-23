use super::*;

pub fn read_register_ref(
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

#[cfg(any(test, feature = "test-support"))]
pub fn register_read_counts_by_block(program: &RegisterProgram) -> Vec<Vec<u32>> {
    program
        .blocks
        .iter()
        .map(|block| register_read_counts_for_block(program.max_registers, block))
        .collect()
}

#[cfg(any(test, feature = "test-support"))]
fn register_read_counts_for_block(max_registers: u32, block: &RegisterBlock) -> Vec<u32> {
    let mut counts = vec![0_u32; max_registers as usize];
    for_each_register_read(block, |register| {
        increment_register_read_count(&mut counts, register)
    });
    counts
}

pub fn sparse_register_read_counts_by_block(program: &RegisterProgram) -> Vec<Vec<(u32, u32)>> {
    program
        .blocks
        .iter()
        .map(|block| {
            let mut counts = BTreeMap::<u32, u32>::new();
            for_each_register_read(block, |register| {
                let count = counts.entry(register.index()).or_insert(0);
                *count = count.saturating_add(1);
            });
            counts.into_iter().collect()
        })
        .collect()
}

fn for_each_register_read(block: &RegisterBlock, mut visit: impl FnMut(RegisterId)) {
    for instruction in &block.instructions {
        for_each_instruction_register_read(instruction, &mut visit);
    }
    for slot in 0..block.entry_stack_depth {
        visit(RegisterId(slot));
    }
}

/// Visit the operands read by one instruction, including normalization moves.
/// Fusion and execution accounting share this definition of a register use.
pub(super) fn for_each_instruction_register_read(
    instruction: &RegisterInstr,
    mut visit: impl FnMut(RegisterId),
) {
    match instruction {
        RegisterInstr::CallNative { args, .. } => {
            for arg in args {
                visit(*arg);
            }
        }
        RegisterInstr::SizeOfValue { src, .. } => {
            visit(*src);
        }
        RegisterInstr::RefField { base, .. } => {
            visit(*base);
        }
        RegisterInstr::RefIndex { base, index, .. } => {
            visit(*base);
            visit(*index);
        }
        RegisterInstr::LoadDynamic { reference, .. } => {
            visit(*reference);
        }
        RegisterInstr::StoreDynamic { reference, value } => {
            visit(*reference);
            visit(*value);
        }
        RegisterInstr::StoreSelfFieldDynamic { value, .. } => {
            visit(*value);
        }
        RegisterInstr::Unary { src, .. } => {
            visit(*src);
        }
        RegisterInstr::Binary { left, right, .. } => {
            visit(*left);
            visit(*right);
        }
        RegisterInstr::StoreRef { src, .. } => {
            visit(*src);
        }
        RegisterInstr::Move { src, .. } => {
            visit(*src);
        }
        RegisterInstr::JumpIf { cond, .. } => {
            visit(*cond);
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
        | RegisterInstr::ForStepZero
        | RegisterInstr::VmFallback { .. } => {}
    }
}

#[cfg(any(test, feature = "test-support"))]
fn increment_register_read_count(counts: &mut [u32], register: RegisterId) {
    if let Some(slot) = counts.get_mut(register.index() as usize) {
        *slot = slot.saturating_add(1);
    }
}

pub fn read_register_with_counts(
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
        Ok(core::mem::replace(slot, Value::Null))
    } else {
        profile.record_value_op(RegisterValueOpKind::RegisterReadClone);
        Ok(slot.clone())
    }
}

pub fn register_field_name(
    module: &VmModule,
    field_idx: u32,
) -> Result<&smol_str::SmolStr, RuntimeError> {
    module.strings().get(field_idx as usize).ok_or_else(|| {
        VmTrap::BytecodeDecode(format!("invalid index {field_idx} for string").into())
            .into_runtime_error()
    })
}

pub fn current_self_instance(frames: &FrameStack) -> Result<InstanceId, RuntimeError> {
    let frame = frames
        .current()
        .ok_or_else(|| VmTrap::CallStackUnderflow.into_runtime_error())?;
    frame
        .runtime_instance
        .ok_or_else(|| VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error())
}

pub fn resolve_instance_field_ref(
    runtime: &impl RegisterContext,
    instance_id: InstanceId,
    field: &smol_str::SmolStr,
) -> Result<crate::value::ValueRef, RuntimeError> {
    runtime
        .storage()
        .resolved_instance_field_ref(instance_id, field.as_str())
        .ok_or_else(|| {
            VmTrap::Runtime(RuntimeError::UndefinedField(field.clone())).into_runtime_error()
        })
}

#[cfg(feature = "test-support")]
pub fn load_instance_field_dynamic(
    runtime: &impl RegisterContext,
    frames: &FrameStack,
    instance_id: InstanceId,
    field: &smol_str::SmolStr,
) -> Result<Value, RuntimeError> {
    let reference = resolve_instance_field_ref(runtime, instance_id, field)?;
    dynamic_load_ref(runtime, frames, &reference).map_err(VmTrap::into_runtime_error)
}

#[cfg(feature = "test-support")]
pub fn read_reference_register_with_counts(
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

pub fn read_bool_register(registers: &[Value], register: RegisterId) -> Result<bool, RuntimeError> {
    match read_register_ref(registers, register)? {
        Value::Bool(value) => Ok(*value),
        _ => Err(VmTrap::ConditionNotBool.into_runtime_error()),
    }
}

#[cfg(feature = "test-support")]
pub fn read_reference_register(
    registers: &[Value],
    register: RegisterId,
) -> Result<crate::value::ValueRef, RuntimeError> {
    read_reference_register_ref(registers, register).cloned()
}

/// Borrow a reference operand without copying its compound path.
pub fn read_reference_register_ref(
    registers: &[Value],
    register: RegisterId,
) -> Result<&crate::value::ValueRef, RuntimeError> {
    match read_register_ref(registers, register)? {
        Value::Reference(Some(reference)) => Ok(reference),
        Value::Reference(None) => Err(VmTrap::NullReference.into_runtime_error()),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()),
    }
}

pub fn write_register(
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

pub fn checked_read_register_with_counts(
    runtime: &mut impl RegisterContext,
    registers: &mut [Value],
    reads: &mut [u32],
    register: RegisterId,
) -> Result<Value, RuntimeError> {
    if reads.get(register.index() as usize).copied() != Some(1) {
        runtime.before_value_clone(read_register_ref(registers, register)?)?;
    }
    read_register_with_counts(runtime.register_profile_mut(), registers, reads, register)
}

pub fn checked_read_reference_register_with_counts(
    runtime: &mut impl RegisterContext,
    registers: &mut [Value],
    reads: &mut [u32],
    register: RegisterId,
) -> Result<crate::value::ValueRef, RuntimeError> {
    match checked_read_register_with_counts(runtime, registers, reads, register)? {
        Value::Reference(Some(reference)) => Ok(reference),
        Value::Reference(None) => Err(VmTrap::NullReference.into_runtime_error()),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub fn checked_read_bool_register_with_counts(
    runtime: &mut impl RegisterContext,
    registers: &mut [Value],
    reads: &mut [u32],
    register: RegisterId,
) -> Result<bool, RuntimeError> {
    match checked_read_register_with_counts(runtime, registers, reads, register)? {
        Value::Bool(value) => Ok(value),
        _ => Err(VmTrap::ConditionNotBool.into_runtime_error()),
    }
}

/// Most operands borrow existing storage; a string element is a scalar CHAR or
/// WCHAR materialized by the same reference path used by stack dispatch.
pub fn register_ref_operand<'a>(
    runtime: &'a impl RegisterContext,
    module: &'a VmModule,
    frames: &'a FrameStack,
    index: u32,
) -> Result<Cow<'a, Value>, VmTrap> {
    match peek_ref(runtime, module, frames, index) {
        Ok(value) => Ok(Cow::Borrowed(value)),
        Err(VmTrap::NullReference) => load_ref(runtime, module, frames, index).map(Cow::Owned),
        Err(error) => Err(error),
    }
}
