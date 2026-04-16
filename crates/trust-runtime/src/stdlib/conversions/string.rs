use crate::error::RuntimeError;
use crate::value::Value;
use trust_hir::TypeId;

pub(super) fn convert_to_string(value: &Value, dst: TypeId) -> Result<Value, RuntimeError> {
    let text = match value {
        Value::String(s) => Some(s.to_string()),
        Value::WString(s) => Some(s.clone()),
        Value::Char(c) => Some((*c as char).to_string()),
        Value::WChar(c) => {
            let ch = std::char::from_u32(*c as u32).ok_or(RuntimeError::TypeMismatch)?;
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
        Value::Real(v) => Some(format_real_string(f64::from(*v))),
        Value::LReal(v) => Some(format_real_string(*v)),
        _ => None,
    };

    match dst {
        TypeId::STRING => text
            .map(|value| Value::String(value.into()))
            .ok_or(RuntimeError::TypeMismatch),
        TypeId::WSTRING => text.map(Value::WString).ok_or(RuntimeError::TypeMismatch),
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

pub(super) fn parse_int_text(text: &str) -> Result<i128, RuntimeError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::TypeMismatch);
    }
    let cleaned: String = trimmed.chars().filter(|c| *c != '_').collect();
    if let Some((base_str, digits)) = cleaned.split_once('#') {
        let base: u32 = base_str.parse().map_err(|_| RuntimeError::TypeMismatch)?;
        if digits.is_empty() {
            return Err(RuntimeError::TypeMismatch);
        }
        let negative = digits.starts_with('-');
        let digits = digits.strip_prefix(['+', '-']).unwrap_or(digits);
        let value = i128::from_str_radix(digits, base).map_err(|_| RuntimeError::TypeMismatch)?;
        return if negative { Ok(-value) } else { Ok(value) };
    }
    cleaned
        .parse::<i128>()
        .map_err(|_| RuntimeError::TypeMismatch)
}

pub(super) fn parse_real_text(text: &str) -> Result<f64, RuntimeError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(RuntimeError::TypeMismatch);
    }
    let cleaned: String = trimmed.chars().filter(|c| *c != '_').collect();
    cleaned
        .parse::<f64>()
        .map_err(|_| RuntimeError::TypeMismatch)
}

pub(super) fn convert_to_char(value: &Value, dst: TypeId) -> Result<Value, RuntimeError> {
    match dst {
        TypeId::CHAR => match value {
            Value::Char(c) => Ok(Value::Char(*c)),
            Value::WChar(c) => {
                if *c > u8::MAX as u16 {
                    return Err(RuntimeError::Overflow);
                }
                Ok(Value::Char(*c as u8))
            }
            Value::String(s) => string_to_char(s.as_str(), false),
            Value::WString(s) => string_to_char(s, false),
            Value::SInt(v) => numeric_to_char(i128::from(*v), false),
            Value::Int(v) => numeric_to_char(i128::from(*v), false),
            Value::DInt(v) => numeric_to_char(i128::from(*v), false),
            Value::LInt(v) => numeric_to_char(i128::from(*v), false),
            Value::USInt(v) => numeric_to_char(i128::from(*v), false),
            Value::UInt(v) => numeric_to_char(i128::from(*v), false),
            Value::UDInt(v) => numeric_to_char(i128::from(*v), false),
            Value::ULInt(v) => numeric_to_char(i128::from(*v), false),
            Value::Byte(v) => numeric_to_char(i128::from(*v), false),
            Value::Word(v) => numeric_to_char(i128::from(*v), false),
            Value::DWord(v) => numeric_to_char(i128::from(*v), false),
            Value::LWord(v) => numeric_to_char(i128::from(*v), false),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::WCHAR => match value {
            Value::WChar(c) => Ok(Value::WChar(*c)),
            Value::Char(c) => Ok(Value::WChar(*c as u16)),
            Value::String(s) => string_to_char(s.as_str(), true),
            Value::WString(s) => string_to_char(s, true),
            Value::SInt(v) => numeric_to_char(i128::from(*v), true),
            Value::Int(v) => numeric_to_char(i128::from(*v), true),
            Value::DInt(v) => numeric_to_char(i128::from(*v), true),
            Value::LInt(v) => numeric_to_char(i128::from(*v), true),
            Value::USInt(v) => numeric_to_char(i128::from(*v), true),
            Value::UInt(v) => numeric_to_char(i128::from(*v), true),
            Value::UDInt(v) => numeric_to_char(i128::from(*v), true),
            Value::ULInt(v) => numeric_to_char(i128::from(*v), true),
            Value::Byte(v) => numeric_to_char(i128::from(*v), true),
            Value::Word(v) => numeric_to_char(i128::from(*v), true),
            Value::DWord(v) => numeric_to_char(i128::from(*v), true),
            Value::LWord(v) => numeric_to_char(i128::from(*v), true),
            _ => Err(RuntimeError::TypeMismatch),
        },
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn format_real_string(value: f64) -> String {
    let mut text = value.to_string();
    if !text.contains('.') && !text.contains('e') && !text.contains('E') {
        text.push_str(".0");
    }
    text
}

fn string_to_char(text: &str, wide: bool) -> Result<Value, RuntimeError> {
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

fn numeric_to_char(value: i128, wide: bool) -> Result<Value, RuntimeError> {
    if wide {
        let code = u16::try_from(value).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::WChar(code))
    } else {
        let code = u8::try_from(value).map_err(|_| RuntimeError::Overflow)?;
        Ok(Value::Char(code))
    }
}
