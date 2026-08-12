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
    let y = i128::from(year) - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = i128::from(month) + if month > 2 { -3 } else { 9 };
    let doy = (153 * m + 2) / 5 + i128::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i64::try_from(era * 146097 + doe - 719468).map_err(|_| DateTimeCalcError::Overflow)
}

pub fn ticks_per_day(profile: DateTimeProfile) -> Result<i64, DateTimeCalcError> {
    let res = profile.resolution.as_nanos();
    if res <= 0 || res > NANOS_PER_DAY || NANOS_PER_DAY % res != 0 {
        return Err(DateTimeCalcError::InvalidResolution);
    }
    Ok(NANOS_PER_DAY / res)
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
    ticks_per_day(profile)?;
    let res = profile.resolution.as_nanos();
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
    use super::{
        days_from_civil, days_to_ticks, nanos_to_ticks, ticks_per_day, DateTimeCalcError,
        DivisionMode, NANOS_PER_DAY,
    };
    use crate::value::{DateTimeProfile, DateValue, Duration};

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

    #[test]
    fn civil_day_conversion_handles_epoch_gregorian_and_extreme_year_boundaries() {
        assert_eq!(days_from_civil(1970, 1, 1), Ok(0));
        assert_eq!(days_from_civil(1969, 12, 31), Ok(-1));

        let leap_span =
            days_from_civil(2000, 3, 1).unwrap() - days_from_civil(2000, 2, 28).unwrap();
        let common_span =
            days_from_civil(1900, 3, 1).unwrap() - days_from_civil(1900, 2, 28).unwrap();
        assert_eq!(leap_span, 2);
        assert_eq!(common_span, 1);

        for (year, month, day) in [
            (2024, 0, 1),
            (2024, 13, 1),
            (2024, 1, 0),
            (2024, 1, 32),
            (1900, 2, 29),
        ] {
            assert_eq!(
                days_from_civil(year, month, day),
                Err(DateTimeCalcError::InvalidDate)
            );
        }
        assert_eq!(
            days_from_civil(i64::MIN, 1, 1),
            Err(DateTimeCalcError::Overflow)
        );
        assert_eq!(
            days_from_civil(i64::MAX, 12, 31),
            Err(DateTimeCalcError::Overflow)
        );
    }

    #[test]
    fn date_time_profile_rejects_unrepresentable_day_resolutions() {
        assert_eq!(ticks_per_day(DateTimeProfile::default()), Ok(86_400_000));
        assert_eq!(
            ticks_per_day(profile(0, NANOS_PER_DAY)),
            Ok(1),
            "one complete day is the coarsest representable resolution"
        );
        assert_eq!(
            ticks_per_day(profile(0, 0)),
            Err(DateTimeCalcError::InvalidResolution)
        );
        assert_eq!(
            ticks_per_day(profile(0, -1)),
            Err(DateTimeCalcError::InvalidResolution)
        );
        assert_eq!(
            ticks_per_day(profile(0, NANOS_PER_DAY + 1)),
            Err(DateTimeCalcError::InvalidResolution)
        );
        assert_eq!(
            ticks_per_day(profile(0, 7)),
            Err(DateTimeCalcError::InvalidResolution),
            "a day must contain an integral number of profile ticks"
        );
        assert_eq!(
            days_to_ticks(1, profile(0, 7)),
            Err(DateTimeCalcError::InvalidResolution)
        );
        assert_eq!(
            nanos_to_ticks(1, profile(0, 7), DivisionMode::Trunc),
            Err(DateTimeCalcError::InvalidResolution)
        );
        assert_eq!(
            nanos_to_ticks(
                NANOS_PER_DAY,
                profile(0, NANOS_PER_DAY + 1),
                DivisionMode::Euclid,
            ),
            Err(DateTimeCalcError::InvalidResolution)
        );
    }

    #[test]
    fn signed_tick_conversion_preserves_epoch_and_division_mode() {
        let seconds = profile(10, 1_000_000_000);
        assert_eq!(days_to_ticks(2, seconds), Ok(172_810));
        assert_eq!(
            days_to_ticks(1, profile(i64::MAX, NANOS_PER_DAY)),
            Err(DateTimeCalcError::Overflow)
        );
        assert_eq!(
            days_to_ticks(i64::MAX, DateTimeProfile::default()),
            Err(DateTimeCalcError::Overflow)
        );

        assert_eq!(
            nanos_to_ticks(-1_500_000_000, seconds, DivisionMode::Trunc),
            Ok(-1)
        );
        assert_eq!(
            nanos_to_ticks(-1_500_000_000, seconds, DivisionMode::Euclid),
            Ok(-2)
        );
        assert_eq!(
            nanos_to_ticks(1_500_000_000, seconds, DivisionMode::Trunc),
            Ok(1)
        );
        assert_eq!(
            nanos_to_ticks(1_500_000_000, seconds, DivisionMode::Euclid),
            Ok(1)
        );
        assert_eq!(
            nanos_to_ticks(1, profile(0, 0), DivisionMode::Trunc),
            Err(DateTimeCalcError::InvalidResolution)
        );
    }

    fn profile(epoch_ticks: i64, resolution_nanos: i64) -> DateTimeProfile {
        DateTimeProfile {
            epoch: DateValue::new(epoch_ticks),
            resolution: Duration::from_nanos(resolution_nanos),
        }
    }
}
