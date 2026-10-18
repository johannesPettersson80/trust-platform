//! The scalar value grammar shared by every Live Values request that carries a value
//! (specification 11, "Live Values"; specification 34 §4.3.1).
//!
//! One grammar for `set`, `var.force`, `io.write` and `io.force`: a boolean (`TRUE`/`FALSE`, in
//! any ASCII case) or a signed 64-bit integer, with surrounding whitespace ignored. trust-runtime
//! and a device answer the same text the same way because they parse it here; a text that fits
//! neither is refused with the stable `unsupported value '<text>'`.

use alloc::format;
use alloc::string::String;

use trust_runtime_core::value::Value;

/// Parse the scalar grammar of a control value: a boolean or a signed 64-bit integer.
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
