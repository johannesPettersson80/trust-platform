//! `=` and `<>` of the fused reference operators on enumerations and texts (specification 34
//! ARCH-05; checklist C1-20). An operator that `=` or `<>` fuses with the loads of its operands takes
//! an enumeration or a text as the stack's operands, pushed above the registers as the stack's loads
//! push them and compared as the stack compares its top two operands (C1-19a, C1-19b), so neither is
//! read into a value; any other operand keeps the operator's value path. The cost steps and the
//! charges come in the order of the value path's, each charge the one of the value (TIME-08).

use super::registers::{kept_operand_value, ref_operand_of};
use super::*;
use crate::vm::dispatch_refs::load_ref_operand;
use crate::vm::stack::Operand;

/// The second operand of a fused operator.
#[derive(Debug, Clone, Copy)]
pub(crate) enum FusedSource {
    /// A static reference.
    Ref(u32),
    /// A constant.
    Const(u32),
}

/// The first operand of a fused operator, read from a static reference.
pub(crate) enum FusedLeft<'a> {
    /// An operand of the value path, charged by it.
    Value(RefOperand<'a>),
    /// An enumeration or a compound, a text among them, which `=` and `<>` take as the stack's
    /// operand, charged as the stack's load charges it.
    Operand(Operand),
}

/// Whether `op` is `=` or `<>`.
pub(crate) fn is_equality(op: BinaryOp) -> bool {
    matches!(op, BinaryOp::Eq | BinaryOp::Ne)
}

/// Read the first operand of a fused `op` from the static reference `ref_idx`: for `=` and `<>`, an
/// enumeration as its type and number and a compound as its place, as the stack's load reads them;
/// any other operand as the value path reads it.
pub(crate) fn fused_left_ref<'a>(
    runtime: &'a impl RegisterContext,
    module: &'a VmModule,
    frames: &'a FrameStack,
    ref_idx: u32,
    op: BinaryOp,
) -> Result<FusedLeft<'a>, RuntimeError> {
    let peeked = peek_ref_view(runtime, module, frames, ref_idx);
    if is_equality(op)
        && let Ok(view) = &peeked
    {
        // A scalar, the most common operand, keeps the value path with no other check.
        if let Some(value) = view.scalar()? {
            return Ok(FusedLeft::Value(RefOperand::Stored(Cow::Owned(value))));
        }
        if let Some(temp) = runtime.enum_operand(view) {
            runtime.before_enum_clone(temp)?;
            return Ok(FusedLeft::Operand(Operand::Enum(temp)));
        }
        if let Some(place) = runtime.place_of(view) {
            runtime.before_view_clone(view)?;
            return Ok(FusedLeft::Operand(Operand::Place(place)));
        }
    }
    ref_operand_of(runtime, module, frames, ref_idx, peeked)
        .map(FusedLeft::Value)
        .map_err(VmTrap::into_runtime_error)
}

/// Whether a fused `op` whose first operand is the constant `value` takes the stack's route: `=` or
/// `<>` of an enumeration or a text.
pub(crate) fn takes_operands(runtime: &impl RegisterContext, op: BinaryOp, value: &Value) -> bool {
    is_equality(op)
        && (runtime.enum_constant(value).is_some()
            || matches!(value, Value::String(_) | Value::WString(_)))
}

/// Push the constant `value` above the registers as the stack's `CONST` pushes it (C1-12, C1-19a):
/// an enumeration as its type and number, a `WSTRING` into the temporary area, any other value
/// cloned; charged as one copy of the value.
pub(crate) fn push_constant(
    runtime: &impl RegisterContext,
    registers: &mut OperandStack,
    value: &Value,
) -> Result<(), RuntimeError> {
    runtime.before_value_clone(value)?;
    if let Some(temp) = runtime.enum_constant(value) {
        return registers
            .push_any(Operand::Enum(temp))
            .map_err(VmTrap::into_runtime_error);
    }
    match value {
        Value::WString(text) if runtime.keeps_texts() => registers.push_text(text, true),
        _ => registers.push(value.clone()),
    }
    .map_err(VmTrap::into_runtime_error)
}

/// The rest of a fused `=` or `<>` that takes the stack's route, its first operand on top of the
/// registers: after the operator's cost step 1 its second operand, `right`, is pushed as the stack's
/// load pushes it, and after step 2 both are compared as the stack compares them.
pub(crate) fn fused_equality(
    runtime: &mut impl RegisterContext,
    module: &VmModule,
    frames: &FrameStack,
    registers: &mut OperandStack,
    steps: &[usize],
    right: FusedSource,
    op: BinaryOp,
) -> Result<Value, RuntimeError> {
    charge_instruction_cost_step(runtime, steps, 1)?;
    match right {
        FusedSource::Ref(ref_idx) => {
            let operand = load_ref_operand(&*runtime, module, frames, ref_idx)
                .map_err(VmTrap::into_runtime_error)?;
            registers
                .push_any(operand)
                .map_err(VmTrap::into_runtime_error)?;
        }
        FusedSource::Const(const_idx) => {
            let value = module
                .consts()
                .get(const_idx as usize)
                .ok_or(VmTrap::InvalidConstIndex(const_idx))
                .map_err(VmTrap::into_runtime_error)?;
            push_constant(&*runtime, registers, value)?;
        }
    }
    charge_instruction_cost_step(runtime, steps, 2)?;
    compare_top(&*runtime, registers, matches!(op, BinaryOp::Eq))
}

/// `=` (`equal`) or `<>` of the two operands on top of the registers as the stack compares its top
/// two operands (C1-19a, C1-19b): two enumerations by type and number, two texts where they lie,
/// anything else as values. Both are popped and their room given back; the result is the stack's.
pub(crate) fn compare_top(
    runtime: &impl RegisterContext,
    registers: &mut OperandStack,
    equal: bool,
) -> Result<Value, RuntimeError> {
    let both_enums = matches!(
        registers.top_operands(2),
        Some([Operand::Enum(_), Operand::Enum(_)])
    );
    if !both_enums && crate::vm::dispatch_text::equal_texts(runtime, registers).is_none() {
        let right = registers
            .pop_operand()
            .map_err(VmTrap::into_runtime_error)?;
        let left = registers
            .pop_operand()
            .map_err(VmTrap::into_runtime_error)?;
        let released = left.holds_room() || right.holds_room();
        let left = kept_operand_value(runtime, registers, left);
        let right = kept_operand_value(runtime, registers, right);
        if released {
            registers.release_temps();
        }
        let (left, right) = (left?, right?);
        registers
            .push(left)
            .and_then(|()| registers.push(right))
            .map_err(VmTrap::into_runtime_error)?;
    }
    crate::vm::dispatch::compare_equality(runtime, registers, equal)
        .and_then(|()| registers.pop())
        .map_err(VmTrap::into_runtime_error)
}
