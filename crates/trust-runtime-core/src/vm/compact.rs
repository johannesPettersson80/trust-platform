//! The compact code of a POU body (checklist P1-F; specification 34 TIME-08 and MEM-05).
//!
//! Preparation translates a program's or a function block's body once into operations that name
//! each scalar variable by the byte offset preparation gave it: the POU's own variables from the
//! first byte of its instance, a global in the persistent block. The dispatcher runs a body that
//! has compact code with a loop that does only what each operation needs, while no statement hook
//! observes the instructions (`dispatch::compact_loop`). It charges every instruction an operation
//! covers its unit after recording it as the fault site, as its own dispatch does, does the work
//! of every other instruction with the functions the dispatcher's handlers use, and gives a store
//! of a value the general path would convert or refuse to the general store, so a body ends the
//! same way on either.
//!
//! A body runs on the general path when it holds an instruction the compact code has no operation
//! for, keeps variables in its frame, or belongs to a function block another one extends, whose
//! instances keep the inherited variables in a record of their own. A standard function block's
//! body gets no compact code: its calls run its native code, so the body never runs. The compact
//! code never leaves an operand the dispatcher would replace with its value before the next
//! instruction: a call it holds finishes at once with a value (a standard function that is no text
//! function, or a standard function block), and a load through a reference reads a scalar or an
//! instance, never a compound or an enumeration. No reference waits on the stack across a jump.

use alloc::vec::Vec;

use crate::bytecode::{
    PouKind, StorageOwner, TypeData, TypeKind, NATIVE_CALL_KIND_FUNCTION_BLOCK,
    NATIVE_CALL_KIND_STDLIB,
};
use crate::program_model::{BinaryOp, UnaryOp};
use crate::value::Value;

use super::helpers::opcode_operand_len;
use super::module::VmRef;
use super::prepared::call_plans::CallCallee;
use super::type_policy::resolved_alias_type;
use super::PreparedModule;

mod blocks;
pub(crate) mod kinded;
pub(crate) use blocks::BlockCall;
#[cfg(all(test, feature = "std"))]
mod probe_tests;

/// The compact code of one body: the code offset of its first instruction, its operations, the
/// variables they name, and its calls of standard function blocks. `frameless` says that the body
/// is a program's that keeps no variable in a frame and initializes nothing in one, so its entry
/// pushes the frame only when an operation needs it (checklist P1-9; `dispatch::compact_entry`).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CompactCode {
    pub(crate) start: usize,
    pub(crate) ops: Vec<Op>,
    pub(crate) slots: Vec<Slot>,
    pub(crate) blocks: Vec<BlockCall>,
    pub(crate) frameless: bool,
    /// The kinded code of the body, when its operands' kinds are fixed (checklist P1-9).
    pub(crate) kinded: Option<kinded::KindedCode>,
}

/// One operation. `at` is the code offset of the first instruction it covers; `arg` is a constant's
/// index for [`OpKind::Const`], a variable's index in [`CompactCode::slots`] for a load, a store or
/// [`OpKind::OwnRef`], the field's string for [`OpKind::RefField`], the call's symbol for
/// [`OpKind::Convert`], and the operation a jump goes to, which is the number of operations when it
/// leaves the body. `plain` says that the operand stack held only values before it, which a jump to
/// it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Op {
    pub(crate) at: u32,
    pub(crate) arg: u32,
    pub(crate) kind: OpKind,
    pub(crate) plain: bool,
}

/// What an operation does, by the instructions it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OpKind {
    /// `CONST` of a scalar.
    Const,
    /// `LOAD_SELF`, `REF_FIELD` and `LOAD` of the instance's own scalar variable.
    LoadOwn,
    /// `LOAD_SELF`, `REF_FIELD`, `SWAP` and `STORE` of the operand below into that variable.
    StoreOwn,
    /// `LOAD_SELF` and `REF_FIELD`: a reference to one of the instance's own variables.
    OwnRef,
    /// `LOAD_SELF`, `REF_FIELD` and `LOAD` of a function block instance the POU declares itself:
    /// the instance its leaf names (checklist P1-5).
    LoadInstance,
    /// `LOAD_SELF` and `REF_FIELD` of an own scalar variable that the conversion call after them
    /// reads as its target: the variable's value.
    OwnValue,
    /// `LOAD_SELF`, `REF_FIELD` of a function block instance the POU declares itself and
    /// `REF_FIELD` of a scalar variable the block declares itself, which the conversion call after
    /// them reads as its target: the variable's value. `arg` is as for [`OpKind::LoadMember`].
    MemberValue,
    /// `LOAD_SELF` and `REF_FIELD` of a function block instance the POU declares itself, then
    /// `REF_FIELD` of a scalar variable that block declares itself and `LOAD`: `arg` is the index
    /// of the instance's slot, and the member's slot follows it, named by its field string.
    LoadMember,
    /// `REF_FIELD` of the reference on top.
    RefField,
    /// `LOAD` through the reference on top.
    Load,
    /// `LOAD_REF` of a scalar global.
    LoadGlobal,
    /// `STORE_REF` of the operand into a scalar global.
    StoreGlobal,
    /// An arithmetic, logical or ordering operation.
    Binary(BinaryOp),
    /// `EQ` (`true`) or `NE` (`false`).
    Equality(bool),
    /// `NEG` or `NOT`.
    Unary(UnaryOp),
    /// `CALL_NATIVE` of a call that finishes at once with a value.
    Call,
    /// `CALL_NATIVE` of a standard function block that binds input values only (checklist P1-5):
    /// `arg` is the call's index in [`CompactCode::blocks`].
    CallBlock,
    /// `CALL_NATIVE` of a standard conversion of one argument, `arg` the call's symbol: a scalar
    /// argument is converted where it lies (checklist P1-5), any other one by the general call.
    Convert,
    /// `JMP`.
    Jump,
    /// `JMP_TRUE` (`true`) or `JMP_FALSE` (`false`).
    JumpIf(bool),
    /// `DUP`, `POP` and `SWAP`.
    Dup,
    Pop,
    Swap,
    /// `NOP` and `DEBUG_MARK`.
    Skip,
}

/// A variable: its byte offset (from the instance's first byte for the POU's own variable, in the
/// persistent block for a global), its leaf's bytes and primitive when it is a scalar (`bytes` 0
/// otherwise), and how the general path names it: the variable's position among the instance's for
/// the POU's own, the static reference for a global. `direct` says that the general path stores a
/// value of the primitive as it is, unconverted and unrefused, so the compact code may write it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) offset: u32,
    pub(crate) name: u32,
    pub(crate) prim: u16,
    pub(crate) bytes: u8,
    pub(crate) direct: bool,
}

crate::memory::figure::holds_no_heap!(Op, Slot);

impl crate::memory::figure::HeapBytes for CompactCode {
    fn add_heap(&self, count: &mut crate::memory::figure::HeapCount) {
        count.of(&self.ops);
        count.of(&self.slots);
        count.of(&self.blocks);
        if let Some(kinded) = &self.kinded {
            crate::memory::figure::HeapBytes::add_heap(kinded, count);
        }
    }
}

impl CompactCode {
    /// The compact code of every body that has one, by POU. Each body is translated twice: the
    /// first pass counts its operations and variables, and the second writes them into vectors of
    /// exactly those lengths, which the body keeps. The preparation of an online change's candidate
    /// therefore holds no operations a body does not keep, beside the operand stack and the jumps
    /// that one scratch keeps for every body (specification 34 Appendix D.2, the working memory).
    pub(crate) fn of_module(prepared: &PreparedModule) -> Vec<(u32, Self)> {
        let mut scratch = Scratch::default();
        let mut bodies = Vec::new();
        for &pou in prepared.vm.pou_by_id.keys() {
            if let Some(code) = Self::of(prepared, pou, &mut scratch) {
                bodies.push((pou, code));
            }
        }
        bodies
    }

    /// The compact code of `pou`'s body, or `None` when it runs on the general path.
    fn of(prepared: &PreparedModule, pou: u32, scratch: &mut Scratch) -> Option<Self> {
        let vm = &prepared.vm;
        let entry = vm.pou(pou)?;
        let fits = match entry.kind {
            PouKind::Program => true,
            // A standard function block's calls run its native code, never its body.
            PouKind::FunctionBlock => {
                !extended(prepared, pou) && prepared.builtin_formals(pou).is_none()
            }
            _ => false,
        };
        if !fits || entry.local_ref_count != 0 {
            return None;
        }
        let code = vm.code.get(entry.code_start..entry.code_end)?;
        scratch.clear();
        let ops = Translation::run(prepared, pou, scratch, false, code, entry.code_start)?;
        let slots = scratch.slots.len();
        let blocks = scratch.blocks.len();
        // The first pass's variables and block calls are freed before the kept ones are reserved,
        // and the vectors start empty, so each holds exactly what the body keeps.
        scratch.slots = Vec::new();
        scratch.blocks = Vec::new();
        scratch.ops = Vec::new();
        scratch.ops.try_reserve_exact(ops).ok()?;
        scratch.slots.try_reserve_exact(slots).ok()?;
        scratch.blocks.try_reserve_exact(blocks).ok()?;
        scratch.jumps.clear();
        scratch.stack.clear();
        Translation::run(prepared, pou, scratch, true, code, entry.code_start)?;
        let frameless = entry.kind == PouKind::Program
            && prepared.frame_action_plan_index(pou).is_none()
            && prepared
                .local_range(pou)
                .is_none_or(|range| range.bytes == 0);
        let mut code = Self {
            start: entry.code_start,
            ops: core::mem::take(&mut scratch.ops),
            slots: core::mem::take(&mut scratch.slots),
            blocks: core::mem::take(&mut scratch.blocks),
            frameless,
            kinded: None,
        };
        code.kinded = kinded::KindedCode::of(&code, vm);
        Some(code)
    }
}

/// Whether another POU extends `pou`.
fn extended(prepared: &PreparedModule, pou: u32) -> bool {
    prepared
        .vm
        .parent_pou_ids
        .values()
        .any(|&parent| parent == pou)
}

/// What an operand on the stack is, as far as the translation follows it: a value, a reference to
/// a target of a known type, or a reference whose target's type it does not follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operand {
    Value,
    Ref(u32),
    Opaque,
    /// A variable's value read for the conversion call that follows, which the call would read
    /// through a reference to the variable (its target argument).
    Target,
    /// An instance of standard function block `u32` that a block call can have as its receiver.
    Block(u32),
}

impl Operand {
    /// Whether the operand is a value: a block instance is one, a reference is not.
    fn is_value(&self) -> bool {
        matches!(self, Operand::Value | Operand::Block(_))
    }
}

/// The vectors a body's translation fills: the operations, which only the second pass writes, the
/// variables they name, each jump's operation and target offset (from the body's start), and the
/// operands on the stack.
#[derive(Default)]
struct Scratch {
    ops: Vec<Op>,
    slots: Vec<Slot>,
    blocks: Vec<BlockCall>,
    jumps: Vec<(usize, usize)>,
    stack: Vec<Operand>,
}

impl Scratch {
    /// Empty every vector, keeping the storage of the jumps and the stack.
    fn clear(&mut self) {
        self.ops.clear();
        self.slots.clear();
        self.blocks.clear();
        self.jumps.clear();
        self.stack.clear();
    }
}

/// The state of one pass over a body: the module, the body's POU, the scratch it fills, whether it
/// writes the operations and links the jumps (the second pass) or only counts the operations, and
/// the operations so far.
struct Translation<'a, 's> {
    prepared: &'a PreparedModule,
    pou: u32,
    s: &'s mut Scratch,
    writes: bool,
    count: usize,
}

impl<'a, 's> Translation<'a, 's> {
    /// One pass over `code`, the body of `pou`, whose first byte is at `start` in the module's
    /// code: the number of its operations, or `None` when it runs on the general path.
    fn run(
        prepared: &'a PreparedModule,
        pou: u32,
        s: &'s mut Scratch,
        writes: bool,
        code: &[u8],
        start: usize,
    ) -> Option<usize> {
        let mut translation = Self {
            prepared,
            pou,
            s,
            writes,
            count: 0,
        };
        let mut pc = 0;
        while pc < code.len() {
            pc = translation.op_at(code, pc, start)?;
        }
        if writes {
            translation.link(start, code.len())?;
        }
        Some(translation.count)
    }
}

impl Translation<'_, '_> {
    /// Translate the instructions at `pc`, an offset in `code` (the body, whose first byte is at
    /// `start` in the module's code), into one operation. Answers the offset after them, or `None`
    /// when the compact code has no operation for them.
    fn op_at(&mut self, code: &[u8], pc: usize, start: usize) -> Option<usize> {
        let opcode = *code.get(pc)?;
        let operand_end = pc.checked_add(1 + opcode_operand_len(opcode)?)?;
        let operand = code.get(pc + 1..operand_end)?;
        let word = |at: usize| {
            operand
                .get(at..at + 4)?
                .try_into()
                .ok()
                .map(u32::from_le_bytes)
        };
        let at = u32::try_from(start.checked_add(pc)?).ok()?;
        let plain = self.s.stack.iter().all(Operand::is_value);
        let (kind, arg, end) = match opcode {
            0x00 | 0x70 => (OpKind::Skip, 0, operand_end),
            0x02..=0x04 => {
                let offset = i32::from_le_bytes(operand.try_into().ok()?);
                let target = i64::try_from(operand_end).ok()? + i64::from(offset);
                let target = usize::try_from(target).ok().filter(|&t| t <= code.len())?;
                if opcode != 0x02 {
                    self.pop(1)?;
                }
                // No reference waits on the stack across a jump.
                if !self.s.stack.iter().all(Operand::is_value) {
                    return None;
                }
                if self.writes {
                    self.s.jumps.push((self.count, target));
                }
                let kind = match opcode {
                    0x02 => OpKind::Jump,
                    0x03 => OpKind::JumpIf(true),
                    _ => OpKind::JumpIf(false),
                };
                (kind, 0, operand_end)
            }
            0x09 => {
                let (kind, arg) = self.native_call(at, word(0)?, word(4)?, word(8)?)?;
                (kind, arg, operand_end)
            }
            0x10 => {
                let index = word(0)?;
                self.scalar_constant(index)?;
                self.s.stack.push(Operand::Value);
                (OpKind::Const, index, operand_end)
            }
            0x11 => {
                let top = *self.s.stack.last()?;
                self.s.stack.push(top);
                (OpKind::Dup, 0, operand_end)
            }
            0x12 => {
                self.pop(1)?;
                (OpKind::Pop, 0, operand_end)
            }
            0x13 => {
                let len = self.s.stack.len();
                self.s.stack.swap(len.checked_sub(1)?, len.checked_sub(2)?);
                (OpKind::Swap, 0, operand_end)
            }
            0x20 | 0x21 => {
                let ref_idx = word(0)?;
                let slot = self.global(ref_idx)?;
                let kind = if opcode == 0x20 {
                    self.s.stack.push(Operand::Value);
                    OpKind::LoadGlobal
                } else {
                    self.pop(1)?;
                    OpKind::StoreGlobal
                };
                (kind, self.slot(slot)?, operand_end)
            }
            0x23 => self.own_variable(code, operand_end)?,
            0x30 => {
                let field = word(0)?;
                // REF_FIELD takes a reference: a body that gives it a value, an instance among
                // them, runs on the general path.
                let next = match self.pop(1)? {
                    Operand::Ref(type_id) => self.field_type(type_id, field),
                    Operand::Opaque => None,
                    Operand::Value | Operand::Target | Operand::Block(_) => return None,
                };
                self.s
                    .stack
                    .push(next.map_or(Operand::Opaque, Operand::Ref));
                (OpKind::RefField, field, operand_end)
            }
            0x32 => {
                let Operand::Ref(type_id) = self.pop(1)? else {
                    return None;
                };
                if !self.loads_a_value(type_id) {
                    return None;
                }
                let loaded = self
                    .block_pou(type_id)
                    .map_or(Operand::Value, Operand::Block);
                self.s.stack.push(loaded);
                (OpKind::Load, 0, operand_end)
            }
            0x40..=0x44 | 0x46..=0x48 | 0x4C | 0x50..=0x55 => {
                self.pop(2)?;
                self.s.stack.push(Operand::Value);
                let kind = match opcode {
                    0x40 => OpKind::Binary(BinaryOp::Add),
                    0x41 => OpKind::Binary(BinaryOp::Sub),
                    0x42 => OpKind::Binary(BinaryOp::Mul),
                    0x43 => OpKind::Binary(BinaryOp::Div),
                    0x44 => OpKind::Binary(BinaryOp::Mod),
                    0x46 => OpKind::Binary(BinaryOp::And),
                    0x47 => OpKind::Binary(BinaryOp::Or),
                    0x48 => OpKind::Binary(BinaryOp::Xor),
                    0x4C => OpKind::Binary(BinaryOp::Pow),
                    0x50 => OpKind::Equality(true),
                    0x51 => OpKind::Equality(false),
                    0x52 => OpKind::Binary(BinaryOp::Lt),
                    0x53 => OpKind::Binary(BinaryOp::Le),
                    0x54 => OpKind::Binary(BinaryOp::Gt),
                    _ => OpKind::Binary(BinaryOp::Ge),
                };
                (kind, 0, operand_end)
            }
            0x45 | 0x49 => {
                self.pop(1)?;
                self.s.stack.push(Operand::Value);
                let unary = if opcode == 0x45 {
                    UnaryOp::Neg
                } else {
                    UnaryOp::Not
                };
                (OpKind::Unary(unary), 0, operand_end)
            }
            _ => return None,
        };
        if self.writes {
            self.s.ops.push(Op {
                at,
                arg,
                kind,
                plain,
            });
        }
        self.count += 1;
        Some(end)
    }

    /// The operation of the `CALL_NATIVE` at `at` of `kind`, `symbol` and `count` operands, and its
    /// argument: a block call, a conversion, or another call that finishes at once with a value.
    fn native_call(
        &mut self,
        at: u32,
        kind: u32,
        symbol: u32,
        count: u32,
    ) -> Option<(OpKind, u32)> {
        self.immediate_call(at, kind, symbol, count)?;
        let block = self.block_call(at, kind, symbol, count);
        let converts = kind == NATIVE_CALL_KIND_STDLIB
            && match self.s.stack.last() {
                Some(Operand::Value) => self.conversion(symbol, count, false),
                Some(Operand::Target) => self.conversion(symbol, count, true),
                _ => false,
            };
        // A value read for a target is only ever followed by its conversion.
        if !converts && self.s.stack.last() == Some(&Operand::Target) {
            return None;
        }
        self.pop(usize::try_from(count).ok()?)?;
        self.s.stack.push(Operand::Value);
        Some(match block {
            Some(block) => (OpKind::CallBlock, block),
            None if converts => (OpKind::Convert, symbol),
            None => (OpKind::Call, 0),
        })
    }

    /// The operation of a POU's own variable, which the `LOAD_SELF` whose operand ends at
    /// `operand_end` in `code` begins with its `REF_FIELD`, its argument and the offset after the
    /// instructions it covers: then `LOAD`, or `SWAP` and `STORE`, of a scalar; a scalar or a
    /// member of an own function block instance that a conversion reads as its target, or that
    /// `LOAD` reads; or a reference to the variable.
    fn own_variable(&mut self, code: &[u8], operand_end: usize) -> Option<(OpKind, u32, usize)> {
        if code.get(operand_end) != Some(&0x30) {
            return None;
        }
        let field_end = operand_end.checked_add(5)?;
        let field = u32::from_le_bytes(code.get(operand_end + 1..field_end)?.try_into().ok()?);
        let (slot, type_id) = self.own(field)?;
        let after = code.get(field_end..)?;
        let member = match after {
            [0x30, a, b, c, d, 0x32 | 0x09, ..] if slot.bytes == 0 => {
                self.member(type_id, u32::from_le_bytes([*a, *b, *c, *d]))
            }
            _ => None,
        };
        Some(match after {
            [0x32, ..] if slot.bytes != 0 => {
                self.s.stack.push(Operand::Value);
                (OpKind::LoadOwn, self.slot(slot)?, field_end + 1)
            }
            [0x32, ..] if self.holds_instance(type_id) => {
                let loaded = self
                    .block_pou(type_id)
                    .map_or(Operand::Value, Operand::Block);
                self.s.stack.push(loaded);
                (OpKind::LoadInstance, self.slot(slot)?, field_end + 1)
            }
            [0x09, ..] if slot.bytes != 0 && self.target_conversion(after) => {
                self.s.stack.push(Operand::Target);
                (OpKind::OwnValue, self.slot(slot)?, field_end)
            }
            [0x30, _, _, _, _, 0x09, ..]
                if member.is_some()
                    && after
                        .get(5..)
                        .is_some_and(|call| self.target_conversion(call)) =>
            {
                self.s.stack.push(Operand::Target);
                (
                    OpKind::MemberValue,
                    self.pair(slot, member?)?,
                    field_end + 5,
                )
            }
            [0x13, 0x33, ..] if slot.bytes != 0 => {
                self.pop(1)?;
                (OpKind::StoreOwn, self.slot(slot)?, field_end + 2)
            }
            [0x30, _, _, _, _, 0x32, ..] if member.is_some() => {
                self.s.stack.push(Operand::Value);
                (OpKind::LoadMember, self.pair(slot, member?)?, field_end + 6)
            }
            _ => {
                self.s.stack.push(Operand::Ref(type_id));
                (OpKind::OwnRef, self.slot(slot)?, field_end)
            }
        })
    }

    /// Point every jump at the operation that begins at its target, which the body's end `len`
    /// stands for after the last operation; `start` is the body's first code offset. `None` when a
    /// target lies inside an operation, or when a reference waits on the stack there.
    fn link(&mut self, start: usize, len: usize) -> Option<()> {
        let s = &mut *self.s;
        for &(op, target) in &s.jumps {
            let to = if target == len {
                s.ops.len()
            } else {
                let at = u32::try_from(start.checked_add(target)?).ok()?;
                let to = s.ops.binary_search_by_key(&at, |op| op.at).ok()?;
                if !s.ops.get(to)?.plain {
                    return None;
                }
                to
            };
            s.ops.get_mut(op)?.arg = u32::try_from(to).ok()?;
        }
        Some(())
    }

    /// Take `count` operands off the stack and answer the last one taken.
    fn pop(&mut self, count: usize) -> Option<Operand> {
        let len = self.s.stack.len().checked_sub(count)?;
        let first = *self.s.stack.get(len)?;
        self.s.stack.truncate(len);
        Some(first)
    }

    /// Add `holder` and `member` to the variables as two neighbours, unless they are there already,
    /// and answer the index of `holder`.
    fn pair(&mut self, holder: Slot, member: Slot) -> Option<u32> {
        let slots = &mut self.s.slots;
        let index = match slots
            .windows(2)
            .position(|known| known[0] == holder && known[1] == member)
        {
            Some(index) => index,
            None => {
                slots.push(holder);
                slots.push(member);
                slots.len() - 2
            }
        };
        u32::try_from(index).ok()
    }

    /// Add `slot` to the variables and answer its index.
    fn slot(&mut self, slot: Slot) -> Option<u32> {
        let index = match self.s.slots.iter().position(|known| *known == slot) {
            Some(index) => index,
            None => {
                self.s.slots.push(slot);
                self.s.slots.len() - 1
            }
        };
        u32::try_from(index).ok()
    }

    /// Constant `index`, when it is a scalar that `CONST` pushes as its value.
    fn scalar_constant(&self, index: u32) -> Option<()> {
        match self.prepared.vm.consts.get(index as usize)? {
            Value::Bool(_)
            | Value::SInt(_)
            | Value::Int(_)
            | Value::DInt(_)
            | Value::LInt(_)
            | Value::USInt(_)
            | Value::UInt(_)
            | Value::UDInt(_)
            | Value::ULInt(_)
            | Value::Real(_)
            | Value::LReal(_)
            | Value::Byte(_)
            | Value::Word(_)
            | Value::DWord(_)
            | Value::LWord(_)
            | Value::Time(_)
            | Value::LTime(_)
            | Value::Date(_)
            | Value::LDate(_)
            | Value::Tod(_)
            | Value::LTod(_)
            | Value::Dt(_)
            | Value::Ldt(_)
            | Value::Char(_)
            | Value::WChar(_) => Some(()),
            _ => None,
        }
    }

    /// Whether the `CALL_NATIVE` at `at` finishes at once with a value: a standard function that
    /// is no text function, or a function block whose prepared plan is a standard one.
    fn immediate_call(&self, at: u32, kind: u32, symbol: u32, count: u32) -> Option<()> {
        let prepared = self.prepared;
        match kind {
            NATIVE_CALL_KIND_STDLIB => {
                let spec = prepared.vm.native_symbol_spec(symbol).ok()?;
                let count = usize::try_from(count).ok()?;
                super::dispatch_text::text_call_of(&prepared.stdlib, spec, count)
                    .is_none()
                    .then_some(())
            }
            NATIVE_CALL_KIND_FUNCTION_BLOCK => match prepared.call_plan(at)?.callee {
                CallCallee::FunctionBlock {
                    builtin: Some(_), ..
                } => Some(()),
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether `call`, the code from a `CALL_NATIVE` on, is a standard conversion whose one
    /// argument is a target that the call reads (checklist P1-5).
    fn target_conversion(&self, call: &[u8]) -> bool {
        let word = |at: usize| {
            call.get(at..at + 4)
                .and_then(|bytes| bytes.try_into().ok())
                .map(u32::from_le_bytes)
        };
        matches!(
            (call.first(), word(1), word(5), word(9)),
            (Some(0x09), Some(kind), Some(symbol), Some(count))
                if kind == NATIVE_CALL_KIND_STDLIB
                    // A standard call's plan is not looked up, so the call's offset is not needed.
                    && self.immediate_call(0, kind, symbol, count).is_some()
                    && self.conversion(symbol, count, true)
        )
    }

    /// Whether standard call `symbol` with `count` operands is a conversion that the general call
    /// binds and converts with no other step: a conversion of one positional argument or one named
    /// `IN`, a target that the call reads when `target` is set and a value otherwise, under a name
    /// that is no clock and no `SPLIT_` function (checklist P1-5;
    /// `call::stdlib::dispatch_native_stdlib_call`).
    fn conversion(&self, symbol: u32, count: u32, target: bool) -> bool {
        let Ok(super::module::VmNativeSymbolSpec::Parsed {
            target_name,
            normalized_target_name,
            conversion_spec: Some(_),
            arg_specs,
            ..
        }) = self.prepared.vm.native_symbol_spec(symbol)
        else {
            return false;
        };
        let name = normalized_target_name.as_str();
        count == 1
            && !target_name.is_empty()
            && !crate::stdlib::time::is_runtime_clock_name(name)
            && !crate::stdlib::time::is_split_name(name)
            && matches!(arg_specs.as_slice(), [argument] if argument.is_target == target
                && argument
                    .name
                    .as_ref()
                    .is_none_or(|name| name.eq_ignore_ascii_case("IN")))
    }

    /// The POU's own variable whose name is string `field`, which the compiler gives a field and
    /// its declaration alike, with its type.
    fn own(&self, field: u32) -> Option<(Slot, u32)> {
        let prepared = self.prepared;
        let declarations = prepared.declarations(StorageOwner::Instance, Some(self.pou));
        let ordinal = declarations.iter().position(|&id| {
            prepared
                .layout
                .entries
                .get(id as usize)
                .is_some_and(|entry| entry.name_idx == field)
        })?;
        let id = declarations[ordinal];
        let type_id = prepared.layout.entries.get(id as usize)?.type_id?;
        let offset = prepared.declaration_offset(id)?;
        let ordinal = u32::try_from(ordinal).ok()?;
        let slot = self.scalar(id, offset, ordinal, None).unwrap_or(Slot {
            offset,
            name: ordinal,
            prim: 0,
            bytes: 0,
            direct: false,
        });
        Some((slot, type_id))
    }

    /// The scalar variable named `field` (a string) that function block `type_id` declares itself,
    /// when no other block extends it: its slot from the first byte of the block's instance, named
    /// by the field string for the general path (checklist P1-F). A variable of a block's type
    /// holds an instance of that block, which a variable of an interface's type need not.
    fn member(&self, type_id: u32, field: u32) -> Option<Slot> {
        let prepared = self.prepared;
        let types = &prepared.vm.types;
        let entry = types
            .entries
            .get(resolved_alias_type(types, type_id, 0)? as usize)?;
        let TypeData::Pou { pou_id } = entry.data else {
            return None;
        };
        if entry.kind != TypeKind::FunctionBlock || extended(prepared, pou_id) {
            return None;
        }
        let id = *prepared
            .declarations(StorageOwner::Instance, Some(pou_id))
            .iter()
            .find(|&&id| {
                prepared
                    .layout
                    .entries
                    .get(id as usize)
                    .is_some_and(|entry| entry.name_idx == field)
            })?;
        let offset = prepared.declaration_offset(id)?;
        self.scalar(id, offset, field, None)
    }

    /// The type of member `field` (a string) of a value of `type_id`: a variable a function block
    /// declares itself, or a structure's field. `None` when the translation does not follow it.
    fn field_type(&self, type_id: u32, field: u32) -> Option<u32> {
        let prepared = self.prepared;
        let types = &prepared.vm.types;
        let resolved = resolved_alias_type(types, type_id, 0)?;
        match &types.entries.get(resolved as usize)?.data {
            TypeData::Pou { pou_id } if !extended(prepared, *pou_id) => {
                prepared
                    .declarations(StorageOwner::Instance, Some(*pou_id))
                    .iter()
                    .filter_map(|&id| prepared.layout.entries.get(id as usize))
                    .find(|entry| entry.name_idx == field)?
                    .type_id
            }
            TypeData::Struct { fields } => fields
                .iter()
                .find(|member| member.name_idx == field)
                .map(|member| member.type_id),
            _ => None,
        }
    }

    /// Whether a variable of `type_id` holds a function block instance in its leaf: a variable of
    /// a function block's type, whose LOAD pushes the instance (MEM-12's instance leaf).
    fn holds_instance(&self, type_id: u32) -> bool {
        let types = &self.prepared.vm.types;
        resolved_alias_type(types, type_id, 0)
            .and_then(|resolved| types.entries.get(resolved as usize))
            .is_some_and(|entry| {
                entry.kind == TypeKind::FunctionBlock && matches!(entry.data, TypeData::Pou { .. })
            })
    }

    /// Whether `LOAD` through a reference to a value of `type_id` pushes a value: a scalar, or an
    /// instance identity.
    fn loads_a_value(&self, type_id: u32) -> bool {
        let prepared = self.prepared;
        if prepared
            .type_layout(type_id)
            .is_some_and(|layout| layout.scalar.is_some())
        {
            return true;
        }
        let types = &prepared.vm.types;
        resolved_alias_type(types, type_id, 0)
            .and_then(|resolved| types.entries.get(resolved as usize))
            .is_some_and(|entry| matches!(entry.data, TypeData::Pou { .. }))
    }

    /// The scalar global static reference `ref_idx` names without a path.
    fn global(&self, ref_idx: u32) -> Option<Slot> {
        let prepared = self.prepared;
        let VmRef::Global { offset, path } = prepared.vm.refs.get(ref_idx as usize)? else {
            return None;
        };
        if !path.is_empty() {
            return None;
        }
        let id = *prepared
            .declarations(StorageOwner::Global, None)
            .get(*offset)?;
        let start = prepared.global_range(id)?.start;
        self.scalar(id, start, ref_idx, prepared.vm.ref_type(ref_idx))
    }

    /// The variable of storage declaration `id` at `offset`, when its leaf is a scalar without a
    /// presence byte. A store of a value of its primitive is direct when the declaration is not
    /// constant and its type, and the static reference's `declared` type when there is one, is that
    /// primitive itself, which the general path neither converts nor bounds.
    fn scalar(&self, id: u32, offset: u32, name: u32, declared: Option<u32>) -> Option<Slot> {
        let prepared = self.prepared;
        let entry = prepared.layout.entries.get(id as usize)?;
        let type_id = entry.type_id?;
        let layout = prepared.type_layout(type_id)?;
        let prim = layout.scalar?;
        if prepared.presence_offset(id, 0).is_some() {
            return None;
        }
        let primitive = |ty: u32| {
            let types = &prepared.vm.types;
            let resolved = resolved_alias_type(types, ty, 0)?;
            match types.entries.get(resolved as usize)?.data {
                TypeData::Primitive { prim_id, .. } => Some(prim_id),
                _ => None,
            }
        };
        let direct = !entry.is_constant()
            && primitive(type_id) == Some(prim)
            && declared.is_none_or(|ty| primitive(ty) == Some(prim));
        Some(Slot {
            offset,
            name,
            prim,
            bytes: u8::try_from(layout.shape.bytes).ok()?,
            direct,
        })
    }
}
