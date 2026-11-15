//! The initial value of a variable, `#[init(...)]` (specification 48 RU-26).

use std::fmt::Write as _;

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
    /// A bit string's: its number of bits and its value within them.
    Bits {
        /// The bit string's bits: 8, 16, 32 or 64.
        bits: u32,
        /// The value.
        value: u64,
    },
    /// A `CHAR`'s, one byte, or a `WCHAR`'s (`wide`), one UTF-16 code unit.
    Char {
        /// Whether it is a `WCHAR`'s.
        wide: bool,
        /// The byte or the code unit.
        code: u32,
    },
    /// A `TIME`'s or an `LTIME`'s (`long`) duration in nanoseconds.
    Duration {
        /// Whether it is an `LTIME`'s.
        long: bool,
        /// The nanoseconds.
        nanos: i64,
    },
    /// A `DATE`'s, `TIME_OF_DAY`'s or `DATE_AND_TIME`'s value, or a long form's, as the program's
    /// areas hold it: milliseconds since 1970-01-01, a time of day's since midnight, and
    /// nanoseconds for a long form.
    Instant {
        /// The date type.
        ty: Elementary,
        /// The value.
        value: i64,
    },
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
            Self::Bits { bits, value } => {
                format!("16#{value:0width$X}", width = (bits / 4) as usize)
            }
            Self::Char { wide, code } => character_literal(wide, code),
            Self::Duration { long, nanos } => duration_literal(long, nanos),
            Self::Instant { ty, value } => crate::dates::literal(ty, value),
        }
    }
}

/// A character as an IEC character literal (IEC §6.3.3, Table 6): a printable ASCII character as
/// itself, any other byte or code unit as `$` and its hexadecimal digits.
fn character_literal(wide: bool, code: u32) -> String {
    let (quote, digits) = if wide { ('"', 4) } else { ('\'', 2) };
    match char::from_u32(code) {
        Some(character)
            if (' '..='~').contains(&character) && character != quote && character != '$' =>
        {
            format!("{quote}{character}{quote}")
        }
        _ => format!("{quote}${code:0digits$X}{quote}"),
    }
}

/// A duration as an IEC duration literal (IEC §6.3.4, Table 8), its units from days to
/// nanoseconds, each that is not zero: `T#1s500ms`, `LTIME#-2h`, `T#0s`.
fn duration_literal(long: bool, nanos: i64) -> String {
    let prefix = if long { "LTIME#" } else { "T#" };
    let sign = if nanos < 0 { "-" } else { "" };
    let mut rest = nanos.unsigned_abs();
    let mut text = String::new();
    for (unit, scale) in DURATION_UNITS {
        let count = rest / scale;
        if count > 0 {
            let _ = write!(text, "{count}{}", unit.to_ascii_lowercase());
            rest %= scale;
        }
    }
    if text.is_empty() {
        text.push_str("0s");
    }
    format!("{prefix}{sign}{text}")
}

/// The units of an IEC duration (IEC §6.3.4), from the largest, each with its nanoseconds.
const DURATION_UNITS: [(&str, u64); 7] = [
    ("D", 86_400_000_000_000),
    ("H", 3_600_000_000_000),
    ("M", 60_000_000_000),
    ("S", 1_000_000_000),
    ("MS", 1_000_000),
    ("US", 1_000),
    ("NS", 1),
];

/// The nanoseconds of the IEC duration literal `text`: `T#`, `TIME#`, `LT#` or `LTIME#`, an optional
/// `-`, then numbers with their units from the largest to the smallest, each once, a fraction only
/// on the last, `_` anywhere between digits (IEC §6.3.4, Table 8); `None` for another text, for a
/// fraction of a nanosecond and for a duration beyond 64 bits.
pub(crate) fn duration_nanos(text: &str) -> Option<i64> {
    let upper = text.trim().to_ascii_uppercase().replace('_', "");
    let body = ["LTIME#", "TIME#", "LT#", "T#"]
        .iter()
        .find_map(|prefix| upper.strip_prefix(prefix))?;
    let (negative, mut rest) = match body.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, body),
    };
    if rest.is_empty() {
        return None;
    }
    let mut total: i128 = 0;
    let mut last: Option<usize> = None;
    while !rest.is_empty() {
        let number_end = rest
            .find(|character: char| !(character.is_ascii_digit() || character == '.'))
            .unwrap_or(rest.len());
        let (number, after) = rest.split_at(number_end);
        let unit_end = after
            .find(|character: char| !character.is_ascii_alphabetic())
            .unwrap_or(after.len());
        let (unit, after) = after.split_at(unit_end);
        let rank = DURATION_UNITS.iter().position(|(name, _)| *name == unit)?;
        if last.is_some_and(|last| rank <= last) {
            return None;
        }
        let scale = i128::from(DURATION_UNITS[rank].1);
        let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
        let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
        if whole.is_empty() || !digits(whole) || !digits(fraction) {
            return None;
        }
        if number.contains('.') && (fraction.is_empty() || !after.is_empty()) {
            return None;
        }
        total = total.checked_add(whole.parse::<i128>().ok()?.checked_mul(scale)?)?;
        if !fraction.is_empty() {
            let denominator = 10_i128.checked_pow(u32::try_from(fraction.len()).ok()?)?;
            let numerator = fraction.parse::<i128>().ok()?.checked_mul(scale)?;
            if numerator % denominator != 0 {
                return None;
            }
            total = total.checked_add(numerator / denominator)?;
        }
        last = Some(rank);
        rest = after;
    }
    i64::try_from(if negative { -total } else { total }).ok()
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
            Elementary::Byte | Elementary::Word | Elementary::Dword | Elementary::Lword => "0x0F",
            Elementary::Char | Elementary::WChar => "'A'",
            Elementary::Time | Elementary::LTime => "\"T#500ms\"",
            Elementary::Date | Elementary::LDate => "\"D#2024-02-29\"",
            Elementary::Tod | Elementary::LTod => "\"TOD#12:30:15.25\"",
            Elementary::Dt | Elementary::Ldt => "\"DT#2024-02-29-12:30:15\"",
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
        (_, Lit::Int(literal)) if !negative && ty.bits().is_some() => bit_string(literal, ty),
        (Elementary::Char, Lit::Byte(byte)) if !negative => Ok(Value::Char {
            wide: false,
            code: u32::from(byte.value()),
        }),
        (Elementary::Char | Elementary::WChar, Lit::Char(character)) if !negative => {
            let code = u32::from(character.value());
            let (wide, limit, what) = if ty == Elementary::WChar {
                (
                    true,
                    0xFFFF,
                    "one UTF-16 code unit: a character of the basic multilingual plane",
                )
            } else {
                (
                    false,
                    0x7F,
                    "one byte of a STRING's UTF-8 text: an ASCII character, or a byte `b'\\xE9'`",
                )
            };
            if code <= limit {
                Ok(Value::Char { wide, code })
            } else {
                Err(Error::new(
                    character.span(),
                    format!("a {} is {what}", ty.iec()),
                ))
            }
        }
        (Elementary::Time | Elementary::LTime, Lit::Str(text)) if !negative => {
            duration_nanos(&text.value())
                .map(|nanos| Value::Duration {
                    long: ty == Elementary::LTime,
                    nanos,
                })
                .ok_or_else(|| {
                    Error::new(
                        text.span(),
                        format!(
                            "a {} is an IEC duration in whole nanoseconds within 64 bits: \
                             `\"T#1h30m\"`, `\"T#500ms\"`, `\"T#1.5s\"` (IEC §6.3.4)",
                            ty.iec()
                        ),
                    )
                })
        }
        (_, Lit::Str(text)) if !negative && crate::dates::is_date(ty) => {
            crate::dates::initial(text, ty)
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

fn bit_string(literal: &LitInt, ty: Elementary) -> Result<Value, Error> {
    let bits = ty.bits().unwrap_or(64);
    if !literal.suffix().is_empty() {
        return Err(Error::new(
            literal.span(),
            format!("a {} needs no suffix: `0x0F`", ty.iec()),
        ));
    }
    let value: u128 = literal.base10_parse()?;
    let largest = (1_u128 << bits) - 1;
    match u64::try_from(value) {
        Ok(value) if u128::from(value) <= largest => Ok(Value::Bits { bits, value }),
        _ => Err(Error::new(
            literal.span(),
            format!("{value} is outside {} (0 to {largest})", ty.iec()),
        )),
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
