use crate::value::Value;
use trust_hir::{Type, TypeId};

use crate::bytecode::BYTECODE_MAX_CONST_NESTING;

use super::{BytecodeEncoder, BytecodeError, ConstEntry};

impl<'a> BytecodeEncoder<'a> {
    pub(super) fn const_index_for(&mut self, value: &Value) -> Result<u32, BytecodeError> {
        let type_id = match value {
            Value::Enum(enum_value) => self
                .runtime
                .registry()
                .lookup(enum_value.type_name().as_str())
                .ok_or_else(|| {
                    BytecodeError::InvalidSection(
                        format!("unsupported const enum type '{}'", enum_value.type_name()).into(),
                    )
                })?,
            _ => type_id_for_value(value)
                .ok_or_else(|| BytecodeError::InvalidSection("unsupported const value".into()))?,
        };
        self.const_index_for_type(value, type_id)
    }

    pub(super) fn const_index_for_type(
        &mut self,
        value: &Value,
        type_id: TypeId,
    ) -> Result<u32, BytecodeError> {
        let type_idx = self.type_index(type_id)?;
        let payload = self.encode_const_payload_for_type(value, type_id, 0)?;
        let idx = self.const_pool.len() as u32;
        self.const_pool.push(ConstEntry {
            type_id: type_idx,
            payload,
        });
        Ok(idx)
    }

    fn encode_const_payload_for_type(
        &mut self,
        value: &Value,
        type_id: TypeId,
        depth: u8,
    ) -> Result<Vec<u8>, BytecodeError> {
        if depth > BYTECODE_MAX_CONST_NESTING {
            return Err(BytecodeError::InvalidSection(
                "const payload type recursion overflow".into(),
            ));
        }
        let ty = self
            .runtime
            .registry()
            .get(type_id)
            .cloned()
            .ok_or_else(|| BytecodeError::InvalidSection("unknown const type id".into()))?;
        match ty {
            Type::Alias { target, .. } | Type::Subrange { base: target, .. } => {
                self.encode_const_payload_for_type(value, target, depth + 1)
            }
            Type::Enum { base, .. } if self.runtime.registry().is_named_value_type(type_id) => {
                self.encode_const_payload_for_type(value, base, depth + 1)
            }
            Type::Array {
                element,
                dimensions,
            } => {
                let Value::Array(array) = value else {
                    return Err(const_payload_type_mismatch("ARRAY", value));
                };
                if array.dimensions() != dimensions {
                    return Err(BytecodeError::InvalidSection(
                        "const ARRAY dimensions mismatch".into(),
                    ));
                }
                let mut payload = u32::try_from(array.elements().len())
                    .map_err(|_| BytecodeError::InvalidSection("const ARRAY too large".into()))?
                    .to_le_bytes()
                    .to_vec();
                for element_value in array.elements() {
                    let child =
                        self.encode_const_payload_for_type(element_value, element, depth + 1)?;
                    push_child_payload(&mut payload, &child)?;
                }
                Ok(payload)
            }
            Type::Struct { fields, .. } => {
                let Value::Struct(struct_value) = value else {
                    return Err(const_payload_type_mismatch("STRUCT", value));
                };
                let mut payload = u32::try_from(fields.len())
                    .map_err(|_| BytecodeError::InvalidSection("const STRUCT too large".into()))?
                    .to_le_bytes()
                    .to_vec();
                for field in fields {
                    let field_value = struct_value
                        .fields()
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(field.name.as_str()))
                        .map(|(_, value)| value)
                        .ok_or_else(|| {
                            BytecodeError::InvalidSection(
                                format!("const STRUCT missing field '{}'", field.name).into(),
                            )
                        })?;
                    let child =
                        self.encode_const_payload_for_type(field_value, field.type_id, depth + 1)?;
                    push_child_payload(&mut payload, &child)?;
                }
                Ok(payload)
            }
            Type::Union { variants, .. } => {
                let Value::Struct(union_value) = value else {
                    return Err(const_payload_type_mismatch("UNION", value));
                };
                let mut payload = u32::try_from(variants.len())
                    .map_err(|_| BytecodeError::InvalidSection("const UNION too large".into()))?
                    .to_le_bytes()
                    .to_vec();
                for variant in variants {
                    let variant_value = union_value
                        .fields()
                        .iter()
                        .find(|(name, _)| name.eq_ignore_ascii_case(variant.name.as_str()))
                        .map(|(_, value)| value)
                        .ok_or_else(|| {
                            BytecodeError::InvalidSection(
                                format!("const UNION missing field '{}'", variant.name).into(),
                            )
                        })?;
                    let child = self.encode_const_payload_for_type(
                        variant_value,
                        variant.type_id,
                        depth + 1,
                    )?;
                    push_child_payload(&mut payload, &child)?;
                }
                Ok(payload)
            }
            Type::Reference { .. } | Type::Pointer { .. } => match value {
                Value::Reference(None) | Value::Null => Ok(u32::MAX.to_le_bytes().to_vec()),
                Value::Reference(Some(_)) => Err(BytecodeError::InvalidSection(
                    "non-NULL reference constant is unsupported".into(),
                )),
                _ => Err(const_payload_type_mismatch("REFERENCE", value)),
            },
            Type::Bool
            | Type::SInt
            | Type::Int
            | Type::DInt
            | Type::LInt
            | Type::USInt
            | Type::UInt
            | Type::UDInt
            | Type::ULInt
            | Type::Real
            | Type::LReal
            | Type::Byte
            | Type::Word
            | Type::DWord
            | Type::LWord
            | Type::Time
            | Type::LTime
            | Type::Date
            | Type::LDate
            | Type::Tod
            | Type::LTod
            | Type::Dt
            | Type::Ldt
            | Type::String { .. }
            | Type::WString { .. }
            | Type::Char
            | Type::WChar
            | Type::Enum { .. } => encode_const_payload(value),
            _ => Err(BytecodeError::InvalidSection(
                "unsupported const payload type".into(),
            )),
        }
    }
}

fn push_child_payload(payload: &mut Vec<u8>, child: &[u8]) -> Result<(), BytecodeError> {
    let len = u32::try_from(child.len())
        .map_err(|_| BytecodeError::InvalidSection("const child payload too large".into()))?;
    payload.extend_from_slice(&len.to_le_bytes());
    payload.extend_from_slice(child);
    Ok(())
}

fn const_payload_type_mismatch(expected: &str, value: &Value) -> BytecodeError {
    BytecodeError::InvalidSection(
        format!("const payload expected {expected}, got {value:?}").into(),
    )
}

pub(super) fn type_id_for_value(value: &Value) -> Option<TypeId> {
    match value {
        Value::Bool(_) => Some(TypeId::BOOL),
        Value::SInt(_) => Some(TypeId::SINT),
        Value::Int(_) => Some(TypeId::INT),
        Value::DInt(_) => Some(TypeId::DINT),
        Value::LInt(_) => Some(TypeId::LINT),
        Value::USInt(_) => Some(TypeId::USINT),
        Value::UInt(_) => Some(TypeId::UINT),
        Value::UDInt(_) => Some(TypeId::UDINT),
        Value::ULInt(_) => Some(TypeId::ULINT),
        Value::Real(_) => Some(TypeId::REAL),
        Value::LReal(_) => Some(TypeId::LREAL),
        Value::Byte(_) => Some(TypeId::BYTE),
        Value::Word(_) => Some(TypeId::WORD),
        Value::DWord(_) => Some(TypeId::DWORD),
        Value::LWord(_) => Some(TypeId::LWORD),
        Value::Char(_) => Some(TypeId::CHAR),
        Value::WChar(_) => Some(TypeId::WCHAR),
        Value::String(_) => Some(TypeId::STRING),
        Value::WString(_) => Some(TypeId::WSTRING),
        Value::Time(_) => Some(TypeId::TIME),
        Value::LTime(_) => Some(TypeId::LTIME),
        Value::Date(_) => Some(TypeId::DATE),
        Value::LDate(_) => Some(TypeId::LDATE),
        Value::Tod(_) => Some(TypeId::TOD),
        Value::LTod(_) => Some(TypeId::LTOD),
        Value::Dt(_) => Some(TypeId::DT),
        Value::Ldt(_) => Some(TypeId::LDT),
        Value::Enum(_) => Some(TypeId::INT),
        _ => None,
    }
}

fn encode_const_payload(value: &Value) -> Result<Vec<u8>, BytecodeError> {
    let mut payload = Vec::new();
    match value {
        Value::Bool(v) => payload.push(u8::from(*v)),
        Value::SInt(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::Int(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::DInt(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::LInt(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::USInt(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::UInt(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::UDInt(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::ULInt(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::Real(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::LReal(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::Byte(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::Word(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::DWord(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::LWord(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::Char(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::WChar(v) => payload.extend_from_slice(&v.to_le_bytes()),
        Value::String(value) => {
            payload.extend_from_slice(value.as_bytes());
        }
        Value::WString(value) => {
            for unit in value.encode_utf16() {
                payload.extend_from_slice(&unit.to_le_bytes());
            }
        }
        Value::Time(value) | Value::LTime(value) => {
            payload.extend_from_slice(&value.as_nanos().to_le_bytes());
        }
        Value::Date(value) => {
            payload.extend_from_slice(&value.ticks().to_le_bytes());
        }
        Value::LDate(value) => {
            payload.extend_from_slice(&value.nanos().to_le_bytes());
        }
        Value::Tod(value) => {
            payload.extend_from_slice(&value.ticks().to_le_bytes());
        }
        Value::LTod(value) => {
            payload.extend_from_slice(&value.nanos().to_le_bytes());
        }
        Value::Dt(value) => {
            payload.extend_from_slice(&value.ticks().to_le_bytes());
        }
        Value::Ldt(value) => {
            payload.extend_from_slice(&value.nanos().to_le_bytes());
        }
        Value::Enum(value) => {
            payload.extend_from_slice(&value.numeric_value().to_le_bytes());
        }
        _ => {
            return Err(BytecodeError::InvalidSection(
                "unsupported const payload".into(),
            ));
        }
    }
    Ok(payload)
}
