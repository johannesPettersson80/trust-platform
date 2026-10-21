//! The loop that runs a body's compact code (checklist P1-F; `vm::compact`).
//!
//! The dispatcher enters it at a frame's first instruction and finishes the frame itself when the
//! loop reaches the body's end, so the frame's entry and exit are the general path's. Before the
//! work of each instruction an operation covers, the loop records that instruction as the fault
//! site and charges its unit (MEM-05, TIME-08). An operation does its instructions' work with the
//! functions the dispatcher's handlers use, except a load or a store of a scalar variable, which
//! reads or writes the variable's leaf where it lies; a store of a value the general path would
//! convert or refuse is given to the general store.

use super::super::call::stdlib::convert_argument;
use super::super::compact::{CompactCode, Op, OpKind, Slot};
use super::super::context::ExecutionContext;
use super::super::dispatch_ops::{execute_binary, execute_unary};
use super::super::dispatch_refs::{
    dynamic_load_operand, dynamic_ref_field, dynamic_store_ref, pop_reference, store_ref,
};
use super::super::errors::invalid_field_string_index;
use super::super::module::VmModule;
use super::super::module::VmNativeSymbolSpec;
use super::super::type_policy::primitive_value_matches;
use super::super::{FrameStack, Operand, OperandStack, VmTrap};
use super::continuation::{start_call, CallStart};
use super::{compare_equality, push_loaded};
use crate::error::RuntimeError;
use crate::fault::FaultSite;
use crate::memory::layout::{ByteRange, LayoutRegion};
use crate::memory::view::StoreAccess;
use crate::memory::MemoryLocation;
use crate::value::{RefPath, Value, ValueRef};

/// Run `code`, the compact code of the current frame's body, from its first operation to the
/// body's end, which `end` is; `pc` is then `end`. `pou` is the frame's POU.
#[allow(clippy::too_many_arguments)]
pub(super) fn run(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    code: &CompactCode,
    operand_stack: &mut OperandStack,
    frames: &mut FrameStack,
    fault_site: &mut Option<FaultSite>,
    pou: u32,
    pc: &mut usize,
    end: usize,
) -> Result<(), RuntimeError> {
    let instance = frames.current().and_then(|frame| frame.runtime_instance);
    let own = instance.and_then(|instance| runtime.instance_bytes(instance));
    let mut next = 0;
    while let Some(op) = code.ops.get(next) {
        next += 1;
        let at = op.at;
        match op.kind {
            OpKind::Const => {
                begin(runtime, fault_site, pou, at)?;
                let value = module
                    .consts
                    .get(op.arg as usize)
                    .ok_or(VmTrap::InvalidConstIndex(op.arg))
                    .map_err(VmTrap::into_runtime_error)?;
                runtime.before_value_clone(value)?;
                push(operand_stack, value.clone())?;
            }
            OpKind::LoadOwn => {
                // LOAD_SELF, REF_FIELD and LOAD.
                begin_each(runtime, fault_site, pou, at, [0, 1, 6])?;
                let value = read(runtime, own, slot(code, op.arg)?)?;
                runtime.before_value_clone(&value)?;
                push(operand_stack, value)?;
            }
            OpKind::StoreOwn => {
                // LOAD_SELF, REF_FIELD, SWAP and STORE.
                begin_each(runtime, fault_site, pou, at, [0, 1, 6, 7])?;
                let slot = slot(code, op.arg)?;
                let value = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                if !write(runtime, own, slot, &value) {
                    let reference = own_ref(runtime, instance, slot)?;
                    dynamic_store_ref(runtime, module, frames, &reference, value)
                        .map_err(VmTrap::into_runtime_error)?;
                    runtime.recycle_reference_path(reference.path);
                }
            }
            OpKind::OwnRef => {
                // LOAD_SELF and REF_FIELD: the reference REF_FIELD gives to an own variable.
                begin_each(runtime, fault_site, pou, at, [0, 1])?;
                own_reference(runtime, operand_stack, instance, slot(code, op.arg)?)?;
            }
            OpKind::LoadInstance => {
                // LOAD_SELF, REF_FIELD and LOAD of an own function block instance: the instance its
                // leaf names, or, when it names none, the general path's three instructions.
                let holder = slot(code, op.arg)?;
                if let Some(value) = own_instance(runtime, own, holder).map(Value::Instance) {
                    begin_each(runtime, fault_site, pou, at, [0, 1, 6])?;
                    runtime.before_value_clone(&value)?;
                    push(operand_stack, value)?;
                } else {
                    begin_each(runtime, fault_site, pou, at, [0, 1])?;
                    own_reference(runtime, operand_stack, instance, holder)?;
                    begin(runtime, fault_site, pou, at + 6)?;
                    load(runtime, frames, operand_stack)?;
                }
            }
            OpKind::OwnValue => {
                // LOAD_SELF and REF_FIELD of an own scalar that the conversion after them reads as
                // its target: its value, or, when it is not read where it lies, the reference.
                begin_each(runtime, fault_site, pou, at, [0, 1])?;
                let slot = slot(code, op.arg)?;
                match read(runtime, own, slot) {
                    Ok(value) => push(operand_stack, value)?,
                    Err(_) => own_reference(runtime, operand_stack, instance, slot)?,
                }
            }
            OpKind::MemberValue => {
                // LOAD_SELF, REF_FIELD of an own function block instance and REF_FIELD of its
                // member, which the conversion after them reads as its target: the member's value,
                // or, when it is not read where it lies, the reference REF_FIELD gives.
                let holder = slot(code, op.arg)?;
                let member = slot(code, op.arg + 1)?;
                if let Some(value) = member_value(runtime, own, holder, member) {
                    begin_each(runtime, fault_site, pou, at, [0, 1, 6])?;
                    push(operand_stack, value)?;
                } else {
                    begin_each(runtime, fault_site, pou, at, [0, 1])?;
                    own_reference(runtime, operand_stack, instance, holder)?;
                    begin(runtime, fault_site, pou, at + 6)?;
                    ref_field(runtime, module, frames, operand_stack, member.name)?;
                }
            }
            OpKind::LoadMember => {
                // LOAD_SELF, REF_FIELD of an own function block instance, REF_FIELD of its member
                // and LOAD. A member that is not read where it lies takes the general path's four
                // instructions.
                let holder = slot(code, op.arg)?;
                let member = slot(code, op.arg + 1)?;
                if let Some(value) = member_value(runtime, own, holder, member) {
                    begin_each(runtime, fault_site, pou, at, [0, 1, 6, 11])?;
                    runtime.before_value_clone(&value)?;
                    push(operand_stack, value)?;
                } else {
                    begin_each(runtime, fault_site, pou, at, [0, 1])?;
                    own_reference(runtime, operand_stack, instance, holder)?;
                    begin(runtime, fault_site, pou, at + 6)?;
                    ref_field(runtime, module, frames, operand_stack, member.name)?;
                    begin(runtime, fault_site, pou, at + 11)?;
                    load(runtime, frames, operand_stack)?;
                }
            }
            OpKind::RefField => {
                begin(runtime, fault_site, pou, at)?;
                ref_field(runtime, module, frames, operand_stack, op.arg)?;
            }
            OpKind::Load => {
                begin(runtime, fault_site, pou, at)?;
                load(runtime, frames, operand_stack)?;
            }
            OpKind::Call | OpKind::CallBlock | OpKind::Convert => {
                begin(runtime, fault_site, pou, at)?;
                native_call(runtime, module, code, frames, operand_stack, *op)?;
            }
            OpKind::LoadGlobal => {
                begin(runtime, fault_site, pou, at)?;
                let value = read(runtime, globals(), slot(code, op.arg)?)?;
                runtime.before_value_clone(&value)?;
                push(operand_stack, value)?;
            }
            OpKind::StoreGlobal => {
                begin(runtime, fault_site, pou, at)?;
                let slot = slot(code, op.arg)?;
                let value = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
                if !write(runtime, globals(), slot, &value) {
                    store_ref(runtime, module, frames, slot.name, value)
                        .map_err(VmTrap::into_runtime_error)?;
                }
            }
            OpKind::Binary(binary) => {
                begin(runtime, fault_site, pou, at)?;
                execute_binary(runtime.profile(), operand_stack, binary)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            OpKind::Equality(equal) => {
                begin(runtime, fault_site, pou, at)?;
                compare_equality(runtime, operand_stack, equal)
                    .map_err(VmTrap::into_runtime_error)?;
            }
            OpKind::Unary(unary) => {
                begin(runtime, fault_site, pou, at)?;
                execute_unary(operand_stack, unary).map_err(VmTrap::into_runtime_error)?;
            }
            OpKind::Jump => {
                begin(runtime, fault_site, pou, at)?;
                next = op.arg as usize;
            }
            OpKind::JumpIf(when) => {
                begin(runtime, fault_site, pou, at)?;
                match operand_stack.pop().map_err(VmTrap::into_runtime_error)? {
                    Value::Bool(condition) if condition == when => next = op.arg as usize,
                    Value::Bool(_) => {}
                    _ => return Err(VmTrap::ConditionNotBool.into_runtime_error()),
                }
            }
            OpKind::Dup => {
                begin(runtime, fault_site, pou, at)?;
                let value = operand_stack.peek().map_err(VmTrap::into_runtime_error)?;
                runtime.before_value_clone(value)?;
                let value = value.clone();
                push(operand_stack, value)?;
            }
            OpKind::Pop => {
                begin(runtime, fault_site, pou, at)?;
                discard(runtime, operand_stack)?;
            }
            OpKind::Swap => {
                begin(runtime, fault_site, pou, at)?;
                operand_stack
                    .swap_top()
                    .map_err(VmTrap::into_runtime_error)?;
            }
            OpKind::Skip => begin(runtime, fault_site, pou, at)?,
        }
    }
    *pc = end;
    Ok(())
}

/// The `CALL_NATIVE` of `op`: a block call or a conversion where the compact code runs it, and
/// any other call, or one of those it does not run, as the dispatcher runs it.
fn native_call(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    code: &CompactCode,
    frames: &mut FrameStack,
    operand_stack: &mut OperandStack,
    op: Op,
) -> Result<(), RuntimeError> {
    let done = match op.kind {
        OpKind::CallBlock => {
            let Some(&block) = code.blocks.get(op.arg as usize) else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            super::compact_block::call_block(runtime, code, operand_stack, block)?
        }
        // A text and a reference, which the call reads as a target, go through the call.
        OpKind::Convert
            if operand_stack.peek().is_ok_and(|argument| {
                !matches!(
                    argument,
                    Value::String(_) | Value::WString(_) | Value::Reference(_)
                )
            }) =>
        {
            convert(runtime, module, operand_stack, op.arg)?;
            true
        }
        _ => false,
    };
    if done {
        Ok(())
    } else {
        call(runtime, module, frames, operand_stack, op.at)
    }
}

/// The `CALL_NATIVE` at `at`, which finishes at once, as the dispatcher runs it: its value is
/// pushed.
fn call(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    frames: &mut FrameStack,
    operand_stack: &mut OperandStack,
    at: u32,
) -> Result<(), RuntimeError> {
    let word = |offset: u32| -> Result<u32, RuntimeError> {
        let start = (at + offset) as usize;
        module
            .code
            .get(start..start + 4)
            .and_then(|bytes| bytes.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or(RuntimeError::InvalidExecutionState)
    };
    let (kind, symbol, count) = (word(1)?, word(5)?, word(9)?);
    let plan = runtime.call_plan(at);
    match start_call(
        runtime,
        module,
        frames,
        operand_stack,
        kind,
        symbol,
        count,
        plan.as_deref(),
    )? {
        CallStart::Immediate(value) => push(operand_stack, value),
        // Translation holds only calls that finish at once.
        CallStart::User(_) => Err(RuntimeError::InvalidExecutionState),
    }
}

/// The conversion call of `symbol` on the scalar on top, as the general call runs it (checklist
/// P1-5): the deadline is checked, the argument is bound and converted with the call's charges, the
/// deadline is checked again once the call has its value, and the value is pushed.
fn convert(
    runtime: &mut impl ExecutionContext,
    module: &VmModule,
    operand_stack: &mut OperandStack,
    symbol: u32,
) -> Result<(), RuntimeError> {
    if runtime.deadline_exceeded() {
        return Err(VmTrap::DeadlineExceeded.into_runtime_error());
    }
    let Ok(VmNativeSymbolSpec::Parsed {
        conversion_spec: Some(spec),
        ..
    }) = module.native_symbol_spec(symbol)
    else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    let argument = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
    let value = convert_argument(runtime, *spec, &argument).map_err(VmTrap::into_runtime_error)?;
    runtime.check_execution_deadline()?;
    push(operand_stack, value)
}

/// What the dispatcher does before the instruction at `at`: it records the instruction as the
/// fault site and charges its unit.
fn begin(
    runtime: &impl ExecutionContext,
    fault_site: &mut Option<FaultSite>,
    pou: u32,
    at: u32,
) -> Result<(), RuntimeError> {
    *fault_site = Some(FaultSite { pou, pc: at });
    runtime.charge_execution_work(1)
}

/// [`begin`] for each instruction an operation covers, in order, at `offsets` from `at`.
fn begin_each<const N: usize>(
    runtime: &impl ExecutionContext,
    fault_site: &mut Option<FaultSite>,
    pou: u32,
    at: u32,
    offsets: [u32; N],
) -> Result<(), RuntimeError> {
    for offset in offsets {
        begin(runtime, fault_site, pou, at + offset)?;
    }
    Ok(())
}

/// POP, as the dispatcher runs it: a reference's path is recycled and a temporary is released.
fn discard(
    runtime: &impl ExecutionContext,
    operand_stack: &mut OperandStack,
) -> Result<(), RuntimeError> {
    match operand_stack
        .pop_operand()
        .map_err(VmTrap::into_runtime_error)?
    {
        Operand::Value(Value::Reference(Some(reference))) => {
            runtime.recycle_reference_path(reference.path);
        }
        Operand::Text(temp) => operand_stack.release_text(temp),
        Operand::Copy(temp) => operand_stack.release_copy(temp),
        _ => {}
    }
    Ok(())
}

fn push(operand_stack: &mut OperandStack, value: Value) -> Result<(), RuntimeError> {
    operand_stack
        .push(value)
        .map_err(VmTrap::into_runtime_error)
}

fn slot(code: &CompactCode, index: u32) -> Result<Slot, RuntimeError> {
    // The error is built only on a refusal (checklist P1-7).
    match code.slots.get(index as usize) {
        Some(slot) => Ok(*slot),
        None => Err(RuntimeError::InvalidExecutionState),
    }
}

/// The globals' bytes: the persistent block from its first byte.
fn globals() -> Option<ByteRange> {
    Some(ByteRange {
        region: LayoutRegion::Persistent,
        start: 0,
        bytes: 0,
    })
}

/// The leaf of `slot` in the bytes that start at `base`.
fn leaf(base: ByteRange, slot: Slot) -> Option<ByteRange> {
    Some(ByteRange {
        region: base.region,
        start: base.start.checked_add(slot.offset)?,
        bytes: u32::from(slot.bytes),
    })
}

/// Read the scalar of `slot` from the bytes at `base`.
fn read(
    runtime: &impl ExecutionContext,
    base: Option<ByteRange>,
    slot: Slot,
) -> Result<Value, RuntimeError> {
    // The error is built only on a refusal (checklist P1-7).
    match base
        .and_then(|base| leaf(base, slot))
        .and_then(|range| runtime.scalar_at(range, slot.prim))
    {
        Some(value) => Ok(value),
        None => Err(RuntimeError::InvalidExecutionState),
    }
}

/// The scalar of `member` in the persistent instance that the instance leaf of `holder` in the
/// bytes at `own` names, read where it lies; `None` when the leaf names no persistent instance.
fn member_value(
    runtime: &impl ExecutionContext,
    own: Option<ByteRange>,
    holder: Slot,
    member: Slot,
) -> Option<Value> {
    let instance = own_instance(runtime, own, holder)?;
    let bytes = runtime.instance_bytes(instance)?;
    read(runtime, Some(bytes), member).ok()
}

/// The persistent instance that the instance leaf of `holder` in the bytes at `own` names; `None`
/// when it names none.
fn own_instance(
    runtime: &impl ExecutionContext,
    own: Option<ByteRange>,
    holder: Slot,
) -> Option<crate::memory::InstanceId> {
    let own = own?;
    let leaf = ByteRange {
        region: own.region,
        start: own.start.checked_add(holder.offset)?,
        bytes: INSTANCE_LEAF_BYTES,
    };
    runtime.instance_in_leaf(leaf)
}

/// The bytes of an instance leaf (MEM-12).
const INSTANCE_LEAF_BYTES: u32 = 8;

/// LOAD_SELF and REF_FIELD of own variable `slot`: push the reference REF_FIELD gives.
fn own_reference(
    runtime: &impl ExecutionContext,
    operand_stack: &mut OperandStack,
    instance: Option<crate::memory::InstanceId>,
    slot: Slot,
) -> Result<(), RuntimeError> {
    let reference = own_ref(runtime, instance, slot)?;
    push(operand_stack, Value::Reference(Some(reference)))
}

/// The reference REF_FIELD gives to own variable `slot` of `instance`.
fn own_ref(
    runtime: &impl ExecutionContext,
    instance: Option<crate::memory::InstanceId>,
    slot: Slot,
) -> Result<ValueRef, RuntimeError> {
    let instance = instance.ok_or(RuntimeError::InvalidExecutionState)?;
    Ok(ValueRef {
        location: MemoryLocation::Instance(instance),
        offset: slot.name,
        generation: runtime.storage().generation(),
        path: RefPath::new(),
    })
}

/// REF_FIELD of the field named by string `field` on the reference on top, as the dispatcher runs
/// it.
fn ref_field(
    runtime: &impl ExecutionContext,
    module: &VmModule,
    frames: &FrameStack,
    operand_stack: &mut OperandStack,
    field: u32,
) -> Result<(), RuntimeError> {
    let name = module
        .strings
        .name(field as usize)
        .ok_or_else(|| invalid_field_string_index(field))?;
    let reference = pop_reference(operand_stack).map_err(VmTrap::into_runtime_error)?;
    let next =
        dynamic_ref_field(runtime, frames, reference, name).map_err(VmTrap::into_runtime_error)?;
    push(operand_stack, Value::Reference(Some(next)))
}

/// LOAD through the reference on top, as the dispatcher runs it.
fn load(
    runtime: &mut impl ExecutionContext,
    frames: &FrameStack,
    operand_stack: &mut OperandStack,
) -> Result<(), RuntimeError> {
    let reference = pop_reference(operand_stack).map_err(VmTrap::into_runtime_error)?;
    let operand =
        dynamic_load_operand(runtime, frames, &reference).map_err(VmTrap::into_runtime_error)?;
    runtime.recycle_reference_path(reference.path);
    push_loaded(operand_stack, operand).map_err(VmTrap::into_runtime_error)
}

/// Write `value` into the scalar of `slot` in the bytes at `base`, when the general path would
/// store it as it is; `false` leaves the store to the general path.
fn write(
    runtime: &mut impl ExecutionContext,
    base: Option<ByteRange>,
    slot: Slot,
    value: &Value,
) -> bool {
    slot.direct
        && primitive_value_matches(slot.prim, value)
        && base
            .and_then(|base| leaf(base, slot))
            .is_some_and(|range| runtime.store_scalar_at(range, slot.prim, value))
}
