//! Literals (IEC §6.3, Tables 5 to 9; specification 05): the value a literal's tokens spell, and
//! that value as a Rust literal of the type its context wants.

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::Lowerer;
use crate::harness::literal_text;
use crate::native::types::{Elem, Kind, Ty};
use crate::native::NativeError;
use crate::value::Value;

impl Lowerer<'_, '_> {
    /// A literal of type `ty` as a Rust value.
    pub(super) fn literal(&mut self, node: &SyntaxNode, ty: Ty) -> Result<String, NativeError> {
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
