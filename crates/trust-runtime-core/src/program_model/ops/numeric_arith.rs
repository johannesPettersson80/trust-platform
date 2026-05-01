fn numeric_arith(op: BinaryOp, left: Value, right: Value) -> Result<Value, RuntimeError> {
    let left_kind = numeric_kind(&left).ok_or(RuntimeError::TypeMismatch)?;
    let right_kind = numeric_kind(&right).ok_or(RuntimeError::TypeMismatch)?;
    let target = wider_numeric(left_kind, right_kind);
    match target {
        NumericKind::Real | NumericKind::LReal => {
            if matches!(op, BinaryOp::Mod) {
                return Err(RuntimeError::TypeMismatch);
            }
            let a = to_f64(&left)?;
            let b = to_f64(&right)?;
            if matches!(op, BinaryOp::Div) && b == 0.0 {
                return Err(RuntimeError::DivisionByZero);
            }
            let result = match op {
                BinaryOp::Add => a + b,
                BinaryOp::Sub => a - b,
                BinaryOp::Mul => a * b,
                BinaryOp::Div => a / b,
                BinaryOp::Pow => a.powf(b),
                _ => return Err(RuntimeError::TypeMismatch),
            };
            if !result.is_finite() {
                return Err(RuntimeError::Overflow);
            }
            Ok(match target {
                NumericKind::Real => Value::Real(result as f32),
                NumericKind::LReal => Value::LReal(result),
                _ => unreachable!(),
            })
        }
        NumericKind::SInt | NumericKind::Int | NumericKind::DInt | NumericKind::LInt => {
            let a = i128::from(to_i64(&left)?);
            let b = i128::from(to_i64(&right)?);
            let result = match op {
                BinaryOp::Add => a + b,
                BinaryOp::Sub => a - b,
                BinaryOp::Mul => a * b,
                BinaryOp::Div => {
                    if b == 0 {
                        return Err(RuntimeError::DivisionByZero);
                    }
                    a / b
                }
                BinaryOp::Mod => {
                    if b == 0 {
                        return Err(RuntimeError::ModuloByZero);
                    }
                    a % b
                }
                BinaryOp::Pow => {
                    if b < 0 {
                        return Err(RuntimeError::TypeMismatch);
                    }
                    let exp = u32::try_from(b).map_err(|_| RuntimeError::Overflow)?;
                    a.checked_pow(exp).ok_or(RuntimeError::Overflow)?
                }
                _ => return Err(RuntimeError::TypeMismatch),
            };
            signed_from_i128(target, result)
        }
        NumericKind::USInt | NumericKind::UInt | NumericKind::UDInt | NumericKind::ULInt => {
            let a = u128::from(to_u64(&left)?);
            let b = u128::from(to_u64(&right)?);
            let result = match op {
                BinaryOp::Add => a + b,
                BinaryOp::Sub => a.checked_sub(b).ok_or(RuntimeError::Overflow)?,
                BinaryOp::Mul => a * b,
                BinaryOp::Div => {
                    if b == 0 {
                        return Err(RuntimeError::DivisionByZero);
                    }
                    a / b
                }
                BinaryOp::Mod => {
                    if b == 0 {
                        return Err(RuntimeError::ModuloByZero);
                    }
                    a % b
                }
                BinaryOp::Pow => {
                    let exp = u32::try_from(b).map_err(|_| RuntimeError::Overflow)?;
                    a.checked_pow(exp).ok_or(RuntimeError::Overflow)?
                }
                _ => return Err(RuntimeError::TypeMismatch),
            };
            unsigned_from_u128(target, result)
        }
    }
}
