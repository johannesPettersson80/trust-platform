//! The register tier's dynamic references (specification 34 ARCH-05; checklist C1-20): a load
//! through a reference gives the stack's operand, a store through one takes a register's operand
//! as the stack's `STORE_DYNAMIC` takes it, and a reference a register held gives its path back to
//! the engine's buffers once it is used up, as the stack's does (C1-10). Every result, charge and
//! refusal is the one a value had (TIME-08).

use super::operands::settle_register_places;
use super::registers::{checked_read_operand_with_counts, write_operand};
use super::*;
use crate::value::ValueRef;
use crate::vm::dispatch_refs::{
    dynamic_load_operand, dynamic_store_copy, dynamic_store_enum, dynamic_store_place,
    dynamic_store_text,
};
use crate::vm::stack::Operand;

/// Load through `reference` into `dest` as the stack's `LOAD_DYNAMIC` loads (C1-10c, C1-19a): a
/// compound as its place, an enumeration as its type and number, any other value read. The
/// reference's path goes back to the engine.
pub(crate) fn load_through(
    runtime: &mut impl RegisterContext,
    frames: &FrameStack,
    registers: &mut OperandStack,
    reference: ValueRef,
    dest: RegisterId,
) -> Result<(), RuntimeError> {
    let operand = dynamic_load_operand(&*runtime, frames, &reference);
    runtime.recycle_reference_path(reference.path);
    write_operand(
        registers,
        dest,
        operand.map_err(VmTrap::into_runtime_error)?,
    )
}

/// Store register `src`'s operand through `reference` as the stack's `STORE_DYNAMIC` stores its
/// operand (C1-10c, C1-12, C1-19a, C1-19b): a place's bytes copied, an enumeration's number, a
/// text's and a copy's bytes written, a value written. The reference's path goes back to the
/// engine. The caller settles the registers' places first, all but `src`.
pub(crate) fn store_through(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    frames: &mut FrameStack,
    registers: &mut OperandStack,
    reads: &mut [u32],
    reference: ValueRef,
    src: RegisterId,
) -> Result<(), RuntimeError> {
    let operand = match checked_read_operand_with_counts(runtime, registers, reads, src) {
        Ok(operand) => operand,
        Err(error) => {
            runtime.recycle_reference_path(reference.path);
            return Err(error);
        }
    };
    let stored = match operand {
        Operand::Value(value) => dynamic_store_ref(runtime, module, frames, &reference, value),
        Operand::Place(place) => dynamic_store_place(runtime, module, frames, &reference, place),
        Operand::Enum(temp) => dynamic_store_enum(runtime, module, frames, &reference, temp),
        Operand::Text(temp) => registers.text(temp).and_then(|text| {
            dynamic_store_text(runtime, module, frames, &reference, text, temp.wide())
        }),
        Operand::Copy(temp) => registers.copy_bytes(temp).and_then(|bytes| {
            dynamic_store_copy(runtime, module, frames, &reference, bytes, temp.place())
        }),
    };
    registers.release_temps();
    runtime.recycle_reference_path(reference.path);
    stored.map_err(VmTrap::into_runtime_error)
}

/// `STORE_DYNAMIC` of register `value` through the reference register `reference`: the registers'
/// places but `value`'s settle first, as the stack keeps only the place a store copies.
pub(crate) fn store_dynamic(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    frames: &mut FrameStack,
    registers: &mut OperandStack,
    reads: &mut [u32],
    reference: RegisterId,
    value: RegisterId,
) -> Result<(), RuntimeError> {
    settle_register_places(runtime, registers, Some(value))?;
    runtime
        .register_profile_mut()
        .record_ref_op(RegisterRefOpKind::StoreDynamic);
    let reference =
        checked_read_reference_register_with_counts(runtime, registers, reads, reference)?;
    store_through(runtime, module, frames, registers, reads, reference, value)
}
