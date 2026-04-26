use crate::value::Value;
use trust_hir::TypeId;

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
        let type_idx = self.type_index(type_id)?;
        let payload = encode_const_payload(value)?;
        let idx = self.const_pool.len() as u32;
        self.const_pool.push(ConstEntry {
            type_id: type_idx,
            payload,
        });
        Ok(idx)
    }

    pub(super) fn const_value_from_expr(
        &self,
        expr: &crate::program_model::Expr,
    ) -> Result<Value, BytecodeError> {
        crate::helper_eval::eval_const_expr(expr, &self.runtime.profile()).map_err(|err| {
            let message = match err {
                crate::helper_eval::ConstExprError::UnsupportedExpr => {
                    "unsupported const expression".to_string()
                }
                crate::helper_eval::ConstExprError::Runtime(runtime) => {
                    format!("unsupported const expression: {runtime}")
                }
            };
            BytecodeError::InvalidSection(message.into())
        })
    }
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
