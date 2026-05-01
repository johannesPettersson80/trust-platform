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
        (Value::LTime(lhs), Value::LTime(rhs)) => time_cmp_values(op, lhs.as_nanos(), rhs.as_nanos()),
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
    let nanos = i128::from(time.as_nanos());
    let result = match factor {
        NumericFactor::Integer(value) => match op {
            BinaryOp::Mul => nanos.checked_mul(value).ok_or(RuntimeError::Overflow)?,
            BinaryOp::Div => {
                if value == 0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                nanos / value
            }
            _ => return Err(RuntimeError::TypeMismatch),
        },
        NumericFactor::Real(value) => {
            if matches!(op, BinaryOp::Div) && value == 0.0 {
                return Err(RuntimeError::DivisionByZero);
            }
            let result = match op {
                BinaryOp::Mul => (nanos as f64) * value,
                BinaryOp::Div => (nanos as f64) / value,
                _ => return Err(RuntimeError::TypeMismatch),
            };
            let truncated = result.trunc();
            if !truncated.is_finite() {
                return Err(RuntimeError::Overflow);
            }
            if truncated < i128::MIN as f64 || truncated > i128::MAX as f64 {
                return Err(RuntimeError::Overflow);
            }
            truncated as i128
        }
    };
    let nanos = i64::try_from(result).map_err(|_| RuntimeError::Overflow)?;
    Ok(Duration::from_nanos(nanos))
}

enum NumericFactor {
    Integer(i128),
    Real(f64),
}

fn numeric_factor(value: &Value) -> Result<NumericFactor, RuntimeError> {
    match value {
        Value::Real(v) => Ok(NumericFactor::Real(*v as f64)),
        Value::LReal(v) => Ok(NumericFactor::Real(*v)),
        Value::SInt(v) => Ok(NumericFactor::Integer(i128::from(*v))),
        Value::Int(v) => Ok(NumericFactor::Integer(i128::from(*v))),
        Value::DInt(v) => Ok(NumericFactor::Integer(i128::from(*v))),
        Value::LInt(v) => Ok(NumericFactor::Integer(i128::from(*v))),
        Value::USInt(v) => Ok(NumericFactor::Integer(i128::from(*v))),
        Value::UInt(v) => Ok(NumericFactor::Integer(i128::from(*v))),
        Value::UDInt(v) => Ok(NumericFactor::Integer(i128::from(*v))),
        Value::ULInt(v) => Ok(NumericFactor::Integer(i128::from(*v))),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn duration_to_ticks(time: Duration, profile: &DateTimeProfile) -> Result<i64, RuntimeError> {
    let resolution = profile.resolution.as_nanos();
    if resolution == 0 {
        return Err(RuntimeError::Overflow);
    }
    let ticks = i128::from(time.as_nanos()) / i128::from(resolution);
    i64::try_from(ticks).map_err(|_| RuntimeError::Overflow)
}

fn ticks_to_duration(ticks: i128, profile: &DateTimeProfile) -> Result<Duration, RuntimeError> {
    let nanos = ticks
        .checked_mul(i128::from(profile.resolution.as_nanos()))
        .ok_or(RuntimeError::Overflow)?;
    let nanos = i64::try_from(nanos).map_err(|_| RuntimeError::Overflow)?;
    Ok(Duration::from_nanos(nanos))
}
