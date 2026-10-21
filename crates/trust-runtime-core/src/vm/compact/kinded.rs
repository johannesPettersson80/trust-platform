//! Compact code whose operands' kinds preparation fixes (checklist P1-9; specification 34 PERF-05).
//!
//! When every operation of a body's compact code takes and leaves operands whose kinds follow from
//! the body's variables, constants and operations alone, preparation records what each operation
//! does on those kinds, and the dispatcher runs the body on the operands' bits
//! (`dispatch::kinded_loop`): an integer as its 64-bit value, a BOOL as 0 or 1, any other scalar as
//! the bits its leaf holds, an instance as its identity. The arithmetic, comparison and logic of each
//! operation are chosen here, by the rules the typed loop applies to its cells, and a run reads and
//! writes the leaves' bytes and builds no value. A body with an operation whose operand kind is not
//! fixed, or whose work the loop does not do on bits, keeps the typed loop.
//!
//! A jump keeps the operands below its condition: a BOOL's short-circuit AND or OR keeps its left
//! operand across the jump past its right one. So the pass records the kinds that the first path to
//! an operation a jump goes to brings there, and every other path must bring the same; a body whose
//! paths bring different kinds, or that leaves operands at its end, keeps the typed loop.

use alloc::vec::Vec;

use super::{CompactCode, Op, OpKind};
use crate::memory::cells::{scalar_slot, Cell};
use crate::numeric::{wider_numeric, NumericKind};
use crate::program_model::{BinaryOp, UnaryOp};
use crate::stdlib::conversions::{ConversionSpec, ConversionType};
use crate::vm::module::{VmModule, VmNativeSymbolSpec};

/// The kind of an operand: the primitive of a scalar as the layout numbers it (BOOL 1, BYTE to
/// LWORD 2 to 5, SINT to ULINT 6 to 13, REAL 14, LREAL 15, TIME to LDT 16 to 23, CHAR 26, WCHAR
/// 27), [`INSTANCE`] or [`NULL`].
pub(crate) type Kind = u8;

/// The kind of a BOOL.
pub(crate) const BOOL: Kind = 1;
/// The kind of a TIME.
pub(crate) const TIME: Kind = 16;
/// The kind of a function block instance's identity.
pub(crate) const INSTANCE: Kind = 0xF0;
/// The kind of the value a standard block call leaves, which only a POP takes.
pub(crate) const NULL: Kind = 0xF1;
/// The primitive of a native duration slot, a TIME or an LTIME by the family byte after its
/// ticks. Its operand is held as a TIME; a slot that holds an LTIME hands the body over.
pub(crate) const DURATION_SLOT: u16 = 0x0101;

/// What one operation does on its operands' bits; the compact operation of the same index gives its
/// code offset, its variable, constant, block call or symbol, and its charges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KindedOp {
    /// Push the constant at `index` of [`KindedCode::consts`], of `kind`.
    Const {
        index: u16,
        kind: Kind,
    },
    /// Push the leaf of the operation's variable, of this kind.
    Load(Kind),
    /// Push the leaf of the member the operation names, of this kind.
    LoadMember(Kind),
    /// Push the identity of the instance the operation's variable names.
    LoadInstance,
    /// Write the operand, of the variable's primitive, into the variable's leaf.
    Store,
    /// Give the operand, of this kind, to the general store.
    StoreValue(Kind),
    /// Arithmetic or comparison of two integers of one signedness in their common type, which is
    /// `common`; a comparison leaves a BOOL.
    Integer {
        op: BinaryOp,
        signed: bool,
        common: Kind,
    },
    /// Logic or equality of two BOOLs.
    Bool(BinaryOp),
    /// NOT of a BOOL.
    Not,
    /// A unary operation on the operand's value, of this kind, which leaves a value of the kind.
    Unary(UnaryOp, Kind),
    /// Jump when the BOOL on top is this.
    JumpIf(bool),
    Jump,
    Pop,
    Dup,
    Swap,
    Skip,
    /// The call of the standard block the operation names.
    Block,
    /// The standard conversion the operation calls, of a scalar operand, which leaves a value of
    /// kind `to`: an integer converted to an integer type by the conversion's integer branch, any
    /// other scalar by the general conversion (`compact_loop::converted`).
    Convert {
        to: Kind,
    },
    /// The standard conversion the operation calls of an integer of kind `from` to the integer kind
    /// `to` that names no other source type: the conversion's integer branch, run on the operand's
    /// bits ([`integer_bits`]).
    ConvertInteger {
        from: Kind,
        to: Kind,
    },
}

/// The kinded code of one body: an operation for each compact operation, and the constants' bits.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct KindedCode {
    pub(crate) ops: Vec<KindedOp>,
    pub(crate) consts: Vec<u64>,
}

crate::memory::figure::holds_no_heap!(KindedOp);

impl crate::memory::figure::HeapBytes for KindedCode {
    fn add_heap(&self, count: &mut crate::memory::figure::HeapCount) {
        count.of(&self.ops);
        count.of(&self.consts);
    }
}

/// The deepest operand stack the loop keeps.
pub(crate) const DEPTH: usize = 16;

impl KindedCode {
    /// The kinded code of `code`, the compact code of a body of `module`, or `None` when an operation
    /// takes an operand whose kind is not fixed or does work the loop does not do on bits.
    pub(crate) fn of(code: &CompactCode, module: &VmModule) -> Option<Self> {
        let count = code.ops.len();
        let mut targets = Vec::new();
        targets.try_reserve_exact(count).ok()?;
        targets.resize(count, false);
        for op in &code.ops {
            if matches!(op.kind, OpKind::Jump | OpKind::JumpIf(_))
                && let Some(target) = targets.get_mut(op.arg as usize)
            {
                *target = true;
            }
        }
        let consts = code
            .ops
            .iter()
            .filter(|op| op.kind == OpKind::Const)
            .count();
        let mut pass = Pass {
            code,
            module,
            stack: [NULL; DEPTH],
            depth: 0,
            arrivals: Vec::new(),
            kinded: Self::default(),
        };
        pass.kinded.ops.try_reserve_exact(count).ok()?;
        pass.kinded.consts.try_reserve_exact(consts).ok()?;
        let mut reachable = true;
        for (index, op) in code.ops.iter().enumerate() {
            if targets[index] {
                if !reachable {
                    pass.enter_by_jump(index);
                }
                pass.arrive(index)?;
                reachable = true;
            }
            if !reachable {
                // An operation no path reaches keeps its place.
                pass.kinded.ops.push(KindedOp::Skip);
                continue;
            }
            let kinded = pass.op(op)?;
            reachable = kinded != KindedOp::Jump;
            pass.kinded.ops.push(kinded);
        }
        // The body's end takes no operand.
        if reachable && pass.depth != 0 {
            return None;
        }
        Some(pass.kinded)
    }
}

/// The kinds of the operands that the paths to an operation a jump goes to bring there, the
/// deepest first.
#[derive(Clone, Copy)]
struct Arrival {
    /// The operation's index.
    at: usize,
    kinds: [Kind; DEPTH],
    depth: usize,
}

/// One pass over a body's compact code: the kinds on the operand stack, the deepest first.
struct Pass<'a> {
    code: &'a CompactCode,
    module: &'a VmModule,
    stack: [Kind; DEPTH],
    depth: usize,
    /// The kinds the first path to each operation a jump goes to brought there.
    arrivals: Vec<Arrival>,
    kinded: KindedCode,
}

impl Pass<'_> {
    /// The kinded operation of `op`, with the kinds it leaves on the stack.
    fn op(&mut self, op: &Op) -> Option<KindedOp> {
        match op.kind {
            OpKind::Const
            | OpKind::LoadOwn
            | OpKind::LoadGlobal
            | OpKind::OwnValue
            | OpKind::LoadMember
            | OpKind::MemberValue
            | OpKind::LoadInstance => self.load(op),
            OpKind::StoreOwn | OpKind::StoreGlobal => self.store(op.arg),
            OpKind::Binary(binary) => self.binary(binary),
            OpKind::Equality(true) => self.binary(BinaryOp::Eq),
            OpKind::Equality(false) => self.binary(BinaryOp::Ne),
            OpKind::Unary(unary) => self.unary(unary),
            OpKind::JumpIf(_)
            | OpKind::Jump
            | OpKind::Pop
            | OpKind::Dup
            | OpKind::Swap
            | OpKind::Skip => self.control(op),
            OpKind::CallBlock => self.block(op.arg),
            OpKind::Convert => self.convert(op.arg),
            OpKind::Call | OpKind::OwnRef | OpKind::RefField | OpKind::Load => None,
        }
    }

    /// A load of a constant, a variable, a member or an instance.
    fn load(&mut self, op: &Op) -> Option<KindedOp> {
        let kinded = match op.kind {
            OpKind::Const => {
                let (kind, bits) = bits_of(scalar_slot(self.module.consts.get(op.arg as usize)?)?)?;
                let index = u16::try_from(self.kinded.consts.len()).ok()?;
                self.kinded.consts.push(bits);
                KindedOp::Const { index, kind }
            }
            OpKind::LoadOwn | OpKind::LoadGlobal | OpKind::OwnValue => {
                KindedOp::Load(load_kind(self.slot_prim(op.arg)?)?)
            }
            OpKind::LoadMember | OpKind::MemberValue => {
                let prim = self.slot_prim(op.arg.checked_add(1)?)?;
                KindedOp::LoadMember(if prim == DURATION_SLOT {
                    TIME
                } else {
                    load_kind(prim)?
                })
            }
            OpKind::LoadInstance => KindedOp::LoadInstance,
            _ => return None,
        };
        let kind = match kinded {
            KindedOp::Const { kind, .. } | KindedOp::Load(kind) | KindedOp::LoadMember(kind) => {
                kind
            }
            _ => INSTANCE,
        };
        self.push(kind)?;
        Some(kinded)
    }

    /// A store of the operand on top into variable `slot`: written as its bits when it is of the
    /// variable's primitive and the general path stores such a value as it is.
    fn store(&mut self, slot: u32) -> Option<KindedOp> {
        let slot = *self.code.slots.get(slot as usize)?;
        let kind = self.pop()?;
        if !is_scalar(kind) {
            return None;
        }
        Some(if slot.direct && slot.prim == u16::from(kind) {
            KindedOp::Store
        } else {
            KindedOp::StoreValue(kind)
        })
    }

    /// NOT of a BOOL, or the negation of a signed integer.
    fn unary(&mut self, unary: UnaryOp) -> Option<KindedOp> {
        let kind = self.pop()?;
        self.push(kind)?;
        match (unary, kind) {
            (UnaryOp::Not, BOOL) => Some(KindedOp::Not),
            (UnaryOp::Neg, kind) if integer(kind).is_some_and(|(signed, _)| signed) => {
                Some(KindedOp::Unary(unary, kind))
            }
            _ => None,
        }
    }

    /// A jump, POP, DUP, SWAP, NOP or DEBUG_MARK. A jump takes the operands below its condition to
    /// the operation it goes to.
    fn control(&mut self, op: &Op) -> Option<KindedOp> {
        Some(match op.kind {
            OpKind::JumpIf(when) => {
                if self.pop()? != BOOL {
                    return None;
                }
                self.jump(op.arg)?;
                KindedOp::JumpIf(when)
            }
            OpKind::Jump => {
                self.jump(op.arg)?;
                KindedOp::Jump
            }
            OpKind::Pop => {
                self.pop()?;
                KindedOp::Pop
            }
            OpKind::Dup => {
                let kind = self.pop()?;
                self.push(kind)?;
                self.push(kind)?;
                KindedOp::Dup
            }
            OpKind::Swap => {
                let top = self.pop()?;
                let below = self.pop()?;
                self.push(top)?;
                self.push(below)?;
                KindedOp::Swap
            }
            OpKind::Skip => KindedOp::Skip,
            _ => return None,
        })
    }

    /// The call of block call `index` on its receiver, an instance, and its arguments, scalars.
    fn block(&mut self, index: u32) -> Option<KindedOp> {
        let block = self.code.blocks.get(index as usize)?;
        let operands = usize::from(block.operands);
        let first = self.depth.checked_sub(operands)?;
        if operands == 0 || self.stack[first] != INSTANCE {
            return None;
        }
        if !self.stack[first + 1..self.depth]
            .iter()
            .all(|&kind| is_scalar(kind))
        {
            return None;
        }
        self.depth = first;
        self.push(NULL)?;
        Some(KindedOp::Block)
    }

    /// The binary operation `op` on the two operands on top.
    fn binary(&mut self, op: BinaryOp) -> Option<KindedOp> {
        let right = self.pop()?;
        let left = self.pop()?;
        if left == BOOL && right == BOOL {
            if !matches!(
                op,
                BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::Eq | BinaryOp::Ne
            ) {
                return None;
            }
            self.push(BOOL)?;
            return Some(KindedOp::Bool(op));
        }
        let (left_signed, left_kind) = integer(left)?;
        let (right_signed, right_kind) = integer(right)?;
        if left_signed != right_signed {
            return None;
        }
        let common = numeric_prim(wider_numeric(left_kind, right_kind)?)?;
        let result = match op {
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::Pow => common,
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => BOOL,
            _ => return None,
        };
        self.push(result)?;
        Some(KindedOp::Integer {
            op,
            signed: left_signed,
            common,
        })
    }

    /// The conversion of standard call `symbol` of the operand on top.
    fn convert(&mut self, symbol: u32) -> Option<KindedOp> {
        let Ok(VmNativeSymbolSpec::Parsed {
            conversion_spec: Some(ConversionSpec::Convert { src, dst }),
            ..
        }) = self.module.native_symbol_spec(symbol)
        else {
            return None;
        };
        let from = self.pop()?;
        let to = conversion_kind(*dst)?;
        self.push(to)?;
        // A conversion that names another source type converts the operand to it first, and its
        // result is of the target's kind all the same.
        if !is_scalar(from) {
            return None;
        }
        // An integer converted to an integer type, by a conversion that names the integer's own type
        // or none, is the conversion's integer branch (`stdlib::conversions::convert_integer`).
        if integer(from).is_some()
            && integer(to).is_some()
            && src.is_none_or(|src| conversion_kind(src) == Some(from))
        {
            return Some(KindedOp::ConvertInteger { from, to });
        }
        Some(KindedOp::Convert { to })
    }

    /// A jump to operation `target` with the operands on the stack, which arrive there with their
    /// kinds; a jump that leaves the body takes none.
    fn jump(&mut self, target: u32) -> Option<()> {
        let target = target as usize;
        if target < self.code.ops.len() {
            self.arrive(target)
        } else {
            (self.depth == 0).then_some(())
        }
    }

    /// Arrive at operation `at`, which a jump goes to, with the kinds on the stack: the first path
    /// to arrive records them, and every other path must bring the same.
    fn arrive(&mut self, at: usize) -> Option<()> {
        if let Some(arrival) = self.arrivals.iter().find(|arrival| arrival.at == at) {
            return (arrival.kinds[..arrival.depth] == self.stack[..self.depth]).then_some(());
        }
        self.arrivals.try_reserve(1).ok()?;
        self.arrivals.push(Arrival {
            at,
            kinds: self.stack,
            depth: self.depth,
        });
        Some(())
    }

    /// Take, at operation `at`, which no operation falls through to, the kinds the jumps to it
    /// bring: those recorded, or none when only a jump back from a later operation goes there.
    fn enter_by_jump(&mut self, at: usize) {
        let (kinds, depth) = self
            .arrivals
            .iter()
            .find(|arrival| arrival.at == at)
            .map_or(([NULL; DEPTH], 0), |arrival| (arrival.kinds, arrival.depth));
        self.stack = kinds;
        self.depth = depth;
    }

    fn slot_prim(&self, index: u32) -> Option<u16> {
        let slot = self.code.slots.get(index as usize)?;
        (slot.bytes > 0).then_some(slot.prim)
    }

    fn push(&mut self, kind: Kind) -> Option<()> {
        *self.stack.get_mut(self.depth)? = kind;
        self.depth += 1;
        Some(())
    }

    fn pop(&mut self) -> Option<Kind> {
        self.depth = self.depth.checked_sub(1)?;
        Some(self.stack[self.depth])
    }
}

/// Whether `kind` is a scalar's.
pub(crate) fn is_scalar(kind: Kind) -> bool {
    width(kind).is_some()
}

/// The kind a load of a leaf of primitive `prim` pushes, when the loop reads it as its bits.
pub(crate) fn load_kind(prim: u16) -> Option<Kind> {
    let kind = Kind::try_from(prim).ok()?;
    width(kind).map(|_| kind)
}

/// The bytes of a leaf of kind `kind`.
pub(crate) fn width(kind: Kind) -> Option<usize> {
    Some(match kind {
        1 | 2 | 6 | 10 | 26 => 1,
        3 | 7 | 11 | 27 => 2,
        4 | 8 | 12 | 14 => 4,
        5 | 9 | 13 | 15..=23 => 8,
        _ => return None,
    })
}

/// The signedness and numeric kind of an integer kind.
pub(crate) fn integer(kind: Kind) -> Option<(bool, NumericKind)> {
    Some(match kind {
        6 => (true, NumericKind::SInt),
        7 => (true, NumericKind::Int),
        8 => (true, NumericKind::DInt),
        9 => (true, NumericKind::LInt),
        10 => (false, NumericKind::USInt),
        11 => (false, NumericKind::UInt),
        12 => (false, NumericKind::UDInt),
        13 => (false, NumericKind::ULInt),
        _ => return None,
    })
}

/// The bits of `bits`, an integer of kind `from`, converted to the integer kind `to` as the
/// conversions' integer branch converts the value (`stdlib::conversions::convert_integer`;
/// specification 07 §2.1): the same value, held as an operand of `to`, when `to` holds it; `None`
/// when it does not, which the conversion refuses with `RuntimeError::Overflow`, and for a kind
/// that is no integer. The value is the one the operand's cell holds: its bits cut to the width of
/// `from` and extended with the sign of a signed kind.
pub(crate) fn integer_bits(from: Kind, to: Kind, bits: u64) -> Option<u64> {
    let (from_signed, _) = integer(from)?;
    let (to_signed, _) = integer(to)?;
    let value = extend(bits, from_signed, width(from)?);
    // `to` holds the value when its own width gives the value back and, between a signed and an
    // unsigned kind, the value is not negative.
    let kept = extend(value, to_signed, width(to)?) == value
        && (from_signed == to_signed || i64::from_ne_bytes(value.to_ne_bytes()) >= 0);
    kept.then_some(value)
}

/// `bits` cut to `bytes` bytes and extended back to 64 bits, with the sign when `signed`.
fn extend(bits: u64, signed: bool, bytes: usize) -> u64 {
    let shift = 64 - 8 * u32::try_from(bytes).unwrap_or(8);
    if signed {
        let value = i64::from_ne_bytes((bits << shift).to_ne_bytes()) >> shift;
        u64::from_ne_bytes(value.to_ne_bytes())
    } else {
        (bits << shift) >> shift
    }
}

/// The kind of an integer numeric kind.
fn numeric_prim(kind: NumericKind) -> Option<Kind> {
    Some(match kind {
        NumericKind::SInt => 6,
        NumericKind::Int => 7,
        NumericKind::DInt => 8,
        NumericKind::LInt => 9,
        NumericKind::USInt => 10,
        NumericKind::UInt => 11,
        NumericKind::UDInt => 12,
        NumericKind::ULInt => 13,
        NumericKind::Real | NumericKind::LReal => return None,
    })
}

/// The kind of a conversion's type, when it is a scalar's.
fn conversion_kind(ty: ConversionType) -> Option<Kind> {
    Some(match ty {
        ConversionType::Bool => 1,
        ConversionType::Byte => 2,
        ConversionType::Word => 3,
        ConversionType::DWord => 4,
        ConversionType::LWord => 5,
        ConversionType::SInt => 6,
        ConversionType::Int => 7,
        ConversionType::DInt => 8,
        ConversionType::LInt => 9,
        ConversionType::USInt => 10,
        ConversionType::UInt => 11,
        ConversionType::UDInt => 12,
        ConversionType::ULInt => 13,
        ConversionType::Real => 14,
        ConversionType::LReal => 15,
        ConversionType::Time => 16,
        ConversionType::LTime => 17,
        ConversionType::Date => 18,
        ConversionType::LDate => 19,
        ConversionType::Tod => 20,
        ConversionType::LTod => 21,
        ConversionType::Dt => 22,
        ConversionType::Ldt => 23,
        ConversionType::Char => 26,
        ConversionType::WChar => 27,
        _ => return None,
    })
}

/// The kind and bits of a scalar cell: an integer's 64-bit value (a signed one's two's complement),
/// a BOOL's 0 or 1, a REAL's or LREAL's IEEE bits, a date or time's ticks, a character's code.
#[allow(clippy::cast_sign_loss)] // a signed value's bits are its two's complement
pub(crate) fn bits_of(cell: Cell) -> Option<(Kind, u64)> {
    Some(match cell {
        Cell::Bool(value) => (1, u64::from(value)),
        Cell::Byte(value) => (2, u64::from(value)),
        Cell::Word(value) => (3, u64::from(value)),
        Cell::DWord(value) => (4, u64::from(value)),
        Cell::LWord(value) => (5, value),
        Cell::SInt(value) => (6, i64::from(value) as u64),
        Cell::Int(value) => (7, i64::from(value) as u64),
        Cell::DInt(value) => (8, i64::from(value) as u64),
        Cell::LInt(value) => (9, value as u64),
        Cell::USInt(value) => (10, u64::from(value)),
        Cell::UInt(value) => (11, u64::from(value)),
        Cell::UDInt(value) => (12, u64::from(value)),
        Cell::ULInt(value) => (13, value),
        Cell::Real(value) => (14, u64::from(value.to_bits())),
        Cell::LReal(value) => (15, value.to_bits()),
        Cell::Time(ticks) => (16, ticks as u64),
        Cell::LTime(ticks) => (17, ticks as u64),
        Cell::Date(ticks) => (18, ticks as u64),
        Cell::LDate(ticks) => (19, ticks as u64),
        Cell::Tod(ticks) => (20, ticks as u64),
        Cell::LTod(ticks) => (21, ticks as u64),
        Cell::Dt(ticks) => (22, ticks as u64),
        Cell::Ldt(ticks) => (23, ticks as u64),
        Cell::Char(value) => (26, u64::from(value)),
        Cell::WChar(value) => (27, u64::from(value)),
        Cell::Instance(instance) => (INSTANCE, u64::from(instance.0)),
        Cell::Null => (NULL, 0),
        _ => return None,
    })
}

/// The cell of `bits` of kind `kind`, as [`bits_of`] gives them.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // the bits hold the kind's value
pub(crate) fn cell_of_bits(kind: Kind, bits: u64) -> Option<Cell> {
    Some(match kind {
        1 => Cell::Bool(bits != 0),
        2 => Cell::Byte(bits as u8),
        3 => Cell::Word(bits as u16),
        4 => Cell::DWord(bits as u32),
        5 => Cell::LWord(bits),
        6 => Cell::SInt(bits as i8),
        7 => Cell::Int(bits as i16),
        8 => Cell::DInt(bits as i32),
        9 => Cell::LInt(bits as i64),
        10 => Cell::USInt(bits as u8),
        11 => Cell::UInt(bits as u16),
        12 => Cell::UDInt(bits as u32),
        13 => Cell::ULInt(bits),
        14 => Cell::Real(f32::from_bits(bits as u32)),
        15 => Cell::LReal(f64::from_bits(bits)),
        16 => Cell::Time(bits as i64),
        17 => Cell::LTime(bits as i64),
        18 => Cell::Date(bits as i64),
        19 => Cell::LDate(bits as i64),
        20 => Cell::Tod(bits as i64),
        21 => Cell::LTod(bits as i64),
        22 => Cell::Dt(bits as i64),
        23 => Cell::Ldt(bits as i64),
        26 => Cell::Char(bits as u8),
        27 => Cell::WChar(bits as u16),
        INSTANCE => Cell::Instance(crate::memory::InstanceId(bits as u32)),
        NULL => Cell::Null,
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
