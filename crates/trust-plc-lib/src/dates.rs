//! Dates and times (IEC §6.4.2, Table 10; §6.6.2.5.12 and §6.6.2.5.13, Tables 36 and 37) with the
//! runtime's rules (specification 07 §9; specification 10 §2): a `TIME` and an `LTIME` hold
//! nanoseconds; a `DATE`, a `TIME_OF_DAY` and a `DATE_AND_TIME` hold milliseconds since
//! 1970-01-01, the date-time profile every runtime uses; the long forms hold nanoseconds. A time of
//! day is not wrapped at midnight, a duration added to a millisecond value counts in whole
//! milliseconds toward zero, and a calendar computation outside its range is an overflow.

use trust_plc_abi::module::fault;
use trust_plc_abi::{Cx, Fault, FaultCode};

/// Nanoseconds in a millisecond, the tick of a `DATE`, `TIME_OF_DAY` and `DATE_AND_TIME`.
pub const NANOS_PER_TICK: i64 = 1_000_000;
/// Milliseconds in a day.
pub const TICKS_PER_DAY: i64 = 86_400_000;
/// Nanoseconds in a day.
pub const NANOS_PER_DAY: i64 = 86_400_000_000_000;

fn overflow(cx: &mut Cx) -> Fault {
    fault(cx, FaultCode::Overflow, 0, 0, 0)
}

/// `TOD ± TIME` and `DT ± TIME`: the duration in whole milliseconds toward zero, added to the
/// value or taken from it.
///
/// # Errors
///
/// `DateTimeRange` for a result outside the type.
pub fn ticks_add(cx: &mut Cx, ticks: i64, nanos: i64, add: bool) -> Result<i64, Fault> {
    let delta = i128::from(nanos / NANOS_PER_TICK);
    let result = if add {
        i128::from(ticks) + delta
    } else {
        i128::from(ticks) - delta
    };
    i64::try_from(result)
        .map_err(|_| fault(cx, FaultCode::DateTimeRange, ticks, nanos, i64::from(add)))
}

/// `LTOD ± LTIME` and `LDT ± LTIME` in nanoseconds.
///
/// # Errors
///
/// `Overflow` for a result outside the type.
pub fn nanos_add(cx: &mut Cx, nanos: i64, delta: i64, add: bool) -> Result<i64, Fault> {
    let result = if add {
        i128::from(nanos) + i128::from(delta)
    } else {
        i128::from(nanos) - i128::from(delta)
    };
    i64::try_from(result).map_err(|_| overflow(cx))
}

/// `DATE - DATE`, `TOD - TOD` and `DT - DT`: the difference as a `TIME`.
///
/// # Errors
///
/// `Overflow` for a difference no `TIME` holds.
pub fn ticks_diff(cx: &mut Cx, a: i64, b: i64) -> Result<i64, Fault> {
    let nanos = (i128::from(a) - i128::from(b)) * i128::from(NANOS_PER_TICK);
    i64::try_from(nanos).map_err(|_| overflow(cx))
}

/// `DT_TO_DATE`: the day of a `DATE_AND_TIME`.
///
/// # Errors
///
/// `Overflow` for the first day of the range, whose midnight no `DATE` holds.
pub fn dt_to_date(cx: &mut Cx, ticks: i64) -> Result<i64, Fault> {
    ticks
        .div_euclid(TICKS_PER_DAY)
        .checked_mul(TICKS_PER_DAY)
        .ok_or_else(|| overflow(cx))
}

/// `LDT_TO_DATE`: the day of an `LDATE_AND_TIME`, as a `DATE`.
#[must_use]
pub fn ldt_to_date(nanos: i64) -> i64 {
    nanos.div_euclid(NANOS_PER_DAY) * TICKS_PER_DAY
}

/// `DT_TO_TOD`: the time of day of a `DATE_AND_TIME`.
#[must_use]
pub fn dt_to_tod(ticks: i64) -> i64 {
    ticks.rem_euclid(TICKS_PER_DAY)
}

/// `DT_TO_LTOD`: the time of day of a `DATE_AND_TIME`, as an `LTIME_OF_DAY`.
#[must_use]
pub fn dt_to_ltod(ticks: i64) -> i64 {
    ticks.rem_euclid(TICKS_PER_DAY) * NANOS_PER_TICK
}

/// `LDT_TO_TOD`: the time of day of an `LDATE_AND_TIME`, in whole milliseconds.
#[must_use]
pub fn ldt_to_tod(nanos: i64) -> i64 {
    nanos.rem_euclid(NANOS_PER_DAY).div_euclid(NANOS_PER_TICK)
}

/// `LDT_TO_LTOD`: the time of day of an `LDATE_AND_TIME`.
#[must_use]
pub fn ldt_to_ltod(nanos: i64) -> i64 {
    nanos.rem_euclid(NANOS_PER_DAY)
}

/// `LTOD_TO_TOD` and `LDT_TO_DT`: nanoseconds as whole milliseconds, rounded down.
#[must_use]
pub fn nanos_to_ticks(nanos: i64) -> i64 {
    nanos.div_euclid(NANOS_PER_TICK)
}

/// `TOD_TO_LTOD` and `DT_TO_LDT`: milliseconds as nanoseconds.
///
/// # Errors
///
/// `Overflow` for a value no long form holds.
pub fn ticks_to_nanos(cx: &mut Cx, ticks: i64) -> Result<i64, Fault> {
    ticks
        .checked_mul(NANOS_PER_TICK)
        .ok_or_else(|| overflow(cx))
}

/// The days of the date `year-month-day` from 1970-01-01, as the runtime counts them (Howard
/// Hinnant's `days_from_civil`).
///
/// # Errors
///
/// `Overflow` for a month outside 1 to 12, a day outside its month, or a date no `i64` of days
/// holds.
fn days_from_civil(cx: &mut Cx, year: i64, month: i64, day: i64) -> Result<i64, Fault> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(overflow(cx));
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let length = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if leap => 29,
        _ => 28,
    };
    if day > length {
        return Err(overflow(cx));
    }
    let mut era = year.div_euclid(400);
    let mut yoe = year.rem_euclid(400);
    if month <= 2 {
        if yoe == 0 {
            era -= 1;
            yoe = 399;
        } else {
            yoe -= 1;
        }
    }
    let m = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i64::try_from(i128::from(era) * 146_097 + i128::from(doe) - 719_468).map_err(|_| overflow(cx))
}

/// The date of day `days` from 1970-01-01: year, month, day.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (y + i64::from(m <= 2), m, d)
}

/// The nanoseconds of a time of day `hour:minute:second.millisecond`, each in its own range.
///
/// # Errors
///
/// `Overflow` for a component outside its range.
fn clock(cx: &mut Cx, hour: i64, minute: i64, second: i64, millisecond: i64) -> Result<i64, Fault> {
    if !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
        || !(0..=999).contains(&millisecond)
    {
        return Err(overflow(cx));
    }
    Ok((hour * 3_600 + minute * 60 + second) * 1_000_000_000 + millisecond * NANOS_PER_TICK)
}

/// `CONCAT_DATE_TOD`: a date and a time of day as a `DATE_AND_TIME`.
///
/// # Errors
///
/// `DateTimeRange` for a sum no `DATE_AND_TIME` holds.
pub fn concat_date_tod(cx: &mut Cx, date: i64, tod: i64) -> Result<i64, Fault> {
    i64::try_from(i128::from(date) + i128::from(tod))
        .map_err(|_| fault(cx, FaultCode::DateTimeRange, date, tod, 0))
}

/// `CONCAT_DATE_LTOD`: a date and a long time of day as an `LDATE_AND_TIME`.
///
/// # Errors
///
/// `Overflow` for a sum no `LDATE_AND_TIME` holds.
pub fn concat_date_ltod(cx: &mut Cx, date: i64, ltod: i64) -> Result<i64, Fault> {
    date.div_euclid(TICKS_PER_DAY)
        .checked_mul(NANOS_PER_DAY)
        .and_then(|nanos| nanos.checked_add(ltod))
        .ok_or_else(|| overflow(cx))
}

/// `CONCAT_DATE(YEAR, MONTH, DAY)`.
///
/// # Errors
///
/// `Overflow` for an invalid or unrepresentable date.
pub fn concat_date(cx: &mut Cx, year: i64, month: i64, day: i64) -> Result<i64, Fault> {
    let days = days_from_civil(cx, year, month, day)?;
    days.checked_mul(TICKS_PER_DAY).ok_or_else(|| overflow(cx))
}

/// `CONCAT_TOD(HOUR, MINUTE, SECOND, MILLISECOND)`.
///
/// # Errors
///
/// `Overflow` for a component outside its range.
pub fn concat_tod(
    cx: &mut Cx,
    hour: i64,
    minute: i64,
    second: i64,
    millisecond: i64,
) -> Result<i64, Fault> {
    Ok(clock(cx, hour, minute, second, millisecond)? / NANOS_PER_TICK)
}

/// `CONCAT_LTOD(HOUR, MINUTE, SECOND, MILLISECOND)`.
///
/// # Errors
///
/// `Overflow` for a component outside its range.
pub fn concat_ltod(
    cx: &mut Cx,
    hour: i64,
    minute: i64,
    second: i64,
    millisecond: i64,
) -> Result<i64, Fault> {
    clock(cx, hour, minute, second, millisecond)
}

/// `CONCAT_DT(YEAR, MONTH, DAY, HOUR, MINUTE, SECOND, MILLISECOND)`.
///
/// # Errors
///
/// `Overflow` for an invalid date, a component outside its range or an unrepresentable result.
#[allow(clippy::too_many_arguments)]
pub fn concat_dt(
    cx: &mut Cx,
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    millisecond: i64,
) -> Result<i64, Fault> {
    let days = days_from_civil(cx, year, month, day)?;
    let nanos = clock(cx, hour, minute, second, millisecond)?;
    days.checked_mul(TICKS_PER_DAY)
        .and_then(|ticks| ticks.checked_add(nanos / NANOS_PER_TICK))
        .ok_or_else(|| overflow(cx))
}

/// `CONCAT_LDT(YEAR, MONTH, DAY, HOUR, MINUTE, SECOND, MILLISECOND)`.
///
/// # Errors
///
/// `Overflow` for an invalid date, a component outside its range or an unrepresentable result.
#[allow(clippy::too_many_arguments)]
pub fn concat_ldt(
    cx: &mut Cx,
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    millisecond: i64,
) -> Result<i64, Fault> {
    let days = days_from_civil(cx, year, month, day)?;
    let nanos = clock(cx, hour, minute, second, millisecond)?;
    days.checked_mul(NANOS_PER_DAY)
        .and_then(|day_nanos| day_nanos.checked_add(nanos))
        .ok_or_else(|| overflow(cx))
}

/// `DAY_OF_WEEK(IN)`: 0 for a Sunday to 6 for a Saturday.
#[must_use]
pub fn day_of_week(date: i64) -> i16 {
    // The remainder is 0 to 6.
    i16::try_from((date.div_euclid(TICKS_PER_DAY) + 4).rem_euclid(7)).unwrap_or(0)
}

/// The clock of `nanos` into the day: hour, minute, second, millisecond, toward zero for a time of
/// day below zero, as the runtime splits it.
fn clock_of(nanos: i64) -> [i64; 4] {
    let mut remainder = nanos;
    let hour = remainder / 3_600_000_000_000;
    remainder %= 3_600_000_000_000;
    let minute = remainder / 60_000_000_000;
    remainder %= 60_000_000_000;
    let second = remainder / 1_000_000_000;
    remainder %= 1_000_000_000;
    [hour, minute, second, remainder / NANOS_PER_TICK]
}

/// `SPLIT_DATE`: year, month, day.
#[must_use]
pub fn split_date(date: i64) -> [i64; 3] {
    let (year, month, day) = civil_from_days(date.div_euclid(TICKS_PER_DAY));
    [year, month, day]
}

/// `SPLIT_TOD`: hour, minute, second, millisecond.
///
/// # Errors
///
/// `Overflow` for a time of day whose nanoseconds no `i64` holds.
pub fn split_tod(cx: &mut Cx, tod: i64) -> Result<[i64; 4], Fault> {
    let nanos = tod
        .checked_mul(NANOS_PER_TICK)
        .ok_or_else(|| overflow(cx))?;
    Ok(clock_of(nanos))
}

/// `SPLIT_LTOD`: hour, minute, second, millisecond.
#[must_use]
pub fn split_ltod(ltod: i64) -> [i64; 4] {
    clock_of(ltod)
}

/// `SPLIT_DT`: year, month, day, hour, minute, second, millisecond.
#[must_use]
pub fn split_dt(dt: i64) -> [i64; 7] {
    let (year, month, day) = civil_from_days(dt.div_euclid(TICKS_PER_DAY));
    let [hour, minute, second, millisecond] =
        clock_of(dt.rem_euclid(TICKS_PER_DAY) * NANOS_PER_TICK);
    [year, month, day, hour, minute, second, millisecond]
}

/// `SPLIT_LDT`: year, month, day, hour, minute, second, millisecond.
#[must_use]
pub fn split_ldt(ldt: i64) -> [i64; 7] {
    let (year, month, day) = civil_from_days(ldt.div_euclid(NANOS_PER_DAY));
    let [hour, minute, second, millisecond] = clock_of(ldt.rem_euclid(NANOS_PER_DAY));
    [year, month, day, hour, minute, second, millisecond]
}

/// `TIME * n` and `TIME / n`, `MUL_TIME` and `DIV_TIME`, by a signed integer.
///
/// # Errors
///
/// `DivisionByZero` for `/ 0`, `Overflow` for a result no `TIME` holds.
pub fn scale_signed(cx: &mut Cx, nanos: i64, factor: i64, multiply: bool) -> Result<i64, Fault> {
    if multiply {
        nanos.checked_mul(factor).ok_or_else(|| overflow(cx))
    } else if factor == 0 {
        Err(fault(cx, FaultCode::DivisionByZero, nanos, 0, 0))
    } else {
        nanos.checked_div(factor).ok_or_else(|| overflow(cx))
    }
}

/// `TIME * n` and `TIME / n` by an unsigned integer, every factor to the largest `ULINT` taken
/// whole.
///
/// # Errors
///
/// `DivisionByZero` for `/ 0`, `Overflow` for a result no `TIME` holds.
pub fn scale_unsigned(cx: &mut Cx, nanos: i64, factor: u64, multiply: bool) -> Result<i64, Fault> {
    let magnitude = if multiply {
        nanos
            .unsigned_abs()
            .checked_mul(factor)
            .ok_or_else(|| overflow(cx))?
    } else {
        nanos
            .unsigned_abs()
            .checked_div(factor)
            .ok_or_else(|| fault(cx, FaultCode::DivisionByZero, nanos, 0, 0))?
    };
    if nanos < 0 && magnitude == 1u64 << 63 {
        return Ok(i64::MIN);
    }
    let magnitude = i64::try_from(magnitude).map_err(|_| overflow(cx))?;
    Ok(if nanos < 0 { -magnitude } else { magnitude })
}

/// `TIME * x` and `TIME / x` by a real, in binary64, the result cut toward zero.
///
/// # Errors
///
/// `Overflow` for a factor or a result that is not finite or no `TIME` holds, `DivisionByZero` for
/// `/ 0.0`.
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn scale_real(cx: &mut Cx, nanos: i64, factor: f64, multiply: bool) -> Result<i64, Fault> {
    if !factor.is_finite() {
        return Err(overflow(cx));
    }
    if !multiply && factor == 0.0 {
        return Err(fault(cx, FaultCode::DivisionByZero, nanos, 0, 0));
    }
    let result = if multiply {
        nanos as f64 * factor
    } else {
        nanos as f64 / factor
    };
    let truncated = libm::trunc(result);
    if !truncated.is_finite()
        || !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&truncated)
    {
        return Err(overflow(cx));
    }
    Ok(truncated as i64)
}
