//! The initial value of a variable, `#[init(...)]` (specification 48 RU-26).

use syn::spanned::Spanned;
use syn::{Error, Expr, ExprLit, ExprUnary, Lit, LitInt, UnOp};

use crate::Elementary;

/// A variable's initial value, checked against its type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    /// A `BOOL`'s.
    Bool(bool),
    /// An integer's, inside its type's range.
    Integer(i128),
    /// A `REAL`'s: the `f32` the literal rounds to.
    Real(f32),
    /// An `LREAL`'s: the `f64` the literal rounds to.
    Lreal(f64),
}

impl Value {
    /// The value as a Structured Text literal (IEC §6.3.2, Table 5).
    #[must_use]
    pub fn structured_text(self) -> String {
        match self {
            Self::Bool(value) => if value { "TRUE" } else { "FALSE" }.into(),
            Self::Integer(value) => value.to_string(),
            Self::Real(value) => real_literal(&format!("{value:?}")),
            Self::Lreal(value) => real_literal(&format!("{value:?}")),
        }
    }
}

/// Rust's shortest form of a float (`0.1`, `1e300`, `-2.5e-8`) as an IEC real literal, which has a
/// decimal point before its exponent (IEC §6.3.2, Table 5): `1.0E300`.
fn real_literal(shortest: &str) -> String {
    match shortest.split_once('e') {
        Some((mantissa, exponent)) if mantissa.contains('.') => format!("{mantissa}E{exponent}"),
        Some((mantissa, exponent)) => format!("{mantissa}.0E{exponent}"),
        None => shortest.to_string(),
    }
}

/// The initial value `value` gives a variable of type `ty`.
pub(crate) fn initial(value: &Expr, ty: Elementary) -> Result<Value, Error> {
    let refused = || {
        let example = match ty {
            Elementary::Bool => "true",
            Elementary::Real | Elementary::Lreal => "0.5",
            _ => "10",
        };
        Error::new(
            value.span(),
            format!(
                "the initial value of a {} variable is a literal of its type, `#[init({example})]` \
                 (specification 48 RU-26)",
                ty.iec()
            ),
        )
    };
    let (negative, literal) = match value {
        Expr::Lit(ExprLit { lit, .. }) => (false, lit),
        Expr::Unary(ExprUnary {
            op: UnOp::Neg(_),
            expr,
            ..
        }) => match &**expr {
            Expr::Lit(ExprLit { lit, .. }) => (true, lit),
            _ => return Err(refused()),
        },
        _ => return Err(refused()),
    };
    match (ty, literal) {
        (Elementary::Bool, Lit::Bool(value)) if !negative => Ok(Value::Bool(value.value)),
        (Elementary::Real | Elementary::Lreal, Lit::Float(_) | Lit::Int(_)) => {
            real(negative, literal, ty)
        }
        (_, Lit::Int(literal)) => match ty.range() {
            Some(range) => integer(negative, literal, ty, range),
            None => Err(refused()),
        },
        _ => Err(refused()),
    }
}

fn integer(
    negative: bool,
    literal: &LitInt,
    ty: Elementary,
    (min, max): (i128, i128),
) -> Result<Value, Error> {
    suffix(literal.suffix(), ty, literal.span())?;
    let magnitude: i128 = literal.base10_parse()?;
    let value = if negative { -magnitude } else { magnitude };
    if (min..=max).contains(&value) {
        Ok(Value::Integer(value))
    } else {
        Err(Error::new(
            literal.span(),
            format!("{value} is outside {} ({min} to {max})", ty.iec()),
        ))
    }
}

fn real(negative: bool, literal: &Lit, ty: Elementary) -> Result<Value, Error> {
    let (digits, written) = match literal {
        Lit::Float(float) => (float.base10_digits(), float.suffix()),
        Lit::Int(integer) => (integer.base10_digits(), integer.suffix()),
        other => {
            return Err(Error::new(
                other.span(),
                "a real's initial value is a number",
            ))
        }
    };
    suffix(written, ty, literal.span())?;
    let text = if negative {
        format!("-{digits}")
    } else {
        digits.to_string()
    };
    let outside = || Error::new(literal.span(), format!("{text} is outside {}", ty.iec()));
    if ty == Elementary::Real {
        let value: f32 = text.parse().map_err(|_| outside())?;
        if value.is_finite() {
            Ok(Value::Real(value))
        } else {
            Err(outside())
        }
    } else {
        let value: f64 = text.parse().map_err(|_| outside())?;
        if value.is_finite() {
            Ok(Value::Lreal(value))
        } else {
            Err(outside())
        }
    }
}

/// A literal's suffix is none or the variable's own type: `5u8` for a `u8`.
fn suffix(written: &str, ty: Elementary, span: proc_macro2::Span) -> Result<(), Error> {
    if written.is_empty() || written == ty.rust() {
        Ok(())
    } else {
        Err(Error::new(
            span,
            format!(
                "`{written}` is not the variable's type, `{}`; the literal needs no suffix",
                ty.rust()
            ),
        ))
    }
}
