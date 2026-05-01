//! Shared date/time calculation helpers.

#![allow(missing_docs)]

use crate::value::DateTimeProfile;

pub const NANOS_PER_DAY: i64 = 86_400_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeCalcError {
    InvalidDate,
    InvalidResolution,
    Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivisionMode {
    Trunc,
    Euclid,
}

pub fn days_from_civil(year: i64, month: i64, day: i64) -> Result<i64, DateTimeCalcError> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(DateTimeCalcError::InvalidDate);
    }
    if day > days_in_month(year, month)? {
        return Err(DateTimeCalcError::InvalidDate);
    }
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146097 + doe - 719468)
}

pub fn ticks_per_day(profile: DateTimeProfile) -> Result<i64, DateTimeCalcError> {
    let res = profile.resolution.as_nanos();
    if res <= 0 {
        return Err(DateTimeCalcError::InvalidResolution);
    }
    NANOS_PER_DAY
        .checked_div(res)
        .ok_or(DateTimeCalcError::Overflow)
}

pub fn days_to_ticks(days: i64, profile: DateTimeProfile) -> Result<i64, DateTimeCalcError> {
    let per_day = ticks_per_day(profile)?;
    days.checked_mul(per_day)
        .and_then(|v| v.checked_add(profile.epoch.ticks()))
        .ok_or(DateTimeCalcError::Overflow)
}

pub fn nanos_to_ticks(
    nanos: i64,
    profile: DateTimeProfile,
    mode: DivisionMode,
) -> Result<i64, DateTimeCalcError> {
    let res = profile.resolution.as_nanos();
    if res <= 0 {
        return Err(DateTimeCalcError::InvalidResolution);
    }
    match mode {
        DivisionMode::Trunc => nanos.checked_div(res).ok_or(DateTimeCalcError::Overflow),
        DivisionMode::Euclid => Ok(nanos.div_euclid(res)),
    }
}

fn days_in_month(year: i64, month: i64) -> Result<i64, DateTimeCalcError> {
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year)? {
                29
            } else {
                28
            }
        }
        _ => return Err(DateTimeCalcError::InvalidDate),
    };
    Ok(days)
}

fn is_leap_year(year: i64) -> Result<bool, DateTimeCalcError> {
    let divisible_by_4 = year.checked_rem(4).ok_or(DateTimeCalcError::Overflow)? == 0;
    let divisible_by_100 = year.checked_rem(100).ok_or(DateTimeCalcError::Overflow)? == 0;
    let divisible_by_400 = year.checked_rem(400).ok_or(DateTimeCalcError::Overflow)? == 0;
    Ok(divisible_by_4 && (!divisible_by_100 || divisible_by_400))
}

#[cfg(test)]
mod tests {
    use super::{days_from_civil, DateTimeCalcError};

    #[test]
    fn rejects_invalid_non_leap_day() {
        assert_eq!(
            days_from_civil(2023, 2, 29),
            Err(DateTimeCalcError::InvalidDate)
        );
    }

    #[test]
    fn rejects_invalid_month_length() {
        assert_eq!(
            days_from_civil(2023, 4, 31),
            Err(DateTimeCalcError::InvalidDate)
        );
    }
}
