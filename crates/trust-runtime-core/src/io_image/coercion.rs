use crate::{
    error::RuntimeError, io_address::IoSize, stdlib::conversions::ConversionType, value::Value,
};
use alloc::{boxed::Box, format, vec::Vec};
use smol_str::SmolStr;

/// Enum identity and named values used by process-image conversion.
#[derive(Debug, Clone)]
pub struct IoEnumBinding {
    /// Declared enum type name.
    pub type_name: SmolStr,
    /// Declared variant names and their integer representation.
    pub variants: Vec<(SmolStr, i64)>,
}

/// Return the fixed image width of a supported scalar category.
pub fn expected_size_for_type(value_type: ConversionType) -> Option<IoSize> {
    match value_type {
        ConversionType::Bool => Some(IoSize::Bit),
        ConversionType::SInt
        | ConversionType::USInt
        | ConversionType::Byte
        | ConversionType::Char => Some(IoSize::Byte),
        ConversionType::Int
        | ConversionType::UInt
        | ConversionType::Word
        | ConversionType::WChar => Some(IoSize::Word),
        ConversionType::DInt
        | ConversionType::UDInt
        | ConversionType::DWord
        | ConversionType::Real
        | ConversionType::Time => Some(IoSize::DWord),
        ConversionType::LInt
        | ConversionType::ULInt
        | ConversionType::LWord
        | ConversionType::LReal => Some(IoSize::LWord),
        ConversionType::String => None,
        _ => None,
    }
}

/// Return the diagnostic IEC name of a supported image value category.
pub fn io_value_type_name(value_type: ConversionType) -> Option<&'static str> {
    match value_type {
        ConversionType::Bool => Some("BOOL"),
        ConversionType::SInt => Some("SINT"),
        ConversionType::Int => Some("INT"),
        ConversionType::DInt => Some("DINT"),
        ConversionType::LInt => Some("LINT"),
        ConversionType::USInt => Some("USINT"),
        ConversionType::UInt => Some("UINT"),
        ConversionType::UDInt => Some("UDINT"),
        ConversionType::ULInt => Some("ULINT"),
        ConversionType::Real => Some("REAL"),
        ConversionType::LReal => Some("LREAL"),
        ConversionType::Byte => Some("BYTE"),
        ConversionType::Word => Some("WORD"),
        ConversionType::DWord => Some("DWORD"),
        ConversionType::LWord => Some("LWORD"),
        ConversionType::Time => Some("TIME"),
        ConversionType::String => Some("STRING"),
        ConversionType::Char => Some("CHAR"),
        ConversionType::WChar => Some("WCHAR"),
        _ => None,
    }
}

/// Decode an optional scalar or enum binding from an image value.
pub fn coerce_binding_from_io(
    value: Value,
    wire_type: Option<ConversionType>,
    enum_type: Option<&IoEnumBinding>,
) -> Result<Value, RuntimeError> {
    let Some(wire_type) = wire_type else {
        return Ok(value);
    };
    let value = coerce_from_io(value, wire_type)?;
    let Some(enum_type) = enum_type else {
        return Ok(value);
    };
    let numeric_value = crate::numeric::to_i64(&value)?;
    let Some((variant_name, _)) = enum_type
        .variants
        .iter()
        .find(|(_, declared)| *declared == numeric_value)
    else {
        return Err(RuntimeError::IoDriver(
            format!(
                "process-image value {numeric_value} is not declared by enum {}",
                enum_type.type_name
            )
            .into(),
        ));
    };
    Ok(Value::Enum(Box::new(
        crate::value::EnumValue::from_canonical_parts(
            enum_type.type_name.clone(),
            variant_name.clone(),
            numeric_value,
        ),
    )))
}

/// Encode an optional scalar or enum binding to the requested image width.
pub fn coerce_binding_to_io(
    value: Value,
    wire_type: Option<ConversionType>,
    enum_type: Option<&IoEnumBinding>,
    size: IoSize,
) -> Result<Value, RuntimeError> {
    let Some(wire_type) = wire_type else {
        return Ok(value);
    };
    let Some(enum_type) = enum_type else {
        return coerce_to_io(value, wire_type, size);
    };
    let numeric_value = match &value {
        Value::Enum(value) => {
            if !value
                .type_name()
                .eq_ignore_ascii_case(enum_type.type_name.as_str())
            {
                return Err(RuntimeError::TypeMismatch);
            }
            let valid = enum_type.variants.iter().any(|(variant, numeric)| {
                variant.eq_ignore_ascii_case(value.variant_name().as_str())
                    && *numeric == value.numeric_value()
            });
            if !valid {
                return Err(RuntimeError::IoDriver(
                    format!("invalid value for enum {}", enum_type.type_name).into(),
                ));
            }
            value.numeric_value()
        }
        value => crate::numeric::to_i64(value)?,
    };
    if !enum_type
        .variants
        .iter()
        .any(|(_, declared)| *declared == numeric_value)
    {
        return Err(RuntimeError::IoDriver(
            format!(
                "value {numeric_value} is not declared by enum {}",
                enum_type.type_name
            )
            .into(),
        ));
    }
    coerce_to_io(Value::LInt(numeric_value), wire_type, size)
}

/// Decode the scalar image representation as the declared IEC category.
pub fn coerce_from_io(value: Value, target: ConversionType) -> Result<Value, RuntimeError> {
    match target {
        ConversionType::Bool => match value {
            Value::Bool(flag) => Ok(Value::Bool(flag)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::SInt => match value {
            Value::Byte(byte) => Ok(Value::SInt(byte as i8)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::USInt => match value {
            Value::Byte(byte) => Ok(Value::USInt(byte)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::Byte => match value {
            Value::Byte(byte) => Ok(Value::Byte(byte)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::Char => match value {
            Value::Byte(byte) => Ok(Value::Char(byte)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::Int => match value {
            Value::Word(word) => Ok(Value::Int(word as i16)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::UInt => match value {
            Value::Word(word) => Ok(Value::UInt(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::Word => match value {
            Value::Word(word) => Ok(Value::Word(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::WChar => match value {
            Value::Word(word) => Ok(Value::WChar(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::DInt => match value {
            Value::DWord(word) => Ok(Value::DInt(word as i32)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::UDInt => match value {
            Value::DWord(word) => Ok(Value::UDInt(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::DWord => match value {
            Value::DWord(word) => Ok(Value::DWord(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::Real => match value {
            Value::DWord(word) => {
                let value = f32::from_bits(word);
                if value.is_finite() {
                    Ok(Value::Real(value))
                } else {
                    Err(RuntimeError::IoDriver(
                        "typed REAL process-image value must be finite".into(),
                    ))
                }
            }
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::Time => match value {
            Value::DWord(word) => Ok(Value::Time(crate::value::Duration::from_millis(i64::from(
                word,
            )))),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::String => match value {
            Value::String(text) => Ok(Value::String(text)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::LInt => match value {
            Value::LWord(word) => Ok(Value::LInt(word as i64)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::ULInt => match value {
            Value::LWord(word) => Ok(Value::ULInt(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::LWord => match value {
            Value::LWord(word) => Ok(Value::LWord(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::LReal => match value {
            Value::LWord(word) => {
                let value = f64::from_bits(word);
                if value.is_finite() {
                    Ok(Value::LReal(value))
                } else {
                    Err(RuntimeError::IoDriver(
                        "typed LREAL process-image value must be finite".into(),
                    ))
                }
            }
            _ => Err(RuntimeError::TypeMismatch),
        },
        _ => Err(RuntimeError::TypeMismatch),
    }
}

/// Encode a scalar or bounded string and enforce the requested image width.
pub fn coerce_to_io(
    value: Value,
    target: ConversionType,
    size: IoSize,
) -> Result<Value, RuntimeError> {
    match target {
        ConversionType::String => match (value, size) {
            (Value::String(text), IoSize::Bytes(len)) => {
                if text.len() > len as usize {
                    return Err(RuntimeError::Overflow);
                }
                Ok(Value::String(text))
            }
            _ => Err(RuntimeError::TypeMismatch),
        },
        _ => {
            let Some(expected) = expected_size_for_type(target) else {
                return Err(RuntimeError::TypeMismatch);
            };
            if expected != size {
                return Err(RuntimeError::TypeMismatch);
            }
            coerce_scalar_to_io(value, target)
        }
    }
}

/// Encode a scalar value using the declared IEC category and width rules.
pub fn coerce_scalar_to_io(value: Value, target: ConversionType) -> Result<Value, RuntimeError> {
    match target {
        ConversionType::Bool => match value {
            Value::Bool(flag) => Ok(Value::Bool(flag)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::SInt => {
            let val = match value {
                Value::SInt(val) => val,
                _ => i8::try_from(crate::numeric::to_i64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Byte(val as u8))
        }
        ConversionType::USInt => {
            let val = match value {
                Value::USInt(val) => val,
                _ => u8::try_from(crate::numeric::to_u64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Byte(val))
        }
        ConversionType::Byte => match value {
            Value::Byte(val) => Ok(Value::Byte(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::Char => match value {
            Value::Char(val) => Ok(Value::Byte(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::Int => {
            let val = match value {
                Value::Int(val) => val,
                _ => i16::try_from(crate::numeric::to_i64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Word(val as u16))
        }
        ConversionType::UInt => {
            let val = match value {
                Value::UInt(val) => val,
                _ => u16::try_from(crate::numeric::to_u64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Word(val))
        }
        ConversionType::Word => match value {
            Value::Word(val) => Ok(Value::Word(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::WChar => match value {
            Value::WChar(val) => Ok(Value::Word(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::DInt => {
            let val = match value {
                Value::DInt(val) => val,
                _ => i32::try_from(crate::numeric::to_i64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::DWord(val as u32))
        }
        ConversionType::UDInt => {
            let val = match value {
                Value::UDInt(val) => val,
                _ => u32::try_from(crate::numeric::to_u64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::DWord(val))
        }
        ConversionType::DWord => match value {
            Value::DWord(val) => Ok(Value::DWord(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::Real => {
            let val = match value {
                Value::Real(val) => val,
                _ => crate::numeric::to_f64(&value)? as f32,
            };
            if !val.is_finite() {
                return Err(RuntimeError::IoDriver(
                    "typed REAL process-image value must be finite".into(),
                ));
            }
            Ok(Value::DWord(val.to_bits()))
        }
        ConversionType::Time => match value {
            Value::Time(value) => {
                let millis = value.as_millis();
                let millis = u32::try_from(millis).map_err(|_| RuntimeError::Overflow)?;
                Ok(Value::DWord(millis))
            }
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::LInt => {
            let val = match value {
                Value::LInt(val) => val,
                _ => crate::numeric::to_i64(&value)?,
            };
            Ok(Value::LWord(val as u64))
        }
        ConversionType::ULInt => {
            let val = match value {
                Value::ULInt(val) => val,
                _ => crate::numeric::to_u64(&value)?,
            };
            Ok(Value::LWord(val))
        }
        ConversionType::LWord => match value {
            Value::LWord(val) => Ok(Value::LWord(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        ConversionType::LReal => {
            let val = match value {
                Value::LReal(val) => val,
                _ => crate::numeric::to_f64(&value)?,
            };
            if !val.is_finite() {
                return Err(RuntimeError::IoDriver(
                    "typed LREAL process-image value must be finite".into(),
                ));
            }
            Ok(Value::LWord(val.to_bits()))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}
