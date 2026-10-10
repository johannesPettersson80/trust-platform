fn time_arith(
    op: BinaryOp,
    left: &Value,
    right: &Value,
    profile: &DateTimeProfile,
) -> Option<Result<Value, RuntimeError>> {
    match (left, right) {
        (Value::Time(lhs), Value::Time(rhs)) if matches!(op, BinaryOp::Add | BinaryOp::Sub) => {
            return Some(time_duration_op(op, *lhs, *rhs).map(Value::Time));
        }
        (Value::LTime(lhs), Value::LTime(rhs)) if matches!(op, BinaryOp::Add | BinaryOp::Sub) => {
            return Some(time_duration_op(op, *lhs, *rhs).map(Value::LTime));
        }
        (Value::Tod(lhs), Value::Time(rhs)) if matches!(op, BinaryOp::Add | BinaryOp::Sub) => {
            return Some(time_of_day_with_time(op, *lhs, *rhs, profile).map(Value::Tod));
        }
        (Value::Time(lhs), Value::Tod(rhs)) if matches!(op, BinaryOp::Add) => {
            return Some(time_of_day_with_time(op, *rhs, *lhs, profile).map(Value::Tod));
        }
        (Value::LTod(lhs), Value::LTime(rhs)) if matches!(op, BinaryOp::Add | BinaryOp::Sub) => {
            return Some(long_tod_with_time(op, *lhs, *rhs).map(Value::LTod));
        }
        (Value::LTime(lhs), Value::LTod(rhs)) if matches!(op, BinaryOp::Add) => {
            return Some(long_tod_with_time(op, *rhs, *lhs).map(Value::LTod));
        }
        (Value::Dt(lhs), Value::Time(rhs)) if matches!(op, BinaryOp::Add | BinaryOp::Sub) => {
            return Some(datetime_with_time(op, *lhs, *rhs, profile).map(Value::Dt));
        }
        (Value::Time(lhs), Value::Dt(rhs)) if matches!(op, BinaryOp::Add) => {
            return Some(datetime_with_time(op, *rhs, *lhs, profile).map(Value::Dt));
        }
        (Value::Ldt(lhs), Value::LTime(rhs)) if matches!(op, BinaryOp::Add | BinaryOp::Sub) => {
            return Some(long_datetime_with_time(op, *lhs, *rhs).map(Value::Ldt));
        }
        (Value::LTime(lhs), Value::Ldt(rhs)) if matches!(op, BinaryOp::Add) => {
            return Some(long_datetime_with_time(op, *rhs, *lhs).map(Value::Ldt));
        }
        (Value::Date(lhs), Value::Date(rhs)) if matches!(op, BinaryOp::Sub) => {
            return Some(date_diff(*lhs, *rhs, profile).map(Value::Time));
        }
        (Value::LDate(lhs), Value::LDate(rhs)) if matches!(op, BinaryOp::Sub) => {
            return Some(long_date_diff(*lhs, *rhs).map(Value::LTime));
        }
        (Value::Tod(lhs), Value::Tod(rhs)) if matches!(op, BinaryOp::Sub) => {
            return Some(tod_diff(*lhs, *rhs, profile).map(Value::Time));
        }
        (Value::LTod(lhs), Value::LTod(rhs)) if matches!(op, BinaryOp::Sub) => {
            return Some(long_tod_diff(*lhs, *rhs).map(Value::LTime));
        }
        (Value::Dt(lhs), Value::Dt(rhs)) if matches!(op, BinaryOp::Sub) => {
            return Some(dt_diff(*lhs, *rhs, profile).map(Value::Time));
        }
        (Value::Ldt(lhs), Value::Ldt(rhs)) if matches!(op, BinaryOp::Sub) => {
            return Some(long_dt_diff(*lhs, *rhs).map(Value::LTime));
        }
        _ => {}
    }

    if matches!(op, BinaryOp::Mul | BinaryOp::Div) {
        if let Some(result) = time_scale(op, left, right) {
            return Some(result);
        }
    }

    None
}

fn time_cmp(op: BinaryOp, left: &Value, right: &Value) -> Option<Result<Value, RuntimeError>> {
    if !matches!(
        op,
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
    ) {
        return None;
    }
    let result = match (left, right) {
        (Value::Time(lhs), Value::Time(rhs)) => time_cmp_values(op, lhs.as_nanos(), rhs.as_nanos()),
        (Value::LTime(lhs), Value::LTime(rhs)) => {
            time_cmp_values(op, lhs.as_nanos(), rhs.as_nanos())
        }
        (Value::Date(lhs), Value::Date(rhs)) => time_cmp_values(op, lhs.ticks(), rhs.ticks()),
        (Value::LDate(lhs), Value::LDate(rhs)) => time_cmp_values(op, lhs.nanos(), rhs.nanos()),
        (Value::Tod(lhs), Value::Tod(rhs)) => time_cmp_values(op, lhs.ticks(), rhs.ticks()),
        (Value::LTod(lhs), Value::LTod(rhs)) => time_cmp_values(op, lhs.nanos(), rhs.nanos()),
        (Value::Dt(lhs), Value::Dt(rhs)) => time_cmp_values(op, lhs.ticks(), rhs.ticks()),
        (Value::Ldt(lhs), Value::Ldt(rhs)) => time_cmp_values(op, lhs.nanos(), rhs.nanos()),
        _ => return None,
    };
    Some(result.map(Value::Bool))
}

fn time_cmp_values(op: BinaryOp, lhs: i64, rhs: i64) -> Result<bool, RuntimeError> {
    let result = match op {
        BinaryOp::Lt => lhs < rhs,
        BinaryOp::Le => lhs <= rhs,
        BinaryOp::Gt => lhs > rhs,
        BinaryOp::Ge => lhs >= rhs,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    Ok(result)
}

fn time_duration_op(op: BinaryOp, lhs: Duration, rhs: Duration) -> Result<Duration, RuntimeError> {
    let lhs = i128::from(lhs.as_nanos());
    let rhs = i128::from(rhs.as_nanos());
    let result = match op {
        BinaryOp::Add => lhs + rhs,
        BinaryOp::Sub => lhs - rhs,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let nanos = i64::try_from(result).map_err(|_| RuntimeError::Overflow)?;
    Ok(Duration::from_nanos(nanos))
}

fn time_of_day_with_time(
    op: BinaryOp,
    tod: TimeOfDayValue,
    time: Duration,
    profile: &DateTimeProfile,
) -> Result<TimeOfDayValue, RuntimeError> {
    let delta_ticks = duration_to_ticks(time, profile)?;
    let base = i128::from(tod.ticks());
    let result = match op {
        BinaryOp::Add => base + i128::from(delta_ticks),
        BinaryOp::Sub => base - i128::from(delta_ticks),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    TimeOfDayValue::try_from_ticks(result).map_err(RuntimeError::from)
}

fn long_tod_with_time(
    op: BinaryOp,
    tod: LTimeOfDayValue,
    time: Duration,
) -> Result<LTimeOfDayValue, RuntimeError> {
    let base = i128::from(tod.nanos());
    let delta = i128::from(time.as_nanos());
    let result = match op {
        BinaryOp::Add => base + delta,
        BinaryOp::Sub => base - delta,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let nanos = i64::try_from(result).map_err(|_| RuntimeError::Overflow)?;
    Ok(LTimeOfDayValue::new(nanos))
}

fn datetime_with_time(
    op: BinaryOp,
    dt: DateTimeValue,
    time: Duration,
    profile: &DateTimeProfile,
) -> Result<DateTimeValue, RuntimeError> {
    let delta_ticks = duration_to_ticks(time, profile)?;
    let base = i128::from(dt.ticks());
    let result = match op {
        BinaryOp::Add => base + i128::from(delta_ticks),
        BinaryOp::Sub => base - i128::from(delta_ticks),
        _ => return Err(RuntimeError::TypeMismatch),
    };
    DateTimeValue::try_from_ticks(result).map_err(RuntimeError::from)
}

fn long_datetime_with_time(
    op: BinaryOp,
    dt: LDateTimeValue,
    time: Duration,
) -> Result<LDateTimeValue, RuntimeError> {
    let base = i128::from(dt.nanos());
    let delta = i128::from(time.as_nanos());
    let result = match op {
        BinaryOp::Add => base + delta,
        BinaryOp::Sub => base - delta,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let nanos = i64::try_from(result).map_err(|_| RuntimeError::Overflow)?;
    Ok(LDateTimeValue::new(nanos))
}

fn date_diff(
    lhs: DateValue,
    rhs: DateValue,
    profile: &DateTimeProfile,
) -> Result<Duration, RuntimeError> {
    let diff = i128::from(lhs.ticks()) - i128::from(rhs.ticks());
    ticks_to_duration(diff, profile)
}

fn long_date_diff(lhs: LDateValue, rhs: LDateValue) -> Result<Duration, RuntimeError> {
    let diff = i128::from(lhs.nanos()) - i128::from(rhs.nanos());
    let nanos = i64::try_from(diff).map_err(|_| RuntimeError::Overflow)?;
    Ok(Duration::from_nanos(nanos))
}

fn tod_diff(
    lhs: TimeOfDayValue,
    rhs: TimeOfDayValue,
    profile: &DateTimeProfile,
) -> Result<Duration, RuntimeError> {
    let diff = i128::from(lhs.ticks()) - i128::from(rhs.ticks());
    ticks_to_duration(diff, profile)
}

fn long_tod_diff(lhs: LTimeOfDayValue, rhs: LTimeOfDayValue) -> Result<Duration, RuntimeError> {
    let diff = i128::from(lhs.nanos()) - i128::from(rhs.nanos());
    let nanos = i64::try_from(diff).map_err(|_| RuntimeError::Overflow)?;
    Ok(Duration::from_nanos(nanos))
}

fn dt_diff(
    lhs: DateTimeValue,
    rhs: DateTimeValue,
    profile: &DateTimeProfile,
) -> Result<Duration, RuntimeError> {
    let diff = i128::from(lhs.ticks()) - i128::from(rhs.ticks());
    ticks_to_duration(diff, profile)
}

fn long_dt_diff(lhs: LDateTimeValue, rhs: LDateTimeValue) -> Result<Duration, RuntimeError> {
    let diff = i128::from(lhs.nanos()) - i128::from(rhs.nanos());
    let nanos = i64::try_from(diff).map_err(|_| RuntimeError::Overflow)?;
    Ok(Duration::from_nanos(nanos))
}

fn time_scale(op: BinaryOp, left: &Value, right: &Value) -> Option<Result<Value, RuntimeError>> {
    match (left, right) {
        (Value::Time(time), rhs) => {
            return Some(scale_duration(*time, rhs, op).map(Value::Time));
        }
        (lhs, Value::Time(time)) if matches!(op, BinaryOp::Mul) => {
            return Some(scale_duration(*time, lhs, op).map(Value::Time));
        }
        (Value::LTime(time), rhs) => {
            return Some(scale_duration(*time, rhs, op).map(Value::LTime));
        }
        (lhs, Value::LTime(time)) if matches!(op, BinaryOp::Mul) => {
            return Some(scale_duration(*time, lhs, op).map(Value::LTime));
        }
        _ => {}
    }
    None
}

fn scale_duration(time: Duration, factor: &Value, op: BinaryOp) -> Result<Duration, RuntimeError> {
    let factor = numeric_factor(factor)?;
    let nanos = time.as_nanos();
    let result = match factor {
        NumericFactor::Signed(value) => match op {
            BinaryOp::Mul => nanos.checked_mul(value).ok_or(RuntimeError::Overflow)?,
            BinaryOp::Div => {
                if value == 0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                nanos.checked_div(value).ok_or(RuntimeError::Overflow)?
            }
            _ => return Err(RuntimeError::TypeMismatch),
        },
        NumericFactor::Unsigned(value) => {
            let magnitude = match op {
                BinaryOp::Mul => nanos
                    .unsigned_abs()
                    .checked_mul(value)
                    .ok_or(RuntimeError::Overflow)?,
                BinaryOp::Div => {
                    if value == 0 {
                        return Err(RuntimeError::DivisionByZero);
                    }
                    nanos.unsigned_abs() / value
                }
                _ => return Err(RuntimeError::TypeMismatch),
            };
            // The one magnitude beyond i64::MAX is representable only when
            // negative. No unsigned factor is truncated, including ULINT_MAX.
            if nanos < 0 && magnitude == (1u64 << 63) {
                i64::MIN
            } else {
                let magnitude = i64::try_from(magnitude).map_err(|_| RuntimeError::Overflow)?;
                if nanos < 0 {
                    -magnitude
                } else {
                    magnitude
                }
            }
        }
        NumericFactor::Real(value) => {
            if matches!(op, BinaryOp::Div) && value == 0.0 {
                return Err(RuntimeError::DivisionByZero);
            }
            let result = match op {
                BinaryOp::Mul => (nanos as f64) * value,
                BinaryOp::Div => (nanos as f64) / value,
                _ => return Err(RuntimeError::TypeMismatch),
            };
            let truncated = crate::numeric::math::trunc(result);
            if !truncated.is_finite() {
                return Err(RuntimeError::Overflow);
            }
            if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&truncated) {
                return Err(RuntimeError::Overflow);
            }
            return Ok(Duration::from_nanos(truncated as i64));
        }
    };
    Ok(Duration::from_nanos(result))
}

enum NumericFactor {
    Signed(i64),
    Unsigned(u64),
    Real(f64),
}

fn numeric_factor(value: &Value) -> Result<NumericFactor, RuntimeError> {
    match value {
        Value::Real(v) => Ok(NumericFactor::Real(*v as f64)),
        Value::LReal(v) => Ok(NumericFactor::Real(*v)),
        Value::SInt(v) => Ok(NumericFactor::Signed(i64::from(*v))),
        Value::Int(v) => Ok(NumericFactor::Signed(i64::from(*v))),
        Value::DInt(v) => Ok(NumericFactor::Signed(i64::from(*v))),
        Value::LInt(v) => Ok(NumericFactor::Signed(*v)),
        Value::USInt(v) => Ok(NumericFactor::Unsigned(u64::from(*v))),
        Value::UInt(v) => Ok(NumericFactor::Unsigned(u64::from(*v))),
        Value::UDInt(v) => Ok(NumericFactor::Unsigned(u64::from(*v))),
        Value::ULInt(v) => Ok(NumericFactor::Unsigned(*v)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn duration_to_ticks(time: Duration, profile: &DateTimeProfile) -> Result<i64, RuntimeError> {
    let resolution = profile.resolution.as_nanos();
    if resolution == 0 {
        return Err(RuntimeError::Overflow);
    }
    time.as_nanos()
        .checked_div(resolution)
        .ok_or(RuntimeError::Overflow)
}

fn ticks_to_duration(ticks: i128, profile: &DateTimeProfile) -> Result<Duration, RuntimeError> {
    let nanos = ticks
        .checked_mul(i128::from(profile.resolution.as_nanos()))
        .ok_or(RuntimeError::Overflow)?;
    let nanos = i64::try_from(nanos).map_err(|_| RuntimeError::Overflow)?;
    Ok(Duration::from_nanos(nanos))
}

#[cfg(test)]
mod width_tests {
    use super::*;

    #[test]
    fn narrow_tick_division_matches_wide_arithmetic_including_minimum_duration() {
        for nanos in [i64::MIN, -1, 0, 1, i64::MAX] {
            for resolution in [i64::MIN, -1, 0, 1, 1000, i64::MAX] {
                let profile = DateTimeProfile {
                    resolution: Duration::from_nanos(resolution),
                    ..Default::default()
                };
                let expected = if resolution == 0 {
                    Err(RuntimeError::Overflow)
                } else {
                    i64::try_from(i128::from(nanos) / i128::from(resolution))
                        .map_err(|_| RuntimeError::Overflow)
                };
                assert_eq!(
                    duration_to_ticks(Duration::from_nanos(nanos), &profile),
                    expected
                );
            }
        }
    }

    #[test]
    fn real_duration_scaling_keeps_wide_conversion_bounds_and_fault_precedence() {
        for nanos in [i64::MIN, -1, 0, 1, i64::MAX] {
            for factor in [
                f64::NEG_INFINITY,
                f64::NAN,
                f64::INFINITY,
                -1.0,
                -0.5,
                -0.0,
                0.0,
                0.5,
                1.0,
                2.0,
            ] {
                for op in [BinaryOp::Mul, BinaryOp::Div] {
                    let expected = if matches!(op, BinaryOp::Div) && factor == 0.0 {
                        Err(RuntimeError::DivisionByZero)
                    } else {
                        let value = if matches!(op, BinaryOp::Mul) {
                            nanos as f64 * factor
                        } else {
                            nanos as f64 / factor
                        };
                        let value = crate::numeric::math::trunc(value);
                        if !value.is_finite()
                            || value < i128::MIN as f64
                            || value > i128::MAX as f64
                        {
                            Err(RuntimeError::Overflow)
                        } else {
                            i64::try_from(value as i128)
                                .map(Duration::from_nanos)
                                .map_err(|_| RuntimeError::Overflow)
                        }
                    };
                    assert_eq!(
                        scale_duration(Duration::from_nanos(nanos), &Value::LReal(factor), op),
                        expected
                    );
                }
            }
        }
        assert_eq!(
            scale_duration(Duration::ZERO, &Value::ULInt(u64::MAX), BinaryOp::Mul),
            Ok(Duration::ZERO)
        );
        assert_eq!(
            scale_duration(
                Duration::from_nanos(i64::MIN),
                &Value::ULInt(u64::MAX),
                BinaryOp::Div
            ),
            Ok(Duration::ZERO)
        );
    }
    #[test]
    fn integer_duration_scaling_matches_wide_formula_for_signed_and_unsigned_domains() {
        let factors = [
            (Value::SInt(i8::MIN), i128::from(i8::MIN)),
            (Value::SInt(i8::MAX), i128::from(i8::MAX)),
            (Value::Int(i16::MIN), i128::from(i16::MIN)),
            (Value::Int(i16::MAX), i128::from(i16::MAX)),
            (Value::DInt(i32::MIN), i128::from(i32::MIN)),
            (Value::DInt(i32::MAX), i128::from(i32::MAX)),
            (Value::LInt(i64::MIN), i128::from(i64::MIN)),
            (Value::LInt(i64::MAX), i128::from(i64::MAX)),
            (Value::LInt(-1), -1),
            (Value::LInt(0), 0),
            (Value::LInt(1), 1),
            (Value::USInt(u8::MAX), i128::from(u8::MAX)),
            (Value::UInt(u16::MAX), i128::from(u16::MAX)),
            (Value::UDInt(u32::MAX), i128::from(u32::MAX)),
            (Value::ULInt(u64::MAX), i128::from(u64::MAX)),
            (Value::ULInt(1u64 << 63), 1i128 << 63),
            (Value::ULInt((1u64 << 63) + 1), (1i128 << 63) + 1),
            (Value::ULInt(0), 0),
            (Value::ULInt(1), 1),
            (Value::ULInt(2), 2),
        ];
        for nanos in [
            i64::MIN,
            i64::MIN + 1,
            -3,
            -1,
            0,
            1,
            3,
            i64::MAX - 1,
            i64::MAX,
        ] {
            for (factor, wide) in &factors {
                for op in [BinaryOp::Mul, BinaryOp::Div, BinaryOp::Add] {
                    let expected = match op {
                        BinaryOp::Mul => i64::try_from(i128::from(nanos) * wide)
                            .map_err(|_| RuntimeError::Overflow),
                        BinaryOp::Div if *wide == 0 => Err(RuntimeError::DivisionByZero),
                        BinaryOp::Div => i64::try_from(i128::from(nanos) / wide)
                            .map_err(|_| RuntimeError::Overflow),
                        _ => Err(RuntimeError::TypeMismatch),
                    }
                    .map(Duration::from_nanos);
                    assert_eq!(
                        scale_duration(Duration::from_nanos(nanos), factor, op),
                        expected,
                        "nanos={nanos}, factor={factor:?}, op={op:?}"
                    );
                }
            }
        }
        assert_eq!(
            scale_duration(Duration::ZERO, &Value::Bool(false), BinaryOp::Div),
            Err(RuntimeError::TypeMismatch)
        );
    }
}
