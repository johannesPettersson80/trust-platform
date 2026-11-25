//! User-facing runtime value formatting.

use super::Value;
use alloc::{
    format,
    string::{String, ToString},
};
use core::fmt::Write as _;

/// Format a runtime value for user-visible diagnostics, tests, and debug output.
#[must_use]
pub fn format_user_value(value: &Value) -> String {
    match value {
        Value::Bool(value) => {
            if *value {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        Value::SInt(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::DInt(value) => value.to_string(),
        Value::LInt(value) => value.to_string(),
        Value::USInt(value) => value.to_string(),
        Value::UInt(value) => value.to_string(),
        Value::UDInt(value) => value.to_string(),
        Value::ULInt(value) => value.to_string(),
        Value::Real(value) => super::float_text::real(f64::from(*value)),
        Value::LReal(value) => super::float_text::real(*value),
        Value::Byte(value) => value.to_string(),
        Value::Word(value) => value.to_string(),
        Value::DWord(value) => value.to_string(),
        Value::LWord(value) => value.to_string(),
        Value::Time(value) => format_duration("T", value.as_nanos()),
        Value::LTime(value) => format_duration("LT", value.as_nanos()),
        Value::Date(value) => format!("D#{}", value.ticks()),
        Value::LDate(value) => format!("LD#{}", value.nanos()),
        Value::Tod(value) => format!("TOD#{}", value.ticks()),
        Value::LTod(value) => format!("LTOD#{}", value.nanos()),
        Value::Dt(value) => format!("DT#{}", value.ticks()),
        Value::Ldt(value) => format!("LDT#{}", value.nanos()),
        Value::String(value) => quote_st_string(value.as_str()),
        Value::WString(value) => format!("W{}", quote_st_string(value)),
        Value::Char(value) => quote_st_string(&char::from(*value).to_string()),
        Value::WChar(value) => {
            let ch = char::from_u32((*value).into()).unwrap_or('?');
            format!("W{}", quote_st_string(&ch.to_string()))
        }
        Value::Array(value) => format!("[{}]", value.elements().len()),
        Value::Struct(value) => format!("{} {{...}}", value.type_name()),
        Value::Enum(value) => format!("{}::{}", value.type_name(), value.variant_name()),
        Value::Reference(Some(_)) => "REF".to_string(),
        Value::Reference(None) => "NULL_REF".to_string(),
        Value::Instance(_) => "Instance".to_string(),
        Value::Null => "NULL".to_string(),
    }
}

/// The Structured Text literal of `value`, as IEC 61131-3 writes one (Tables 5 to 8, §6.4.4.2), for
/// every type a screen passes to a method (specification 39 §21.2): a boolean, an integer or a bit
/// string in decimal, a real with its decimal point, a duration, a text in single quotes or a
/// double-byte one in double quotes with their `$` combinations, and an enumeration's value after
/// its type's own name and `#`. `None` for a real that is not finite, which no literal spells, and
/// for the types a screen does not pass, such as a date or a structure. A cycle trace records a
/// screen's method inputs as these literals (specification 48 Appendix C, C.6).
#[must_use]
pub fn st_literal(value: &Value) -> Option<String> {
    Some(match value {
        Value::Bool(_)
        | Value::SInt(_)
        | Value::Int(_)
        | Value::DInt(_)
        | Value::LInt(_)
        | Value::USInt(_)
        | Value::UInt(_)
        | Value::UDInt(_)
        | Value::ULInt(_)
        | Value::Byte(_)
        | Value::Word(_)
        | Value::DWord(_)
        | Value::LWord(_) => format_user_value(value),
        Value::Real(real) => real_literal(f64::from(*real))?,
        Value::LReal(real) => real_literal(*real)?,
        Value::Time(duration) => format_duration("T", duration.as_nanos()),
        Value::LTime(duration) => format_duration("LT", duration.as_nanos()),
        Value::String(text) => text_literal(text, false),
        Value::WString(text) => text_literal(text, true),
        // `Enum_Value` (§6.4.4.2): the type's name is an identifier, which no namespace qualifies.
        Value::Enum(value) if !value.variant_name().is_empty() => {
            let own_name = value.type_name().rsplit('.').next().unwrap_or_default();
            format!("{own_name}#{}", value.variant_name())
        }
        _ => return None,
    })
}

/// A real literal: expanded decimal digits, always with the decimal point that tells a real from an
/// integer (`4000.0`); none for a value that is not finite.
fn real_literal(value: f64) -> Option<String> {
    value.is_finite().then(|| super::float_text::real(value))
}

/// A text literal (Table 6): a single-byte one in single quotes, a double-byte one (`wide`) in
/// double quotes, with `$$`, `$N`, `$R`, `$T`, `$P` and `$` before its own quote, and the code of
/// another control character, which a literal may not hold as it is, in two hexadecimal digits, or
/// four in a double-byte one.
fn text_literal(text: &str, wide: bool) -> String {
    let quote = if wide { '"' } else { '\'' };
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push(quote);
    for character in text.chars() {
        match character {
            '$' => literal.push_str("$$"),
            '\n' => literal.push_str("$N"),
            '\r' => literal.push_str("$R"),
            '\t' => literal.push_str("$T"),
            '\u{000C}' => literal.push_str("$P"),
            _ if character == quote => {
                literal.push('$');
                literal.push(quote);
            }
            control if control.is_ascii_control() => {
                let code = u32::from(control);
                let _ = if wide {
                    write!(literal, "${code:04X}")
                } else {
                    write!(literal, "${code:02X}")
                };
            }
            other => literal.push(other),
        }
    }
    literal.push(quote);
    literal
}

fn format_duration(prefix: &str, nanos: i64) -> String {
    if nanos % 1_000_000_000 == 0 {
        return format!("{prefix}#{}s", nanos / 1_000_000_000);
    }
    if nanos % 1_000_000 == 0 {
        return format!("{prefix}#{}ms", nanos / 1_000_000);
    }
    if nanos % 1_000 == 0 {
        return format!("{prefix}#{}us", nanos / 1_000);
    }
    format!("{prefix}#{nanos}ns")
}

fn quote_st_string(value: &str) -> String {
    format!("'{}'", value.replace('$', "$$").replace('\'', "$'"))
}
