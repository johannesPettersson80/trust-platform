//! Untyped integer literals where a bit string goes without a typed partner (IEC 61131-3 Ed.3
//! §6.3.2; specification 05 §5.2.1): the input of `SHL`, `SHR`, `ROL` and `ROR`, the operand of
//! `NOT`, and both operands of `AND`, `OR`, `XOR` and `&`. A based number, or a decimal one that is
//! not negative, is a bit string literal there. Its expression is open: it takes the bit string
//! type where it goes (an assignment's target, a return, a typed parameter, the other operand) when
//! every literal's value fits that type, and otherwise the smallest of `BYTE`, `WORD`, `DWORD` and
//! `LWORD` that holds them. A typed input keeps its own type whatever the result is stored in
//! (IEC §6.6.1.7.2), so a call with one is never open.

use super::literals::{exact_int_literal_value, is_untyped_int_literal_expr};
use super::*;

/// A call whose bit string type its untyped literal inputs leave open.
#[derive(Debug, Clone)]
pub(super) struct OpenBitCall {
    /// Its literal inputs and its open call inputs, which take the call's type with it.
    inputs: Vec<SyntaxNode>,
    /// The smallest bit string type that holds every literal.
    least: TypeId,
}

/// The value of `node` when it is a bit string literal: an untyped integer literal, in parentheses
/// or not, that is not negative.
pub(super) fn bit_string_literal_value(node: &SyntaxNode) -> Option<u64> {
    let literal = without_parentheses(node);
    if literal.kind() != SyntaxKind::Literal || !is_untyped_int_literal_expr(&literal) {
        return None;
    }
    u64::try_from(exact_int_literal_value(&literal)?).ok()
}

/// The smallest of `BYTE`, `WORD`, `DWORD` and `LWORD` that holds `value`. `BOOL` takes a literal
/// only where a `BOOL` is expected (specification 05 §5.2.1).
pub(super) fn smallest_bit_string_type(value: u64) -> TypeId {
    if value <= u64::from(u8::MAX) {
        TypeId::BYTE
    } else if value <= u64::from(u16::MAX) {
        TypeId::WORD
    } else if value <= u64::from(u32::MAX) {
        TypeId::DWORD
    } else {
        TypeId::LWORD
    }
}

fn without_parentheses(node: &SyntaxNode) -> SyntaxNode {
    let mut node = node.clone();
    while node.kind() == SyntaxKind::ParenExpr {
        let Some(inner) = node.children().next() else {
            break;
        };
        node = inner;
    }
    node
}

fn key(node: &SyntaxNode) -> (u32, u32) {
    let range = node.text_range();
    (u32::from(range.start()), u32::from(range.end()))
}

impl TypeChecker<'_> {
    /// The smallest bit string type of the open input `input`, a bit string literal or an open
    /// call; `None` for any other input.
    pub(super) fn open_bit_input_type(&self, input: &SyntaxNode) -> Option<TypeId> {
        if let Some(value) = bit_string_literal_value(input) {
            return Some(smallest_bit_string_type(value));
        }
        self.open_bit_calls
            .get(&key(&without_parentheses(input)))
            .map(|call| call.least)
    }

    /// Records `call` as open with the open `inputs`, all of the type `least`, the call's own type
    /// until a context gives it another.
    pub(super) fn record_open_bit_call(
        &mut self,
        call: &SyntaxNode,
        inputs: Vec<SyntaxNode>,
        least: TypeId,
    ) {
        for input in &inputs {
            self.retype_open_bit(input, least);
        }
        self.open_bit_calls
            .insert(key(call), OpenBitCall { inputs, least });
    }

    /// Gives the open call `expr` the bit string type `expected` where it goes, when every literal
    /// of it fits: true when it did, so that the context reads `expr` as of type `expected`.
    pub(super) fn contextualize_open_bit(&mut self, expected: TypeId, expr: &SyntaxNode) -> bool {
        let Some(least) = self
            .open_bit_calls
            .get(&key(&without_parentheses(expr)))
            .map(|call| call.least)
        else {
            return false;
        };
        let base = self.resolve_subrange_base(expected);
        let fits = match (self.resolved_type(base), self.resolved_type(least)) {
            (Some(target), Some(least))
                if target.is_bit_string() && !matches!(target, Type::Bool) =>
            {
                matches!(
                    (target.bit_size(), least.bit_size()),
                    (Some(target), Some(least)) if target >= least
                )
            }
            _ => false,
        };
        if fits {
            self.retype_open_bit(expr, base);
        }
        fits
    }

    /// An open call `open` beside the bit string literal `literal`, as the operands of one operator:
    /// both take the wider of the call's smallest type and the literal's, so that
    /// `SHL(16#01, n) = 16#8000` compares two `WORD`s. `None` when they are not such a pair.
    pub(super) fn unify_open_bit_with_literal(
        &mut self,
        open: &SyntaxNode,
        literal: &SyntaxNode,
    ) -> Option<TypeId> {
        let value = bit_string_literal_value(literal)?;
        let least = self
            .open_bit_calls
            .get(&key(&without_parentheses(open)))
            .map(|call| call.least)?;
        let own = smallest_bit_string_type(value);
        let width = |checker: &Self, ty: TypeId| {
            checker
                .resolved_type(ty)
                .and_then(Type::bit_size)
                .unwrap_or(0)
        };
        let target = if width(self, own) > width(self, least) {
            own
        } else {
            least
        };
        self.retype_open_bit(open, target);
        self.retype_open_bit(literal, target);
        Some(target)
    }

    /// Records `node`, its parentheses' contents and, for an open call, its inputs as of type
    /// `type_id`.
    fn retype_open_bit(&mut self, node: &SyntaxNode, type_id: TypeId) {
        self.record_expression_type(node, type_id);
        if node.kind() == SyntaxKind::ParenExpr {
            if let Some(inner) = node.children().next() {
                self.retype_open_bit(&inner, type_id);
            }
            return;
        }
        let inputs = self
            .open_bit_calls
            .get(&key(node))
            .map(|call| call.inputs.clone())
            .unwrap_or_default();
        for input in inputs {
            self.retype_open_bit(&input, type_id);
        }
    }
}
