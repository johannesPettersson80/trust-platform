//! The value grammar shared by every Live Values and debugger request that carries a value
//! (specification 11, "Live Values"; specification 34 §4.3.1).
//!
//! A value is an IEC 61131-3 literal of its destination's declared type (IEC 61131-3 Ed.3 §6.3.2
//! to §6.3.5, Tables 5 to 9; §6.4.4.2): [`literal`] reads its syntax and [`typed`] admits it as a
//! value of the type the program's type table declares, refusing with a plain reason a literal of
//! another type, outside the type's range or longer than the type holds. Where the destination's
//! type is not known, at an address that no variable is bound to or on a platform that has no
//! program at hand, the untyped grammar [`parse_value`] applies: a boolean (`TRUE`/`FALSE`, in any
//! ASCII case) or a signed 64-bit integer, with surrounding whitespace ignored, whose type the
//! destination gives it when the write is applied. trust-runtime and a device answer the same
//! text the same way because they parse it here; a text that fits neither is refused with the
//! stable `unsupported value '<text>'`.

pub mod literal;
pub mod typed;

use alloc::format;
use alloc::string::String;

use trust_runtime_core::value::Value;

pub use literal::{parse_literal, Literal};
pub use typed::{address_value, typed_value};

/// Parse the untyped grammar of a control value: a boolean or a signed 64-bit integer.
///
/// The text is trimmed and upper-cased first, so `TRUE`, ` \tTrUe\n` and `false` are booleans and
/// `+0007`, `-7` and `9223372036854775807` are `LInt`. An integer separator, a decimal point, an
/// exponent, a hexadecimal prefix, a quoted text, an empty text or a value outside `i64` is
/// refused.
///
/// # Errors
///
/// The stable refusal text `unsupported value '<text>'` (with the request's own text) when it is
/// neither a boolean nor a signed 64-bit integer.
pub fn parse_value(text: &str) -> Result<Value, String> {
    let upper = text.trim().to_ascii_uppercase();
    if upper == "TRUE" {
        return Ok(Value::Bool(true));
    }
    if upper == "FALSE" {
        return Ok(Value::Bool(false));
    }
    if let Ok(value) = upper.parse::<i64>() {
        return Ok(Value::LInt(value));
    }
    Err(format!("unsupported value '{text}'"))
}

#[cfg(test)]
mod tests;
