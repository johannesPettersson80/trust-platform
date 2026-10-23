//! Range checks for untyped integer literals at the destination they are given: an assignment
//! target, a call argument, a RETURN value, a FOR bound or a variable initializer.

use super::literals::{
    exact_int_literal_value, integer_type_range_fits, is_untyped_int_literal_expr,
};
use super::*;

impl<'a> TypeChecker<'a> {
    /// Reports an untyped integer literal that lies outside the integer destination it is given.
    /// A literal outside the range of the destination's base type is reported with its value and
    /// the base type. With `check_bounds`, a literal inside the base type but outside a subrange's
    /// bounds is reported with the subrange. Callers run this only when their own checks reported
    /// nothing for the expression.
    pub(super) fn check_untyped_literal_range(
        &mut self,
        destination: TypeId,
        expr: &SyntaxNode,
        check_bounds: bool,
    ) {
        let resolved = self.resolve_alias_type(destination);
        let (base, bounds) = match self.symbols.type_by_id(resolved) {
            Some(Type::Subrange { base, lower, upper }) => (
                self.resolve_alias_type(*base),
                Some((i128::from(*lower), i128::from(*upper))),
            ),
            Some(ty) if ty.is_integer() => (resolved, None),
            _ => return,
        };
        let Some(base_type) = self.symbols.type_by_id(base) else {
            return;
        };
        let base_name = self.type_name(base);
        if let Some(message) = untyped_literal_out_of_range_message(expr, base_type, &base_name) {
            self.diagnostics
                .error(DiagnosticCode::OutOfRange, expr.text_range(), message);
            return;
        }
        let Some((lower, upper)) = bounds.filter(|_| check_bounds) else {
            return;
        };
        if !is_untyped_int_literal_expr(expr) || !is_literal_chain(expr) {
            return;
        }
        let Some(value) = exact_int_literal_value(expr) else {
            return;
        };
        if !(lower..=upper).contains(&value) {
            self.diagnostics.error(
                DiagnosticCode::OutOfRange,
                expr.text_range(),
                format!("value {value} outside subrange {lower}..{upper}"),
            );
        }
    }
}

/// The `OutOfRange` message for an untyped integer literal whose exact value does not fit the
/// integer type `base`, named `base_name` in the message. The literal may carry parentheses or
/// signs; a binary expression is not a literal. None for any other expression, for a type that is
/// not an integer, and for a literal that fits.
pub(crate) fn untyped_literal_out_of_range_message(
    expr: &SyntaxNode,
    base: &Type,
    base_name: &str,
) -> Option<String> {
    if !base.is_integer() || !is_untyped_int_literal_expr(expr) || !is_literal_chain(expr) {
        return None;
    }
    let value = exact_int_literal_value(expr);
    if value.is_some_and(|value| integer_type_range_fits(base, value)) {
        return None;
    }
    let shown = value.map_or_else(
        || expr.text().to_string().trim().to_string(),
        |value| value.to_string(),
    );
    Some(format!(
        "integer literal {shown} is outside the range of {base_name}"
    ))
}

/// Whether `node` is an integer literal under any number of parentheses or unary signs. A binary
/// expression is not: its value comes from evaluating its operands, not from the literal text.
fn is_literal_chain(node: &SyntaxNode) -> bool {
    match node.kind() {
        SyntaxKind::Literal => true,
        SyntaxKind::ParenExpr | SyntaxKind::UnaryExpr => node
            .children()
            .next()
            .is_some_and(|child| is_literal_chain(&child)),
        _ => false,
    }
}
