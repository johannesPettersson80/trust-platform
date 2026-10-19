use super::ConversionType;
use crate::error::RuntimeError;
use crate::stdlib::helpers::require_arity;
use crate::stdlib::text_kernels::TextResult;
use crate::value::{check_string_result, Value};
use alloc::string::String;

use super::bcd::{from_bcd, to_bcd};
use super::bitstring::convert_to_bit_string;
use super::numeric::{convert_to_int, convert_to_real, finite_real_result, signed_int_from_i128};
use super::spec::ConversionSpec;
use super::string::{
    convert_to_char, convert_to_string, convert_to_string_into, parse_int_text,
    parse_real_text_with, string_to_char,
};
use super::time::{convert_to_date, convert_to_dt, convert_to_time, convert_to_tod};
use super::util::{is_conversion_allowed, is_integer_type, value_type_id};
use super::ConversionMode;

pub(super) fn apply_conversion(
    spec: ConversionSpec,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    let value = &args[0];
    match spec {
        ConversionSpec::Convert { src, dst } => {
            convert_with_mode(value, src, dst, ConversionMode::Round)
        }
        ConversionSpec::Trunc { src, dst } => trunc_convert(value, src, dst),
        ConversionSpec::ToBcd { src, dst } => to_bcd(value, src, dst),
        ConversionSpec::BcdTo { src, dst } => from_bcd(value, src, dst),
    }
}

fn convert_with_mode(
    value: &Value,
    src: Option<ConversionType>,
    dst: ConversionType,
    mode: ConversionMode,
) -> Result<Value, RuntimeError> {
    let (value, actual_src) = normalize_source_value(value, src, mode)?;
    if !is_conversion_allowed(actual_src, dst) {
        return Err(RuntimeError::TypeMismatch);
    }
    convert_value(&value, dst, mode)
}

fn trunc_convert(
    value: &Value,
    src: Option<ConversionType>,
    dst: ConversionType,
) -> Result<Value, RuntimeError> {
    let (value, actual_src) = normalize_source_value(value, src, ConversionMode::Trunc)?;
    if !matches!(actual_src, ConversionType::Real | ConversionType::LReal) {
        return Err(RuntimeError::TypeMismatch);
    }
    if !is_integer_type(dst) {
        return Err(RuntimeError::TypeMismatch);
    }
    convert_value(&value, dst, ConversionMode::Trunc)
}

fn normalize_source_value(
    value: &Value,
    src: Option<ConversionType>,
    mode: ConversionMode,
) -> Result<(Value, ConversionType), RuntimeError> {
    let actual_src = value_type_id(value).ok_or_else(|| RuntimeError::TypeMismatch)?;
    let Some(expected) = src else {
        return Ok((value.clone(), actual_src));
    };
    if actual_src == expected {
        return Ok((value.clone(), actual_src));
    }
    if !is_conversion_allowed(actual_src, expected) {
        return Err(RuntimeError::TypeMismatch);
    }
    // Runtime storage can widen scalar values (for example INT -> DINT after arithmetic).
    // Exact-source conversions should normalize back to the requested source type first.
    let coerced = convert_value(value, expected, mode)?;
    Ok((coerced, expected))
}

fn convert_value(
    value: &Value,
    dst: ConversionType,
    mode: ConversionMode,
) -> Result<Value, RuntimeError> {
    if let Some(src) = value_type_id(value)
        && src == dst
    {
        return Ok(value.clone());
    }

    match dst {
        ConversionType::SInt
        | ConversionType::Int
        | ConversionType::DInt
        | ConversionType::LInt
        | ConversionType::USInt
        | ConversionType::UInt
        | ConversionType::UDInt
        | ConversionType::ULInt => convert_to_int(value, dst, mode),
        ConversionType::Real | ConversionType::LReal => convert_to_real(value, dst),
        ConversionType::Byte
        | ConversionType::Word
        | ConversionType::DWord
        | ConversionType::LWord => convert_to_bit_string(value, dst),
        ConversionType::Time | ConversionType::LTime => convert_to_time(value, dst),
        ConversionType::Date | ConversionType::LDate => convert_to_date(value, dst),
        ConversionType::Tod | ConversionType::LTod => convert_to_tod(value, dst),
        ConversionType::Dt | ConversionType::Ldt => convert_to_dt(value, dst),
        ConversionType::String | ConversionType::WString => convert_to_string(value, dst),
        ConversionType::Char | ConversionType::WChar => convert_to_char(value, dst),
        ConversionType::Bool => Err(RuntimeError::TypeMismatch),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

/// [`apply_conversion`] of `ConversionSpec::Convert { src, dst }` for the value `value`, a text
/// result written into `out` instead of a new value (C1-12): what it gives, the text in `out`.
pub(crate) fn convert_value_into(
    src: Option<ConversionType>,
    dst: ConversionType,
    value: &Value,
    out: &mut String,
) -> Result<TextResult, RuntimeError> {
    let (value, actual_src) = normalize_source_value(value, src, ConversionMode::Round)?;
    if !is_conversion_allowed(actual_src, dst) {
        return Err(RuntimeError::TypeMismatch);
    }
    converted_into(value, dst, out)
}

/// [`apply_conversion`] of `ConversionSpec::Convert { src, dst }` for a text read where it lies,
/// a `WSTRING` when `wide` (C1-12): parsed without a copy, a text result written into `out`, a real
/// with separators cleaned in `scratch`. What it gives for the text's value.
pub(crate) fn convert_text_into(
    src: Option<ConversionType>,
    dst: ConversionType,
    text: &str,
    wide: bool,
    out: &mut String,
    scratch: &mut String,
) -> Result<TextResult, RuntimeError> {
    let mut actual = if wide {
        ConversionType::WString
    } else {
        ConversionType::String
    };
    if let Some(expected) = src
        && expected != actual
    {
        // The source is first converted to the type the conversion names, as for its value.
        if !is_conversion_allowed(actual, expected) {
            return Err(RuntimeError::TypeMismatch);
        }
        if !matches!(expected, ConversionType::String | ConversionType::WString) {
            let value = text_value_as(expected, text, scratch)?;
            if !is_conversion_allowed(expected, dst) {
                return Err(RuntimeError::TypeMismatch);
            }
            return converted_into(value, dst, out);
        }
        check_string_result(text, expected == ConversionType::WString)?;
        actual = expected;
    }
    if !is_conversion_allowed(actual, dst) {
        return Err(RuntimeError::TypeMismatch);
    }
    if actual == dst {
        out.push_str(text);
        return Ok(TextResult::Text {
            wide: dst == ConversionType::WString,
        });
    }
    match dst {
        ConversionType::String | ConversionType::WString => {
            let wide = dst == ConversionType::WString;
            check_string_result(text, wide)?;
            out.push_str(text);
            Ok(TextResult::Text { wide })
        }
        _ => text_value_as(dst, text, scratch).map(TextResult::Value),
    }
}

/// The value of the text `text` converted to the type `ty` that is no text: an integer, a real or
/// a character, as `convert_value` gives it for the text's value.
fn text_value_as(
    ty: ConversionType,
    text: &str,
    scratch: &mut String,
) -> Result<Value, RuntimeError> {
    match ty {
        ConversionType::SInt
        | ConversionType::Int
        | ConversionType::DInt
        | ConversionType::LInt
        | ConversionType::USInt
        | ConversionType::UInt
        | ConversionType::UDInt
        | ConversionType::ULInt => signed_int_from_i128(parse_int_text(text)?, ty),
        ConversionType::Real | ConversionType::LReal => {
            finite_real_result(parse_real_text_with(text, scratch)?, ty)
        }
        ConversionType::Char | ConversionType::WChar => {
            string_to_char(text, ty == ConversionType::WChar)
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

/// The last step of a conversion of the value `value` to `dst`: a text into `out`, any other
/// result as a value (C1-12).
fn converted_into(
    value: Value,
    dst: ConversionType,
    out: &mut String,
) -> Result<TextResult, RuntimeError> {
    if value_type_id(&value) == Some(dst) {
        return Ok(TextResult::Value(value));
    }
    match dst {
        ConversionType::String | ConversionType::WString => {
            convert_to_string_into(&value, dst, out).map(|wide| TextResult::Text { wide })
        }
        _ => convert_value(&value, dst, ConversionMode::Round).map(TextResult::Value),
    }
}
