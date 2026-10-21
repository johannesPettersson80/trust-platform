//! The loop that runs a body's compact code on its scalars (checklist P1-9; `vm::compact`).
//!
//! The dispatcher enters it where it would enter the compact loop, and the entry of a frameless
//! program body enters it before the body's frame exists, which it then pushes before the first
//! operation that needs it (`compact_entry`). It holds the body's operands itself, as the cells
//! their leaves decode to, in a stack of [`DEPTH`]: a load decodes the variable's leaf and a store
//! encodes the operand into it, and the arithmetic and comparisons of two integers and the logic
//! and equality of two BOOLs are computed on the cells by the rules the general path applies to
//! their values (`program_model::ops`). Every other operation is done by the function the compact
//! loop calls for it, on its operands as values. Each operation records and charges its
//! instructions as the compact loop does, before its work.
//!
//! Before an operation that takes a reference, that finds an operand it does not hold, that the
//! compact loop gives the general path before its charges, or that would push past the stack's
//! room, the loop hands the body over: its operands go onto the operand stack as values, the
//! deepest first, and the compact loop runs the operations that remain. An operation whose result
//! is no scalar hands it over after its work, with the result on top. A body therefore ends as on
//! the compact loop: with the same result, the same charges, the same fault record and the same
//! values.

use super::super::compact::{CompactCode, Op, OpKind, Slot};
use super::super::context::ExecutionContext;
use super::super::dispatch_refs::store_ref;
use super::super::module::VmModule;
use super::super::type_policy::primitive_cell_matches;
use super::super::{FrameStack, OperandStack, VmTrap, VM_MAX_OPERAND_STACK};
use super::compact_block;
use super::compact_loop::{self, begin, begin_each, globals, leaf, own_instance, push};
use crate::error::RuntimeError;
use crate::fault::FaultSite;
use crate::memory::cells::{cell_slot, scalar_slot, Cell};
use crate::memory::layout::ByteRange;
use crate::memory::InstanceId;
use crate::numeric::{wider_numeric, NumericKind};
use crate::program_model::ops::{signed_arith, unsigned_arith};
use crate::program_model::{apply_binary, apply_unary, BinaryOp, UnaryOp};
use crate::value::Value;

/// The operands the loop holds at most.
const DEPTH: usize = 16;

/// The frame of the body the loop runs.
#[derive(Clone, Copy)]
pub(super) enum Frame {
    /// The dispatcher pushed it before the body.
    Pushed,
    /// The entry of a frameless program body (`compact_entry`) deferred it: the program's instance
    /// and the call depth the frame is pushed at when an operation needs it.
    Deferred {
        instance: Option<InstanceId>,
        depth: u32,
    },
}

/// Run `code`, the compact code of the body of POU `pou`, from its first operation to the body's
/// end, which `end` is; `pc` is then `end`. The body's frame is `frame`; a deferred one is pushed,
/// as the dispatcher pushes and initializes it, before the first operation that needs it: a
/// hand-over to the compact loop, a call the dispatcher's functions run or a store the general
/// path makes. Answers whether it pushed the deferred frame, which the caller then finishes. When
/// the operand stack lacks the room for the operands the loop may hold, the compact loop runs the
/// body, so a push meets the operand stack's limit where it would.
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
    frame: Frame,
) -> Result<bool, RuntimeError> {
    let (instance, deferred) = match frame {
        Frame::Pushed => (
            frames.current().and_then(|frame| frame.runtime_instance),
            None,
        ),
        Frame::Deferred { instance, depth } => (instance, Some(depth)),
    };
    let own = instance.and_then(|instance| runtime.instance_bytes(instance));
    let mut body = Body {
        runtime,
        module,
        code,
        frames,
        fault_site,
        pou,
        instance,
        own,
        deferred,
        pushed: false,
    };
    if operand_stack.depth() + DEPTH > VM_MAX_OPERAND_STACK {
        body.ensure_frame()?;
        return body.hand_over(operand_stack, pc, end, 0);
    }
    // A body whose operands' kinds preparation fixed runs on their bits (checklist P1-9).
    if let Some(kinded_code) = code.kinded.as_ref() {
        return kinded::run(body, kinded_code, operand_stack, pc, end);
    }
    let mut operands = Operands {
        cells: [Cell::Null; DEPTH],
        len: 0,
    };
    let mut next = 0;
    while let Some(op) = code.ops.get(next) {
        let (first, result) = match body.step(&mut operands, operand_stack, *op)? {
            Flow::Next => {
                next += 1;
                continue;
            }
            Flow::Jump(to) => {
                next = to;
                continue;
            }
            Flow::Before => (next, None),
            Flow::After(value) => (next + 1, Some(value)),
        };
        body.ensure_frame()?;
        for cell in operands.top(operands.len).unwrap_or_default() {
            push(operand_stack, value_of(*cell)?)?;
        }
        if let Some(value) = result {
            push(operand_stack, value)?;
        }
        return body.hand_over(operand_stack, pc, end, first);
    }
    *pc = end;
    Ok(body.pushed)
}

/// What the loop does after an operation.
enum Flow {
    /// It runs the next operation.
    Next,
    /// It runs the operation at this index next.
    Jump(usize),
    /// It hands the body over before the operation, which it has not begun.
    Before,
    /// It hands the body over after the operation, whose result, no scalar, goes on top.
    After(Value),
}

/// The body's operands, the deepest first.
struct Operands {
    cells: [Cell; DEPTH],
    len: usize,
}

impl Operands {
    /// The top `count` operands, the deepest first.
    fn top(&self, count: usize) -> Option<&[Cell]> {
        self.cells.get(self.len.checked_sub(count)?..self.len)
    }

    /// The operand on top.
    fn last(&self) -> Option<Cell> {
        self.top(1).map(|cells| cells[0])
    }

    /// Whether one more operand fits.
    fn has_room(&self) -> bool {
        self.len < DEPTH
    }

    /// Push `cell`; the caller has checked the room.
    fn push(&mut self, cell: Cell) {
        if let Some(free) = self.cells.get_mut(self.len) {
            *free = cell;
            self.len += 1;
        }
    }

    /// Take `count` operands off the top; the caller has checked they are there.
    fn discard(&mut self, count: usize) {
        self.len = self.len.saturating_sub(count);
    }
}

/// What an operation reaches: the engine, the module, the body's compact code, the frames, the
/// fault site, the frame's POU and instance, and the instance's bytes; the call depth of a frame
/// the entry deferred, and whether the loop pushed it.
struct Body<'a, R: ExecutionContext> {
    runtime: &'a mut R,
    module: &'a VmModule,
    code: &'a CompactCode,
    frames: &'a mut FrameStack,
    fault_site: &'a mut Option<FaultSite>,
    pou: u32,
    instance: Option<InstanceId>,
    own: Option<ByteRange>,
    deferred: Option<u32>,
    pushed: bool,
}

impl<R: ExecutionContext> Body<'_, R> {
    /// Push the frame the entry deferred, as the dispatcher pushes and initializes it.
    fn ensure_frame(&mut self) -> Result<(), RuntimeError> {
        if let Some(depth) = self.deferred.take() {
            super::compact_entry::push_frame(
                self.runtime,
                self.frames,
                self.module,
                self.pou,
                self.instance,
                depth,
            )?;
            self.pushed = true;
        }
        Ok(())
    }

    /// Run the body on the compact loop from operation `first`, the operands on the operand stack.
    fn hand_over(
        self,
        operand_stack: &mut OperandStack,
        pc: &mut usize,
        end: usize,
        first: usize,
    ) -> Result<bool, RuntimeError> {
        compact_loop::run(
            self.runtime,
            self.module,
            self.code,
            operand_stack,
            self.frames,
            self.fault_site,
            self.pou,
            pc,
            end,
            first,
        )?;
        Ok(self.pushed)
    }

    /// Run `op` on `operands`, as the compact loop runs it on the operand stack, which a call is
    /// given its arguments on.
    fn step(
        &mut self,
        operands: &mut Operands,
        operand_stack: &mut OperandStack,
        op: Op,
    ) -> Result<Flow, RuntimeError> {
        let at = op.at;
        match op.kind {
            OpKind::Const => {
                let cell = self.module.consts.get(op.arg as usize).and_then(cell_of);
                self.push_read(operands, cell, at, [0], true)
            }
            OpKind::LoadOwn => {
                let cell = self.slot(op.arg).and_then(|slot| self.read(self.own, slot));
                self.push_read(operands, cell, at, [0, 1, 6], true)
            }
            OpKind::LoadGlobal => {
                let cell = self
                    .slot(op.arg)
                    .and_then(|slot| self.read(globals(), slot));
                self.push_read(operands, cell, at, [0], true)
            }
            OpKind::LoadInstance => {
                let cell = self
                    .slot(op.arg)
                    .and_then(|holder| own_instance(&*self.runtime, self.own, holder))
                    .map(Cell::Instance);
                self.push_read(operands, cell, at, [0, 1, 6], true)
            }
            OpKind::OwnValue => {
                let cell = self.slot(op.arg).and_then(|slot| self.read(self.own, slot));
                self.push_read(operands, cell, at, [0, 1], false)
            }
            OpKind::MemberValue => {
                let cell = self.member(op.arg);
                self.push_read(operands, cell, at, [0, 1, 6], false)
            }
            OpKind::LoadMember => {
                let cell = self.member(op.arg);
                self.push_read(operands, cell, at, [0, 1, 6, 11], true)
            }
            OpKind::Dup => {
                let top = operands.last();
                self.push_read(operands, top, at, [0], true)
            }
            OpKind::StoreOwn => self.store(operands, op, self.own, [0, 1, 6, 7]),
            OpKind::StoreGlobal => self.store(operands, op, globals(), [0]),
            OpKind::Binary(binary) => self.binary(operands, at, binary),
            OpKind::Equality(true) => self.binary(operands, at, BinaryOp::Eq),
            OpKind::Equality(false) => self.binary(operands, at, BinaryOp::Ne),
            OpKind::Unary(unary) => self.unary(operands, at, unary),
            OpKind::CallBlock => self.call_block(operands, at, op.arg),
            OpKind::Convert => self.convert(operands, at, op.arg),
            OpKind::Call => self.call(operands, operand_stack, at),
            OpKind::Jump => {
                self.begin(at)?;
                Ok(Flow::Jump(op.arg as usize))
            }
            OpKind::JumpIf(when) => {
                let Some(condition) = operands.last() else {
                    return Ok(Flow::Before);
                };
                self.begin(at)?;
                operands.discard(1);
                match condition {
                    Cell::Bool(condition) if condition == when => Ok(Flow::Jump(op.arg as usize)),
                    Cell::Bool(_) => Ok(Flow::Next),
                    _ => Err(VmTrap::ConditionNotBool.into_runtime_error()),
                }
            }
            OpKind::Pop => {
                if operands.last().is_none() {
                    return Ok(Flow::Before);
                }
                self.begin(at)?;
                operands.discard(1);
                Ok(Flow::Next)
            }
            OpKind::Swap => {
                let Some(&[below, top]) = operands.top(2) else {
                    return Ok(Flow::Before);
                };
                self.begin(at)?;
                operands.discard(2);
                operands.push(top);
                operands.push(below);
                Ok(Flow::Next)
            }
            OpKind::Skip => {
                self.begin(at)?;
                Ok(Flow::Next)
            }
            // A reference is held on the operand stack.
            OpKind::OwnRef | OpKind::RefField | OpKind::Load => Ok(Flow::Before),
        }
    }

    /// Record the instruction at `at` as the fault site and charge its unit.
    fn begin(&mut self, at: u32) -> Result<(), RuntimeError> {
        begin(&*self.runtime, self.fault_site, self.pou, at)
    }

    /// Variable `index` of the body.
    fn slot(&self, index: u32) -> Option<Slot> {
        self.code.slots.get(index as usize).copied()
    }

    /// The cell of the scalar of `slot` in the bytes at `base`.
    fn read(&self, base: Option<ByteRange>, slot: Slot) -> Option<Cell> {
        let range = leaf(base?, slot)?;
        self.runtime.cell_at(range, slot.prim)
    }

    /// The cell of the member in variables `index` and `index + 1`, the own function block instance
    /// that holds it and the member, read where it lies.
    fn member(&self, index: u32) -> Option<Cell> {
        let (holder, member) = (self.slot(index)?, self.slot(index + 1)?);
        let instance = own_instance(&*self.runtime, self.own, holder)?;
        self.read(self.runtime.instance_bytes(instance), member)
    }

    /// Push `cell`, read before the instructions at `offsets` from `at` are recorded and charged,
    /// which they are then, with the admission of its copy when `copies`. Hands over when there is
    /// no cell or no room for it.
    fn push_read<const N: usize>(
        &mut self,
        operands: &mut Operands,
        cell: Option<Cell>,
        at: u32,
        offsets: [u32; N],
        copies: bool,
    ) -> Result<Flow, RuntimeError> {
        let Some(cell) = cell.filter(|_| operands.has_room()) else {
            return Ok(Flow::Before);
        };
        begin_each(&*self.runtime, self.fault_site, self.pou, at, offsets)?;
        if copies {
            self.runtime.before_scalar_clone()?;
        }
        operands.push(cell);
        Ok(Flow::Next)
    }

    /// The store of `op`, which covers the instructions at `offsets` from its offset, of the
    /// operand on top into its variable in the bytes at `base`: written into the leaf when the
    /// general path would store it as it is, and given to the general store otherwise.
    fn store<const N: usize>(
        &mut self,
        operands: &mut Operands,
        op: Op,
        base: Option<ByteRange>,
        offsets: [u32; N],
    ) -> Result<Flow, RuntimeError> {
        let (Some(slot), Some(cell)) = (self.slot(op.arg), operands.last()) else {
            return Ok(Flow::Before);
        };
        begin_each(&*self.runtime, self.fault_site, self.pou, op.at, offsets)?;
        operands.discard(1);
        let written = slot.direct
            && primitive_cell_matches(slot.prim, &cell)
            && base
                .and_then(|base| leaf(base, slot))
                .is_some_and(|range| self.runtime.store_cell_at(range, slot.prim, cell));
        if !written {
            self.ensure_frame()?;
            let value = value_of(cell)?;
            if matches!(op.kind, OpKind::StoreOwn) {
                compact_loop::store_own(
                    self.runtime,
                    self.module,
                    self.frames,
                    self.instance,
                    slot,
                    value,
                )?;
            } else {
                store_ref(self.runtime, self.module, self.frames, slot.name, value)
                    .map_err(VmTrap::into_runtime_error)?;
            }
        }
        Ok(Flow::Next)
    }

    /// The binary operation `op` at `at` on the two operands on top.
    fn binary(
        &mut self,
        operands: &mut Operands,
        at: u32,
        op: BinaryOp,
    ) -> Result<Flow, RuntimeError> {
        let Some(&[left, right]) = operands.top(2) else {
            return Ok(Flow::Before);
        };
        self.begin(at)?;
        operands.discard(2);
        if let Some(result) = cell_binary(op, left, right) {
            operands.push(result?);
            return Ok(Flow::Next);
        }
        let value = apply_binary(
            op,
            value_of(left)?,
            value_of(right)?,
            self.runtime.profile(),
        )?;
        Ok(push_result(operands, value))
    }

    /// The unary operation `op` at `at` on the operand on top.
    fn unary(
        &mut self,
        operands: &mut Operands,
        at: u32,
        op: UnaryOp,
    ) -> Result<Flow, RuntimeError> {
        let Some(operand) = operands.last() else {
            return Ok(Flow::Before);
        };
        self.begin(at)?;
        operands.discard(1);
        if let (UnaryOp::Not, Cell::Bool(value)) = (op, operand) {
            operands.push(Cell::Bool(!value));
            return Ok(Flow::Next);
        }
        let value = apply_unary(op, value_of(operand)?)?;
        Ok(push_result(operands, value))
    }

    /// The call at `at` of the standard function block of the body's block call `index`, on its
    /// receiver and arguments on top, as the compact loop runs it (`compact_block`). A call that
    /// the compact loop gives the general call is handed over.
    fn call_block(
        &mut self,
        operands: &mut Operands,
        at: u32,
        index: u32,
    ) -> Result<Flow, RuntimeError> {
        let Some(&block) = self.code.blocks.get(index as usize) else {
            return Ok(Flow::Before);
        };
        let Ok(inputs) = compact_block::inputs(self.code, block) else {
            return Ok(Flow::Before);
        };
        let count = usize::from(block.operands);
        let Some(arguments) = operands.top(count) else {
            return Ok(Flow::Before);
        };
        let Some((instance, base)) =
            compact_block::receiver(&*self.runtime, block, inputs, arguments)
        else {
            return Ok(Flow::Before);
        };
        begin(&*self.runtime, self.fault_site, self.pou, at)?;
        compact_block::bind(self.runtime, inputs, arguments, base)?;
        operands.discard(count);
        compact_block::run(self.runtime, block, instance, base)?;
        operands.push(Cell::Null);
        Ok(Flow::Next)
    }

    /// The standard conversion `symbol` at `at` of the operand on top.
    fn convert(
        &mut self,
        operands: &mut Operands,
        at: u32,
        symbol: u32,
    ) -> Result<Flow, RuntimeError> {
        let Some(argument) = operands.last().and_then(|cell| value_of(cell).ok()) else {
            return Ok(Flow::Before);
        };
        self.begin(at)?;
        operands.discard(1);
        let value = compact_loop::converted(self.runtime, self.module, symbol, &argument)?;
        Ok(push_result(operands, value))
    }

    /// The `CALL_NATIVE` at `at` of a call that finishes at once with a value, as the compact loop
    /// runs it: its arguments are given to it on the operand stack.
    fn call(
        &mut self,
        operands: &mut Operands,
        operand_stack: &mut OperandStack,
        at: u32,
    ) -> Result<Flow, RuntimeError> {
        let Some(count) = call_operands(self.module, at) else {
            return Ok(Flow::Before);
        };
        let Some(arguments) = operands.top(count) else {
            return Ok(Flow::Before);
        };
        if count == 0 && !operands.has_room() {
            return Ok(Flow::Before);
        }
        self.begin(at)?;
        self.ensure_frame()?;
        for argument in arguments {
            push(operand_stack, value_of(*argument)?)?;
        }
        operands.discard(count);
        compact_loop::call(self.runtime, self.module, self.frames, operand_stack, at)?;
        let value = operand_stack.pop().map_err(VmTrap::into_runtime_error)?;
        Ok(push_result(operands, value))
    }
}

/// Push `value`, an operation's result, as its cell; a value that has none is handed over.
fn push_result(operands: &mut Operands, value: Value) -> Flow {
    match cell_of(&value) {
        Some(cell) => {
            operands.push(cell);
            Flow::Next
        }
        None => Flow::After(value),
    }
}

/// The operands a `CALL_NATIVE` at `at` takes.
fn call_operands(module: &VmModule, at: u32) -> Option<usize> {
    let start = (at as usize).checked_add(9)?;
    let count = module.code.get(start..start.checked_add(4)?)?;
    usize::try_from(u32::from_le_bytes(count.try_into().ok()?)).ok()
}

/// The cell the loop holds for `value`: a scalar's, an instance's or no value's.
fn cell_of(value: &Value) -> Option<Cell> {
    match value {
        Value::Null => Some(Cell::Null),
        Value::Instance(instance) => Some(Cell::Instance(*instance)),
        _ => scalar_slot(value),
    }
}

/// The value of a cell the loop holds.
fn value_of(cell: Cell) -> Result<Value, RuntimeError> {
    match cell {
        Cell::Null => Ok(Value::Null),
        Cell::Instance(instance) => Ok(Value::Instance(instance)),
        _ => cell_slot(cell).ok_or_else(|| RuntimeError::InvalidExecutionState),
    }
}

/// `op` on two integers or two BOOLs, computed on their cells by the rules `apply_binary` applies
/// to their values; `None` for any other pair, which `apply_binary` computes.
fn cell_binary(op: BinaryOp, left: Cell, right: Cell) -> Option<Result<Cell, RuntimeError>> {
    if let (Cell::Bool(a), Cell::Bool(b)) = (left, right) {
        // `logical_or_bitwise` and `numeric_eq`'s equality of two values that are no numbers; an
        // ordering or arithmetic takes `apply_binary`.
        let result = match op {
            BinaryOp::And => a && b,
            BinaryOp::Or => a || b,
            BinaryOp::Xor => a ^ b,
            BinaryOp::Eq => a == b,
            BinaryOp::Ne => a != b,
            _ => return None,
        };
        return Some(Ok(Cell::Bool(result)));
    }
    Some(integers(op, Integer::of(left)?, Integer::of(right)?))
}

/// An integer cell's kind and value, widened as `to_i64` widens a signed and `to_u64` an unsigned
/// one.
#[derive(Clone, Copy)]
enum Integer {
    Signed(NumericKind, i64),
    Unsigned(NumericKind, u64),
}

impl Integer {
    fn of(cell: Cell) -> Option<Self> {
        Some(match cell {
            Cell::SInt(value) => Self::Signed(NumericKind::SInt, value.into()),
            Cell::Int(value) => Self::Signed(NumericKind::Int, value.into()),
            Cell::DInt(value) => Self::Signed(NumericKind::DInt, value.into()),
            Cell::LInt(value) => Self::Signed(NumericKind::LInt, value),
            Cell::USInt(value) => Self::Unsigned(NumericKind::USInt, value.into()),
            Cell::UInt(value) => Self::Unsigned(NumericKind::UInt, value.into()),
            Cell::UDInt(value) => Self::Unsigned(NumericKind::UDInt, value.into()),
            Cell::ULInt(value) => Self::Unsigned(NumericKind::ULInt, value),
            _ => return None,
        })
    }

    fn kind(self) -> NumericKind {
        match self {
            Self::Signed(kind, _) | Self::Unsigned(kind, _) => kind,
        }
    }
}

/// `op` on two integers, as `apply_binary` computes it on their values: the logic of
/// `logical_or_bitwise` refuses them, and the arithmetic of `numeric_arith`, the equality of
/// `numeric_eq` and the ordering of `numeric_cmp` take their common type, which `wider_numeric`
/// finds within one signedness, and compute in its 64-bit width.
fn integers(op: BinaryOp, left: Integer, right: Integer) -> Result<Cell, RuntimeError> {
    if matches!(
        op,
        BinaryOp::And | BinaryOp::Or | BinaryOp::AndThen | BinaryOp::OrElse | BinaryOp::Xor
    ) {
        return Err(RuntimeError::TypeMismatch);
    }
    let target =
        wider_numeric(left.kind(), right.kind()).ok_or_else(|| RuntimeError::TypeMismatch)?;
    let arithmetic = matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::Pow
    );
    let ordering = match (left, right) {
        (Integer::Signed(_, a), Integer::Signed(_, b)) => {
            if arithmetic {
                return signed_cell(target, signed_arith(op, a, b)?);
            }
            a.cmp(&b)
        }
        (Integer::Unsigned(_, a), Integer::Unsigned(_, b)) => {
            if arithmetic {
                return unsigned_cell(target, unsigned_arith(op, a, b)?);
            }
            a.cmp(&b)
        }
        // A common type is never found across the signedness.
        _ => return Err(RuntimeError::TypeMismatch),
    };
    Ok(Cell::Bool(match op {
        BinaryOp::Eq => ordering.is_eq(),
        BinaryOp::Ne => ordering.is_ne(),
        BinaryOp::Lt => ordering.is_lt(),
        BinaryOp::Le => ordering.is_le(),
        BinaryOp::Gt => ordering.is_gt(),
        _ => ordering.is_ge(),
    }))
}

/// The cell of `value` narrowed to signed `target`, as `signed_from_i128` narrows it.
fn signed_cell(target: NumericKind, value: i64) -> Result<Cell, RuntimeError> {
    let overflow = |_| RuntimeError::Overflow;
    Ok(match target {
        NumericKind::SInt => Cell::SInt(i8::try_from(value).map_err(overflow)?),
        NumericKind::Int => Cell::Int(i16::try_from(value).map_err(overflow)?),
        NumericKind::DInt => Cell::DInt(i32::try_from(value).map_err(overflow)?),
        NumericKind::LInt => Cell::LInt(value),
        _ => return Err(RuntimeError::TypeMismatch),
    })
}

/// The cell of `value` narrowed to unsigned `target`, as `unsigned_from_u128` narrows it.
fn unsigned_cell(target: NumericKind, value: u64) -> Result<Cell, RuntimeError> {
    let overflow = |_| RuntimeError::Overflow;
    Ok(match target {
        NumericKind::USInt => Cell::USInt(u8::try_from(value).map_err(overflow)?),
        NumericKind::UInt => Cell::UInt(u16::try_from(value).map_err(overflow)?),
        NumericKind::UDInt => Cell::UDInt(u32::try_from(value).map_err(overflow)?),
        NumericKind::ULInt => Cell::ULInt(value),
        _ => return Err(RuntimeError::TypeMismatch),
    })
}

mod kinded;
#[cfg(test)]
mod tests;
