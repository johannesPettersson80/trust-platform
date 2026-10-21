//! A statement that kinded code runs as one operation (`compact::kinded::Fusion`; checklist P1-9).
//!
//! The statement's operands are read and its chain computed first, which changes nothing. When the
//! value is the one each operation would leave, the store can write the leaf, the copies the loads
//! admit are admitted and every instruction's unit charge is admitted at once without a deadline
//! sample, the leaf is written and the store's last instruction is left as the fault site: the
//! allowance, the site and the values are those the operations one by one leave. Otherwise the
//! statement's first operation runs as it would, and the others follow one by one, so a refusal, a
//! fault, a hand-over and a deadline sample fall where they fall without the statement.

use super::super::super::compact_loop::{globals, leaf};
use super::{integers, read, step, width, Bits, Body, ExecutionContext, Kind, Op, Step, DEPTH};
use crate::error::RuntimeError;
use crate::fault::FaultSite;
use crate::value::Value;
use crate::vm::compact::kinded::{FusedOperand, Fusion, KindedCode};

/// The statement `kinded.fusions[index]`, which starts at `op`, run as one operation when it can
/// be, else its first operation.
pub(super) fn statement<R: ExecutionContext>(
    body: &mut Body<'_, R>,
    operands: &mut Bits,
    aside: &mut Option<Value>,
    kinded: &KindedCode,
    op: Op,
    index: u16,
) -> Result<Step, RuntimeError> {
    let Some(fusion) = kinded.fusions.get(usize::from(index)) else {
        return Err(RuntimeError::InvalidExecutionState);
    };
    if run(body, operands, kinded, fusion)? {
        return Ok(Step::Over(usize::from(fusion.len)));
    }
    step(body, operands, aside, kinded, op, fusion.first)
}

/// Run `fusion` at once; `false`, having changed nothing, when it does not run so.
fn run<R: ExecutionContext>(
    body: &mut Body<'_, R>,
    operands: &Bits,
    kinded: &KindedCode,
    fusion: &Fusion,
) -> Result<bool, RuntimeError> {
    // The loads hand the body over when the loop has no room for their two operands.
    if operands.len + 2 > DEPTH {
        return Ok(false);
    }
    let Some((mut kind, mut bits)) = operand(body, kinded, fusion.operand) else {
        return Ok(false);
    };
    for link in fusion.chain.iter().flatten() {
        let Some((_, right)) = operand(body, kinded, link.operand) else {
            return Ok(false);
        };
        let Ok(result) = integers(link.op, link.signed, link.common, bits, right) else {
            return Ok(false);
        };
        (kind, bits) = result;
    }
    let (slot, own) = fusion.target;
    let base = if own { body.own } else { globals() };
    let (Some(range), Some(width)) = (
        body.slot(slot).and_then(|slot| leaf(base?, slot)),
        width(kind),
    ) else {
        return Ok(false);
    };
    if body.runtime.leaf_bytes(range).is_none()
        || body.runtime.before_scalar_clone().is_err()
        || !body
            .runtime
            .execution_budget()
            .charge_run(usize::from(fusion.units))
    {
        return Ok(false);
    }
    let bytes = bits.to_le_bytes();
    // The leaf was read above, so the store holds its bytes.
    if !body.runtime.write_leaf_bytes(range, &bytes[..width]) {
        return Err(RuntimeError::InvalidExecutionState);
    }
    *body.fault_site = Some(FaultSite {
        pou: body.pou,
        pc: fusion.site,
    });
    Ok(true)
}

/// The kind and bits of `operand`, read as its load reads it.
fn operand<R: ExecutionContext>(
    body: &Body<'_, R>,
    kinded: &KindedCode,
    operand: FusedOperand,
) -> Option<(Kind, u64)> {
    match operand {
        FusedOperand::Const { index, kind } => {
            Some((kind, *kinded.consts.get(usize::from(index))?))
        }
        FusedOperand::Leaf { slot, own, kind } => {
            let base = if own { body.own } else { globals() };
            Some((kind, read(body, base, body.slot(slot)?, kind)?))
        }
    }
}
