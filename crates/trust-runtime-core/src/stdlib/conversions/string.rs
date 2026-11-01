use super::ConversionType;
use crate::error::RuntimeError;
use crate::value::{check_string_result, Value};
use alloc::string::{String, ToString};

pub(super) fn convert_to_string(value: &Value, dst: ConversionType) -> Result<Value, RuntimeError> {
    let text = match value {
        Value::String(s) => Some(s.to_string()),
        Value::WString(s) => Some(s.clone()),
        Value::Char(c) => Some((*c as char).to_string()),
        Value::WChar(c) => {
            let ch = core::char::from_u32(*c as u32).ok_or(RuntimeError::TypeMismatch)?;
            Some(ch.to_string())
        }
        Value::SInt(v) => Some(v.to_string()),
        Value::Int(v) => Some(v.to_string()),
        Value::DInt(v) => Some(v.to_string()),
        Value::LInt(v) => Some(v.to_string()),
        Value::USInt(v) => Some(v.to_string()),
        Value::UInt(v) => Some(v.to_string()),
        Value::UDInt(v) => Some(v.to_string()),
        Value::ULInt(v) => Some(v.to_string()),
        Value::Byte(v) => Some(v.to_string()),
        Value::Word(v) => Some(v.to_string()),
        Value::DWord(v) => Some(v.to_string()),
        Value::LWord(v) => Some(v.to_string()),
        Value::Real(v) => Some(crate::value::float_text::real(f64::from(*v))),
        Value::LReal(v) => Some(crate::value::float_text::real(*v)),
        Value::Bool(v) => {
            if *v {
                Some("TRUE".to_string())
            } else {
                Some("FALSE".to_string())
            }
        }
        Value::Time(_) | Value::LTime(_) => Some(crate::value::format_user_value(value)),
        Value::Enum(v) => Some(v.variant_name().to_string()),
        _ => None,
    };

    // A text result longer than the implementer maximum is an error (IEC 61131-3 Ed.3
    // §6.6.2.5.11): a WSTRING of 65,535 code units can need up to 196,605 bytes as STRING.
    match dst {
        ConversionType::String => {
            let text = text.ok_or(RuntimeError::TypeMismatch)?;
            check_string_result(&text, false)?;
            Ok(Value::String(text.into()))
        }
        ConversionType::WString => {
            let text = text.ok_or(RuntimeError::TypeMismatch)?;
            check_string_result(&text, true)?;
            Ok(Value::WString(text))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn string_input(value: &Value) -> Result<&str, RuntimeError> {
    match value {
        Value::String(s) => Ok(s.as_str()),
        Value::WString(s) => Ok(s.as_str()),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

// Keep the wide parser: a syntactically valid i128 outside an IEC destination
// faults with Overflow, while text outside i128 faults with TypeMismatch. The
// text is read where it lies, its separators skipped, so parsing allocates
// nothing (C1-12).
pub(super) fn parse_int_text(text: &str) -> Result<i128, RuntimeError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::TypeMismatch);
    }
    if let Some((base_str, digits)) = trimmed.split_once('#') {
        // Spec 07 section 2.8: the optional sign belongs to the digits, never to the base.
        // `u32::from_str` accepts a leading `+`, so the base must be decimal digits only.
        check_separators(base_str, 10)?;
        if !base_str
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'_')
        {
            return Err(RuntimeError::TypeMismatch);
        }
        let base = digits_value(base_str, 10)
            .and_then(|base| u32::try_from(base).ok())
            .ok_or(RuntimeError::TypeMismatch)?;
        if !(2..=36).contains(&base) {
            return Err(RuntimeError::TypeMismatch);
        }
        check_separators(digits, base)?;
        // Spec 07 section 2.8: one optional sign on the digits. `from_str_radix` would
        // accept a second sign, so a sign left after the first one is rejected here.
        let (negative, unsigned) = match digits.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, digits.strip_prefix('+').unwrap_or(digits)),
        };
        if unsigned.starts_with(['+', '-']) {
            return Err(RuntimeError::TypeMismatch);
        }
        let value = digits_value(unsigned, base).ok_or(RuntimeError::TypeMismatch)?;
        return if negative { Ok(-value) } else { Ok(value) };
    }
    check_separators(trimmed, 10)?;
    let (negative, unsigned) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
    };
    if negative {
        signed_digits_value(unsigned).ok_or(RuntimeError::TypeMismatch)
    } else {
        digits_value(unsigned, 10).ok_or(RuntimeError::TypeMismatch)
    }
}

/// The value of the digits of `text` in `radix`, its separators skipped, as `from_str_radix`
/// reads the text without them: `None` for no digit, a character that is no digit of `radix`, a
/// sign, and a value above `i128::MAX`.
fn digits_value(text: &str, radix: u32) -> Option<i128> {
    let mut value: Option<i128> = None;
    for character in text.chars().filter(|character| *character != '_') {
        let digit = i128::from(character.to_digit(radix)?);
        value = Some(
            value
                .unwrap_or(0)
                .checked_mul(i128::from(radix))?
                .checked_add(digit)?,
        );
    }
    value
}

/// The negative value of the decimal digits of `text`, its separators skipped, as `str::parse`
/// reads `-` and the text without them, down to `i128::MIN`.
fn signed_digits_value(text: &str) -> Option<i128> {
    let mut value: Option<i128> = None;
    for character in text.chars().filter(|character| *character != '_') {
        let digit = i128::from(character.to_digit(10)?);
        value = Some(value.unwrap_or(0).checked_mul(10)?.checked_sub(digit)?);
    }
    value
}

pub(super) fn parse_real_text(text: &str) -> Result<f64, RuntimeError> {
    parse_real_text_with(text, &mut String::new())
}

/// [`parse_real_text`], a text with separators cleaned in `scratch`, so that parsing allocates
/// nothing once `scratch` has room (C1-12).
pub(super) fn parse_real_text_with(text: &str, scratch: &mut String) -> Result<f64, RuntimeError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::TypeMismatch);
    }
    check_separators(trimmed, 10)?;
    let cleaned = if trimmed.contains('_') {
        scratch.clear();
        scratch.extend(trimmed.chars().filter(|character| *character != '_'));
        scratch.as_str()
    } else {
        trimmed
    };
    let value = crate::value::float_text::parse(cleaned)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(RuntimeError::Overflow)
    }
}

// IEC 61131-3 section 6.3.2: a single `_` between two digits of a numeric
// literal is not significant, and no other use is allowed. A leading, trailing
// or repeated `_` is malformed text. `radix` is the digit set of the literal:
// decimal for real text and for a base prefix, the base itself for `digits`.
fn check_separators(text: &str, radix: u32) -> Result<(), RuntimeError> {
    let mut previous: Option<char> = None;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '_' {
            let after_digit = chars.peek().is_some_and(|next| next.is_digit(radix));
            let before_digit = previous.is_some_and(|prev| prev.is_digit(radix));
            if !(before_digit && after_digit) {
                return Err(RuntimeError::TypeMismatch);
            }
        }
        previous = Some(ch);
    }
    Ok(())
}

pub(super) fn convert_to_char(value: &Value, dst: ConversionType) -> Result<Value, RuntimeError> {
    match dst {
        ConversionType::Char => match value {
            Value::Char(c) => Ok(Value::Char(*c)),
            Value::WChar(c) => {
                if *c > u8::MAX as u16 {
                    return Err(RuntimeError::Overflow);
                }
                Ok(Value::Char(*c as u8))
            }
            Value::String(s) => string_to_char(s.as_str(), false),
            Value::WString(s) => string_to_char(s, false),
            Value::SInt(v) => numeric_to_char(*v as u64, false),
            Value::Int(v) => numeric_to_char(*v as u64, false),
            Value::DInt(v) => numeric_to_char(*v as u64, false),
            Value::LInt(v) => numeric_to_char(*v as u64, false),
            Value::USInt(v) => numeric_to_char(*v as u64, false),
            Value::UInt(v) => numeric_to_char(*v as u64, false),
            Value::UDInt(v) => numeric_to_char(*v as u64, false),
            Value::ULInt(v) => numeric_to_char(*v, false),
            Value::Byte(v) => numeric_to_char(*v as u64, false),
            Value::Word(v) => numeric_to_char(*v as u64, false),
            Value::DWord(v) => numeric_to_char(*v as u64, false),
            Value::LWord(v) => numeric_to_char(*v, false),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::WChar => match value {
            Value::WChar(c) => Ok(Value::WChar(*c)),
            Value::Char(c) => Ok(Value::WChar(*c as u16)),
            Value::String(s) => string_to_char(s.as_str(), true),
            Value::WString(s) => string_to_char(s, true),
            Value::SInt(v) => numeric_to_char(*v as u64, true),
            Value::Int(v) => numeric_to_char(*v as u64, true),
            Value::DInt(v) => numeric_to_char(*v as u64, true),
            Value::LInt(v) => numeric_to_char(*v as u64, true),
            Value::USInt(v) => numeric_to_char(*v as u64, true),
            Value::UInt(v) => numeric_to_char(*v as u64, true),
            Value::UDInt(v) => numeric_to_char(*v as u64, true),
            Value::ULInt(v) => numeric_to_char(*v, true),
            Value::Byte(v) => numeric_to_char(*v as u64, true),
            Value::Word(v) => numeric_to_char(*v as u64, true),
            Value::DWord(v) => numeric_to_char(*v as u64, true),
            Value::LWord(v) => numeric_to_char(*v, true),
            _ => Err(RuntimeError::TypeMismatch),
        },
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn string_to_char(text: &str, wide: bool) -> Result<Value, RuntimeError> {
    let mut chars = text.chars();
    let ch = chars.next().ok_or(RuntimeError::TypeMismatch)?;
    if chars.next().is_some() {
        return Err(RuntimeError::TypeMismatch);
    }
    if wide {
        let code = u16::try_from(ch as u32).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::WChar(code))
    } else {
        let code = u8::try_from(ch as u32).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::Char(code))
    }
}

// Signed inputs arrive as their u64 bit patterns. Negative values exceed both
// character widths, preserving the previous checked-conversion Overflow result.
fn numeric_to_char(value: u64, wide: bool) -> Result<Value, RuntimeError> {
    if wide {
        let code = u16::try_from(value).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::WChar(code))
    } else {
        let code = u8::try_from(value).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::Char(code))
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;

    #[test]
    fn character_conversions_preserve_signed_and_unsigned_limits() {
        for value in [i64::MIN, -1, 0, 1, 255, 256, 65535, 65536, i64::MAX] {
            assert_eq!(
                convert_to_char(&Value::LInt(value), ConversionType::Char),
                u8::try_from(i128::from(value))
                    .map(Value::Char)
                    .map_err(|_| RuntimeError::Overflow)
            );
            assert_eq!(
                convert_to_char(&Value::LInt(value), ConversionType::WChar),
                u16::try_from(i128::from(value))
                    .map(Value::WChar)
                    .map_err(|_| RuntimeError::Overflow)
            );
        }
        for value in [0, 255, 256, 65535, 65536, u64::MAX] {
            assert_eq!(
                convert_to_char(&Value::ULInt(value), ConversionType::Char),
                u8::try_from(value)
                    .map(Value::Char)
                    .map_err(|_| RuntimeError::Overflow)
            );
            assert_eq!(
                convert_to_char(&Value::ULInt(value), ConversionType::WChar),
                u16::try_from(value)
                    .map(Value::WChar)
                    .map_err(|_| RuntimeError::Overflow)
            );
        }
    }

    #[test]
    fn retained_wide_text_parser_keeps_radix_sign_and_error_precedence() {
        for (text, expected) in [
            ("-9223372036854775808", i64::MIN as i128),
            ("18446744073709551615", u64::MAX as i128),
            ("16#-8000_0000_0000_0000", i64::MIN as i128),
            ("16#FFFF_FFFF_FFFF_FFFF", u64::MAX as i128),
            ("2#+1010", 10),
        ] {
            assert_eq!(parse_int_text(text), Ok(expected));
        }
        assert_eq!(
            super::super::call_conversion(
                "STRING_TO_ULINT",
                &[Value::String("18446744073709551616".into())]
            ),
            Some(Err(RuntimeError::Overflow))
        );
        assert_eq!(
            super::super::call_conversion(
                "STRING_TO_ULINT",
                &[Value::String(
                    "170141183460469231731687303715884105728".into()
                )]
            ),
            Some(Err(RuntimeError::TypeMismatch))
        );
        assert_eq!(parse_int_text("37#1"), Err(RuntimeError::TypeMismatch));
        assert_eq!(parse_int_text("16#-"), Err(RuntimeError::TypeMismatch));
    }
}
