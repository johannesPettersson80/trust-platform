//! Time and date standard functions.

#![allow(missing_docs)]

use crate::datetime::{
    days_from_civil, days_to_ticks, nanos_to_ticks, ticks_per_day, DivisionMode, NANOS_PER_DAY,
};
use crate::error::RuntimeError;
use crate::program_model::{apply_binary, BinaryOp};
use crate::stdlib::helpers::{require_arity, scale_time, to_i64};
use crate::stdlib::StandardLibrary;
use crate::value::{
    combine_date_and_tod, DateTimeProfile, DateTimeValue, DateValue, LDateTimeValue,
    LTimeOfDayValue, TimeOfDayValue, Value,
};

pub fn register(lib: &mut StandardLibrary) {
    lib.register("ADD_TIME", &["IN1", "IN2"], add_time);
    lib.register("ADD_LTIME", &["IN1", "IN2"], add_ltime);
    lib.register("ADD_TOD_TIME", &["IN1", "IN2"], add_tod_time);
    lib.register("ADD_LTOD_LTIME", &["IN1", "IN2"], add_ltod_ltime);
    lib.register("ADD_DT_TIME", &["IN1", "IN2"], add_dt_time);
    lib.register("ADD_LDT_LTIME", &["IN1", "IN2"], add_ldt_ltime);

    lib.register("SUB_TIME", &["IN1", "IN2"], sub_time);
    lib.register("SUB_LTIME", &["IN1", "IN2"], sub_ltime);
    lib.register("SUB_DATE_DATE", &["IN1", "IN2"], sub_date_date);
    lib.register("SUB_LDATE_LDATE", &["IN1", "IN2"], sub_ldate_ldate);
    lib.register("SUB_TOD_TIME", &["IN1", "IN2"], sub_tod_time);
    lib.register("SUB_LTOD_LTIME", &["IN1", "IN2"], sub_ltod_ltime);
    lib.register("SUB_TOD_TOD", &["IN1", "IN2"], sub_tod_tod);
    lib.register("SUB_LTOD_LTOD", &["IN1", "IN2"], sub_ltod_ltod);
    lib.register("SUB_DT_TIME", &["IN1", "IN2"], sub_dt_time);
    lib.register("SUB_LDT_LTIME", &["IN1", "IN2"], sub_ldt_ltime);
    lib.register("SUB_DT_DT", &["IN1", "IN2"], sub_dt_dt);
    lib.register("SUB_LDT_LDT", &["IN1", "IN2"], sub_ldt_ldt);

    lib.register("MUL_TIME", &["IN1", "IN2"], mul_time);
    lib.register("MUL_LTIME", &["IN1", "IN2"], mul_ltime);
    lib.register("DIV_TIME", &["IN1", "IN2"], div_time);
    lib.register("DIV_LTIME", &["IN1", "IN2"], div_ltime);

    lib.register("CONCAT_DATE_TOD", &["DATE", "TOD"], concat_date_tod);
    lib.register("CONCAT_DATE_LTOD", &["DATE", "LTOD"], concat_date_ltod);
    lib.register("CONCAT_DATE", &["YEAR", "MONTH", "DAY"], concat_date);
    lib.register(
        "CONCAT_TOD",
        &["HOUR", "MINUTE", "SECOND", "MILLISECOND"],
        concat_tod,
    );
    lib.register(
        "CONCAT_LTOD",
        &["HOUR", "MINUTE", "SECOND", "MILLISECOND"],
        concat_ltod,
    );
    lib.register(
        "CONCAT_DT",
        &[
            "YEAR",
            "MONTH",
            "DAY",
            "HOUR",
            "MINUTE",
            "SECOND",
            "MILLISECOND",
        ],
        concat_dt,
    );
    lib.register(
        "CONCAT_LDT",
        &[
            "YEAR",
            "MONTH",
            "DAY",
            "HOUR",
            "MINUTE",
            "SECOND",
            "MILLISECOND",
        ],
        concat_ldt,
    );

    lib.register("DAY_OF_WEEK", &["IN"], day_of_week);
}

type SplitDateTime = (i64, i64, i64, i64, i64, i64, i64);

fn add_time(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_time_pair, BinaryOp::Add)
}

fn add_ltime(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_ltime_pair, BinaryOp::Add)
}

fn add_tod_time(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_tod_time_pair, BinaryOp::Add)
}

fn add_ltod_ltime(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_ltod_ltime_pair, BinaryOp::Add)
}

fn add_dt_time(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_dt_time_pair, BinaryOp::Add)
}

fn add_ldt_ltime(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_ldt_ltime_pair, BinaryOp::Add)
}

fn sub_time(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_time_pair, BinaryOp::Sub)
}

fn sub_ltime(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_ltime_pair, BinaryOp::Sub)
}

fn sub_date_date(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_date_pair, BinaryOp::Sub)
}

fn sub_ldate_ldate(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_ldate_pair, BinaryOp::Sub)
}

fn sub_tod_time(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_tod_time_pair, BinaryOp::Sub)
}

fn sub_ltod_ltime(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_ltod_ltime_pair, BinaryOp::Sub)
}

fn sub_tod_tod(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_tod_pair, BinaryOp::Sub)
}

fn sub_ltod_ltod(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_ltod_pair, BinaryOp::Sub)
}

fn sub_dt_time(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_dt_time_pair, BinaryOp::Sub)
}

fn sub_ldt_ltime(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_ldt_ltime_pair, BinaryOp::Sub)
}

fn sub_dt_dt(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_dt_pair, BinaryOp::Sub)
}

fn sub_ldt_ldt(args: &[Value]) -> Result<Value, RuntimeError> {
    bin_time(args, expect_ldt_pair, BinaryOp::Sub)
}

fn mul_time(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    match &args[0] {
        Value::Time(duration) => scale_time(*duration, &args[1], true).map(Value::Time),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn mul_ltime(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    match &args[0] {
        Value::LTime(duration) => scale_time(*duration, &args[1], true).map(Value::LTime),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn div_time(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    match &args[0] {
        Value::Time(duration) => scale_time(*duration, &args[1], false).map(Value::Time),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn div_ltime(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    match &args[0] {
        Value::LTime(duration) => scale_time(*duration, &args[1], false).map(Value::LTime),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn concat_date_tod(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    match (&args[0], &args[1]) {
        (Value::Date(date), Value::Tod(tod)) => Ok(Value::Dt(combine_date_and_tod(*date, *tod)?)),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn concat_date_ltod(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    let profile = DateTimeProfile::default();
    match (&args[0], &args[1]) {
        (Value::Date(date), Value::LTod(tod)) => {
            let days = date_ticks_to_days(date, profile)?;
            let nanos = days
                .checked_mul(NANOS_PER_DAY)
                .and_then(|v| v.checked_add(tod.nanos()))
                .ok_or(RuntimeError::Overflow)?;
            Ok(Value::Ldt(LDateTimeValue::new(nanos)))
        }
        _ => Err(RuntimeError::TypeMismatch),
    }
}

fn concat_date(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 3)?;
    let profile = DateTimeProfile::default();
    let year = to_i64(&args[0])?;
    let month = to_i64(&args[1])?;
    let day = to_i64(&args[2])?;
    let days = days_from_civil(year, month, day)?;
    let ticks = days_to_ticks(days, profile)?;
    Ok(Value::Date(DateValue::new(ticks)))
}

fn concat_tod(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 4)?;
    let profile = DateTimeProfile::default();
    let nanos = tod_components_to_nanos(args)?;
    let ticks = nanos_to_ticks(nanos, profile, DivisionMode::Trunc)?;
    Ok(Value::Tod(TimeOfDayValue::new(ticks)))
}

fn concat_ltod(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 4)?;
    let nanos = tod_components_to_nanos(args)?;
    Ok(Value::LTod(LTimeOfDayValue::new(nanos)))
}

fn concat_dt(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 7)?;
    let profile = DateTimeProfile::default();
    let (days, nanos) = date_time_components(args)?;
    let date_ticks = days_to_ticks(days, profile)?;
    let tod_ticks = nanos_to_ticks(nanos, profile, DivisionMode::Trunc)?;
    let ticks = date_ticks
        .checked_add(tod_ticks)
        .ok_or(RuntimeError::Overflow)?;
    Ok(Value::Dt(DateTimeValue::new(ticks)))
}

fn concat_ldt(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 7)?;
    let (days, nanos) = date_time_components(args)?;
    let date_nanos = days
        .checked_mul(NANOS_PER_DAY)
        .ok_or(RuntimeError::Overflow)?;
    let total = date_nanos
        .checked_add(nanos)
        .ok_or(RuntimeError::Overflow)?;
    Ok(Value::Ldt(LDateTimeValue::new(total)))
}

fn day_of_week(args: &[Value]) -> Result<Value, RuntimeError> {
    require_arity(args, 1)?;
    let profile = DateTimeProfile::default();
    let date = match &args[0] {
        Value::Date(date) => date,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let days = date_ticks_to_days(date, profile)?;
    let dow = (days + 4).rem_euclid(7);
    Ok(Value::Int(dow as i16))
}

fn bin_time(
    args: &[Value],
    checker: fn(&Value, &Value) -> bool,
    op: BinaryOp,
) -> Result<Value, RuntimeError> {
    require_arity(args, 2)?;
    if !checker(&args[0], &args[1]) {
        return Err(RuntimeError::TypeMismatch);
    }
    let profile = DateTimeProfile::default();
    apply_binary(op, args[0].clone(), args[1].clone(), &profile)
}

fn expect_time_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Time(_), Value::Time(_)))
}

fn expect_ltime_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::LTime(_), Value::LTime(_)))
}

fn expect_date_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Date(_), Value::Date(_)))
}

fn expect_ldate_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::LDate(_), Value::LDate(_)))
}

fn expect_tod_time_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Tod(_), Value::Time(_)))
}

fn expect_ltod_ltime_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::LTod(_), Value::LTime(_)))
}

fn expect_tod_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Tod(_), Value::Tod(_)))
}

fn expect_ltod_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::LTod(_), Value::LTod(_)))
}

fn expect_dt_time_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Dt(_), Value::Time(_)))
}

fn expect_ldt_ltime_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Ldt(_), Value::LTime(_)))
}

fn expect_dt_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Dt(_), Value::Dt(_)))
}

fn expect_ldt_pair(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Ldt(_), Value::Ldt(_)))
}

fn date_time_components(args: &[Value]) -> Result<(i64, i64), RuntimeError> {
    let year = to_i64(&args[0])?;
    let month = to_i64(&args[1])?;
    let day = to_i64(&args[2])?;
    let days = days_from_civil(year, month, day)?;
    let tod_args = &args[3..];
    let nanos = tod_components_to_nanos(tod_args)?;
    Ok((days, nanos))
}

fn tod_components_to_nanos(args: &[Value]) -> Result<i64, RuntimeError> {
    let hour = to_i64(&args[0])?;
    let minute = to_i64(&args[1])?;
    let second = to_i64(&args[2])?;
    let millis = to_i64(&args[3])?;
    if hour < 0 || minute < 0 || second < 0 || millis < 0 {
        return Err(RuntimeError::Overflow);
    }
    let total = hour
        .checked_mul(3_600)
        .and_then(|v| v.checked_add(minute.checked_mul(60)?))
        .and_then(|v| v.checked_add(second))
        .ok_or(RuntimeError::Overflow)?;
    let nanos = total
        .checked_mul(1_000_000_000)
        .and_then(|v| v.checked_add(millis.checked_mul(1_000_000)?))
        .ok_or(RuntimeError::Overflow)?;
    if nanos >= NANOS_PER_DAY {
        return Err(RuntimeError::Overflow);
    }
    Ok(nanos)
}

fn date_ticks_to_days(date: &DateValue, profile: DateTimeProfile) -> Result<i64, RuntimeError> {
    let ticks = date
        .ticks()
        .checked_sub(profile.epoch.ticks())
        .ok_or(RuntimeError::Overflow)?;
    let ticks_per_day = ticks_per_day(profile)?;
    Ok(ticks.div_euclid(ticks_per_day))
}

pub fn is_split_name(name: &str) -> bool {
    matches!(
        name,
        "SPLIT_DATE" | "SPLIT_TOD" | "SPLIT_LTOD" | "SPLIT_DT" | "SPLIT_LDT"
    )
}

pub fn is_runtime_clock_name(name: &str) -> bool {
    name == "TIME"
}

pub fn split_date(
    value: &Value,
    profile: DateTimeProfile,
) -> Result<(i64, i64, i64), RuntimeError> {
    let date = match value {
        Value::Date(date) => date,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let days = date_ticks_to_days(date, profile)?;
    Ok(civil_from_days(days))
}

pub fn split_tod(
    value: &Value,
    profile: DateTimeProfile,
) -> Result<(i64, i64, i64, i64), RuntimeError> {
    let tod = match value {
        Value::Tod(tod) => tod,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let res = profile.resolution.as_nanos();
    if res <= 0 {
        return Err(RuntimeError::Overflow);
    }
    let nanos = tod.ticks().checked_mul(res).ok_or(RuntimeError::Overflow)?;
    Ok(tod_from_nanos(nanos))
}

pub fn split_ltod(value: &Value) -> Result<(i64, i64, i64, i64), RuntimeError> {
    let tod = match value {
        Value::LTod(tod) => tod,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    Ok(tod_from_nanos(tod.nanos()))
}

pub fn split_dt(value: &Value, profile: DateTimeProfile) -> Result<SplitDateTime, RuntimeError> {
    let dt = match value {
        Value::Dt(dt) => dt,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let ticks = dt
        .ticks()
        .checked_sub(profile.epoch.ticks())
        .ok_or(RuntimeError::Overflow)?;
    let ticks_per_day = ticks_per_day(profile)?;
    let days = ticks.div_euclid(ticks_per_day);
    let day_ticks = ticks.rem_euclid(ticks_per_day);
    let nanos = day_ticks
        .checked_mul(profile.resolution.as_nanos())
        .ok_or(RuntimeError::Overflow)?;
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second, millis) = tod_from_nanos(nanos);
    Ok((year, month, day, hour, minute, second, millis))
}

pub fn split_ldt(value: &Value) -> Result<SplitDateTime, RuntimeError> {
    let dt = match value {
        Value::Ldt(dt) => dt,
        _ => return Err(RuntimeError::TypeMismatch),
    };
    let nanos = dt.nanos();
    let days = nanos.div_euclid(NANOS_PER_DAY);
    let day_nanos = nanos.rem_euclid(NANOS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second, millis) = tod_from_nanos(day_nanos);
    Ok((year, month, day, hour, minute, second, millis))
}

fn tod_from_nanos(nanos: i64) -> (i64, i64, i64, i64) {
    let mut remainder = nanos;
    let hours = remainder / 3_600_000_000_000;
    remainder %= 3_600_000_000_000;
    let minutes = remainder / 60_000_000_000;
    remainder %= 60_000_000_000;
    let seconds = remainder / 1_000_000_000;
    remainder %= 1_000_000_000;
    let millis = remainder / 1_000_000;
    (hours, minutes, seconds, millis)
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if m <= 2 { 1 } else { 0 };
    (year, m, d)
}
