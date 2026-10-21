//! Statements that kinded code runs as one operation (checklist P1-9; specification 34 PERF-05).
//!
//! An assignment to a scalar variable of a constant or a variable, or of a chain of up to
//! [`MAX_STEPS`] integer operations on such operands, the left one first (`x := x + 1`,
//! `y := a * 1000 + b`), is a run of compact operations: the operand loads, each operation after
//! its right operand, and a store of the variable's own primitive. Preparation records each such
//! statement that no jump enters past its first operation ([`Fusion`]), and the kinded loop runs
//! it as one operation: it reads the operands and computes the value, and when the value is the
//! one each operation would leave, the store can write the leaf, every instruction's unit charge
//! is admitted at once without a deadline sample (`ExecutionBudget::charge_run`) and the copies
//! the loads admit are admitted, it writes the leaf and leaves the store's last instruction as the
//! fault site, as the operations one by one leave the allowance, the site and the values. Any
//! other time it runs the statement's first operation, and the rest follow one by one, so a
//! refusal, a fault and a deadline sample fall on the same instruction as without the statement.

use super::super::{Op, OpKind};
use super::{Kind, KindedOp};
use crate::program_model::BinaryOp;

/// The most integer operations a statement chains: `x := a op b op c op d`.
pub(crate) const MAX_STEPS: usize = 3;

/// The code offsets, from an operation's own, of the instructions a load of a POU's own scalar
/// variable covers, `LOAD_SELF`, `REF_FIELD` and `LOAD` (`OpKind::LoadOwn`), which it charges in
/// this order.
pub(crate) const LOAD_OWN: [u32; 3] = [0, 1, 6];
/// The instructions of an own scalar variable that a conversion reads as its target, `LOAD_SELF`
/// and `REF_FIELD` (`OpKind::OwnValue`).
pub(crate) const OWN_VALUE: [u32; 2] = [0, 1];
/// The instructions of a store into a POU's own scalar variable, `LOAD_SELF`, `REF_FIELD`, `SWAP`
/// and `STORE` (`OpKind::StoreOwn`).
pub(crate) const STORE_OWN: [u32; 4] = [0, 1, 6, 7];
/// The one instruction of a constant's load, a global's load or store, or an integer operation.
pub(crate) const ONE: [u32; 1] = [0];

/// An operand of a statement: a constant of the kinded code or the leaf of a scalar variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FusedOperand {
    /// The constant at `index` of [`super::KindedCode::consts`], of `kind`.
    Const { index: u16, kind: Kind },
    /// The leaf of the body's variable `slot`, of `kind`: the POU's own (`own`) or a global.
    Leaf { slot: u32, own: bool, kind: Kind },
}

/// One integer operation of a statement's chain: `op` on the value so far and `operand`, by the
/// kinded loop's integer rules for the signedness `signed` and the common kind `common`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FusedStep {
    pub(crate) operand: FusedOperand,
    pub(crate) op: BinaryOp,
    pub(crate) signed: bool,
    pub(crate) common: Kind,
}

/// A statement the kinded loop runs as one operation, at the place of its first operation
/// (`KindedOp::Fused`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fusion {
    /// The kinded form of the first operation, which runs when the statement does not.
    pub(crate) first: KindedOp,
    /// The compact operations the statement covers.
    pub(crate) len: u8,
    /// The instructions its operations charge, together.
    pub(crate) units: u8,
    /// The instruction the store's charges leave as the fault site, its last.
    pub(crate) site: u32,
    /// The first operand.
    pub(crate) operand: FusedOperand,
    /// The chain's operations in order, then `None`.
    pub(crate) chain: [Option<FusedStep>; MAX_STEPS],
    /// The variable the store writes: its slot, and whether it is the POU's own.
    pub(crate) target: (u32, bool),
}

/// The statement that starts at operation `at` of a body's compact operations `ops`, whose kinded
/// forms are `kinded`, when it is one the loop runs at once and `targets`, the operations jumps go
/// to, holds none of its operations but the first.
pub(super) fn fusion_at(
    ops: &[Op],
    kinded: &[KindedOp],
    targets: &[bool],
    at: usize,
) -> Option<Fusion> {
    let first = *kinded.get(at)?;
    let (operand, mut units) = load(ops.get(at)?, first)?;
    let mut chain = [None; MAX_STEPS];
    let mut next = at + 1;
    for step in &mut chain {
        let Some((right, right_units)) = load(ops.get(next)?, *kinded.get(next)?) else {
            break;
        };
        let KindedOp::Integer { op, signed, common } = *kinded.get(next + 1)? else {
            return None;
        };
        *step = Some(FusedStep {
            operand: right,
            op,
            signed,
            common,
        });
        units = units
            .checked_add(right_units)?
            .checked_add(u8::try_from(ONE.len()).ok()?)?;
        next += 2;
    }
    let (target, store_units, site) = store(ops.get(next)?, *kinded.get(next)?)?;
    if targets.get(at + 1..=next)?.iter().any(|&target| target) {
        return None;
    }
    Some(Fusion {
        first,
        len: u8::try_from(next + 1 - at).ok()?,
        units: units.checked_add(store_units)?,
        site,
        operand,
        chain,
        target,
    })
}

/// A load of an operand as the kinded loop runs it, which admits a copy of the operand: the
/// operand and the instructions it charges.
fn load(op: &Op, kinded: KindedOp) -> Option<(FusedOperand, u8)> {
    let leaf = |own, kind| FusedOperand::Leaf {
        slot: op.arg,
        own,
        kind,
    };
    let (operand, offsets): (_, &[u32]) = match (op.kind, kinded) {
        (OpKind::Const, KindedOp::Const { index, kind }) => {
            (FusedOperand::Const { index, kind }, &ONE)
        }
        (OpKind::LoadOwn, KindedOp::Load(kind)) => (leaf(true, kind), &LOAD_OWN),
        (OpKind::LoadGlobal, KindedOp::Load(kind)) => (leaf(false, kind), &ONE),
        _ => return None,
    };
    Some((operand, u8::try_from(offsets.len()).ok()?))
}

/// A store of the operand on top into a variable as its bits, as the kinded loop runs it: the
/// variable, the instructions it charges and the fault site they leave, its last instruction.
fn store(op: &Op, kinded: KindedOp) -> Option<((u32, bool), u8, u32)> {
    let (own, offsets): (_, &[u32]) = match (op.kind, kinded) {
        (OpKind::StoreOwn, KindedOp::Store) => (true, &STORE_OWN),
        (OpKind::StoreGlobal, KindedOp::Store) => (false, &ONE),
        _ => return None,
    };
    Some((
        (op.arg, own),
        u8::try_from(offsets.len()).ok()?,
        op.at.checked_add(*offsets.last()?)?,
    ))
}
