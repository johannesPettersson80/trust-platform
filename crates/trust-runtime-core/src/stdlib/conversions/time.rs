use super::ConversionType;
use crate::datetime::{days_to_ticks, nanos_to_ticks, ticks_per_day, DivisionMode, NANOS_PER_DAY};
use crate::error::RuntimeError;
use crate::value::{
    DateTimeProfile, DateTimeValue, DateValue, LDateTimeValue, LTimeOfDayValue, TimeOfDayValue,
    Value,
};

pub(super) fn convert_to_time(value: &Value, dst: ConversionType) -> Result<Value, RuntimeError> {
    match (value, dst) {
        (Value::Time(duration), ConversionType::Time) => Ok(Value::Time(*duration)),
        (Value::LTime(duration), ConversionType::LTime) => Ok(Value::LTime(*duration)),
        (Value::Time(duration), ConversionType::LTime) => Ok(Value::LTime(*duration)),
        (Value::LTime(duration), ConversionType::Time) => Ok(Value::Time(*duration)),
        (Value::DWord(value), ConversionType::Time) => Ok(Value::Time(
            crate::value::Duration::from_millis(i64::from(*value)),
        )),
        // Round 2 (IEC_DEVIATIONS 2026-11-06): an integer is a millisecond
        // count; a negative count saturates to zero, following the measured
        // `REAL_TO_TIME(-1.0) = T#0ms`. A REAL/LREAL value is read as
        // milliseconds truncated toward zero — measured
        // `REAL_TO_TIME(2.5) = T#2ms`, not the DEV-024 rounding — and a
        // non-finite value keeps the overflow fault (specification 07 2.10).
        (Value::SInt(value), ConversionType::Time) => time_from_ms_count(i64::from(*value)),
        (Value::Int(value), ConversionType::Time) => time_from_ms_count(i64::from(*value)),
        (Value::DInt(value), ConversionType::Time) => time_from_ms_count(i64::from(*value)),
        (Value::LInt(value), ConversionType::Time) => time_from_ms_count(*value),
        (Value::USInt(value), ConversionType::Time) => time_from_ms_count(i64::from(*value)),
        (Value::UInt(value), ConversionType::Time) => time_from_ms_count(i64::from(*value)),
        (Value::UDInt(value), ConversionType::Time) => time_from_ms_count(i64::from(*value)),
        (Value::ULInt(value), ConversionType::Time) => {
            // TIME is a 32-bit vendor type; a ULINT count above the i64
            // storage saturates (the low-order-bit rule covers the rest).
            let millis = (*value).min(i64::MAX as u64) as i64;
            Ok(Value::Time(crate::value::Duration::from_millis(millis)))
        }
        (Value::Real(value), ConversionType::Time) => time_from_real_millis(f64::from(*value)),
        (Value::LReal(value), ConversionType::Time) => time_from_real_millis(*value),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

/// The duration of a millisecond count, a negative count saturated to zero
/// (the measured `REAL_TO_TIME(-1.0) = T#0ms` vendor behaviour).
fn time_from_ms_count(millis: i64) -> Result<Value, RuntimeError> {
    Ok(Value::Time(crate::value::Duration::from_millis(
        millis.max(0),
    )))
}

/// The duration of a REAL/LREAL millisecond value: truncated toward zero, a
/// negative value saturated to zero, a non-finite value an overflow fault.
fn time_from_real_millis(value: f64) -> Result<Value, RuntimeError> {
    if !value.is_finite() {
        return Err(RuntimeError::Overflow);
    }
    let millis = value.trunc().max(0.0);
    Ok(Value::Time(crate::value::Duration::from_millis(
        millis as i64,
    )))
}

pub(super) fn convert_to_date(value: &Value, dst: ConversionType) -> Result<Value, RuntimeError> {
    let profile = DateTimeProfile::default();
    match (value, dst) {
        (Value::Date(date), ConversionType::Date) => Ok(Value::Date(*date)),
        (Value::LDate(date), ConversionType::LDate) => Ok(Value::LDate(*date)),
        (Value::Dt(dt), ConversionType::Date) => {
            let days = dt_ticks_to_days(dt, profile)?;
            let ticks = days_to_ticks(days, profile)?;
            Ok(Value::Date(DateValue::new(ticks)))
        }
        (Value::Ldt(dt), ConversionType::Date) => {
            let days = ldt_nanos_to_days(dt)?;
            let ticks = days_to_ticks(days, profile)?;
            Ok(Value::Date(DateValue::new(ticks)))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn convert_to_tod(value: &Value, dst: ConversionType) -> Result<Value, RuntimeError> {
    let profile = DateTimeProfile::default();
    let day_ticks = ticks_per_day(profile)?;
    match (value, dst) {
        // Round 2 (IEC_DEVIATIONS 2026-11-06): a DWORD reads as a ms-of-day
        // count modulo 24 h — measured
        // `DWORD_TO_TOD(DWORD#86400000) = TOD#0:0:0` and
        // `DWORD_TO_TOD(DWORD#16#FFFFFFFF) = TOD#17:02:47.295`.
        (Value::DWord(value), ConversionType::Tod) => Ok(Value::Tod(TimeOfDayValue::new(
            i64::from(*value) % day_ticks,
        ))),
        (Value::Tod(tod), ConversionType::Tod) => Ok(Value::Tod(*tod)),
        (Value::LTod(tod), ConversionType::LTod) => Ok(Value::LTod(*tod)),
        (Value::LTod(tod), ConversionType::Tod) => {
            let ticks = nanos_to_ticks(tod.nanos(), profile, DivisionMode::Euclid)?;
            Ok(Value::Tod(TimeOfDayValue::new(ticks)))
        }
        (Value::Tod(tod), ConversionType::LTod) => {
            let nanos = ticks_to_nanos(tod.ticks(), profile)?;
            Ok(Value::LTod(LTimeOfDayValue::new(nanos)))
        }
        (Value::Dt(dt), ConversionType::Tod) => {
            let ticks = dt_ticks_to_tod_ticks(dt, profile)?;
            Ok(Value::Tod(TimeOfDayValue::new(ticks)))
        }
        (Value::Dt(dt), ConversionType::LTod) => {
            let nanos = dt_ticks_to_tod_nanos(dt, profile)?;
            Ok(Value::LTod(LTimeOfDayValue::new(nanos)))
        }
        (Value::Ldt(dt), ConversionType::Tod) => {
            let ticks = ldt_nanos_to_tod_ticks(dt, profile)?;
            Ok(Value::Tod(TimeOfDayValue::new(ticks)))
        }
        (Value::Ldt(dt), ConversionType::LTod) => {
            let nanos = ldt_nanos_to_tod_nanos(dt)?;
            Ok(Value::LTod(LTimeOfDayValue::new(nanos)))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn convert_to_dt(value: &Value, dst: ConversionType) -> Result<Value, RuntimeError> {
    let profile = DateTimeProfile::default();
    match (value, dst) {
        (Value::Dt(dt), ConversionType::Dt) => Ok(Value::Dt(*dt)),
        (Value::Ldt(dt), ConversionType::Ldt) => Ok(Value::Ldt(*dt)),
        (Value::Dt(dt), ConversionType::Ldt) => {
            let nanos = dt_ticks_to_nanos(dt, profile)?;
            Ok(Value::Ldt(LDateTimeValue::new(nanos)))
        }
        (Value::Ldt(dt), ConversionType::Dt) => {
            let ticks = ldt_nanos_to_ticks(dt, profile)?;
            Ok(Value::Dt(DateTimeValue::new(ticks)))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn dt_ticks_to_days(dt: &DateTimeValue, profile: DateTimeProfile) -> Result<i64, RuntimeError> {
    let ticks = dt
        .ticks()
        .checked_sub(profile.epoch.ticks())
        .ok_or(RuntimeError::Overflow)?;
    let per_day = ticks_per_day(profile)?;
    Ok(ticks.div_euclid(per_day))
}

fn ldt_nanos_to_days(dt: &LDateTimeValue) -> Result<i64, RuntimeError> {
    Ok(dt.nanos().div_euclid(NANOS_PER_DAY))
}

fn dt_ticks_to_nanos(dt: &DateTimeValue, profile: DateTimeProfile) -> Result<i64, RuntimeError> {
    let res = profile.resolution.as_nanos();
    dt.ticks()
        .checked_sub(profile.epoch.ticks())
        .and_then(|v| v.checked_mul(res))
        .ok_or(RuntimeError::Overflow)
}

fn ldt_nanos_to_ticks(dt: &LDateTimeValue, profile: DateTimeProfile) -> Result<i64, RuntimeError> {
    let res = profile.resolution.as_nanos();
    let ticks = dt.nanos().div_euclid(res);
    ticks
        .checked_add(profile.epoch.ticks())
        .ok_or(RuntimeError::Overflow)
}

fn dt_ticks_to_tod_ticks(
    dt: &DateTimeValue,
    profile: DateTimeProfile,
) -> Result<i64, RuntimeError> {
    let ticks = dt
        .ticks()
        .checked_sub(profile.epoch.ticks())
        .ok_or(RuntimeError::Overflow)?;
    let per_day = ticks_per_day(profile)?;
    Ok(ticks.rem_euclid(per_day))
}

fn dt_ticks_to_tod_nanos(
    dt: &DateTimeValue,
    profile: DateTimeProfile,
) -> Result<i64, RuntimeError> {
    let ticks = dt_ticks_to_tod_ticks(dt, profile)?;
    ticks_to_nanos(ticks, profile)
}

fn ldt_nanos_to_tod_ticks(
    dt: &LDateTimeValue,
    profile: DateTimeProfile,
) -> Result<i64, RuntimeError> {
    let nanos = ldt_nanos_to_tod_nanos(dt)?;
    Ok(nanos_to_ticks(nanos, profile, DivisionMode::Euclid)?)
}

fn ldt_nanos_to_tod_nanos(dt: &LDateTimeValue) -> Result<i64, RuntimeError> {
    Ok(dt.nanos().rem_euclid(NANOS_PER_DAY))
}

fn ticks_to_nanos(ticks: i64, profile: DateTimeProfile) -> Result<i64, RuntimeError> {
    let res = profile.resolution.as_nanos();
    ticks.checked_mul(res).ok_or(RuntimeError::Overflow)
}
