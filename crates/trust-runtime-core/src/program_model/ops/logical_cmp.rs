fn logical_or_bitwise(op: BinaryOp, left: Value, right: Value) -> Result<Value, RuntimeError> {
    match (left, right) {
        (Value::Bool(a), Value::Bool(b)) => {
            let result = match op {
                BinaryOp::And => a && b,
                BinaryOp::Or => a || b,
                BinaryOp::Xor => a ^ b,
                _ => return Err(RuntimeError::TypeMismatch),
            };
            Ok(Value::Bool(result))
        }
        (Value::Byte(a), Value::Byte(b)) => Ok(Value::Byte(bit_op(op, a, b)?)),
        (Value::Word(a), Value::Word(b)) => Ok(Value::Word(bit_op(op, a, b)?)),
        (Value::DWord(a), Value::DWord(b)) => Ok(Value::DWord(bit_op(op, a, b)?)),
        (Value::LWord(a), Value::LWord(b)) => Ok(Value::LWord(bit_op(op, a, b)?)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn bit_op<T>(op: BinaryOp, left: T, right: T) -> Result<T, RuntimeError>
where
    T: core::ops::BitAnd<Output = T>
        + core::ops::BitOr<Output = T>
        + core::ops::BitXor<Output = T>
        + Copy,
{
    let result = match op {
        BinaryOp::And => left & right,
        BinaryOp::Or => left | right,
        BinaryOp::Xor => left ^ right,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    Ok(result)
}

fn numeric_eq(left: Value, right: Value, is_eq: bool) -> Result<Value, RuntimeError> {
    let left_nullish = matches!(left, Value::Null | Value::Reference(None));
    let right_nullish = matches!(right, Value::Null | Value::Reference(None));
    if left_nullish || right_nullish {
        let matches = left_nullish && right_nullish;
        return Ok(Value::Bool(if is_eq { matches } else { !matches }));
    }

    let left_kind = numeric_kind(&left);
    let right_kind = numeric_kind(&right);
    let Some(left_kind) = left_kind else {
        return Ok(Value::Bool(if is_eq { left == right } else { left != right }));
    };
    let Some(right_kind) = right_kind else {
        return Ok(Value::Bool(if is_eq { left == right } else { left != right }));
    };

    let target = wider_numeric(left_kind, right_kind);
    let matches = match target {
        NumericKind::Real | NumericKind::LReal => {
            let a = to_f64(&left)?;
            let b = to_f64(&right)?;
            a == b
        }
        NumericKind::SInt | NumericKind::Int | NumericKind::DInt | NumericKind::LInt => {
            let a = to_i64(&left)?;
            let b = to_i64(&right)?;
            a == b
        }
        NumericKind::USInt | NumericKind::UInt | NumericKind::UDInt | NumericKind::ULInt => {
            let a = to_u64(&left)?;
            let b = to_u64(&right)?;
            a == b
        }
    };
    Ok(Value::Bool(if is_eq { matches } else { !matches }))
}

fn non_numeric_cmp(
    op: BinaryOp,
    left: &Value,
    right: &Value,
) -> Option<Result<Value, RuntimeError>> {
    let result = match (left, right) {
        (Value::String(a), Value::String(b)) => ord_cmp(op, a.as_str(), b.as_str()),
        (Value::WString(a), Value::WString(b)) => ord_cmp(op, a.as_str(), b.as_str()),
        (Value::Char(a), Value::Char(b)) => ord_cmp(op, *a, *b),
        (Value::WChar(a), Value::WChar(b)) => ord_cmp(op, *a, *b),
        (Value::Bool(a), Value::Bool(b)) => ord_cmp(op, *a as u8, *b as u8),
        (Value::Byte(a), Value::Byte(b)) => ord_cmp(op, *a, *b),
        (Value::Word(a), Value::Word(b)) => ord_cmp(op, *a, *b),
        (Value::DWord(a), Value::DWord(b)) => ord_cmp(op, *a, *b),
        (Value::LWord(a), Value::LWord(b)) => ord_cmp(op, *a, *b),
        _ => return None,
    };
    Some(result)
}

fn ord_cmp<T: Ord>(op: BinaryOp, left: T, right: T) -> Result<Value, RuntimeError> {
    let result = match op {
        BinaryOp::Lt => left < right,
        BinaryOp::Le => left <= right,
        BinaryOp::Gt => left > right,
        BinaryOp::Ge => left >= right,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    Ok(Value::Bool(result))
}

fn numeric_cmp(op: BinaryOp, left: Value, right: Value) -> Result<Value, RuntimeError> {
    let left_kind = numeric_kind(&left).ok_or(RuntimeError::TypeMismatch)?;
    let right_kind = numeric_kind(&right).ok_or(RuntimeError::TypeMismatch)?;
    let target = wider_numeric(left_kind, right_kind);
    let result = match target {
        NumericKind::Real | NumericKind::LReal => {
            let a = to_f64(&left)?;
            let b = to_f64(&right)?;
            match op {
                BinaryOp::Lt => a < b,
                BinaryOp::Le => a <= b,
                BinaryOp::Gt => a > b,
                BinaryOp::Ge => a >= b,
                _ => return Err(RuntimeError::TypeMismatch),
            }
        }
        NumericKind::SInt | NumericKind::Int | NumericKind::DInt | NumericKind::LInt => {
            let a = to_i64(&left)?;
            let b = to_i64(&right)?;
            match op {
                BinaryOp::Lt => a < b,
                BinaryOp::Le => a <= b,
                BinaryOp::Gt => a > b,
                BinaryOp::Ge => a >= b,
                _ => return Err(RuntimeError::TypeMismatch),
            }
        }
        NumericKind::USInt | NumericKind::UInt | NumericKind::UDInt | NumericKind::ULInt => {
            let a = to_u64(&left)?;
            let b = to_u64(&right)?;
            match op {
                BinaryOp::Lt => a < b,
                BinaryOp::Le => a <= b,
                BinaryOp::Gt => a > b,
                BinaryOp::Ge => a >= b,
                _ => return Err(RuntimeError::TypeMismatch),
            }
        }
    };
    Ok(Value::Bool(result))
}
