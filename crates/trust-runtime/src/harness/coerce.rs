use smol_str::SmolStr;

use crate::value::{default_value_for_type_id, DateTimeProfile, StructValue, Value};
use indexmap::IndexMap;
use trust_hir::types::{StructField, TypeRegistry, UnionVariant};
use trust_hir::{Type, TypeId};

use super::CompileError;

pub fn coerce_value_to_type(value: Value, type_id: TypeId) -> Result<Value, CompileError> {
    match type_id {
        TypeId::BOOL => match value {
            Value::Bool(_) => Ok(value),
            _ => Err(CompileError::new("expected BOOL initializer")),
        },
        TypeId::SINT | TypeId::INT | TypeId::DINT | TypeId::LINT => coerce_signed(value, type_id),
        TypeId::USINT | TypeId::UINT | TypeId::UDINT | TypeId::ULINT => {
            coerce_unsigned(value, type_id)
        }
        TypeId::BYTE | TypeId::WORD | TypeId::DWORD | TypeId::LWORD => {
            coerce_bitstring(value, type_id)
        }
        TypeId::REAL | TypeId::LREAL => coerce_real(value, type_id),
        TypeId::STRING | TypeId::WSTRING => coerce_string(value, type_id),
        TypeId::CHAR | TypeId::WCHAR => coerce_char(value, type_id),
        TypeId::TIME | TypeId::LTIME => coerce_time(value, type_id),
        TypeId::DATE | TypeId::LDATE => coerce_date(value, type_id),
        TypeId::TOD | TypeId::LTOD => coerce_tod(value, type_id),
        TypeId::DT | TypeId::LDT => coerce_dt(value, type_id),
        _ => Ok(value),
    }
}

pub fn coerce_initializer_value_to_type(
    value: Value,
    type_id: TypeId,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
) -> Result<Value, CompileError> {
    let Some(ty) = registry.get(type_id) else {
        return coerce_value_to_type(value, type_id);
    };
    coerce_initializer_value_to_runtime_type(value, ty, type_id, registry, profile)
}

fn coerce_initializer_value_to_runtime_type(
    value: Value,
    ty: &Type,
    type_id: TypeId,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
) -> Result<Value, CompileError> {
    match ty {
        Type::Alias { target, .. } => {
            let Some(target_ty) = registry.get(*target) else {
                return coerce_value_to_type(value, *target);
            };
            coerce_initializer_value_to_runtime_type(value, target_ty, *target, registry, profile)
        }
        Type::Array {
            element,
            dimensions,
        } => {
            let Value::Array(array) = value else {
                return Err(CompileError::new("expected array initializer"));
            };
            let mut coerced = default_value_for_type_id(type_id, registry, profile)
                .map_err(|_| CompileError::new("default value error for array initializer"))?;
            let Value::Array(ref mut target_array) = coerced else {
                return Err(CompileError::new("expected array initializer"));
            };
            if array.elements().len() > target_array.elements().len() {
                return Err(CompileError::new("too many array initializer elements"));
            }
            if target_array.dimensions() != dimensions.as_slice() {
                target_array
                    .set_dimensions(dimensions.clone())
                    .map_err(|err| {
                        CompileError::new(format!("invalid array initializer shape: {err}"))
                    })?;
            }
            for (slot, element_value) in target_array
                .elements_mut()
                .iter_mut()
                .zip(array.elements().iter())
            {
                *slot = coerce_initializer_value_to_type(
                    element_value.clone(),
                    *element,
                    registry,
                    profile,
                )?;
            }
            Ok(coerced)
        }
        Type::Subrange { base, .. } => {
            coerce_initializer_value_to_type(value, *base, registry, profile)
        }
        Type::Struct { fields, .. } => {
            coerce_struct_initializer(value, type_id, fields, registry, profile)
        }
        Type::Union { variants, .. } => {
            coerce_union_initializer(value, type_id, variants, registry, profile)
        }
        _ => coerce_value_to_type(value, type_id),
    }
}

fn coerce_struct_initializer(
    value: Value,
    type_id: TypeId,
    fields: &[StructField],
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
) -> Result<Value, CompileError> {
    let Value::Struct(input) = value else {
        return Err(CompileError::new("expected struct initializer"));
    };
    let mut values = default_struct_fields(type_id, registry, profile)?;
    for (name, value) in input.fields() {
        let Some(field) = fields
            .iter()
            .find(|field| field.name.eq_ignore_ascii_case(name.as_str()))
        else {
            return Err(CompileError::new(format!(
                "unknown aggregate field '{name}'"
            )));
        };
        let coerced =
            coerce_initializer_value_to_type(value.clone(), field.type_id, registry, profile)?;
        values.insert(field.name.clone(), coerced);
    }
    let value = StructValue::new(registry, type_id, values)
        .map_err(|err| CompileError::new(format!("invalid struct initializer: {err}")))?;
    Ok(Value::Struct(std::sync::Arc::new(value)))
}

fn coerce_union_initializer(
    value: Value,
    type_id: TypeId,
    variants: &[UnionVariant],
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
) -> Result<Value, CompileError> {
    let Value::Struct(input) = value else {
        return Err(CompileError::new("expected union initializer"));
    };
    let mut values = default_struct_fields(type_id, registry, profile)?;
    for (name, value) in input.fields() {
        let Some(variant) = variants
            .iter()
            .find(|variant| variant.name.eq_ignore_ascii_case(name.as_str()))
        else {
            return Err(CompileError::new(format!(
                "unknown aggregate field '{name}'"
            )));
        };
        let coerced =
            coerce_initializer_value_to_type(value.clone(), variant.type_id, registry, profile)?;
        values.insert(variant.name.clone(), coerced);
    }
    let value = StructValue::new(registry, type_id, values)
        .map_err(|err| CompileError::new(format!("invalid union initializer: {err}")))?;
    Ok(Value::Struct(std::sync::Arc::new(value)))
}

fn default_struct_fields(
    type_id: TypeId,
    registry: &TypeRegistry,
    profile: &DateTimeProfile,
) -> Result<IndexMap<SmolStr, Value>, CompileError> {
    let value = default_value_for_type_id(type_id, registry, profile)
        .map_err(|_| CompileError::new("default value error for aggregate initializer"))?;
    let Value::Struct(value) = value else {
        return Err(CompileError::new("expected aggregate default value"));
    };
    Ok(value.fields().clone())
}

fn coerce_signed(value: Value, type_id: TypeId) -> Result<Value, CompileError> {
    let value = match value {
        Value::SInt(v) => v as i64,
        Value::Int(v) => v as i64,
        Value::DInt(v) => v as i64,
        Value::LInt(v) => v,
        Value::USInt(v) => v as i64,
        Value::UInt(v) => v as i64,
        Value::UDInt(v) => v as i64,
        Value::ULInt(v) => {
            i64::try_from(v).map_err(|_| CompileError::new("initializer out of signed range"))?
        }
        _ => return Err(CompileError::new("expected integer initializer")),
    };
    match type_id {
        TypeId::SINT => i8::try_from(value)
            .map(Value::SInt)
            .map_err(|_| CompileError::new("initializer out of SINT range")),
        TypeId::INT => i16::try_from(value)
            .map(Value::Int)
            .map_err(|_| CompileError::new("initializer out of INT range")),
        TypeId::DINT => i32::try_from(value)
            .map(Value::DInt)
            .map_err(|_| CompileError::new("initializer out of DINT range")),
        TypeId::LINT => Ok(Value::LInt(value)),
        _ => Ok(Value::LInt(value)),
    }
}

fn coerce_unsigned(value: Value, type_id: TypeId) -> Result<Value, CompileError> {
    let value =
        match value {
            Value::USInt(v) => v as u64,
            Value::UInt(v) => v as u64,
            Value::UDInt(v) => v as u64,
            Value::ULInt(v) => v,
            Value::SInt(v) => u64::try_from(v)
                .map_err(|_| CompileError::new("initializer out of unsigned range"))?,
            Value::Int(v) => u64::try_from(v)
                .map_err(|_| CompileError::new("initializer out of unsigned range"))?,
            Value::DInt(v) => u64::try_from(v)
                .map_err(|_| CompileError::new("initializer out of unsigned range"))?,
            Value::LInt(v) => u64::try_from(v)
                .map_err(|_| CompileError::new("initializer out of unsigned range"))?,
            _ => return Err(CompileError::new("expected unsigned integer initializer")),
        };
    match type_id {
        TypeId::USINT => u8::try_from(value)
            .map(Value::USInt)
            .map_err(|_| CompileError::new("initializer out of USINT range")),
        TypeId::UINT => u16::try_from(value)
            .map(Value::UInt)
            .map_err(|_| CompileError::new("initializer out of UINT range")),
        TypeId::UDINT => u32::try_from(value)
            .map(Value::UDInt)
            .map_err(|_| CompileError::new("initializer out of UDINT range")),
        TypeId::ULINT => Ok(Value::ULInt(value)),
        _ => Ok(Value::ULInt(value)),
    }
}

fn coerce_bitstring(value: Value, type_id: TypeId) -> Result<Value, CompileError> {
    let value =
        match value {
            Value::Byte(v) => v as u64,
            Value::Word(v) => v as u64,
            Value::DWord(v) => v as u64,
            Value::LWord(v) => v,
            Value::USInt(v) => v as u64,
            Value::UInt(v) => v as u64,
            Value::UDInt(v) => v as u64,
            Value::ULInt(v) => v,
            Value::SInt(v) => u64::try_from(v)
                .map_err(|_| CompileError::new("initializer out of unsigned range"))?,
            Value::Int(v) => u64::try_from(v)
                .map_err(|_| CompileError::new("initializer out of unsigned range"))?,
            Value::DInt(v) => u64::try_from(v)
                .map_err(|_| CompileError::new("initializer out of unsigned range"))?,
            Value::LInt(v) => u64::try_from(v)
                .map_err(|_| CompileError::new("initializer out of unsigned range"))?,
            _ => return Err(CompileError::new("expected integer initializer")),
        };
    match type_id {
        TypeId::BYTE => u8::try_from(value)
            .map(Value::Byte)
            .map_err(|_| CompileError::new("initializer out of BYTE range")),
        TypeId::WORD => u16::try_from(value)
            .map(Value::Word)
            .map_err(|_| CompileError::new("initializer out of WORD range")),
        TypeId::DWORD => u32::try_from(value)
            .map(Value::DWord)
            .map_err(|_| CompileError::new("initializer out of DWORD range")),
        TypeId::LWORD => Ok(Value::LWord(value)),
        _ => Ok(Value::LWord(value)),
    }
}

fn coerce_real(value: Value, type_id: TypeId) -> Result<Value, CompileError> {
    let value = match value {
        Value::Real(v) => v as f64,
        Value::LReal(v) => v,
        Value::SInt(v) => v as f64,
        Value::Int(v) => v as f64,
        Value::DInt(v) => v as f64,
        Value::LInt(v) => v as f64,
        Value::USInt(v) => v as f64,
        Value::UInt(v) => v as f64,
        Value::UDInt(v) => v as f64,
        Value::ULInt(v) => v as f64,
        _ => return Err(CompileError::new("expected numeric initializer")),
    };
    match type_id {
        TypeId::REAL => Ok(Value::Real(value as f32)),
        TypeId::LREAL => Ok(Value::LReal(value)),
        _ => Ok(Value::LReal(value)),
    }
}

fn coerce_string(value: Value, type_id: TypeId) -> Result<Value, CompileError> {
    match type_id {
        TypeId::STRING => match value {
            Value::String(_) => Ok(value),
            Value::WString(w) => Ok(Value::String(SmolStr::new(w))),
            Value::Char(c) => Ok(Value::String(SmolStr::new((c as char).to_string()))),
            _ => Err(CompileError::new("expected STRING initializer")),
        },
        TypeId::WSTRING => match value {
            Value::WString(_) => Ok(value),
            Value::String(s) => Ok(Value::WString(s.to_string())),
            Value::Char(c) => Ok(Value::WString((c as char).to_string())),
            Value::WChar(c) => Ok(Value::WString(
                std::char::from_u32(c as u32)
                    .unwrap_or('\u{FFFD}')
                    .to_string(),
            )),
            _ => Err(CompileError::new("expected WSTRING initializer")),
        },
        _ => Ok(value),
    }
}

fn coerce_char(value: Value, type_id: TypeId) -> Result<Value, CompileError> {
    let to_char = |ch: char| -> Result<Value, CompileError> {
        match type_id {
            TypeId::CHAR => Ok(Value::Char(ch as u8)),
            TypeId::WCHAR => Ok(Value::WChar(ch as u16)),
            _ => Ok(Value::Char(ch as u8)),
        }
    };
    match value {
        Value::Char(_) if type_id == TypeId::CHAR => Ok(value),
        Value::WChar(_) if type_id == TypeId::WCHAR => Ok(value),
        Value::String(s) => {
            let mut chars = s.chars();
            let ch = chars
                .next()
                .ok_or_else(|| CompileError::new("expected single character"))?;
            if chars.next().is_some() {
                return Err(CompileError::new("expected single character"));
            }
            to_char(ch)
        }
        Value::WString(s) => {
            let mut chars = s.chars();
            let ch = chars
                .next()
                .ok_or_else(|| CompileError::new("expected single character"))?;
            if chars.next().is_some() {
                return Err(CompileError::new("expected single character"));
            }
            to_char(ch)
        }
        _ => Err(CompileError::new("expected CHAR initializer")),
    }
}

fn coerce_time(value: Value, type_id: TypeId) -> Result<Value, CompileError> {
    match type_id {
        TypeId::TIME => match value {
            Value::Time(_) => Ok(value),
            Value::LTime(duration) => Ok(Value::Time(duration)),
            _ => Err(CompileError::new("expected TIME initializer")),
        },
        TypeId::LTIME => match value {
            Value::LTime(_) => Ok(value),
            Value::Time(duration) => Ok(Value::LTime(duration)),
            _ => Err(CompileError::new("expected LTIME initializer")),
        },
        _ => Ok(value),
    }
}

fn coerce_date(value: Value, type_id: TypeId) -> Result<Value, CompileError> {
    match type_id {
        TypeId::DATE => match value {
            Value::Date(_) => Ok(value),
            _ => Err(CompileError::new("expected DATE initializer")),
        },
        TypeId::LDATE => match value {
            Value::LDate(_) => Ok(value),
            _ => Err(CompileError::new("expected LDATE initializer")),
        },
        _ => Ok(value),
    }
}

fn coerce_tod(value: Value, type_id: TypeId) -> Result<Value, CompileError> {
    match type_id {
        TypeId::TOD => match value {
            Value::Tod(_) => Ok(value),
            _ => Err(CompileError::new("expected TOD initializer")),
        },
        TypeId::LTOD => match value {
            Value::LTod(_) => Ok(value),
            _ => Err(CompileError::new("expected LTOD initializer")),
        },
        _ => Ok(value),
    }
}

fn coerce_dt(value: Value, type_id: TypeId) -> Result<Value, CompileError> {
    match type_id {
        TypeId::DT => match value {
            Value::Dt(_) => Ok(value),
            _ => Err(CompileError::new("expected DT initializer")),
        },
        TypeId::LDT => match value {
            Value::Ldt(_) => Ok(value),
            _ => Err(CompileError::new("expected LDT initializer")),
        },
        _ => Ok(value),
    }
}
