//! User-facing runtime value formatting.

use super::Value;
use alloc::{
    format,
    string::{String, ToString},
};

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
        Value::Real(value) => format_real(f64::from(*value)),
        Value::LReal(value) => format_real(*value),
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

fn format_real(value: f64) -> String {
    let mut text = value.to_string();
    if !text.contains('.') && !text.contains('e') && !text.contains('E') {
        text.push_str(".0");
    }
    text
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
