use crate::datetime::{days_to_ticks, nanos_to_ticks, ticks_per_day, DivisionMode, NANOS_PER_DAY};
use crate::error::RuntimeError;
use crate::value::{
    DateTimeProfile, DateTimeValue, DateValue, LDateTimeValue, LTimeOfDayValue, TimeOfDayValue,
    Value,
};
use trust_hir::TypeId;

pub(super) fn convert_to_time(value: &Value, dst: TypeId) -> Result<Value, RuntimeError> {
    match (value, dst) {
        (Value::Time(duration), TypeId::TIME) => Ok(Value::Time(*duration)),
        (Value::LTime(duration), TypeId::LTIME) => Ok(Value::LTime(*duration)),
        (Value::Time(duration), TypeId::LTIME) => Ok(Value::LTime(*duration)),
        (Value::LTime(duration), TypeId::TIME) => Ok(Value::Time(*duration)),
        (Value::DWord(value), TypeId::TIME) => Ok(Value::Time(
            crate::value::Duration::from_millis(i64::from(*value)),
        )),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn convert_to_date(value: &Value, dst: TypeId) -> Result<Value, RuntimeError> {
    let profile = DateTimeProfile::default();
    match (value, dst) {
        (Value::Date(date), TypeId::DATE) => Ok(Value::Date(*date)),
        (Value::LDate(date), TypeId::LDATE) => Ok(Value::LDate(*date)),
        (Value::Dt(dt), TypeId::DATE) => {
            let days = dt_ticks_to_days(dt, profile)?;
            let ticks = days_to_ticks(days, profile)?;
            Ok(Value::Date(DateValue::new(ticks)))
        }
        (Value::Ldt(dt), TypeId::DATE) => {
            let days = ldt_nanos_to_days(dt)?;
            let ticks = days_to_ticks(days, profile)?;
            Ok(Value::Date(DateValue::new(ticks)))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn convert_to_tod(value: &Value, dst: TypeId) -> Result<Value, RuntimeError> {
    let profile = DateTimeProfile::default();
    match (value, dst) {
        (Value::Tod(tod), TypeId::TOD) => Ok(Value::Tod(*tod)),
        (Value::LTod(tod), TypeId::LTOD) => Ok(Value::LTod(*tod)),
        (Value::LTod(tod), TypeId::TOD) => {
            let ticks = nanos_to_ticks(tod.nanos(), profile, DivisionMode::Euclid)?;
            Ok(Value::Tod(TimeOfDayValue::new(ticks)))
        }
        (Value::Tod(tod), TypeId::LTOD) => {
            let nanos = ticks_to_nanos(tod.ticks(), profile)?;
            Ok(Value::LTod(LTimeOfDayValue::new(nanos)))
        }
        (Value::Dt(dt), TypeId::TOD) => {
            let ticks = dt_ticks_to_tod_ticks(dt, profile)?;
            Ok(Value::Tod(TimeOfDayValue::new(ticks)))
        }
        (Value::Dt(dt), TypeId::LTOD) => {
            let nanos = dt_ticks_to_tod_nanos(dt, profile)?;
            Ok(Value::LTod(LTimeOfDayValue::new(nanos)))
        }
        (Value::Ldt(dt), TypeId::TOD) => {
            let ticks = ldt_nanos_to_tod_ticks(dt, profile)?;
            Ok(Value::Tod(TimeOfDayValue::new(ticks)))
        }
        (Value::Ldt(dt), TypeId::LTOD) => {
            let nanos = ldt_nanos_to_tod_nanos(dt)?;
            Ok(Value::LTod(LTimeOfDayValue::new(nanos)))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

pub(super) fn convert_to_dt(value: &Value, dst: TypeId) -> Result<Value, RuntimeError> {
    let profile = DateTimeProfile::default();
    match (value, dst) {
        (Value::Dt(dt), TypeId::DT) => Ok(Value::Dt(*dt)),
        (Value::Ldt(dt), TypeId::LDT) => Ok(Value::Ldt(*dt)),
        (Value::Dt(dt), TypeId::LDT) => {
            let nanos = dt_ticks_to_nanos(dt, profile)?;
            Ok(Value::Ldt(LDateTimeValue::new(nanos)))
        }
        (Value::Ldt(dt), TypeId::DT) => {
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
