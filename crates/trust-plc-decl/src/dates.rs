//! The date and time-of-day literals of a Rust POU's initial values (IEC §6.3.5, Table 9):
//! `DATE`, `TIME_OF_DAY` and `DATE_AND_TIME` and their long forms, each read into the value the
//! program's areas hold (specification 44 §6.2; specification 10 §2): milliseconds since
//! 1970-01-01, a time of day's since midnight, and nanoseconds for the long forms. The days are
//! counted by the program library's calendar, the code `CONCAT_DATE` runs.

use std::fmt::Write as _;

use trust_plc_lib::{civil_days, split_date, NANOS_PER_DAY, NANOS_PER_TICK, TICKS_PER_DAY};

use crate::Elementary;

/// The prefixes of each kind's literals, the longest first, so that none is taken for a shorter
/// one.
const DATE_PREFIXES: [&str; 4] = ["LDATE#", "DATE#", "LD#", "D#"];
const TOD_PREFIXES: [&str; 4] = ["LTIME_OF_DAY#", "TIME_OF_DAY#", "LTOD#", "TOD#"];
const DT_PREFIXES: [&str; 4] = ["LDATE_AND_TIME#", "DATE_AND_TIME#", "LDT#", "DT#"];

/// What a date type holds and how its literals are written.
struct Kind {
    /// The prefixes a literal of it may have: its kind's, short or long.
    prefixes: [&'static str; 4],
    /// The prefix its literal is written with.
    written: &'static str,
    /// Whether it holds a date.
    date: bool,
    /// Whether it holds a time of day.
    clock: bool,
    /// Whether it counts nanoseconds, a long form, instead of milliseconds.
    long: bool,
}

/// What the date type `ty` holds; `None` for another type.
fn kind(ty: Elementary) -> Option<Kind> {
    let (prefixes, written, date, clock, long) = match ty {
        Elementary::Date => (DATE_PREFIXES, "DATE#", true, false, false),
        Elementary::LDate => (DATE_PREFIXES, "LDATE#", true, false, true),
        Elementary::Tod => (TOD_PREFIXES, "TOD#", false, true, false),
        Elementary::LTod => (TOD_PREFIXES, "LTOD#", false, true, true),
        Elementary::Dt => (DT_PREFIXES, "DT#", true, true, false),
        Elementary::Ldt => (DT_PREFIXES, "LDT#", true, true, true),
        _ => return None,
    };
    Some(Kind {
        prefixes,
        written,
        date,
        clock,
        long,
    })
}

/// Whether `ty` is a date or a time of day, with or without the other, or a long form of one.
pub(crate) fn is_date(ty: Elementary) -> bool {
    kind(ty).is_some()
}

/// The initial value that the IEC literal `text` gives a variable of the date type `ty`
/// (specification 48 RU-26).
pub(crate) fn initial(text: &syn::LitStr, ty: Elementary) -> Result<crate::Value, syn::Error> {
    value(&text.value(), ty)
        .map(|value| crate::Value::Instant { ty, value })
        .ok_or_else(|| {
            let unit = if kind(ty).is_some_and(|kind| kind.long) {
                "nanoseconds"
            } else {
                "milliseconds"
            };
            syn::Error::new(
                text.span(),
                format!(
                    "a {} is an IEC literal of its kind that exists, in whole {unit} within 64 \
                     bits: `\"D#2024-02-29\"`, `\"TOD#12:30:15.25\"`, \
                     `\"DT#2024-02-29-12:30:15\"` (IEC §6.3.5, Table 9)",
                    ty.iec()
                ),
            )
        })
}

/// The value a variable of the date type `ty` holds for the literal `text`, as its areas store it:
/// a literal of the type's kind, short or long (`D#`, `DATE#`, `LD#`, `LDATE#` for a date), then
/// `year-month-day`, `hour:minute:second` with the seconds a fixed-point number, or both joined by
/// `-`, `_` anywhere between digits. `None` for another text, a date or a time of day that does not
/// exist, a fraction finer than the type keeps (a millisecond; a long form's nanosecond) and a
/// value beyond 64 bits.
pub(crate) fn value(text: &str, ty: Elementary) -> Option<i64> {
    let kind = kind(ty)?;
    let upper = text.trim().to_ascii_uppercase();
    // The prefixes hold `_` themselves (`TIME_OF_DAY#`), so only the digits after them lose it.
    let body = kind
        .prefixes
        .iter()
        .find_map(|prefix| upper.strip_prefix(prefix))?
        .replace('_', "");
    let mut rest = body.as_str();
    let mut days: i64 = 0;
    if kind.date {
        let (year, after) = number(rest)?;
        let (month, after) = number(after.strip_prefix('-')?)?;
        let (day, after) = number(after.strip_prefix('-')?)?;
        days = civil_days(year, month, day)?;
        rest = after;
        if kind.clock {
            rest = rest.strip_prefix('-')?;
        }
    }
    let nanos = if kind.clock { clock_nanos(rest)? } else { 0 };
    if !kind.clock && !rest.is_empty() {
        return None;
    }
    if kind.long {
        return days.checked_mul(NANOS_PER_DAY)?.checked_add(nanos);
    }
    if nanos % NANOS_PER_TICK != 0 {
        return None;
    }
    days.checked_mul(TICKS_PER_DAY)?
        .checked_add(nanos / NANOS_PER_TICK)
}

/// The leading digits of `text` as a number, and the text after them; `None` without a digit or
/// for a number beyond 64 bits.
fn number(text: &str) -> Option<(i64, &str)> {
    let end = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());
    if end == 0 {
        return None;
    }
    let (digits, rest) = text.split_at(end);
    Some((digits.parse().ok()?, rest))
}

/// The nanoseconds since midnight of the whole text `text`, `hour:minute:second` with the seconds a
/// fixed-point number; `None` for another text, a time of day that does not exist, and a fraction
/// finer than a nanosecond.
fn clock_nanos(text: &str) -> Option<i64> {
    let (hour, rest) = number(text)?;
    let (minute, rest) = number(rest.strip_prefix(':')?)?;
    let (second, rest) = number(rest.strip_prefix(':')?)?;
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let fraction = match rest.strip_prefix('.') {
        None if rest.is_empty() => 0,
        Some(digits) if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) => {
            let (kept, beyond) = digits.split_at(digits.len().min(9));
            if beyond.bytes().any(|byte| byte != b'0') {
                return None;
            }
            let scale = 10_i64.pow(9 - u32::try_from(kept.len()).ok()?);
            kept.parse::<i64>().ok()? * scale
        }
        _ => return None,
    };
    Some(((hour * 60 + minute) * 60 + second) * 1_000_000_000 + fraction)
}

/// The Structured Text literal of `value`, a value a variable of the date type `ty` holds:
/// `DATE#2024-02-29`, `TOD#12:30:15.25`, `LDT#2024-02-29-12:30:15.000000001`.
pub(crate) fn literal(ty: Elementary, value: i64) -> String {
    let Some(kind) = kind(ty) else {
        return value.to_string();
    };
    let per_day = if kind.long {
        NANOS_PER_DAY
    } else {
        TICKS_PER_DAY
    };
    let (days, into_day) = if kind.date {
        (value.div_euclid(per_day), value.rem_euclid(per_day))
    } else {
        (0, value)
    };
    let mut text = kind.written.to_string();
    if kind.date {
        let [year, month, day] = split_date(days * TICKS_PER_DAY);
        let _ = write!(text, "{year:04}-{month:02}-{day:02}");
        if kind.clock {
            text.push('-');
        }
    }
    if kind.clock {
        let nanos = if kind.long {
            into_day
        } else {
            into_day * NANOS_PER_TICK
        };
        let seconds = nanos / 1_000_000_000;
        let _ = write!(
            text,
            "{:02}:{:02}:{:02}",
            seconds / 3_600,
            seconds / 60 % 60,
            seconds % 60
        );
        let fraction = nanos % 1_000_000_000;
        if fraction != 0 {
            let digits = format!("{fraction:09}");
            let _ = write!(text, ".{}", digits.trim_end_matches('0'));
        }
    }
    text
}

#[cfg(test)]
#[path = "dates_tests.rs"]
mod tests;
