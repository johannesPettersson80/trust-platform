fn numeric_arith(op: BinaryOp, left: Value, right: Value) -> Result<Value, RuntimeError> {
    let left_kind = numeric_kind(&left).ok_or(RuntimeError::TypeMismatch)?;
    let right_kind = numeric_kind(&right).ok_or(RuntimeError::TypeMismatch)?;
    let target = wider_numeric(left_kind, right_kind).ok_or(RuntimeError::TypeMismatch)?;
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
                BinaryOp::Pow => crate::numeric::math::pow(a, b),
                _ => return Err(RuntimeError::TypeMismatch),
            };
            if !result.is_finite() {
                return Err(RuntimeError::Overflow);
            }
            Ok(match target {
                NumericKind::Real => {
                    let narrowed = result as f32;
                    if !narrowed.is_finite() {
                        return Err(RuntimeError::Overflow);
                    }
                    Value::Real(narrowed)
                }
                NumericKind::LReal => Value::LReal(result),
                _ => unreachable!(),
            })
        }
        NumericKind::SInt | NumericKind::Int | NumericKind::DInt | NumericKind::LInt => {
            let a = to_i64(&left)?;
            let b = to_i64(&right)?;
            let result = match op {
                BinaryOp::Add => a.checked_add(b).ok_or(RuntimeError::Overflow)?,
                BinaryOp::Sub => a.checked_sub(b).ok_or(RuntimeError::Overflow)?,
                BinaryOp::Mul => a.checked_mul(b).ok_or(RuntimeError::Overflow)?,
                BinaryOp::Div => {
                    if b == 0 {
                        return Err(RuntimeError::DivisionByZero);
                    }
                    a.checked_div(b).ok_or(RuntimeError::Overflow)?
                }
                BinaryOp::Mod => {
                    if b == 0 {
                        return Err(RuntimeError::ModuloByZero);
                    }
                    // Wide remainder returns zero for MIN % -1; signed native
                    // remainder would overflow even though that result is valid.
                    if b == -1 {
                        0
                    } else {
                        a % b
                    }
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
            signed_from_i128(target, i128::from(result))
        }
        NumericKind::USInt | NumericKind::UInt | NumericKind::UDInt | NumericKind::ULInt => {
            let a = to_u64(&left)?;
            let b = to_u64(&right)?;
            let result = match op {
                BinaryOp::Add => a.checked_add(b).ok_or(RuntimeError::Overflow)?,
                BinaryOp::Sub => a.checked_sub(b).ok_or(RuntimeError::Overflow)?,
                BinaryOp::Mul => a.checked_mul(b).ok_or(RuntimeError::Overflow)?,
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
            unsigned_from_u128(target, u128::from(result))
        }
    }
}

#[cfg(test)]
mod integer_width_tests {
    use super::*;
    use alloc::vec;
    fn wide_numeric_arith(op: BinaryOp, left: Value, right: Value) -> Result<Value, RuntimeError> {
        let left_kind = numeric_kind(&left).ok_or(RuntimeError::TypeMismatch)?;
        let right_kind = numeric_kind(&right).ok_or(RuntimeError::TypeMismatch)?;
        let target = wider_numeric(left_kind, right_kind).ok_or(RuntimeError::TypeMismatch)?;
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
                    BinaryOp::Pow => crate::numeric::math::pow(a, b),
                    _ => return Err(RuntimeError::TypeMismatch),
                };
                if !result.is_finite() {
                    return Err(RuntimeError::Overflow);
                }
                Ok(match target {
                    NumericKind::Real => {
                        let narrowed = result as f32;
                        if !narrowed.is_finite() {
                            return Err(RuntimeError::Overflow);
                        }
                        Value::Real(narrowed)
                    }
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

    #[test]
    fn checked_integer_operations_match_wide_results_for_all_widths_and_mixed_kinds() {
        let mut values = vec![
            Value::SInt(i8::MIN),
            Value::SInt(-1),
            Value::SInt(0),
            Value::SInt(1),
            Value::SInt(i8::MAX),
            Value::Int(i16::MIN),
            Value::Int(-1),
            Value::Int(0),
            Value::Int(1),
            Value::Int(i16::MAX),
            Value::DInt(i32::MIN),
            Value::DInt(-1),
            Value::DInt(0),
            Value::DInt(1),
            Value::DInt(i32::MAX),
            Value::LInt(i64::MIN),
            Value::LInt(i64::MIN + 1),
            Value::LInt(-1),
            Value::LInt(0),
            Value::LInt(1),
            Value::LInt(i64::MAX),
            Value::USInt(0),
            Value::USInt(1),
            Value::USInt(u8::MAX),
            Value::UInt(0),
            Value::UInt(1),
            Value::UInt(u16::MAX),
            Value::UDInt(0),
            Value::UDInt(1),
            Value::UDInt(u32::MAX),
            Value::ULInt(0),
            Value::ULInt(1),
            Value::ULInt(u64::MAX),
            Value::LInt(i64::from(u32::MAX) + 1),
            Value::ULInt(u64::from(u32::MAX) + 1),
            Value::Real(-0.0),
            Value::Real(0.5),
            Value::Real(f32::INFINITY),
            Value::LReal(-0.5),
            Value::LReal(f64::NAN),
            Value::Bool(false),
            Value::Null,
        ];
        // Integer power/round-trip boundaries and multiplication square roots.
        for n in [
            2,
            3,
            7,
            8,
            15,
            16,
            31,
            32,
            63,
            64,
            127,
            128,
            255,
            256,
            65535,
            65536,
            3_037_000_499,
            3_037_000_500,
        ] {
            values.push(Value::LInt(n));
            values.push(Value::ULInt(n as u64));
        }
        let operations = [
            BinaryOp::Add,
            BinaryOp::Sub,
            BinaryOp::Mul,
            BinaryOp::Div,
            BinaryOp::Mod,
            BinaryOp::Pow,
            BinaryOp::And,
        ];
        for left in &values {
            for right in &values {
                for op in operations {
                    assert_eq!(
                        numeric_arith(op, left.clone(), right.clone()),
                        wide_numeric_arith(op, left.clone(), right.clone()),
                        "left={left:?}, right={right:?}, op={op:?}"
                    );
                }
            }
        }
        assert_eq!(
            numeric_arith(BinaryOp::Mod, Value::LInt(i64::MIN), Value::LInt(-1)),
            Ok(Value::LInt(0))
        );
        assert_eq!(
            numeric_arith(BinaryOp::Div, Value::LInt(i64::MIN), Value::LInt(-1)),
            Err(RuntimeError::Overflow)
        );
        assert_eq!(
            numeric_arith(BinaryOp::Mod, Value::LInt(i64::MIN), Value::LInt(0)),
            Err(RuntimeError::ModuloByZero)
        );
        assert_eq!(
            numeric_arith(BinaryOp::Pow, Value::LInt(0), Value::LInt(-1)),
            Err(RuntimeError::TypeMismatch)
        );
        assert_eq!(
            numeric_arith(BinaryOp::Pow, Value::ULInt(1), Value::ULInt(u64::MAX)),
            Err(RuntimeError::Overflow)
        );
    }
}
