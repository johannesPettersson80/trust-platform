//! The loop that runs a body's kinded code on its operands' bits (checklist P1-9; `vm::compact::
//! kinded`).
//!
//! The typed loop enters it for a body whose compact code has kinded code. Each operation does what
//! the typed loop does for it, in the same order, with the same charges, deadline checks and
//! admissions, on the operands' bits instead of their cells: a load reads the leaf's bytes, a store
//! writes them, and the arithmetic, comparison and logic preparation chose for the operands' kinds
//! run on their 64-bit values by the rules of `program_model::ops`. An operation that the typed loop
//! hands over before it is begun, such as a load of a leaf it cannot read or a block call the compact
//! loop gives the general call, hands the body over in the same way, so a body ends as on the typed
//! loop: with the same result, the same charges, the same fault record and the same values.

use super::super::super::compact::kinded::{
    bits_of, cell_of_bits, integer_bits, width, Kind, KindedCode, KindedOp, BOOL, DEPTH,
    DURATION_SLOT, INSTANCE, NULL, TIME,
};
use super::super::super::compact::{Op, OpKind, Slot};
use super::super::super::context::ExecutionContext;
use super::super::super::dispatch_refs::store_ref;
use super::super::super::{OperandStack, VmTrap};
use super::super::compact_block;
use super::super::compact_loop::{self, begin_each, globals, leaf, own_instance, push};
use super::{value_of, Body};
use crate::error::RuntimeError;
use crate::memory::cells::{scalar_slot, Cell};
use crate::memory::layout::ByteRange;
use crate::program_model::ops::{signed_arith, unsigned_arith};
use crate::program_model::{apply_unary, BinaryOp};
use crate::value::Value;

/// The family byte of a duration slot that holds an LTIME (`memory::flat`).
const LTIME_FAMILY: u8 = 1;

/// The operands of the loop, the deepest first: each one's bits and kind.
struct Bits {
    bits: [u64; DEPTH],
    kinds: [Kind; DEPTH],
    len: usize,
}

impl Bits {
    fn top(&self, count: usize) -> Option<core::ops::Range<usize>> {
        Some(self.len.checked_sub(count)?..self.len)
    }

    fn last(&self) -> Option<u64> {
        self.len.checked_sub(1).map(|index| self.bits[index])
    }

    fn has_room(&self) -> bool {
        self.len < DEPTH
    }

    fn push(&mut self, kind: Kind, bits: u64) {
        if self.len < DEPTH {
            self.bits[self.len] = bits;
            self.kinds[self.len] = kind;
            self.len += 1;
        }
    }

    fn discard(&mut self, count: usize) {
        self.len = self.len.saturating_sub(count);
    }

    /// The cell of the operand at `index`.
    fn cell(&self, index: usize) -> Option<Cell> {
        cell_of_bits(*self.kinds.get(index)?, *self.bits.get(index)?)
    }
}

/// What the loop does after an operation.
enum Step {
    Next,
    Jump(usize),
    /// Hand the body over before the operation, which it has not begun.
    Before,
    /// Hand the body over after the operation, whose result, held aside, goes on top.
    After,
}

/// Run `kinded`, the kinded code of `body`'s compact code, from its first operation to the body's
/// end, as the typed loop runs the compact code (`typed_loop::run`).
pub(super) fn run<R: ExecutionContext>(
    mut body: Body<'_, R>,
    kinded: &KindedCode,
    operand_stack: &mut OperandStack,
    pc: &mut usize,
    end: usize,
) -> Result<bool, RuntimeError> {
    let code = body.code;
    let mut operands = Bits {
        bits: [0; DEPTH],
        kinds: [NULL; DEPTH],
        len: 0,
    };
    let mut aside = None;
    let mut next = 0;
    while let (Some(&op), Some(&kind)) = (code.ops.get(next), kinded.ops.get(next)) {
        let (first, result) = match step(&mut body, &mut operands, &mut aside, kinded, op, kind)? {
            Step::Next => {
                next += 1;
                continue;
            }
            Step::Jump(to) => {
                next = to;
                continue;
            }
            Step::Before => (next, None),
            Step::After => (next + 1, aside.take()),
        };
        body.ensure_frame()?;
        for index in operands.top(operands.len).unwrap_or_default() {
            let cell = operands
                .cell(index)
                .ok_or_else(|| RuntimeError::InvalidExecutionState)?;
            push(operand_stack, value_of(cell)?)?;
        }
        if let Some(value) = result {
            push(operand_stack, value)?;
        }
        return body.hand_over(operand_stack, pc, end, first);
    }
    *pc = end;
    Ok(body.pushed)
}

/// Run `op`, whose kinded operation is `kinded_op`, on `operands`.
fn step<R: ExecutionContext>(
    body: &mut Body<'_, R>,
    operands: &mut Bits,
    aside: &mut Option<Value>,
    kinded: &KindedCode,
    op: Op,
    kinded_op: KindedOp,
) -> Result<Step, RuntimeError> {
    match kinded_op {
        KindedOp::Const { .. }
        | KindedOp::Load(_)
        | KindedOp::LoadMember(_)
        | KindedOp::LoadInstance
        | KindedOp::Dup => load(body, operands, kinded, op, kinded_op),
        KindedOp::Store | KindedOp::StoreValue(_) => store(body, operands, op, kinded_op),
        KindedOp::Integer { .. } | KindedOp::Bool(_) | KindedOp::Not | KindedOp::Unary(..) => {
            compute(body, operands, aside, op.at, kinded_op)
        }
        KindedOp::JumpIf(_) | KindedOp::Jump | KindedOp::Pop | KindedOp::Swap | KindedOp::Skip => {
            control(body, operands, op, kinded_op)
        }
        KindedOp::Block => call_block(body, operands, op.at, op.arg),
        KindedOp::Convert { to } => convert(body, operands, aside, op, to),
        KindedOp::ConvertInteger { from, to } => convert_integer(body, operands, op, from, to),
    }
}

/// A load of `op`: its operand is read, then its instructions charged, as the typed loop's loads
/// are.
fn load<R: ExecutionContext>(
    body: &mut Body<'_, R>,
    operands: &mut Bits,
    kinded: &KindedCode,
    op: Op,
    kinded_op: KindedOp,
) -> Result<Step, RuntimeError> {
    let at = op.at;
    match kinded_op {
        KindedOp::Const { index, kind } => {
            let bits = kinded
                .consts
                .get(usize::from(index))
                .map(|&bits| (kind, bits));
            push_read(body, operands, bits, at, [0], true)
        }
        KindedOp::Load(kind) => {
            let base = match op.kind {
                OpKind::LoadGlobal => globals(),
                _ => body.own,
            };
            let bits = body
                .slot(op.arg)
                .and_then(|slot| read(body, base, slot, kind))
                .map(|bits| (kind, bits));
            match op.kind {
                OpKind::LoadOwn => push_read(body, operands, bits, at, [0, 1, 6], true),
                OpKind::LoadGlobal => push_read(body, operands, bits, at, [0], true),
                _ => push_read(body, operands, bits, at, [0, 1], false),
            }
        }
        KindedOp::LoadMember(kind) => {
            let bits = member(body, op.arg, kind).map(|bits| (kind, bits));
            if matches!(op.kind, OpKind::LoadMember) {
                push_read(body, operands, bits, at, [0, 1, 6, 11], true)
            } else {
                push_read(body, operands, bits, at, [0, 1, 6], false)
            }
        }
        KindedOp::LoadInstance => {
            let instance = body
                .slot(op.arg)
                .and_then(|holder| own_instance(&*body.runtime, body.own, holder));
            let bits = instance.map(|instance| (INSTANCE, u64::from(instance.0)));
            push_read(body, operands, bits, at, [0, 1, 6], true)
        }
        KindedOp::Dup => {
            let top = operands
                .len
                .checked_sub(1)
                .map(|index| (operands.kinds[index], operands.bits[index]));
            push_read(body, operands, top, at, [0], true)
        }
        _ => Ok(Step::Before),
    }
}

/// The arithmetic, comparison or logic `kinded_op` at `at` on the operands on top.
fn compute<R: ExecutionContext>(
    body: &mut Body<'_, R>,
    operands: &mut Bits,
    aside: &mut Option<Value>,
    at: u32,
    kinded_op: KindedOp,
) -> Result<Step, RuntimeError> {
    match kinded_op {
        KindedOp::Integer { op, signed, common } => {
            let Some(range) = operands.top(2) else {
                return Ok(Step::Before);
            };
            let (left, right) = (operands.bits[range.start], operands.bits[range.start + 1]);
            body.begin(at)?;
            operands.discard(2);
            let (kind, bits) = integers(op, signed, common, left, right)?;
            operands.push(kind, bits);
            Ok(Step::Next)
        }
        KindedOp::Bool(op) => {
            let Some(range) = operands.top(2) else {
                return Ok(Step::Before);
            };
            let (left, right) = (
                operands.bits[range.start] != 0,
                operands.bits[range.start + 1] != 0,
            );
            body.begin(at)?;
            operands.discard(2);
            let result = match op {
                BinaryOp::And => left && right,
                BinaryOp::Or => left || right,
                BinaryOp::Xor => left ^ right,
                BinaryOp::Eq => left == right,
                _ => left != right,
            };
            operands.push(BOOL, u64::from(result));
            Ok(Step::Next)
        }
        KindedOp::Not => {
            let Some(value) = operands.last() else {
                return Ok(Step::Before);
            };
            body.begin(at)?;
            operands.discard(1);
            operands.push(BOOL, u64::from(value == 0));
            Ok(Step::Next)
        }
        KindedOp::Unary(op, kind) => {
            let Some(cell) = top_cell(operands) else {
                return Ok(Step::Before);
            };
            body.begin(at)?;
            operands.discard(1);
            let value = apply_unary(op, value_of(cell)?)?;
            Ok(push_result(operands, aside, value, kind))
        }
        _ => Ok(Step::Before),
    }
}

/// The jump, POP, SWAP, NOP or DEBUG_MARK `op`.
fn control<R: ExecutionContext>(
    body: &mut Body<'_, R>,
    operands: &mut Bits,
    op: Op,
    kinded_op: KindedOp,
) -> Result<Step, RuntimeError> {
    let at = op.at;
    match kinded_op {
        KindedOp::JumpIf(when) => {
            let Some(condition) = operands.last() else {
                return Ok(Step::Before);
            };
            body.begin(at)?;
            operands.discard(1);
            if (condition != 0) == when {
                Ok(Step::Jump(op.arg as usize))
            } else {
                Ok(Step::Next)
            }
        }
        KindedOp::Jump => {
            body.begin(at)?;
            Ok(Step::Jump(op.arg as usize))
        }
        KindedOp::Pop => {
            if operands.len == 0 {
                return Ok(Step::Before);
            }
            body.begin(at)?;
            operands.discard(1);
            Ok(Step::Next)
        }
        KindedOp::Swap => {
            let Some(range) = operands.top(2) else {
                return Ok(Step::Before);
            };
            body.begin(at)?;
            operands.bits.swap(range.start, range.start + 1);
            operands.kinds.swap(range.start, range.start + 1);
            Ok(Step::Next)
        }
        KindedOp::Skip => {
            body.begin(at)?;
            Ok(Step::Next)
        }
        _ => Ok(Step::Before),
    }
}

/// The standard conversion of `op`, the call's symbol its argument, of the operand on top, as the
/// typed loop's `convert` runs it; its result is pushed as its bits when it is of `to`.
fn convert<R: ExecutionContext>(
    body: &mut Body<'_, R>,
    operands: &mut Bits,
    aside: &mut Option<Value>,
    op: Op,
    to: Kind,
) -> Result<Step, RuntimeError> {
    let Some(argument) = top_cell(operands).and_then(|cell| value_of(cell).ok()) else {
        return Ok(Step::Before);
    };
    body.begin(op.at)?;
    operands.discard(1);
    let value = compact_loop::converted(body.runtime, body.module, op.arg, &argument)?;
    Ok(push_result(operands, aside, value, to))
}

/// The conversion of `op`, the call's symbol, of the integer on top, of kind `from`, to the integer
/// kind `to`, as [`convert`] runs it through the conversion's integer branch
/// (`compact_loop::converted`), on the operand's bits: the deadline is checked, the copy that the
/// argument's binding makes is admitted, the value is kept or refused as an overflow
/// ([`integer_bits`]), and the deadline is checked again (checklist P1-9).
fn convert_integer<R: ExecutionContext>(
    body: &mut Body<'_, R>,
    operands: &mut Bits,
    op: Op,
    from: Kind,
    to: Kind,
) -> Result<Step, RuntimeError> {
    let Some(bits) = operands.last() else {
        return Ok(Step::Before);
    };
    body.begin(op.at)?;
    operands.discard(1);
    if body.runtime.deadline_exceeded() {
        return Err(VmTrap::DeadlineExceeded.into_runtime_error());
    }
    body.runtime.before_scalar_clone()?;
    let bits = integer_bits(from, to, bits).ok_or_else(|| RuntimeError::Overflow)?;
    body.runtime.check_execution_deadline()?;
    operands.push(to, bits);
    Ok(Step::Next)
}

/// The cell of the operand on top.
fn top_cell(operands: &Bits) -> Option<Cell> {
    operands
        .len
        .checked_sub(1)
        .and_then(|index| operands.cell(index))
}

/// Push `operand`, read before the instructions at `offsets` from `at` are recorded and charged,
/// which they are then, with the admission of its copy when `copies`, as the typed loop's
/// `push_read` does. Hands over when there is no operand or no room for it.
fn push_read<R: ExecutionContext, const N: usize>(
    body: &mut Body<'_, R>,
    operands: &mut Bits,
    operand: Option<(Kind, u64)>,
    at: u32,
    offsets: [u32; N],
    copies: bool,
) -> Result<Step, RuntimeError> {
    let Some((kind, bits)) = operand.filter(|_| operands.has_room()) else {
        return Ok(Step::Before);
    };
    begin_each(&*body.runtime, body.fault_site, body.pou, at, offsets)?;
    if copies {
        body.runtime.before_scalar_clone()?;
    }
    operands.push(kind, bits);
    Ok(Step::Next)
}

/// Push `value`, an operation's result, as its bits when it is of `kind`; any other value is held
/// aside and handed over after the operation, as the typed loop hands over a result it has no cell
/// for.
fn push_result(operands: &mut Bits, aside: &mut Option<Value>, value: Value, kind: Kind) -> Step {
    match scalar_slot(&value).and_then(bits_of) {
        Some((found, bits)) if found == kind && operands.has_room() => {
            operands.push(kind, bits);
            Step::Next
        }
        _ => {
            *aside = Some(value);
            Step::After
        }
    }
}

/// The bits of the leaf of `slot` in the bytes at `base`, of `kind`.
fn read<R: ExecutionContext>(
    body: &Body<'_, R>,
    base: Option<ByteRange>,
    slot: Slot,
    kind: Kind,
) -> Option<u64> {
    let range = leaf(base?, slot)?;
    decode(kind, body.runtime.leaf_bytes(range)?)
}

/// The bits of the member in variables `index` and `index + 1`, the own function block instance
/// that holds it and the member, read where it lies, as the typed loop's `member` reads its cell. A
/// duration slot that holds an LTIME has no bits as a TIME.
fn member<R: ExecutionContext>(body: &Body<'_, R>, index: u32, kind: Kind) -> Option<u64> {
    let (holder, member) = (body.slot(index)?, body.slot(index + 1)?);
    let instance = own_instance(&*body.runtime, body.own, holder)?;
    let range = leaf(body.runtime.instance_bytes(instance)?, member)?;
    let bytes = body.runtime.leaf_bytes(range)?;
    if member.prim == DURATION_SLOT {
        let family = *bytes.get(8)?;
        if kind != TIME || family == LTIME_FAMILY {
            return None;
        }
        return decode(TIME, bytes);
    }
    decode(kind, bytes)
}

/// The bits of the leaf `bytes` of `kind`, as the flat store's codec decodes the leaf: little-endian,
/// a signed integer extended by its sign, a BOOL true for any byte but 0.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)] // two's complement bits
fn decode(kind: Kind, bytes: &[u8]) -> Option<u64> {
    Some(match width(kind)? {
        1 => {
            let byte = *bytes.first()?;
            match kind {
                BOOL => u64::from(byte != 0),
                6 => i64::from(byte as i8) as u64,
                _ => u64::from(byte),
            }
        }
        2 => {
            let value = u16::from_le_bytes(bytes.get(..2)?.try_into().ok()?);
            if kind == 7 {
                i64::from(value as i16) as u64
            } else {
                u64::from(value)
            }
        }
        4 => {
            let value = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?);
            if kind == 8 {
                i64::from(value as i32) as u64
            } else {
                u64::from(value)
            }
        }
        _ => u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?),
    })
}

/// The store of `op` of the operand on top into its variable, as the typed loop's `store` makes it:
/// an operand of the variable's primitive is written into its leaf, any other one is given to the
/// general store.
fn store<R: ExecutionContext>(
    body: &mut Body<'_, R>,
    operands: &mut Bits,
    op: Op,
    kinded_op: KindedOp,
) -> Result<Step, RuntimeError> {
    let (Some(slot), Some(index)) = (body.slot(op.arg), operands.len.checked_sub(1)) else {
        return Ok(Step::Before);
    };
    let (kind, bits) = (operands.kinds[index], operands.bits[index]);
    let base = if matches!(op.kind, OpKind::StoreOwn) {
        begin_each(
            &*body.runtime,
            body.fault_site,
            body.pou,
            op.at,
            [0, 1, 6, 7],
        )?;
        body.own
    } else {
        begin_each(&*body.runtime, body.fault_site, body.pou, op.at, [0])?;
        globals()
    };
    operands.discard(1);
    let written = matches!(kinded_op, KindedOp::Store)
        && width(kind).is_some_and(|width| {
            base.and_then(|base| leaf(base, slot)).is_some_and(|range| {
                let bytes = bits.to_le_bytes();
                body.runtime.write_leaf_bytes(range, &bytes[..width])
            })
        });
    if !written {
        body.ensure_frame()?;
        let cell = cell_of_bits(kind, bits).ok_or_else(|| RuntimeError::InvalidExecutionState)?;
        let value = value_of(cell)?;
        if matches!(op.kind, OpKind::StoreOwn) {
            compact_loop::store_own(
                body.runtime,
                body.module,
                body.frames,
                body.instance,
                slot,
                value,
            )?;
        } else {
            store_ref(body.runtime, body.module, body.frames, slot.name, value)
                .map_err(VmTrap::into_runtime_error)?;
        }
    }
    Ok(Step::Next)
}

/// `op` on two integers of one signedness in their common kind `common`, as the typed loop's
/// `integers` computes it: arithmetic narrowed to `common`, an ordering or an equality a BOOL.
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)] // two's complement bits
fn integers(
    op: BinaryOp,
    signed: bool,
    common: Kind,
    left: u64,
    right: u64,
) -> Result<(Kind, u64), RuntimeError> {
    let arithmetic = matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::Pow
    );
    let ordering = if signed {
        let (a, b) = (left as i64, right as i64);
        if arithmetic {
            return Ok((common, narrow_signed(common, signed_arith(op, a, b)?)?));
        }
        a.cmp(&b)
    } else {
        if arithmetic {
            return Ok((
                common,
                narrow_unsigned(common, unsigned_arith(op, left, right)?)?,
            ));
        }
        left.cmp(&right)
    };
    let result = match op {
        BinaryOp::Eq => ordering.is_eq(),
        BinaryOp::Ne => ordering.is_ne(),
        BinaryOp::Lt => ordering.is_lt(),
        BinaryOp::Le => ordering.is_le(),
        BinaryOp::Gt => ordering.is_gt(),
        _ => ordering.is_ge(),
    };
    Ok((BOOL, u64::from(result)))
}

/// The bits of `value` narrowed to signed `kind`, as the typed loop's `signed_cell` narrows it.
#[allow(clippy::cast_sign_loss)] // a signed integer's bits are its two's complement
fn narrow_signed(kind: Kind, value: i64) -> Result<u64, RuntimeError> {
    let fits = match kind {
        6 => i8::try_from(value).is_ok(),
        7 => i16::try_from(value).is_ok(),
        8 => i32::try_from(value).is_ok(),
        9 => true,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    if fits {
        Ok(value as u64)
    } else {
        Err(RuntimeError::Overflow)
    }
}

/// The bits of `value` narrowed to unsigned `kind`, as the typed loop's `unsigned_cell` narrows it.
fn narrow_unsigned(kind: Kind, value: u64) -> Result<u64, RuntimeError> {
    let fits = match kind {
        10 => u8::try_from(value).is_ok(),
        11 => u16::try_from(value).is_ok(),
        12 => u32::try_from(value).is_ok(),
        13 => true,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    if fits {
        Ok(value)
    } else {
        Err(RuntimeError::Overflow)
    }
}

/// The call at `at` of the standard function block of the body's block call `index`, on its
/// receiver and arguments on top, as the typed loop's `call_block` runs it.
fn call_block<R: ExecutionContext>(
    body: &mut Body<'_, R>,
    operands: &mut Bits,
    at: u32,
    index: u32,
) -> Result<Step, RuntimeError> {
    let Some(&block) = body.code.blocks.get(index as usize) else {
        return Ok(Step::Before);
    };
    let Ok(inputs) = compact_block::inputs(body.code, block) else {
        return Ok(Step::Before);
    };
    let count = usize::from(block.operands);
    let Some(range) = operands.top(count) else {
        return Ok(Step::Before);
    };
    let mut cells = [Cell::Null; DEPTH];
    for (cell, index) in cells.iter_mut().zip(range) {
        let Some(found) = operands.cell(index) else {
            return Ok(Step::Before);
        };
        *cell = found;
    }
    let arguments = &cells[..count];
    let Some((instance, base)) = compact_block::receiver(&*body.runtime, block, inputs, arguments)
    else {
        return Ok(Step::Before);
    };
    body.begin(at)?;
    compact_block::bind(body.runtime, inputs, arguments, base)?;
    operands.discard(count);
    compact_block::run(body.runtime, block, instance, base)?;
    operands.push(NULL, 0);
    Ok(Step::Next)
}

#[cfg(test)]
mod tests {
    use super::super::{integers as cell_integers, Integer};
    use super::*;
    use crate::memory::flat::{self, LeafKind};

    /// A leaf's bits are what the flat store's codec decodes from its bytes, for every scalar kind
    /// and byte pattern: a BOOL of byte 2 is TRUE, a negative SINT, INT and DINT extend their sign,
    /// the unsigned kinds do not. Catches a width, a byte order or a sign extension other than the
    /// codec's.
    #[test]
    fn a_leaf_decodes_as_the_codec_decodes_it() {
        let patterns: [[u8; 8]; 4] = [
            [0; 8],
            [2, 0, 0, 0, 0, 0, 0, 0],
            [0xFF; 8],
            [0x80, 0x01, 0x80, 0x7F, 0x00, 0x80, 0x00, 0x80],
        ];
        for kind in [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 22, 26, 27,
        ] {
            for bytes in &patterns {
                let expected = flat::decode(LeafKind::Primitive(u16::from(kind)), bytes)
                    .ok()
                    .and_then(bits_of)
                    .map(|(_, bits)| bits);
                assert_eq!(
                    decode(kind, bytes),
                    expected,
                    "kind {kind}, bytes {bytes:?}"
                );
            }
        }
    }

    /// Specification 10 §6.4 by the typed loop's rules: every arithmetic and comparison of every
    /// pair of integer kinds of one signedness, at values that overflow, divide by zero and stay in
    /// range, gives the typed loop's result, as bits, or its error. Catches a common kind other than
    /// `wider_numeric`'s, a narrowing that wraps instead of refusing, and an ordering of the wrong
    /// signedness.
    #[test]
    fn integers_compute_as_the_typed_loop_computes_them() {
        let signed = [
            Cell::SInt(i8::MIN),
            Cell::SInt(-1),
            Cell::Int(300),
            Cell::DInt(0),
            Cell::DInt(70_000),
            Cell::LInt(i64::MAX),
        ];
        let unsigned = [
            Cell::USInt(200),
            Cell::UInt(0),
            Cell::UInt(u16::MAX),
            Cell::UDInt(u32::MAX),
            Cell::ULInt(3),
        ];
        let ops = [
            BinaryOp::Add,
            BinaryOp::Sub,
            BinaryOp::Mul,
            BinaryOp::Div,
            BinaryOp::Mod,
            BinaryOp::Pow,
            BinaryOp::Eq,
            BinaryOp::Ne,
            BinaryOp::Lt,
            BinaryOp::Le,
            BinaryOp::Gt,
            BinaryOp::Ge,
        ];
        for (cells, is_signed) in [(&signed[..], true), (&unsigned[..], false)] {
            for &left in cells {
                for &right in cells {
                    let (left_kind, left_bits) = bits_of(left).expect("an integer");
                    let (right_kind, right_bits) = bits_of(right).expect("an integer");
                    let common = crate::numeric::wider_numeric(
                        integer_kind(left_kind),
                        integer_kind(right_kind),
                    )
                    .and_then(|kind| bits_of(kind_cell(kind)))
                    .map(|(kind, _)| kind)
                    .expect("one signedness has a common kind");
                    for op in ops {
                        let expected = cell_integers(
                            op,
                            Integer::of(left).expect("an integer"),
                            Integer::of(right).expect("an integer"),
                        )
                        .map(|cell| bits_of(cell).expect("a scalar"));
                        let found = integers(op, is_signed, common, left_bits, right_bits);
                        assert_eq!(found, expected, "{left:?} {op:?} {right:?}");
                    }
                }
            }
        }
    }

    fn integer_kind(kind: Kind) -> crate::numeric::NumericKind {
        crate::vm::compact::kinded::integer(kind)
            .expect("an integer")
            .1
    }

    /// A zero cell of numeric kind `kind`.
    fn kind_cell(kind: crate::numeric::NumericKind) -> Cell {
        use crate::numeric::NumericKind;
        match kind {
            NumericKind::SInt => Cell::SInt(0),
            NumericKind::Int => Cell::Int(0),
            NumericKind::DInt => Cell::DInt(0),
            NumericKind::LInt => Cell::LInt(0),
            NumericKind::USInt => Cell::USInt(0),
            NumericKind::UInt => Cell::UInt(0),
            NumericKind::UDInt => Cell::UDInt(0),
            NumericKind::ULInt => Cell::ULInt(0),
            NumericKind::Real => Cell::Real(0.0),
            NumericKind::LReal => Cell::LReal(0.0),
        }
    }
}
