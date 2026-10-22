//! Expressions (IEC §7.3.2; specification 05; specification 44 §6.5, §6.6): literals, names,
//! operators and their checks, the implicit conversions of an assignment or an operand, and texts.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::{is_scalar, Lowerer, Place};
use crate::harness::literal_text;
use crate::native::types::{Elem, Kind, Ty};
use crate::native::NativeError;
use crate::value::Value;

/// A part of a text: a literal's bytes or a string's value, as `CONCAT` and an assignment take it.
pub(super) enum Text {
    Bytes(Vec<u8>),
    /// A reference to a `Str<N>`.
    Str(String),
}

impl Text {
    fn piece(&self) -> String {
        match self {
            Self::Bytes(bytes) => format!("lib::Piece::Bytes({})", byte_string(bytes)),
            Self::Str(reference) => format!("lib::Piece::Str(({reference}).view())"),
        }
    }
}

/// `bytes` as a Rust byte string literal.
pub(super) fn byte_string(bytes: &[u8]) -> String {
    let mut out = String::from("b\"");
    for byte in bytes {
        match byte {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(char::from(*byte)),
            other => out.push_str(&format!("\\x{other:02x}")),
        }
    }
    out.push('"');
    out
}

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
        // A literal takes the type of where it goes, as the runtime lowers it.
        if node.kind() == SyntaxKind::Literal && is_scalar(&self.info(target).kind) {
            return self.literal(node, target);
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
            SyntaxKind::ParenExpr => match node.children().next() {
                Some(inner) => self.scalar(&inner),
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
            other => Err(self.unsupported(node, format!("the expression {other:?}"))),
        }
    }

    /// The reference value of `node` (IEC §6.4.4.10): `NULL`, `REF(variable)`, or a variable or a
    /// dereference that holds a reference.
    pub(super) fn reference(&mut self, node: &SyntaxNode) -> Result<String, NativeError> {
        match node.kind() {
            SyntaxKind::ParenExpr => match node.children().next() {
                Some(inner) => self.reference(&inner),
                None => Err(self.unsupported(node, "an empty parenthesis")),
            },
            SyntaxKind::Literal if is_null(node) => Ok("abi::RefValue::NULL_REF".into()),
            SyntaxKind::CallExpr if is_ref_call(node) => {
                let Some(target) = node
                    .children()
                    .find(|child| child.kind() == SyntaxKind::ArgList)
                    .and_then(|list| list.children().find(|arg| arg.kind() == SyntaxKind::Arg))
                    .and_then(|arg| arg.children().last())
                else {
                    return Err(self.unsupported(node, "REF without its variable"));
                };
                let place = self.place(&target)?;
                Ok(self.bind(&format!(
                    "w.areas.refer(cx, {}, abi::RefValue::VARIABLE)?",
                    place.reference()
                )))
            }
            SyntaxKind::NameRef
            | SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::DerefExpr => {
                let place = self.place(node)?;
                if !matches!(self.info(place.ty).kind, Kind::Ref { .. }) {
                    return Err(self.unsupported(node, "a value that is no reference"));
                }
                Ok(format!("{}.get()", place.expr))
            }
            other => Err(self.unsupported(node, format!("the reference expression {other:?}"))),
        }
    }

    /// Whether `node` is a reference: `NULL` or a value of a reference type.
    fn is_reference(&mut self, node: &SyntaxNode) -> bool {
        if is_null(node) {
            return true;
        }
        self.type_of(node)
            .is_ok_and(|ty| matches!(self.info(ty).kind, Kind::Ref { .. }))
    }

    /// The value of an enumeration's value named by `node`, when trust-hir resolved it to one.
    fn enum_value(&self, node: &SyntaxNode, ty: Ty) -> Option<String> {
        let symbol = self.src.symbol_of(node)?;
        let symbol = self.src.analysis.symbols.get(symbol)?;
        if !matches!(symbol.kind, trust_hir::SymbolKind::EnumValue { .. }) {
            return None;
        }
        let Kind::Enum { base, values } = &self.info(ty).kind else {
            return None;
        };
        let value = values
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(&symbol.name))?
            .1;
        Some(format!("{value}{}", base.rust()))
    }

    /// The integer value of a constant `node` (a CASE label): a literal, an enumeration's value, or
    /// a negated literal.
    pub(super) fn constant_integer(
        &mut self,
        node: &SyntaxNode,
        ty: Ty,
    ) -> Result<i64, NativeError> {
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
                    Literal::Enum(value) => Ok(value),
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
        let Some(op) = node
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| !token.kind().is_trivia())
            .map(|token| token.kind())
        else {
            return Err(self.unsupported(node, "an operator without its sign"));
        };
        use SyntaxKind as K;
        match op {
            K::Eq | K::Neq if self.is_reference(&left) || self.is_reference(&right) => {
                // Two references are equal when they name the same variable, or are both `NULL`.
                let a = self.reference(&left)?;
                let b = self.reference(&right)?;
                let sign = if op == K::Eq { "==" } else { "!=" };
                Ok(format!("({a} {sign} {b})"))
            }
            K::Eq | K::Neq | K::Lt | K::LtEq | K::Gt | K::GtEq => {
                let a = self.type_of(&left)?;
                let b = self.type_of(&right)?;
                if matches!(self.info(a).kind, Kind::Str(_))
                    || matches!(self.info(b).kind, Kind::Str(_))
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
        let op = node
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .find(|token| !token.kind().is_trivia())
            .map(|token| token.kind());
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
        let value = self.literal_value(node, ty)?;
        let rust = elem.rust();
        Ok(match value {
            Literal::Bool(value) if elem == Elem::Bool => value.to_string(),
            Literal::Int(value) if elem == Elem::Bool => (value != 0).to_string(),
            Literal::Int(value) if elem.is_real() => {
                format!("({value} as {rust})")
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
            _ => {
                return Err(self.unsupported(node, format!("a literal of type {}", elem.name())));
            }
        })
    }

    /// The value a literal's tokens spell, for type `ty` (its enumeration values).
    fn literal_value(&self, node: &SyntaxNode, ty: Ty) -> Result<Literal, NativeError> {
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
                    let value = i64::try_from(magnitude)
                        .map_err(|_| self.unsupported(node, "an integer literal above i64"))?;
                    result = Some(Literal::Int(value));
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
                SyntaxKind::StringLiteral => {
                    result = Some(Literal::Text(
                        literal_text::string(text, false)
                            .map_err(error)?
                            .into_bytes(),
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

    /// The text of `node` as parts: a literal, a string, a string function's result.
    pub(super) fn text(&mut self, node: &SyntaxNode) -> Result<Vec<Text>, NativeError> {
        match node.kind() {
            SyntaxKind::ParenExpr => match node.children().next() {
                Some(inner) => self.text(&inner),
                None => Err(self.unsupported(node, "an empty parenthesis")),
            },
            SyntaxKind::Literal => {
                let ty = self.type_of(node)?;
                match self.literal_value(node, ty)? {
                    Literal::Text(bytes) => Ok(vec![Text::Bytes(bytes)]),
                    _ => Err(self.unsupported(node, "a literal that is no text")),
                }
            }
            SyntaxKind::NameRef
            | SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::DerefExpr => {
                let place = self.place(node)?;
                Ok(vec![Text::Str(place.reference())])
            }
            SyntaxKind::CallExpr => self.call_text(node),
            other => Err(self.unsupported(node, format!("the text expression {other:?}"))),
        }
    }

    /// `place := value` for a `STRING` place: the text cut at the place's capacity.
    pub(super) fn store_text(
        &mut self,
        place: &Place,
        value: &SyntaxNode,
    ) -> Result<(), NativeError> {
        let parts = self.text(value)?;
        let target = place.reference();
        match parts.as_slice() {
            [Text::Bytes(bytes)] => {
                self.line(&format!(
                    "lib::assign_bytes(({target}).view(), {});",
                    byte_string(bytes)
                ));
            }
            [Text::Str(source)] => {
                self.line(&format!(
                    "lib::assign(({target}).view(), ({source}).view());"
                ));
            }
            _ => {
                let pieces: Vec<String> = parts.iter().map(Text::piece).collect();
                self.line(&format!(
                    "lib::concat(cx, ({target}).view(), &[{}])?;",
                    pieces.join(", ")
                ));
            }
        }
        Ok(())
    }

    /// `parts` written into a fresh temporary `STRING[capacity]`, whose reference is returned.
    pub(super) fn text_temp(&mut self, parts: &[Text], capacity: u32) -> String {
        let temp = self.temp();
        self.line(&format!("let {temp} = Str::<{capacity}>::new();"));
        let pieces: Vec<String> = parts.iter().map(Text::piece).collect();
        self.line(&format!(
            "lib::concat(cx, {temp}.view(), &[{}])?;",
            pieces.join(", ")
        ));
        format!("&{temp}")
    }

    fn compare_texts(
        &mut self,
        node: &SyntaxNode,
        left: &SyntaxNode,
        right: &SyntaxNode,
        op: SyntaxKind,
    ) -> Result<String, NativeError> {
        let operand = |lowerer: &mut Self, side: &SyntaxNode| -> Result<String, NativeError> {
            let parts = lowerer.text(side)?;
            match parts.as_slice() {
                [Text::Str(reference)] => Ok(reference.clone()),
                _ => {
                    let capacity = parts
                        .iter()
                        .map(|part| match part {
                            Text::Bytes(bytes) => u32::try_from(bytes.len()).unwrap_or(u32::MAX),
                            Text::Str(_) => 0,
                        })
                        .sum::<u32>()
                        .max(1);
                    if parts.iter().any(|part| matches!(part, Text::Str(_))) {
                        return Err(lowerer.unsupported(side, "comparing a joined text"));
                    }
                    Ok(lowerer.text_temp(&parts, capacity))
                }
            }
        };
        let a = operand(self, left)?;
        let b = operand(self, right)?;
        let ordering = match op {
            SyntaxKind::Eq => "== core::cmp::Ordering::Equal",
            SyntaxKind::Neq => "!= core::cmp::Ordering::Equal",
            SyntaxKind::Lt => "== core::cmp::Ordering::Less",
            SyntaxKind::Gt => "== core::cmp::Ordering::Greater",
            SyntaxKind::LtEq => "!= core::cmp::Ordering::Greater",
            SyntaxKind::GtEq => "!= core::cmp::Ordering::Less",
            _ => return Err(self.unsupported(node, "this comparison of texts")),
        };
        Ok(format!(
            "(lib::compare(({a}).view(), ({b}).view()) {ordering})"
        ))
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
fn is_null(node: &SyntaxNode) -> bool {
    node.kind() == SyntaxKind::Literal
        && node
            .children_with_tokens()
            .any(|element| element.kind() == SyntaxKind::KwNull)
}

/// Whether `node` is the call `REF(variable)`.
fn is_ref_call(node: &SyntaxNode) -> bool {
    node.children()
        .next()
        .filter(|callee| callee.kind() == SyntaxKind::NameRef)
        .is_some_and(|callee| super::call::ident(&callee).eq_ignore_ascii_case("REF"))
}

/// A literal's value.
pub(super) enum Literal {
    Bool(bool),
    Int(i64),
    Real32(f32),
    Real64(f64),
    Nanos(i64),
    Text(Vec<u8>),
    Enum(i64),
}

/// A binary32 value as an exact Rust literal: its bits.
fn float32(value: f32) -> String {
    format!("f32::from_bits({:#010x})", value.to_bits())
}

/// A binary64 value as an exact Rust literal: its bits.
fn float64(value: f64) -> String {
    format!("f64::from_bits({:#018x})", value.to_bits())
}
