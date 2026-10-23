//! The assertions of a test (specification 44 §6.8): `ASSERT_TRUE`, `ASSERT_FALSE`, the
//! comparisons `ASSERT_EQUAL` to `ASSERT_LESS_OR_EQUAL` and `ASSERT_NEAR`, decided as the runtime's
//! assertion functions decide them; a failed one ends the test with the fault whose record the
//! kernel words as the runtime words it.

use trust_syntax::syntax::SyntaxNode;

use super::call::{Arg, Called};
use super::Lowerer;
use crate::native::types::Elem;
use crate::native::NativeError;

/// The assertion `name` this module lowers: its formal parameters and its comparison, `None` for
/// `ASSERT_TRUE`, `ASSERT_FALSE` and `ASSERT_NEAR`.
pub(super) fn assertion(name: &str) -> Option<(&'static [&'static str], Option<&'static str>)> {
    const EXPECTED_ACTUAL: &[&str] = &["EXPECTED", "ACTUAL"];
    const VALUE_BOUND: &[&str] = &["VALUE", "BOUND"];
    Some(match name {
        "ASSERT_TRUE" | "ASSERT_FALSE" => (&["IN"], None),
        "ASSERT_NEAR" => (&["EXPECTED", "ACTUAL", "DELTA"], None),
        "ASSERT_EQUAL" => (EXPECTED_ACTUAL, Some("==")),
        "ASSERT_NOT_EQUAL" => (EXPECTED_ACTUAL, Some("!=")),
        "ASSERT_GREATER" => (VALUE_BOUND, Some(">")),
        "ASSERT_LESS" => (VALUE_BOUND, Some("<")),
        "ASSERT_GREATER_OR_EQUAL" => (VALUE_BOUND, Some(">=")),
        "ASSERT_LESS_OR_EQUAL" => (VALUE_BOUND, Some("<=")),
        _ => return None,
    })
}

/// The library's name of assertion `name`.
fn variant(name: &str) -> &'static str {
    match name {
        "ASSERT_TRUE" => "True",
        "ASSERT_FALSE" => "False",
        "ASSERT_EQUAL" => "Equal",
        "ASSERT_NOT_EQUAL" => "NotEqual",
        "ASSERT_GREATER" => "Greater",
        "ASSERT_LESS" => "Less",
        "ASSERT_GREATER_OR_EQUAL" => "GreaterOrEqual",
        _ => "LessOrEqual",
    }
}

impl Lowerer<'_, '_> {
    /// The call of assertion `name` with `args`; its inputs are evaluated in their order.
    pub(super) fn call_assertion(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        args: Vec<Arg>,
    ) -> Result<Called, NativeError> {
        let Some((formals, sign)) = assertion(name) else {
            return Err(self.unsupported(node, format!("the assertion {name}")));
        };
        let values = self.formal_order(node, name, formals, args)?;
        match (sign, values.as_slice()) {
            (None, [value]) => {
                let boolean = self.model.types.elem(Elem::Bool);
                let value = self.scalar_to(value, boolean)?;
                let fails = if name == "ASSERT_TRUE" {
                    format!("!{value}")
                } else {
                    value
                };
                self.line(&format!(
                    "if {fails} {{ return Err(lib::assertion_failed(cx, lib::Assertion::{}, 0, 0, 0, 0)); }}",
                    variant(name)
                ));
            }
            (None, [expected, actual, delta]) => {
                let lreal = self.model.types.elem(Elem::LReal);
                let expected = self.scalar_to(expected, lreal)?;
                let expected = self.bind(&expected);
                let actual = self.scalar_to(actual, lreal)?;
                let actual = self.bind(&actual);
                let delta = self.scalar_to(delta, lreal)?;
                self.line(&format!(
                    "lib::assert_near(cx, {expected}, {actual}, {delta})?;"
                ));
            }
            (Some(sign), [left, right]) => {
                let left_ty = self.type_of(left)?;
                let right_ty = self.type_of(right)?;
                let (Some(_), Some(_)) =
                    (self.info(left_ty).scalar(), self.info(right_ty).scalar())
                else {
                    return Err(
                        self.unsupported(node, format!("{name} of values that are no scalars"))
                    );
                };
                let common = self.common_of(node, &[left.clone(), right.clone()])?;
                let left_value = self.scalar(left)?;
                let left_value = self.bind(&left_value);
                let right_value = self.scalar(right)?;
                let right_value = self.bind(&right_value);
                let a = self.convert(left, &left_value, left_ty, common)?;
                let b = self.convert(right, &right_value, right_ty, common)?;
                self.line(&format!(
                    "if !({a} {sign} {b}) {{ return Err(lib::assertion_failed(cx, lib::Assertion::{}, lib::Operand::bits({left_value}), lib::Operand::bits({right_value}), {}, {})); }}",
                    variant(name),
                    left_ty.0,
                    right_ty.0
                ));
            }
            _ => return Err(self.unsupported(node, format!("{name} with these inputs"))),
        }
        Ok(Called::Nothing)
    }
}
