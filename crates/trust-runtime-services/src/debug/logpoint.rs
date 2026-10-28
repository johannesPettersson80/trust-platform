//! Logpoints (specification 13, "Breakpoint evaluation, source resolution, and hook contract";
//! specification 11 §6.13.2): a message of texts and compiled expressions, formatted at a hit with
//! each expression replaced by its value as a logpoint shows a value, or by `<error: detail>`.

use alloc::format;
use alloc::string::{String, ToString};

use trust_runtime_core::debug::StatementSpan;
use trust_runtime_core::value::Value;

use super::expression::{evaluate, Expression, Names};

/// The most bytes of a logpoint's message (specification 11 §6.13.2).
pub const MAX_MESSAGE_BYTES: usize = 1024;

/// One part of a logpoint's message.
#[derive(Debug, Clone, PartialEq)]
pub enum LogPart {
    /// Literal text.
    Text(String),
    /// An expression, replaced by its value.
    Expression(Expression),
}

/// A logpoint's message as a runtime keeps it until `debug.stops` returns it (specification 11
/// §6.13.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord {
    /// The message.
    pub text: String,
    /// The logpoint's statement.
    pub statement: StatementSpan,
    /// The thread that ran the statement.
    pub thread: Option<u32>,
}

/// The message of `parts`, each expression evaluated with the names of `names`, cut to
/// [`MAX_MESSAGE_BYTES`] at a character boundary.
#[must_use]
pub fn format_message(parts: &[LogPart], names: &dyn Names) -> String {
    let mut message = String::new();
    for part in parts {
        match part {
            LogPart::Text(text) => message.push_str(text),
            LogPart::Expression(expression) => match evaluate(expression, names) {
                Ok(value) => message.push_str(&log_text(&value)),
                Err(error) => {
                    message.push_str("<error: ");
                    message.push_str(error.detail());
                    message.push('>');
                }
            },
        }
        if message.len() > MAX_MESSAGE_BYTES {
            break;
        }
    }
    if message.len() > MAX_MESSAGE_BYTES {
        let mut end = MAX_MESSAGE_BYTES;
        while !message.is_char_boundary(end) {
            end -= 1;
        }
        message.truncate(end);
    }
    message
}

/// `value` as a logpoint shows it (specification 13): `TRUE` and `FALSE`, a text or a character
/// unquoted, an array as `[<element count>]`, a structure as `<type> {...}`, an enumeration as
/// `<type>::<value>`, `REF`, `NULL_REF`, `Instance` and `NULL`, and any other value in its stable
/// debug representation.
#[must_use]
pub fn log_text(value: &Value) -> String {
    match value {
        Value::Bool(true) => String::from("TRUE"),
        Value::Bool(false) => String::from("FALSE"),
        Value::String(text) => text.to_string(),
        Value::WString(text) => text.clone(),
        Value::Char(value) => char::from(*value).to_string(),
        Value::WChar(value) => char::from_u32(u32::from(*value)).unwrap_or('?').to_string(),
        Value::Array(array) => format!("[{}]", array.elements().len()),
        Value::Struct(value) => format!("{} {{...}}", value.type_name()),
        Value::Enum(value) => format!("{}::{}", value.type_name(), value.variant_name()),
        Value::Reference(Some(_)) => String::from("REF"),
        Value::Reference(None) => String::from("NULL_REF"),
        Value::Instance(_) => String::from("Instance"),
        Value::Null => String::from("NULL"),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
#[path = "logpoint/tests.rs"]
mod tests;
