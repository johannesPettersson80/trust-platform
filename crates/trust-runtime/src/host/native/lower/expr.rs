//! Expressions (IEC §7.3.2; specification 05; specification 44 §6.5, §6.6): literals, names,
//! operators and their checks, and the implicit conversions of an assignment or an operand; texts
//! are lowered in `text`.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::{is_scalar, Lowerer, Place};
use crate::harness::literal_text;
use crate::native::types::{Elem, Kind, Ty};
use crate::native::NativeError;
use crate::value::Value;

/// The rank of a scalar type among the types an operand may widen to (IEC §6.4.2, Table 11's
/// implicit conversions): a wider type of the same family ranks higher.
fn rank(elem: Elem) -> Option<(u8, u32)> {
    let family = if elem.is_signed() {
        0
    } else if elem.is_unsigned() {
        1
    } else if elem.is_real() {
        2
    } else if elem.is_bits() {
        3
    } else if elem.is_duration() {
        4
    } else {
        return None;
    };
    Some((family, elem.size()))
}

impl Lowerer<'_, '_> {
    /// The scalar value of `node` as a Rust value of the type trust-hir gave it.
    pub(super) fn scalar(&mut self, node: &SyntaxNode) -> Result<String, NativeError> {
        let ty = self.type_of(node)?;
        self.scalar_typed(node, ty)
    }

    /// The scalar value of `node` converted to `target`, as an assignment, an argument or an
    /// operand converts it.
    pub(super) fn scalar_to(
        &mut self,
        node: &SyntaxNode,
        target: Ty,
    ) -> Result<String, NativeError> {
        // A literal, also one with a sign (`-273.15`), takes the type of where it goes, as the
        // runtime lowers it.
        if (node.kind() == SyntaxKind::Literal || signed_literal(node))
            && is_scalar(&self.info(target).kind)
        {
            return self.literal(node, target);
        }
        // Arithmetic of untyped literals alone is computed in the type of where it goes, as each of
        // its literals would be (specification 05 §5.2.1): `(16 * 45 + 1) / 2` given to an `INT`
        // is 360, never an overflow of the smallest type of 16.
        if let Some(elem) = self.info(target).scalar()
            && (elem.is_integer() || elem.is_real())
            && untyped_arithmetic(node, elem.is_real())
        {
            return self.scalar_typed(node, target);
        }
        let ty = self.type_of(node)?;
        let value = self.scalar_typed(node, ty)?;
        self.convert(node, &value, ty, target)
    }

    fn scalar_typed(&mut self, node: &SyntaxNode, ty: Ty) -> Result<String, NativeError> {
        if !is_scalar(&self.info(ty).kind) {
            return Err(
                self.unsupported(node, "a value that is no scalar where a scalar is needed")
            );
        }
        match node.kind() {
            // The parenthesis has its inner expression's type, or the type of where untyped
            // literal arithmetic goes.
            SyntaxKind::ParenExpr => match node.children().next() {
                Some(inner) => self.scalar_to(&inner, ty),
                None => Err(self.unsupported(node, "an empty parenthesis")),
            },
            SyntaxKind::Literal => self.literal(node, ty),
            SyntaxKind::NameRef => {
                if let Some(value) = self.enum_value(node, ty) {
                    return Ok(value);
                }
                let place = self.place(node)?;
                Ok(self.load(&place))
            }
            SyntaxKind::FieldExpr | SyntaxKind::IndexExpr => {
                if let Some(value) = self.enum_value(node, ty) {
                    return Ok(value);
                }
                if let Some((base, access)) = self.partial_access(node) {
                    return self.partial_read(node, &base, access, ty);
                }
                if let Some(value) = self.string_element(node)? {
                    return Ok(value);
                }
                let place = self.place(node)?;
                Ok(self.load(&place))
            }
            SyntaxKind::DerefExpr => {
                let place = self.place(node)?;
                Ok(self.load(&place))
            }
            SyntaxKind::BinaryExpr => self.binary(node, ty),
            SyntaxKind::UnaryExpr => self.unary(node, ty),
            SyntaxKind::CallExpr => self.call_scalar(node, ty),
            SyntaxKind::SizeOfExpr => self.size_of(node, ty),
            other => Err(self.unsupported(node, format!("the expression {other:?}"))),
        }
    }

    /// `SIZEOF(operand)` (specification 29 §1): the byte count of the type trust-hir resolved its
    /// operand to, a constant of the target the program is built for, as a value of `ty`.
    fn size_of(&mut self, node: &SyntaxNode, ty: Ty) -> Result<String, NativeError> {
        let Some(operand) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::TypeRef || child.kind().is_expression_node())
        else {
            return Err(self.unsupported(node, "SIZEOF without an operand"));
        };
        let Some(hir) = self.src.type_of(&operand) else {
            return Err(self.unsupported(node, "SIZEOF of an operand without a type"));
        };
        let symbols = self.src.analysis.symbols.clone();
        let bytes = self
            .model
            .types
            .size_of_hir(&symbols, hir, self.pointer_bytes)?;
        let Some(elem) = self.info(ty).scalar() else {
            return Err(self.unsupported(node, "SIZEOF as a value that is no scalar"));
        };
        let fits = match elem.rust() {
            "i8" => i8::try_from(bytes).is_ok(),
            "i16" => i16::try_from(bytes).is_ok(),
            "i32" => i32::try_from(bytes).is_ok(),
            "i64" => i64::try_from(bytes).is_ok(),
            "u8" => u8::try_from(bytes).is_ok(),
            "u16" => u16::try_from(bytes).is_ok(),
            "u32" => u32::try_from(bytes).is_ok(),
            "u64" => true,
            _ => false,
        };
        if !fits || !(elem.is_signed() || elem.is_unsigned()) {
            return Err(self.unsupported(
                node,
                format!("SIZEOF of {bytes} bytes as a {}", elem.name()),
            ));
        }
        Ok(format!("{bytes}{}", elem.rust()))
    }

    /// The reference value of `node` (IEC §6.4.4.10): `NULL`, `REF(variable)`, or a variable or a
    /// dereference that holds a reference.
    pub(super) fn reference(&mut self, node: &SyntaxNode) -> Result<String, NativeError> {
        self.reference_to(node, None)
    }

    /// The reference value of `node` for a reference to `target`: `REF` of an instance of a type
    /// derived from `target` takes the instance's part of type `target`, which lies first in it
    /// (IEC §6.6.8.4).
    pub(super) fn reference_to(
        &mut self,
        node: &SyntaxNode,
        target: Option<Ty>,
    ) -> Result<String, NativeError> {
        match node.kind() {
            SyntaxKind::ParenExpr => match node.children().next() {
                Some(inner) => self.reference_to(&inner, target),
                None => Err(self.unsupported(node, "an empty parenthesis")),
            },
            SyntaxKind::Literal if is_null(node) => Ok("abi::RefValue::NULL_REF".into()),
            SyntaxKind::CallExpr if reference_call(node).is_some() => {
                // `REF` gives a reference, `ADR` a pointer, to the variable (specification 02 §4.2).
                let kind = reference_call(node).unwrap_or("VARIABLE");
                let Some(variable) = node
                    .children()
                    .find(|child| child.kind() == SyntaxKind::ArgList)
                    .and_then(|list| list.children().find(|arg| arg.kind() == SyntaxKind::Arg))
                    .and_then(|arg| arg.children().last())
                else {
                    return Err(self.unsupported(node, "REF or ADR without its variable"));
                };
                self.refer(node, &variable, target, kind)
            }
            // `ADR(variable)` with `ADR` as its keyword: a pointer to the variable.
            SyntaxKind::AddrExpr => {
                let Some(variable) = node
                    .children()
                    .find(|child| super::is_expression(child.kind()))
                else {
                    return Err(self.unsupported(node, "ADR without its variable"));
                };
                self.refer(node, &variable, target, "POINTER")
            }
            SyntaxKind::NameRef
            | SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::DerefExpr => {
                let place = self.place(node)?;
                if !matches!(self.info(place.ty).kind, Kind::Ref { .. } | Kind::Interface) {
                    return Err(self.unsupported(node, "a value that is no reference"));
                }
                Ok(format!("{}.get()", place.expr))
            }
            other => Err(self.unsupported(node, format!("the reference expression {other:?}"))),
        }
    }

    /// A reference of kind `kind`, `VARIABLE` or `POINTER`, to `variable`, as a reference to
    /// `target` when given (specification 02 §4.2).
    fn refer(
        &mut self,
        node: &SyntaxNode,
        variable: &SyntaxNode,
        target: Option<Ty>,
        kind: &str,
    ) -> Result<String, NativeError> {
        let place = match self.object_place(variable) {
            Some(place) => place?,
            None => self.place(variable)?,
        };
        // A reference to a function block or class instance names the instance, so that its
        // class can be asked (§6.4).
        let kind = if kind == "VARIABLE" && self.info(place.ty).kind.is_instance() {
            "INSTANCE"
        } else {
            kind
        };
        let variable = self.as_type(node, &place, target.unwrap_or(place.ty))?;
        Ok(self.bind(&format!(
            "w.areas.refer(cx, {variable}, abi::RefValue::{kind})?"
        )))
    }

    /// Whether `node` is a reference: `NULL` or a value of a reference or an interface type.
    pub(super) fn is_reference(&mut self, node: &SyntaxNode) -> bool {
        if is_null(node) {
            return true;
        }
        self.type_of(node)
            .is_ok_and(|ty| matches!(self.info(ty).kind, Kind::Ref { .. } | Kind::Interface))
    }

    /// Whether `node` is a value of an interface type.
    fn is_interface(&mut self, node: &SyntaxNode) -> bool {
        self.type_of(node)
            .is_ok_and(|ty| matches!(self.info(ty).kind, Kind::Interface))
    }

    /// The value of an enumeration's value named by `node`, when trust-hir resolved it to one.
    fn enum_value(&self, node: &SyntaxNode, ty: Ty) -> Option<String> {
        let symbol = self.src.symbol_of(node)?;
        let symbol = self.src.analysis.symbols.get(symbol)?;
        if !matches!(symbol.kind, trust_hir::SymbolKind::EnumValue { .. }) {
            return None;
        }
        let Kind::Enum { base, values, .. } = &self.info(ty).kind else {
            return None;
        };
        let value = values
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&symbol.name))?
            .1;
        Some(format!("{value}{}", base.rust()))
    }

    /// The integer value of a constant `node` (a CASE label): a literal, an enumeration's value, or
    /// a negated literal, in the range of every integer type (`LINT` to `ULINT`).
    pub(super) fn constant_integer(
        &mut self,
        node: &SyntaxNode,
        ty: Ty,
    ) -> Result<i128, NativeError> {
        if let Some(value) = self.enum_value(node, ty) {
            let digits: String = value
                .chars()
                .take_while(|c| *c == '-' || c.is_ascii_digit())
                .collect();
            return digits
                .parse()
                .map_err(|_| self.unsupported(node, "an enumeration value"));
        }
        match node.kind() {
            SyntaxKind::Literal => {
                let elem = self.info(ty).scalar().unwrap_or(Elem::DInt);
                match self.literal_value(node, ty)? {
                    Literal::Int(value) => Ok(value),
                    Literal::Enum(value) => Ok(i128::from(value)),
                    _ => Err(self.unsupported(node, format!("a CASE label of {}", elem.name()))),
                }
            }
            SyntaxKind::UnaryExpr => {
                let negated = node
                    .children_with_tokens()
                    .any(|element| element.kind() == SyntaxKind::Minus);
                let Some(inner) = node.children().next() else {
                    return Err(self.unsupported(node, "an empty negation"));
                };
                let value = self.constant_integer(&inner, ty)?;
                Ok(if negated { -value } else { value })
            }
            _ => Err(self.unsupported(node, "a CASE label that is not a constant")),
        }
    }

    /// Converts `value` of type `from` to type `to` as IEC's implicit conversions do (Table 11):
    /// to a wider type of the same family, an integer to a real, a subrange's base to the subrange
    /// with its bounds checked (§6.5).
    pub(super) fn convert(
        &mut self,
        node: &SyntaxNode,
        value: &str,
        from: Ty,
        to: Ty,
    ) -> Result<String, NativeError> {
        if from == to {
            return Ok(value.to_string());
        }
        let (Some(source), Some(target)) = (self.info(from).scalar(), self.info(to).scalar())
        else {
            return Err(self.unsupported(node, "a conversion between types that are not scalars"));
        };
        let converted = if source == target {
            value.to_string()
        } else if source.is_integer() && target.is_integer() {
            if source.is_signed() == target.is_signed() && source.size() <= target.size() {
                format!("{}::from({value})", target.rust())
            } else if source.is_signed() {
                self.bind(&format!(
                    "lib::sint_to::<{}>(cx, i64::from({value}))?",
                    target.rust()
                ))
            } else {
                self.bind(&format!(
                    "lib::uint_to::<{}>(cx, u64::from({value}))?",
                    target.rust()
                ))
            }
        } else if source.is_integer() && target.is_real() {
            let (function, wide) = if source.is_signed() {
                ("sint", "i64")
            } else {
                ("uint", "u64")
            };
            let bits = if target == Elem::Real { 32 } else { 64 };
            format!("lib::{function}_to_real{bits}({wide}::from({value}))")
        } else if source == Elem::Real && target == Elem::LReal {
            format!("f64::from({value})")
        } else if (source.is_bits() && target.is_bits()) && source.size() <= target.size() {
            format!("{}::from({value})", target.rust())
        } else if source.is_duration() && target.is_duration() {
            value.to_string()
        } else {
            return Err(self.unsupported(
                node,
                format!(
                    "the implicit conversion of {} to {}",
                    source.name(),
                    target.name()
                ),
            ));
        };
        if let Kind::Subrange { lower, upper, .. } = self.info(to).kind {
            return Ok(self.bind(&format!(
                "lib::subrange(cx, {converted}, {lower}, {upper})?"
            )));
        }
        Ok(converted)
    }

    /// The type of an operand: a literal's own when it has a prefix or is a real, else the type
    /// trust-hir gave it.
    pub(super) fn operand_type(&mut self, node: &SyntaxNode) -> Result<Ty, NativeError> {
        self.type_of(node)
    }

    /// The common type two operands of a comparison or a selection take: the wider.
    pub(super) fn common(&self, node: &SyntaxNode, a: Ty, b: Ty) -> Result<Ty, NativeError> {
        if a == b {
            return Ok(a);
        }
        let (Some(left), Some(right)) = (self.info(a).scalar(), self.info(b).scalar()) else {
            return Err(self.unsupported(node, "a comparison of values that are not scalars"));
        };
        if left == right {
            return Ok(a);
        }
        match (rank(left), rank(right)) {
            (Some((fa, sa)), Some((fb, sb))) if fa == fb => Ok(if sa >= sb { a } else { b }),
            (Some((fa, _)), Some((2, _))) if fa <= 1 => Ok(b),
            (Some((2, _)), Some((fb, _))) if fb <= 1 => Ok(a),
            _ => Err(self.unsupported(
                node,
                format!("a comparison of {} with {}", left.name(), right.name()),
            )),
        }
    }

    fn binary(&mut self, node: &SyntaxNode, ty: Ty) -> Result<String, NativeError> {
        let mut children = node.children();
        let (Some(left), Some(right)) = (children.next(), children.next()) else {
            return Err(self.unsupported(node, "an operator without two operands"));
        };
        let Some(op) = super::effects::operator(node) else {
            return Err(self.unsupported(node, "an operator without its sign"));
        };
        use SyntaxKind as K;
        match op {
            K::Eq | K::Neq if self.is_interface(&left) || self.is_interface(&right) => {
                // Two interfaces are equal when they name the same instance, or are both `NULL`.
                let a = self.interface_value(&left)?;
                let b = self.interface_value(&right)?;
                let sign = if op == K::Eq { "" } else { "!" };
                Ok(format!("({sign}same_target({a}, {b}))"))
            }
            K::Eq | K::Neq if self.is_reference(&left) || self.is_reference(&right) => {
                // Two references are equal when they name the same variable, or are both `NULL`,
                // whatever type each was taken as.
                let a = self.reference(&left)?;
                let b = self.reference(&right)?;
                let sign = if op == K::Eq { "" } else { "!" };
                Ok(format!("({sign}same_target({a}, {b}))"))
            }
            K::Eq | K::Neq | K::Lt | K::LtEq | K::Gt | K::GtEq => {
                let a = self.type_of(&left)?;
                let b = self.type_of(&right)?;
                if matches!(self.info(a).kind, Kind::Str(_) | Kind::WStr(_))
                    || matches!(self.info(b).kind, Kind::Str(_) | Kind::WStr(_))
                {
                    return self.compare_texts(node, &left, &right, op);
                }
                let common = self.common(node, a, b)?;
                let a = self.scalar_to(&left, common)?;
                let b = self.scalar_to(&right, common)?;
                let sign = match op {
                    K::Eq => "==",
                    K::Neq => "!=",
                    K::Lt => "<",
                    K::LtEq => "<=",
                    K::Gt => ">",
                    _ => ">=",
                };
                Ok(format!("({a} {sign} {b})"))
            }
            K::KwAnd | K::Ampersand | K::KwOr if self.info(ty).scalar() == Some(Elem::Bool) => {
                // `AND`, `&` and `OR` of BOOL stop once the left operand decides: the right one
                // then runs nothing, no call, fault, read or write (specification 05, rule 4).
                let and = op != K::KwOr;
                let a = self.scalar_to(&left, ty)?;
                let mark = self.out.len();
                self.indent += 1;
                let b = self.scalar_to(&right, ty);
                self.indent -= 1;
                let b = b?;
                let body = self.out.split_off(mark);
                if body.is_empty() {
                    let sign = if and { "&&" } else { "||" };
                    return Ok(format!("({a} {sign} {b})"));
                }
                let result = self.temp();
                self.line(&format!("let {result}: bool;"));
                self.line(&format!("if {}({a}) {{", if and { "" } else { "!" }));
                self.out.push_str(&body);
                self.indent += 1;
                self.line(&format!("{result} = {b};"));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                self.line(&format!("{result} = {};", !and));
                self.indent -= 1;
                self.line("}");
                Ok(result)
            }
            K::KwAnd | K::Ampersand | K::KwOr | K::KwXor => {
                let a = self.scalar_to(&left, ty)?;
                let b = self.scalar_to(&right, ty)?;
                let sign = match op {
                    K::KwAnd | K::Ampersand => "&",
                    K::KwOr => "|",
                    _ => "^",
                };
                Ok(format!("({a} {sign} {b})"))
            }
            K::Plus | K::Minus | K::Star | K::Slash | K::KwMod | K::Power => {
                if let Some(value) = self.date_time_operator(node, &left, &right, op)? {
                    return Ok(value);
                }
                let Some(elem) = self.info(ty).scalar() else {
                    return Err(self.unsupported(node, "arithmetic on a value that is no scalar"));
                };
                let left_ty = self.type_of(&left)?;
                let right_ty = self.type_of(&right)?;
                let (Some(left_elem), Some(right_elem)) =
                    (self.info(left_ty).scalar(), self.info(right_ty).scalar())
                else {
                    return Err(self.unsupported(node, "arithmetic on a value that is no scalar"));
                };
                if elem.is_duration() && !(left_elem.is_duration() && right_elem.is_duration()) {
                    return Err(self.unsupported(node, "scaling a duration by a number"));
                }
                let a = self.scalar_to(&left, ty)?;
                let b = self.scalar_to(&right, ty)?;
                if elem.is_real()
                    && matches!(op, K::Plus | K::Minus | K::Star)
                    && self.inside_pure_chain(node)
                {
                    // The chain's last operation checks the result (§6.5).
                    let sign = match op {
                        K::Plus => "+",
                        K::Minus => "-",
                        _ => "*",
                    };
                    return Ok(format!("({a} {sign} {b})"));
                }
                let function = match (op, elem.is_real()) {
                    (K::Plus, false) => "add",
                    (K::Minus, false) => "sub",
                    (K::Star, false) => "mul",
                    (K::Slash, false) => "div",
                    (K::KwMod, false) => "rem",
                    (K::Power, false) => "pow",
                    (K::Plus, true) => "fadd",
                    (K::Minus, true) => "fsub",
                    (K::Star, true) => "fmul",
                    (K::Slash, true) => "fdiv",
                    (K::Power, true) => "fpow",
                    _ => return Err(self.unsupported(node, "MOD of reals")),
                };
                Ok(self.bind(&format!("lib::{function}(cx, {a}, {b})?")))
            }
            other => Err(self.unsupported(node, format!("the operator {other:?}"))),
        }
    }

    fn unary(&mut self, node: &SyntaxNode, ty: Ty) -> Result<String, NativeError> {
        let Some(operand) = node.children().next() else {
            return Err(self.unsupported(node, "an operator without its operand"));
        };
        let op = super::effects::operator(node);
        let value = self.scalar_to(&operand, ty)?;
        let Some(elem) = self.info(ty).scalar() else {
            return Err(self.unsupported(node, "an operator on a value that is no scalar"));
        };
        match op {
            Some(SyntaxKind::KwNot) => Ok(format!("(!{value})")),
            Some(SyntaxKind::Minus) if elem.is_real() => Ok(format!("(-{value})")),
            Some(SyntaxKind::Minus) if elem.is_signed() || elem.is_duration() => {
                Ok(self.bind(&format!("lib::neg(cx, {value})?")))
            }
            Some(SyntaxKind::Plus) => Ok(value),
            _ => Err(self.unsupported(node, "this unary operator")),
        }
    }

    /// A literal of type `ty` as a Rust value.
    fn literal(&mut self, node: &SyntaxNode, ty: Ty) -> Result<String, NativeError> {
        let Some(elem) = self.info(ty).scalar() else {
            return Err(self.unsupported(node, "a literal that is no scalar"));
        };
        // A named value (`Signal#High`) where its type's base goes, an integer, is the number its
        // own type gives the name (IEC §6.4.4.3).
        let named = node
            .descendants_with_tokens()
            .any(|element| element.kind() == SyntaxKind::Ident);
        let own = if named && !matches!(self.info(ty).kind, Kind::Enum { .. }) {
            self.type_of(node)
                .ok()
                .filter(|own| matches!(self.info(*own).kind, Kind::Enum { .. }))
                .unwrap_or(ty)
        } else {
            ty
        };
        let value = self.literal_value(node, own)?;
        let rust = elem.rust();
        Ok(match value {
            Literal::Bool(value) if elem == Elem::Bool => value.to_string(),
            Literal::Int(value) if elem == Elem::Bool => (value != 0).to_string(),
            // An integer literal in a real context: the nearest real, as the conversion at run
            // time rounds it (IEC Table 11).
            Literal::Int(value) if elem == Elem::Real => {
                #[allow(clippy::cast_precision_loss)]
                let rounded = value as f32;
                float32(rounded)
            }
            Literal::Int(value) if elem == Elem::LReal => {
                #[allow(clippy::cast_precision_loss)]
                let rounded = value as f64;
                float64(rounded)
            }
            Literal::Int(value) => format!("{value}{rust}"),
            Literal::Enum(value) => format!("{value}{rust}"),
            Literal::Real32(value) if elem == Elem::Real => float32(value),
            Literal::Real64(value) if elem == Elem::Real => {
                // An untyped real literal in a REAL context is rounded through binary64, as the
                // runtime rounds it.
                #[allow(clippy::cast_possible_truncation)]
                let narrowed = value as f32;
                float32(narrowed)
            }
            Literal::Real64(value) if elem == Elem::LReal => float64(value),
            Literal::Real32(value) if elem == Elem::LReal => float64(f64::from(value)),
            Literal::Nanos(value) if elem.is_duration() => format!("{value}i64"),
            Literal::DateTime(kind, value) if kind == elem => format!("{value}i64"),
            // A character literal: one character, U+0000 to U+00FF for a `CHAR` and to U+FFFF
            // for a `WCHAR` (IEC §6.3.3, Table 6).
            Literal::Text(bytes) if matches!(elem, Elem::Char | Elem::WChar) => {
                let mut characters = core::str::from_utf8(&bytes)
                    .map_err(|_| self.unsupported(node, "a character literal of invalid UTF-8"))?
                    .chars();
                let (Some(character), None) = (characters.next(), characters.next()) else {
                    return Err(
                        self.unsupported(node, "a character literal of other than one character")
                    );
                };
                let code = u32::from(character);
                let limit = if elem == Elem::Char { 0xFF } else { 0xFFFF };
                if code > limit {
                    return Err(self.unsupported(
                        node,
                        format!("the character {character:?} as a {}", elem.name()),
                    ));
                }
                format!("{code}{rust}")
            }
            // A `WCHAR` written as a one-character `WSTRING` literal: one code unit.
            Literal::WText(units) if elem == Elem::WChar => {
                let [unit] = units.as_slice() else {
                    return Err(
                        self.unsupported(node, "a character literal of other than one character")
                    );
                };
                format!("{unit}{rust}")
            }
            _ => {
                return Err(self.unsupported(node, format!("a literal of type {}", elem.name())));
            }
        })
    }

    /// The value a literal's tokens spell, for type `ty` (its enumeration values).
    pub(super) fn literal_value(&self, node: &SyntaxNode, ty: Ty) -> Result<Literal, NativeError> {
        let mut negative = false;
        let mut typed = None;
        let mut result = None;
        for token in node
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
        {
            let text = token.text();
            let error =
                |error: crate::harness::CompileError| NativeError::Compile(error.to_string());
            match token.kind() {
                SyntaxKind::Minus => negative = true,
                SyntaxKind::TypedLiteralPrefix => {
                    typed = Some(text.trim_end_matches('#').to_ascii_uppercase());
                }
                SyntaxKind::KwTrue => result = Some(Literal::Bool(true)),
                SyntaxKind::KwFalse => result = Some(Literal::Bool(false)),
                SyntaxKind::IntLiteral => {
                    let magnitude = literal_text::int(text).map_err(error)?;
                    result = Some(Literal::Int(i128::from(magnitude)));
                }
                SyntaxKind::RealLiteral => {
                    result = Some(if typed.as_deref() == Some("REAL") {
                        Literal::Real32(literal_text::real32(text).map_err(error)?)
                    } else {
                        Literal::Real64(literal_text::real(text).map_err(error)?)
                    });
                }
                SyntaxKind::TimeLiteral => match literal_text::time(text).map_err(error)? {
                    Value::Time(duration) | Value::LTime(duration) => {
                        result = Some(Literal::Nanos(duration.as_nanos()));
                    }
                    _ => return Err(self.unsupported(node, "this duration literal")),
                },
                SyntaxKind::DateLiteral => {
                    result = date_time(literal_text::date(text).map_err(error)?);
                }
                SyntaxKind::TimeOfDayLiteral => {
                    result = date_time(literal_text::time_of_day(text).map_err(error)?);
                }
                SyntaxKind::DateAndTimeLiteral => {
                    result = date_time(literal_text::date_and_time(text).map_err(error)?);
                }
                SyntaxKind::StringLiteral => {
                    result = Some(Literal::Text(
                        literal_text::string(text, false)
                            .map_err(error)?
                            .into_bytes(),
                    ));
                }
                SyntaxKind::WideStringLiteral => {
                    result = Some(Literal::WText(
                        literal_text::string(text, true)
                            .map_err(error)?
                            .encode_utf16()
                            .collect(),
                    ));
                }
                SyntaxKind::Ident => {
                    let Kind::Enum { values, .. } = &self.info(ty).kind else {
                        return Err(self.unsupported(
                            node,
                            format!(
                                "the named literal {text} for {}, which is no enumeration",
                                self.info(ty).name
                            ),
                        ));
                    };
                    let Some((_, value)) = values
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(text))
                    else {
                        return Err(self.unsupported(
                            node,
                            format!("the enumeration value {text} of {}", self.info(ty).name),
                        ));
                    };
                    result = Some(Literal::Enum(*value));
                }
                _ => {}
            }
        }
        let Some(value) = result else {
            return Err(self.unsupported(node, "this literal"));
        };
        Ok(if negative {
            match value {
                Literal::Int(value) => Literal::Int(-value),
                Literal::Real32(value) => Literal::Real32(-value),
                Literal::Real64(value) => Literal::Real64(-value),
                Literal::Nanos(value) => Literal::Nanos(-value),
                other => other,
            }
        } else {
            value
        })
    }

    /// The value of `node` as an aggregate of type `ty`: a place, or a function's result.
    pub(super) fn aggregate(&mut self, node: &SyntaxNode, ty: Ty) -> Result<Place, NativeError> {
        match node.kind() {
            SyntaxKind::ParenExpr => match node.children().next() {
                Some(inner) => self.aggregate(&inner, ty),
                None => Err(self.unsupported(node, "an empty parenthesis")),
            },
            SyntaxKind::NameRef
            | SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::DerefExpr => {
                let place = self.place(node)?;
                if place.ty != ty {
                    return Err(self.unsupported(node, "an assignment between different types"));
                }
                Ok(place)
            }
            SyntaxKind::CallExpr => self.call_aggregate(node, ty),
            other => Err(self.unsupported(
                node,
                format!("the value {other:?} of a structure or an array"),
            )),
        }
    }

    /// Evaluates `node`, a call, for what it does, discarding its value.
    pub(super) fn discard(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        if node.kind() != SyntaxKind::CallExpr {
            return Err(self.unsupported(node, "an expression statement that is no call"));
        }
        self.call_statement(node)
    }
}

/// Whether `node` is the literal `NULL`.
pub(super) fn is_null(node: &SyntaxNode) -> bool {
    node.kind() == SyntaxKind::Literal
        && node
            .children_with_tokens()
            .any(|element| element.kind() == SyntaxKind::KwNull)
}

/// The kind of reference the call `node` takes of its variable: `VARIABLE` for `REF(variable)`,
/// `POINTER` for `ADR(variable)`; `None` for another call.
pub(super) fn reference_call(node: &SyntaxNode) -> Option<&'static str> {
    let callee = node
        .children()
        .next()
        .filter(|callee| callee.kind() == SyntaxKind::NameRef)?;
    let name = super::call::ident(&callee);
    if name.eq_ignore_ascii_case("REF") {
        Some("VARIABLE")
    } else if name.eq_ignore_ascii_case("ADR") {
        Some("POINTER")
    } else {
        None
    }
}

/// A literal's value.
pub(super) enum Literal {
    Bool(bool),
    /// An integer: the range of `LINT` and of `ULINT` together, so that `LWORD#16#FFFF_FFFF_FFFF_FFFF`
    /// keeps its value (IEC §6.3.2, Table 5).
    Int(i128),
    Real32(f32),
    Real64(f64),
    Nanos(i64),
    /// A date, a time of day or a date and time: its type and its value.
    DateTime(Elem, i64),
    Text(Vec<u8>),
    /// The UTF-16 code units of a `WSTRING` literal.
    WText(Vec<u16>),
    Enum(i64),
}

/// The literal of a date and time value: milliseconds since 1970-01-01 for a `DATE`, a
/// `TIME_OF_DAY` and a `DATE_AND_TIME`, nanoseconds for their long forms.
fn date_time(value: Value) -> Option<Literal> {
    Some(match value {
        Value::Date(value) => Literal::DateTime(Elem::Date, value.ticks()),
        Value::Tod(value) => Literal::DateTime(Elem::Tod, value.ticks()),
        Value::Dt(value) => Literal::DateTime(Elem::Dt, value.ticks()),
        Value::LDate(value) => Literal::DateTime(Elem::LDate, value.nanos()),
        Value::LTod(value) => Literal::DateTime(Elem::LTod, value.nanos()),
        Value::Ldt(value) => Literal::DateTime(Elem::Ldt, value.nanos()),
        _ => return None,
    })
}

/// Whether `node` is a literal with a sign, `-273.15` or `+5`: a unary minus or plus of a literal,
/// whose tokens `literal_value` reads with the sign.
/// Whether `node` is arithmetic of untyped numeric literals alone, through parentheses, signs and
/// `+ - * / MOD`: integer literals, and real ones too when `reals` (specification 05 §5.2.1).
fn untyped_arithmetic(node: &SyntaxNode, reals: bool) -> bool {
    let tokens = || {
        node.descendants_with_tokens()
            .filter_map(|element| element.into_token())
    };
    match node.kind() {
        SyntaxKind::Literal => {
            tokens().all(|token| token.kind() != SyntaxKind::TypedLiteralPrefix)
                && tokens().any(|token| {
                    token.kind() == SyntaxKind::IntLiteral
                        || (reals && token.kind() == SyntaxKind::RealLiteral)
                })
                && tokens().all(|token| {
                    token.kind().is_trivia()
                        || matches!(
                            token.kind(),
                            SyntaxKind::IntLiteral | SyntaxKind::RealLiteral | SyntaxKind::Minus
                        )
                })
        }
        SyntaxKind::ParenExpr => node
            .children()
            .next()
            .is_some_and(|inner| untyped_arithmetic(&inner, reals)),
        SyntaxKind::UnaryExpr => {
            matches!(
                super::effects::operator(node),
                Some(SyntaxKind::Minus | SyntaxKind::Plus)
            ) && node
                .children()
                .next()
                .is_some_and(|operand| untyped_arithmetic(&operand, reals))
        }
        SyntaxKind::BinaryExpr => {
            matches!(
                super::effects::operator(node),
                Some(
                    SyntaxKind::Plus
                        | SyntaxKind::Minus
                        | SyntaxKind::Star
                        | SyntaxKind::Slash
                        | SyntaxKind::KwMod
                )
            ) && node.children().count() == 2
                && node
                    .children()
                    .all(|operand| untyped_arithmetic(&operand, reals))
        }
        _ => false,
    }
}

pub(super) fn signed_literal(node: &SyntaxNode) -> bool {
    node.kind() == SyntaxKind::UnaryExpr
        && node
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| !token.kind().is_trivia())
            .is_some_and(|token| matches!(token.kind(), SyntaxKind::Minus | SyntaxKind::Plus))
        && node
            .children()
            .next()
            .is_some_and(|operand| operand.kind() == SyntaxKind::Literal)
}

/// A binary32 value as an exact Rust literal: its bits.
fn float32(value: f32) -> String {
    format!("f32::from_bits({:#010x})", value.to_bits())
}

/// A binary64 value as an exact Rust literal: its bits.
fn float64(value: f64) -> String {
    format!("f64::from_bits({:#018x})", value.to_bits())
}
