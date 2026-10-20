use super::*;
use crate::memory::view::{StoreAccess, ValueView};
use crate::vm::stack::Operand;

/// The value register `register` holds. A register that holds a place, a text, an enumeration or a
/// copy (C1-20) is refused, as the stack refuses to peek one.
pub fn read_register_ref(
    registers: &OperandStack,
    register: RegisterId,
) -> Result<&Value, RuntimeError> {
    registers
        .register_value(register.index() as usize)
        .map_err(|_| {
            invalid_bytecode(format!(
                "register-ir executor read out-of-bounds register {}",
                register.index()
            ))
        })?
        .ok_or(RuntimeError::InvalidExecutionState)
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
        | RegisterInstr::Drop { .. }
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

/// Count one read of `register`: whether it is the register's last read of the block, which takes
/// its operand out.
fn count_read(
    remaining_register_reads: &mut [u32],
    register: RegisterId,
) -> Result<bool, RuntimeError> {
    remaining_register_reads
        .get_mut(register.index() as usize)
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
        })
}

/// Read `register`'s operand: its last read takes it out, an earlier one duplicates it (C1-20).
pub(crate) fn read_operand_with_counts(
    profile: &mut RegisterProfileState,
    registers: &mut OperandStack,
    remaining_register_reads: &mut [u32],
    register: RegisterId,
) -> Result<Operand, RuntimeError> {
    let index = register.index() as usize;
    if count_read(remaining_register_reads, register)? {
        profile.record_value_op(RegisterValueOpKind::RegisterReadMove);
        registers.take_register(index)
    } else {
        profile.record_value_op(RegisterValueOpKind::RegisterReadClone);
        registers.duplicate_register(index)
    }
    .map_err(VmTrap::into_runtime_error)
}

#[cfg(any(test, feature = "test-support"))]
pub fn read_register_with_counts(
    profile: &mut RegisterProfileState,
    registers: &mut OperandStack,
    remaining_register_reads: &mut [u32],
    register: RegisterId,
) -> Result<Value, RuntimeError> {
    match read_operand_with_counts(profile, registers, remaining_register_reads, register)? {
        Operand::Value(value) => Ok(value),
        _ => Err(RuntimeError::InvalidExecutionState),
    }
}

pub fn register_field_name(module: &VmModule, field_idx: u32) -> Result<&str, RuntimeError> {
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
    field: &str,
) -> Result<crate::value::ValueRef, RuntimeError> {
    runtime
        .storage()
        .resolved_instance_field_ref(instance_id, field)
        .ok_or_else(|| {
            VmTrap::Runtime(RuntimeError::UndefinedField(smol_str::SmolStr::new(field)))
                .into_runtime_error()
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
    registers: &mut OperandStack,
    remaining_register_reads: &mut [u32],
    register: RegisterId,
) -> Result<crate::value::ValueRef, RuntimeError> {
    match read_register_with_counts(profile, registers, remaining_register_reads, register)? {
        Value::Reference(Some(reference)) => Ok(reference),
        Value::Reference(None) => Err(VmTrap::NullReference.into_runtime_error()),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()),
    }
}

pub fn read_bool_register(
    registers: &OperandStack,
    register: RegisterId,
) -> Result<bool, RuntimeError> {
    match read_register_ref(registers, register)? {
        Value::Bool(value) => Ok(*value),
        _ => Err(VmTrap::ConditionNotBool.into_runtime_error()),
    }
}

#[cfg(feature = "test-support")]
pub fn read_reference_register(
    registers: &OperandStack,
    register: RegisterId,
) -> Result<crate::value::ValueRef, RuntimeError> {
    read_reference_register_ref(registers, register).cloned()
}

/// Borrow a reference operand without copying its compound path.
pub fn read_reference_register_ref(
    registers: &OperandStack,
    register: RegisterId,
) -> Result<&crate::value::ValueRef, RuntimeError> {
    match read_register_ref(registers, register)? {
        Value::Reference(Some(reference)) => Ok(reference),
        Value::Reference(None) => Err(VmTrap::NullReference.into_runtime_error()),
        _ => Err(VmTrap::Runtime(RuntimeError::TypeMismatch).into_runtime_error()),
    }
}

pub fn write_register(
    registers: &mut OperandStack,
    register: RegisterId,
    value: Value,
) -> Result<(), RuntimeError> {
    write_operand(registers, register, Operand::Value(value))
}

/// Put `operand` into `register` (C1-20).
pub(crate) fn write_operand(
    registers: &mut OperandStack,
    register: RegisterId,
    operand: Operand,
) -> Result<(), RuntimeError> {
    registers
        .set_register(register.index() as usize, operand)
        .map_err(|_| {
            invalid_bytecode(format!(
                "register-ir executor write out-of-bounds register {}",
                register.index()
            ))
        })
}

/// Charge the copy that a read of `register` other than its last makes: of the value, of the
/// place's value, of the enumeration's value, of the text, or of the copy's value, as the read of
/// the value it stands for charged before (TIME-08; C1-20).
fn charge_register_copy(
    runtime: &impl RegisterContext,
    registers: &OperandStack,
    register: RegisterId,
) -> Result<(), RuntimeError> {
    match registers
        .register(register.index() as usize)
        .map_err(VmTrap::into_runtime_error)?
    {
        Operand::Value(value) => runtime.before_value_clone(value),
        Operand::Place(place) => {
            let view = runtime
                .place_view(*place)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            runtime.before_view_clone(&view)
        }
        Operand::Enum(temp) => runtime.before_enum_clone(*temp),
        Operand::Text(temp) => runtime.before_text_clone(
            registers.text(*temp).map_err(VmTrap::into_runtime_error)?,
            temp.wide(),
        ),
        // A copy read twice is read into its value, as the stack does before a `DUP` (C1-19b).
        Operand::Copy(temp) => {
            let value = runtime.copy_value(
                registers
                    .copy_bytes(*temp)
                    .map_err(VmTrap::into_runtime_error)?,
                temp.type_id(),
            )?;
            runtime.before_value_clone(&value)
        }
    }
}

/// Read `register`'s operand for an instruction that takes it as it is (C1-20): the last read takes
/// it out, an earlier one charges its copy and duplicates it.
pub(crate) fn checked_read_operand_with_counts(
    runtime: &mut impl RegisterContext,
    registers: &mut OperandStack,
    reads: &mut [u32],
    register: RegisterId,
) -> Result<Operand, RuntimeError> {
    if reads.get(register.index() as usize).copied() != Some(1) {
        charge_register_copy(runtime, registers, register)?;
    }
    read_operand_with_counts(runtime.register_profile_mut(), registers, reads, register)
}

/// The value `operand`, read from `registers`, stands for: a place's, an enumeration's, a text's or
/// a copy's value is read as the stack reads one an instruction pops as a value (C1-20).
pub(crate) fn operand_value(
    runtime: &impl RegisterContext,
    registers: &mut OperandStack,
    operand: Operand,
) -> Result<Value, RuntimeError> {
    let value = kept_operand_value(runtime, registers, operand);
    registers.release_temps();
    value
}

/// The value `operand` stands for, read as [`operand_value`] reads it, its room in `registers`
/// left for the caller to give back: a caller that reads two operands taken out of the registers
/// reads both before it gives their room back.
pub(crate) fn kept_operand_value(
    runtime: &impl RegisterContext,
    registers: &OperandStack,
    operand: Operand,
) -> Result<Value, RuntimeError> {
    match operand {
        Operand::Value(value) => Ok(value),
        Operand::Place(place) => crate::vm::dispatch_refs::materialize_place(runtime, place),
        Operand::Enum(temp) => runtime.enum_value(temp),
        Operand::Text(temp) => registers
            .text_value(temp)
            .map_err(VmTrap::into_runtime_error),
        Operand::Copy(temp) => registers
            .copy_bytes(temp)
            .map_err(VmTrap::into_runtime_error)
            .and_then(|bytes| runtime.copy_value(bytes, temp.type_id())),
    }
}

pub fn checked_read_register_with_counts(
    runtime: &mut impl RegisterContext,
    registers: &mut OperandStack,
    reads: &mut [u32],
    register: RegisterId,
) -> Result<Value, RuntimeError> {
    let operand = checked_read_operand_with_counts(runtime, registers, reads, register)?;
    operand_value(runtime, registers, operand)
}

pub fn checked_read_reference_register_with_counts(
    runtime: &mut impl RegisterContext,
    registers: &mut OperandStack,
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
    registers: &mut OperandStack,
    reads: &mut [u32],
    register: RegisterId,
) -> Result<bool, RuntimeError> {
    match checked_read_register_with_counts(runtime, registers, reads, register)? {
        Value::Bool(value) => Ok(value),
        _ => Err(VmTrap::ConditionNotBool.into_runtime_error()),
    }
}

/// What a static reference gives a register operation (C1-18).
pub enum RefOperand<'a> {
    /// The value stored there: borrowed where it lies in the `Value` half, or read from its cells,
    /// where the read stands for that borrow. Neither is charged here, and whatever charges the
    /// borrow charges the read alike.
    Stored(Cow<'a, Value>),
    /// A value the load built and charged itself: a CHAR or WCHAR of a string.
    Loaded(Value),
}

impl core::ops::Deref for RefOperand<'_> {
    type Target = Value;

    fn deref(&self) -> &Value {
        match self {
            Self::Stored(value) => value,
            Self::Loaded(value) => value,
        }
    }
}

/// Most operands borrow existing storage; a string element is a scalar CHAR or
/// WCHAR materialized by the same reference path used by stack dispatch.
pub fn register_ref_operand<'a>(
    runtime: &'a impl RegisterContext,
    module: &'a VmModule,
    frames: &'a FrameStack,
    index: u32,
) -> Result<RefOperand<'a>, VmTrap> {
    match peek_ref_view(runtime, module, frames, index) {
        Ok(ValueView::Value(value)) => Ok(RefOperand::Stored(Cow::Borrowed(value))),
        // A cell view has no stored `Value` to borrow. Its copy stands for the borrow and is made
        // without a charge, as a borrow is not charged (C1-9d2a, item (b)).
        Ok(view) => view
            .materialize()
            .map(|value| RefOperand::Stored(Cow::Owned(value)))
            .map_err(VmTrap::Runtime),
        Err(VmTrap::NullReference) => {
            load_ref(runtime, module, frames, index).map(RefOperand::Loaded)
        }
        Err(error) => Err(error),
    }
}
