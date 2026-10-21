//! Calls of standard function blocks in compact code (checklist P1-5; specification 34 PERF-05).
//!
//! A call of a standard function block that the POU declares itself, which binds values to inputs
//! only, is one operation. It writes each bound input's argument into the block's member where it
//! lies and runs the block's native code, with the general call's checks of the deadline before and
//! after it, its charges and its refusals (`dispatch::compact_block`). An unbound input keeps its
//! value: the general call reads it and writes it back unchanged. A receiver that is not a
//! persistent instance of the block, or an argument that is not a value of its member's primitive,
//! takes the general call when the operation runs; a call that binds an output takes it always.

use super::*;
use crate::bytecode::{TypeKind, NATIVE_CALL_KIND_FUNCTION_BLOCK};
use crate::stdlib::fbs::members::NO_MEMBER;
use crate::stdlib::fbs::BuiltinFbKind;
use crate::vm::module::VmNativeSymbolSpec;

/// A call of a standard function block that binds input values only: the block's POU and kind, how
/// many operands the call takes (its receiver and its arguments), and its bound inputs in formal
/// order, the `bound` variables from `first` in [`CompactCode::slots`]. Each is the member the input
/// is, from the first byte of the block's instance, and its `name` is the position of the input's
/// argument among the call's arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BlockCall {
    pub(crate) pou: u32,
    pub(crate) first: u32,
    pub(crate) kind: BuiltinFbKind,
    pub(crate) operands: u8,
    pub(crate) bound: u8,
}

crate::memory::figure::holds_no_heap!(BlockCall);

impl Translation<'_, '_> {
    /// The POU of a value of `type_id` that a block call can have as its receiver: a standard
    /// function block that no other block extends.
    pub(super) fn block_pou(&self, type_id: u32) -> Option<u32> {
        let prepared = self.prepared;
        let types = &prepared.vm.types;
        let entry = types
            .entries
            .get(resolved_alias_type(types, type_id, 0)? as usize)?;
        let TypeData::Pou { pou_id } = entry.data else {
            return None;
        };
        (entry.kind == TypeKind::FunctionBlock
            && !extended(prepared, pou_id)
            && prepared.builtin_formals(pou_id).is_some())
        .then_some(pou_id)
    }

    /// The block call of the `CALL_NATIVE` of `kind`, `symbol` and `count` operands at `at`, when its
    /// receiver, below its arguments, is a standard block, every argument is a value, its plan calls
    /// that block, and it binds values to inputs only, each a scalar member it may write: the call's
    /// index among the body's block calls.
    pub(super) fn block_call(
        &mut self,
        at: u32,
        kind: u32,
        symbol: u32,
        count: u32,
    ) -> Option<u32> {
        if kind != NATIVE_CALL_KIND_FUNCTION_BLOCK {
            return None;
        }
        let prepared = self.prepared;
        let plan = prepared.call_plan(at)?;
        if u32::from(plan.kind) != kind || plan.symbol_idx != symbol {
            return None;
        }
        let CallCallee::FunctionBlock {
            pou,
            builtin: Some(block),
        } = plan.callee
        else {
            return None;
        };
        let operands = usize::try_from(count).ok()?;
        let depth = self.s.stack.len().checked_sub(operands)?;
        let (receiver, arguments) = self.s.stack.get(depth..)?.split_first()?;
        if *receiver != Operand::Block(pou)
            || arguments.iter().any(|operand| *operand != Operand::Value)
        {
            return None;
        }
        let Ok(VmNativeSymbolSpec::Parsed { arg_specs, .. }) =
            prepared.vm.native_symbol_spec(symbol)
        else {
            return None;
        };
        if arg_specs.len() + 1 != operands || arg_specs.iter().any(|spec| spec.is_target) {
            return None;
        }
        let params = prepared.vm.pou_params(pou)?;
        let formals = *prepared.builtin_formals(pou)?;
        let declarations = prepared.declarations(StorageOwner::Instance, Some(pou));
        let first = u32::try_from(self.s.slots.len()).ok()?;
        let mut bound = 0u8;
        for (index, param) in params.iter().enumerate() {
            let actual = plan.arguments.actual(index);
            if param.direction != 0 {
                // An output or an in-out the call binds is copied back by the general call.
                if actual.is_some() {
                    return None;
                }
                continue;
            }
            let position = formals.get(index).copied().filter(|&p| p != NO_MEMBER)?;
            let id = *declarations.get(usize::from(position))?;
            let offset = prepared.declaration_offset(id)?;
            let member = self.scalar(id, offset, u32::try_from(actual.unwrap_or(0)).ok()?, None)?;
            if let Some(argument) = actual {
                if !member.direct {
                    return None;
                }
                self.s.slots.push(Slot {
                    name: u32::try_from(argument).ok()?,
                    ..member
                });
                bound = bound.checked_add(1)?;
            }
        }
        let index = u32::try_from(self.s.blocks.len()).ok()?;
        self.s.blocks.push(BlockCall {
            pou,
            first,
            kind: block,
            operands: u8::try_from(operands).ok()?,
            bound,
        });
        Some(index)
    }
}
