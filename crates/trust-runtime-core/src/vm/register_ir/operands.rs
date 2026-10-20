//! The register instructions that take the stack's operand forms (specification 34 ARCH-05;
//! checklist C1-20): a constant, a load and a store of a static reference, a move, and `=` and `<>`
//! take a place, a text, an enumeration and a copy as they are, as the stack's instructions do, so
//! the register tier builds no value of a compound, a text or an enumeration where the stack builds
//! none. Every result, charge and refusal is the register tier's of the value it stood for
//! (TIME-08): a load charges one copy of the value, a read that is not a register's last charges
//! one more, and a store charges the write of the value.

use super::registers::{checked_read_operand_with_counts, write_operand, write_register};
use super::*;
use crate::vm::dispatch_refs::{
    load_ref_operand, store_ref_copy, store_ref_enum, store_ref_place, store_ref_text, view_operand,
};
use crate::vm::helpers::borrowed_value_is_cloned;
use crate::vm::stack::Operand;

/// Before an instruction that writes a block or calls, make every place among the registers but
/// `keep` a copy of the bytes its load read, as the stack does before a call (C1-19b), so that no
/// place shows a write that came after its load.
pub(crate) fn settle_register_places(
    runtime: &impl RegisterContext,
    registers: &mut OperandStack,
    keep: Option<RegisterId>,
) -> Result<(), RuntimeError> {
    if registers.places() == 0 {
        return Ok(());
    }
    registers
        .detach_register_places(keep.map(|register| register.index() as usize), |place| {
            runtime.place_copy(place)
        })
        .map_err(VmTrap::into_runtime_error)
}

/// Load the constant `const_idx` into `dest` as the stack loads it (C1-12, C1-19a): an enumeration
/// as its type and number, a `WSTRING` into the text area, any other value cloned, each charged as
/// one copy of the value.
pub(crate) fn load_constant(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    registers: &mut OperandStack,
    dest: RegisterId,
    const_idx: u32,
) -> Result<(), RuntimeError> {
    let value = module
        .consts()
        .get(const_idx as usize)
        .ok_or(VmTrap::InvalidConstIndex(const_idx))
        .map_err(VmTrap::into_runtime_error)?;
    load_constant_value(runtime, registers, dest, value)
}

/// Load the constant `value` into `dest`, as [`load_constant`] loads the constant it names.
pub(crate) fn load_constant_value(
    runtime: &mut impl RegisterContext,
    registers: &mut OperandStack,
    dest: RegisterId,
    value: &Value,
) -> Result<(), RuntimeError> {
    runtime.before_value_clone(value)?;
    if matches!(value, Value::Enum(_))
        && let Some(temp) = runtime.enum_constant(value)
    {
        return write_operand(registers, dest, Operand::Enum(temp));
    }
    if let Value::WString(text) = value
        && runtime.keeps_texts()
    {
        return registers
            .set_register_text(dest.index() as usize, text, true)
            .map_err(VmTrap::into_runtime_error);
    }
    // The profile counts the clones made (specification 12): an enumeration and a text loaded as
    // operands clone no value.
    if borrowed_value_is_cloned(value) {
        runtime
            .register_profile_mut()
            .record_value_op(RegisterValueOpKind::LoadConstant);
    }
    write_register(registers, dest, value.clone())
}

/// Load the static reference `ref_idx` into `dest` as the stack loads it (C1-10c, C1-19a): a
/// compound as its place, an enumeration as its type and number, any other value read, each charged
/// as one copy of the value.
pub(crate) fn load_reference(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    frames: &FrameStack,
    registers: &mut OperandStack,
    dest: RegisterId,
    ref_idx: u32,
) -> Result<(), RuntimeError> {
    let operand = match peek_ref_view(&*runtime, module, frames, ref_idx) {
        Ok(view) => {
            // A scalar, the most common load, goes into the register at once (C1-20).
            if let Some(value) = view.scalar()? {
                runtime.before_value_clone(&value)?;
                return write_register(registers, dest, value);
            }
            view_operand(&*runtime, &view, true)
        }
        // A string element has no view of its own; the stack's load reads it.
        Err(VmTrap::NullReference) => load_ref_operand(&*runtime, module, frames, ref_idx),
        Err(error) => Err(error),
    }
    .map_err(VmTrap::into_runtime_error)?;
    // The profile counts the clones made (specification 12): a place, an enumeration and a text
    // loaded as operands clone no value.
    if let Operand::Value(value) = &operand
        && borrowed_value_is_cloned(value)
    {
        runtime
            .register_profile_mut()
            .record_value_op(RegisterValueOpKind::ReadReference);
    }
    write_operand(registers, dest, operand)
}

/// Store register `src` through the static reference `ref_idx` as the stack stores its operand
/// (C1-10c, C1-12, C1-19a, C1-19b): a place's bytes copied, an enumeration's number, a text's and a
/// copy's bytes written, a value written; each charged as the write of the value.
pub(crate) fn store_register(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    frames: &mut FrameStack,
    registers: &mut OperandStack,
    reads: &mut [u32],
    ref_idx: u32,
    src: RegisterId,
) -> Result<(), RuntimeError> {
    settle_register_places(runtime, registers, Some(src))?;
    let operand = checked_read_operand_with_counts(runtime, registers, reads, src)?;
    let released = operand.holds_room();
    let stored = match operand {
        Operand::Value(value) => store_ref(runtime, module, frames, ref_idx, value),
        Operand::Place(place) => store_ref_place(runtime, module, frames, ref_idx, place),
        Operand::Enum(temp) => store_ref_enum(runtime, module, frames, ref_idx, temp),
        Operand::Text(temp) => registers
            .text(temp)
            .and_then(|text| store_ref_text(runtime, module, frames, ref_idx, text, temp.wide())),
        Operand::Copy(temp) => registers.copy_bytes(temp).and_then(|bytes| {
            store_ref_copy(runtime, module, frames, ref_idx, bytes, temp.place())
        }),
    };
    if released {
        registers.release_temps();
    }
    stored.map_err(VmTrap::into_runtime_error)
}

/// Move register `src`'s operand into `dest`: its last read moves it, an earlier one charges and
/// duplicates it.
pub(crate) fn move_register(
    runtime: &mut impl RegisterContext,
    registers: &mut OperandStack,
    reads: &mut [u32],
    src: RegisterId,
    dest: RegisterId,
) -> Result<(), RuntimeError> {
    let operand = checked_read_operand_with_counts(runtime, registers, reads, src)?;
    write_operand(registers, dest, operand)
}

/// Whether `=` or `<>` of `left` and `right` takes them as the stack's operands: one of them is a
/// place, a text, an enumeration or a copy.
pub(crate) fn compares_operands(
    registers: &OperandStack,
    op: BinaryOp,
    left: RegisterId,
    right: RegisterId,
) -> Result<bool, RuntimeError> {
    if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
        return Ok(false);
    }
    for register in [left, right] {
        if !matches!(
            registers
                .register(register.index() as usize)
                .map_err(VmTrap::into_runtime_error)?,
            Operand::Value(_)
        ) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `=` (`equal`) or `<>` of registers `left` and `right` as the stack compares its top two operands
/// (C1-19a, C1-19b): two enumerations by type and number, two texts where they lie, anything else as
/// values. The operands are pushed above the registers and the result is the stack's.
pub(crate) fn compare_registers(
    runtime: &mut impl RegisterContext,
    registers: &mut OperandStack,
    reads: &mut [u32],
    left: RegisterId,
    right: RegisterId,
    equal: bool,
) -> Result<Value, RuntimeError> {
    let left = checked_read_operand_with_counts(runtime, registers, reads, left)?;
    let right = checked_read_operand_with_counts(runtime, registers, reads, right)?;
    registers.truncate_to_registers();
    registers
        .push_any(left)
        .and_then(|()| registers.push_any(right))
        .map_err(VmTrap::into_runtime_error)?;
    super::fused::compare_top(&*runtime, registers, equal)
}
