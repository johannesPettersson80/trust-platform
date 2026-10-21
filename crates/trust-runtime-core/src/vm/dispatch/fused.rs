//! A POU's own scalar variable read or written where it lies (checklist P1-3).
//!
//! The compiler emits a POU's own variable as LOAD_SELF and REF_FIELD, then LOAD_DYNAMIC to read it,
//! or SWAP and STORE_DYNAMIC to write the value below it. When the field is a scalar that the
//! instance declares itself, the dispatcher runs REF_FIELD and the instructions after it as one: it
//! makes no reference, and before the work of each instruction it covers it records that
//! instruction as the fault site and charges its unit, as its own dispatch would (MEM-05, TIME-08).
//! Whatever the composition does not read or write in place takes the general path, so the result
//! is the general path's.

use super::super::context::ExecutionContext;
use super::super::{OperandStack, VmTrap};
use crate::error::RuntimeError;
use crate::fault::FaultSite;
use crate::memory::view::StoreMutate;
use crate::memory::{InstanceId, MemoryLocation};

/// LOAD_DYNAMIC, SWAP and STORE_DYNAMIC.
const LOAD_DYNAMIC: u8 = 0x32;
const SWAP: u8 = 0x13;
const STORE_DYNAMIC: u8 = 0x33;

/// Run REF_FIELD `field` (string `field_idx`) of `instance`, whose operand ends at `pc`, together
/// with the LOAD_DYNAMIC, or the SWAP and STORE_DYNAMIC, after it, when `instance` declares `field`
/// as a scalar that the composition reads or writes in place. Answers whether it did; `pc` is then
/// after the last instruction it ran. `frame_end` is the end of the frame's code and `pou` the
/// frame's POU.
#[allow(clippy::too_many_arguments)]
pub(super) fn own_scalar_access(
    runtime: &mut impl ExecutionContext,
    code: &[u8],
    operand_stack: &mut OperandStack,
    pc: &mut usize,
    frame_end: usize,
    fault_site: &mut Option<FaultSite>,
    pou: Option<u32>,
    instance: InstanceId,
    field_idx: u32,
    field: &str,
) -> Result<bool, RuntimeError> {
    // The dispatch of the next instruction replaces a pending place, enumeration or copy with its
    // value; a stack that holds one is left to it.
    if operand_stack.places() != 0 || operand_stack.enums() != 0 || operand_stack.copies() != 0 {
        return Ok(false);
    }
    let at = *pc;
    match code.get(at..) {
        Some([LOAD_DYNAMIC, ..]) if at < frame_end => {
            let Some(value) = runtime.own_scalar(instance, field_idx, field) else {
                return Ok(false);
            };
            begin(runtime, fault_site, pou, at)?;
            runtime.before_value_clone(&value)?;
            operand_stack
                .push(value)
                .map_err(VmTrap::into_runtime_error)?;
            *pc = at + 1;
            Ok(true)
        }
        Some([SWAP, STORE_DYNAMIC, ..]) if at + 1 < frame_end => {
            // A temporary text on top is stored by the general path.
            let Ok(value) = operand_stack.peek() else {
                return Ok(false);
            };
            let Some(slot) = runtime.own_scalar_slot(instance, field_idx, field, value) else {
                return Ok(false);
            };
            begin(runtime, fault_site, pou, at)?;
            begin(runtime, fault_site, pou, at + 1)?;
            let value = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
            if !runtime
                .storage_mut()
                .write_slot(MemoryLocation::Instance(instance), slot, value)
            {
                return Err(VmTrap::NullReference.into_runtime_error());
            }
            *pc = at + 2;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// What the dispatcher does before the instruction at `pc`: it records the instruction as the fault
/// site and charges its unit.
fn begin(
    runtime: &mut impl ExecutionContext,
    fault_site: &mut Option<FaultSite>,
    pou: Option<u32>,
    pc: usize,
) -> Result<(), RuntimeError> {
    *fault_site = pou.map(|pou| FaultSite {
        pou,
        pc: u32::try_from(pc).unwrap_or(u32::MAX),
    });
    runtime.charge_execution_work(1)
}
