fn expected_size_for_type(value_type: TypeId) -> Option<IoSize> {
    match value_type {
        TypeId::BOOL => Some(IoSize::Bit),
        TypeId::SINT | TypeId::USINT | TypeId::BYTE | TypeId::CHAR => Some(IoSize::Byte),
        TypeId::INT | TypeId::UINT | TypeId::WORD | TypeId::WCHAR => Some(IoSize::Word),
        TypeId::DINT | TypeId::UDINT | TypeId::DWORD | TypeId::REAL | TypeId::TIME => {
            Some(IoSize::DWord)
        }
        TypeId::LINT | TypeId::ULINT | TypeId::LWORD | TypeId::LREAL => Some(IoSize::LWord),
        TypeId::STRING => None,
        _ => None,
    }
}

pub fn io_value_type_name(value_type: TypeId) -> Option<&'static str> {
    match value_type {
        TypeId::BOOL => Some("BOOL"),
        TypeId::SINT => Some("SINT"),
        TypeId::INT => Some("INT"),
        TypeId::DINT => Some("DINT"),
        TypeId::LINT => Some("LINT"),
        TypeId::USINT => Some("USINT"),
        TypeId::UINT => Some("UINT"),
        TypeId::UDINT => Some("UDINT"),
        TypeId::ULINT => Some("ULINT"),
        TypeId::REAL => Some("REAL"),
        TypeId::LREAL => Some("LREAL"),
        TypeId::BYTE => Some("BYTE"),
        TypeId::WORD => Some("WORD"),
        TypeId::DWORD => Some("DWORD"),
        TypeId::LWORD => Some("LWORD"),
        TypeId::TIME => Some("TIME"),
        TypeId::STRING => Some("STRING"),
        TypeId::CHAR => Some("CHAR"),
        TypeId::WCHAR => Some("WCHAR"),
        _ => None,
    }
}

fn coerce_binding_from_io(
    value: Value,
    binding: &IoBinding,
    codec: &IoBindingCodec,
) -> Result<Value, RuntimeError> {
    let Some(wire_type) = codec.wire_type.or(binding.value_type) else {
        return Ok(value);
    };
    let value = coerce_from_io(value, wire_type)?;
    let Some(enum_type) = &codec.enum_type else {
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
    Ok(Value::Enum(Box::new(crate::value::EnumValue::from_canonical_parts(
        enum_type.type_name.clone(),
        variant_name.clone(),
        numeric_value,
    ))))
}

fn coerce_binding_to_io(
    value: Value,
    binding: &IoBinding,
    codec: &IoBindingCodec,
) -> Result<Value, RuntimeError> {
    let Some(wire_type) = codec.wire_type.or(binding.value_type) else {
        return Ok(value);
    };
    let Some(enum_type) = &codec.enum_type else {
        return coerce_to_io(value, wire_type, binding.address.size);
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
    coerce_to_io(Value::LInt(numeric_value), wire_type, binding.address.size)
}

fn coerce_from_io(value: Value, target: TypeId) -> Result<Value, RuntimeError> {
    match target {
        TypeId::BOOL => match value {
            Value::Bool(flag) => Ok(Value::Bool(flag)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::SINT => match value {
            Value::Byte(byte) => Ok(Value::SInt(byte as i8)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::USINT => match value {
            Value::Byte(byte) => Ok(Value::USInt(byte)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::BYTE => match value {
            Value::Byte(byte) => Ok(Value::Byte(byte)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::CHAR => match value {
            Value::Byte(byte) => Ok(Value::Char(byte)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::INT => match value {
            Value::Word(word) => Ok(Value::Int(word as i16)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::UINT => match value {
            Value::Word(word) => Ok(Value::UInt(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::WORD => match value {
            Value::Word(word) => Ok(Value::Word(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::WCHAR => match value {
            Value::Word(word) => Ok(Value::WChar(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::DINT => match value {
            Value::DWord(word) => Ok(Value::DInt(word as i32)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::UDINT => match value {
            Value::DWord(word) => Ok(Value::UDInt(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::DWORD => match value {
            Value::DWord(word) => Ok(Value::DWord(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::REAL => match value {
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
        TypeId::TIME => match value {
            Value::DWord(word) => Ok(Value::Time(crate::value::Duration::from_millis(i64::from(
                word,
            )))),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::STRING => match value {
            Value::String(text) => Ok(Value::String(text)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::LINT => match value {
            Value::LWord(word) => Ok(Value::LInt(word as i64)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::ULINT => match value {
            Value::LWord(word) => Ok(Value::ULInt(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::LWORD => match value {
            Value::LWord(word) => Ok(Value::LWord(word)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::LREAL => match value {
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

fn coerce_to_io(value: Value, target: TypeId, size: IoSize) -> Result<Value, RuntimeError> {
    match target {
        TypeId::STRING => match (value, size) {
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

fn coerce_scalar_to_io(value: Value, target: TypeId) -> Result<Value, RuntimeError> {
    match target {
        TypeId::BOOL => match value {
            Value::Bool(flag) => Ok(Value::Bool(flag)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::SINT => {
            let val = match value {
                Value::SInt(val) => val,
                _ => i8::try_from(crate::numeric::to_i64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Byte(val as u8))
        }
        TypeId::USINT => {
            let val = match value {
                Value::USInt(val) => val,
                _ => u8::try_from(crate::numeric::to_u64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Byte(val))
        }
        TypeId::BYTE => match value {
            Value::Byte(val) => Ok(Value::Byte(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::CHAR => match value {
            Value::Char(val) => Ok(Value::Byte(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::INT => {
            let val = match value {
                Value::Int(val) => val,
                _ => i16::try_from(crate::numeric::to_i64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Word(val as u16))
        }
        TypeId::UINT => {
            let val = match value {
                Value::UInt(val) => val,
                _ => u16::try_from(crate::numeric::to_u64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::Word(val))
        }
        TypeId::WORD => match value {
            Value::Word(val) => Ok(Value::Word(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::WCHAR => match value {
            Value::WChar(val) => Ok(Value::Word(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::DINT => {
            let val = match value {
                Value::DInt(val) => val,
                _ => i32::try_from(crate::numeric::to_i64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::DWord(val as u32))
        }
        TypeId::UDINT => {
            let val = match value {
                Value::UDInt(val) => val,
                _ => u32::try_from(crate::numeric::to_u64(&value)?)
                    .map_err(|_| RuntimeError::Overflow)?,
            };
            Ok(Value::DWord(val))
        }
        TypeId::DWORD => match value {
            Value::DWord(val) => Ok(Value::DWord(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::REAL => {
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
        TypeId::TIME => match value {
            Value::Time(value) => {
                let millis = value.as_millis();
                let millis = u32::try_from(millis).map_err(|_| RuntimeError::Overflow)?;
                Ok(Value::DWord(millis))
            }
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::LINT => {
            let val = match value {
                Value::LInt(val) => val,
                _ => crate::numeric::to_i64(&value)?,
            };
            Ok(Value::LWord(val as u64))
        }
        TypeId::ULINT => {
            let val = match value {
                Value::ULInt(val) => val,
                _ => crate::numeric::to_u64(&value)?,
            };
            Ok(Value::LWord(val))
        }
        TypeId::LWORD => match value {
            Value::LWord(val) => Ok(Value::LWord(val)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        TypeId::LREAL => {
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
