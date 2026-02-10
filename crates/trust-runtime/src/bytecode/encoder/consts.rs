use crate::value::Value;
use trust_hir::TypeId;

use super::{BytecodeEncoder, BytecodeError, ConstEntry};

impl<'a> BytecodeEncoder<'a> {
    pub(super) fn const_index_for(&mut self, value: &Value) -> Result<u32, BytecodeError> {
        let type_id = type_id_for_value(value)
            .ok_or_else(|| BytecodeError::InvalidSection("unsupported const value".into()))?;
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
        expr: &crate::eval::expr::Expr,
    ) -> Result<Value, BytecodeError> {
        if !const_expr_supported(expr) {
            return Err(BytecodeError::InvalidSection(
                "unsupported const expression".into(),
            ));
        }
        let mut storage = crate::memory::VariableStorage::default();
        let mut ctx = crate::eval::EvalContext {
            storage: &mut storage,
            registry: self.runtime.registry(),
            profile: self.runtime.profile(),
            now: crate::value::Duration::ZERO,
            debug: None,
            call_depth: 0,
            functions: None,
            stdlib: None,
            function_blocks: None,
            classes: None,
            using: None,
            access: None,
            current_instance: None,
            return_name: None,
            loop_depth: 0,
            pause_requested: false,
            execution_deadline: None,
        };
        crate::eval::expr::eval_expr(&mut ctx, expr)
            .map_err(|_| BytecodeError::InvalidSection("unsupported const expression".into()))
    }
}

fn const_expr_supported(expr: &crate::eval::expr::Expr) -> bool {
    use crate::eval::expr::Expr;
    use crate::eval::ops::{BinaryOp, UnaryOp};
    match expr {
        Expr::Literal(value) => {
            if matches!(value, Value::String(_) | Value::WString(_)) {
                return false;
            }
            type_id_for_value(value).is_some()
        }
        Expr::Unary { op, expr } => {
            matches!(op, UnaryOp::Neg | UnaryOp::Not | UnaryOp::Pos) && const_expr_supported(expr)
        }
        Expr::Binary { op, left, right } => {
            matches!(
                op,
                BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::Div
                    | BinaryOp::Mod
                    | BinaryOp::Pow
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Xor
                    | BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
            ) && const_expr_supported(left)
                && const_expr_supported(right)
        }
        _ => false,
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
            return Err(BytecodeError::InvalidSection(
                format!("string const not supported yet: {value}").into(),
            ));
        }
        Value::WString(_) => {
            return Err(BytecodeError::InvalidSection(
                "wstring const not supported yet".into(),
            ));
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
            payload.extend_from_slice(&value.numeric_value.to_le_bytes());
        }
        _ => {
            return Err(BytecodeError::InvalidSection(
                "unsupported const payload".into(),
            ));
        }
    }
    Ok(payload)
}
