//! A Rust POU's date literals read into the values the program's areas hold (specification 48
//! RU-26; specification 44 §6.2), each worked out by hand: 2024-01-01 is day 54 × 365 + 13 leap
//! days = 19,723 after 1970-01-01, so 2024-02-29 is day 19,782, 1,709,164,800,000 ms; 12:30:15 is
//! 45,015 s after midnight.

use super::{literal, value};
use crate::Elementary;

const FEB_29_MS: i64 = 1_709_164_800_000;
const CLOCK_S: i64 = 45_015;

#[test]
fn a_date_literal_reads_into_milliseconds_or_nanoseconds_since_1970() {
    // RU-26: a DATE, TOD and DT hold milliseconds, their long forms nanoseconds, a time of day
    // since midnight. Catches a day counted off by one, a leap day lost, a time of day counted
    // from the epoch, a long form left in milliseconds and a fraction dropped.
    for (text, ty, expected) in [
        ("D#1970-01-01", Elementary::Date, 0),
        ("D#2024-02-29", Elementary::Date, FEB_29_MS),
        ("DATE#1969-12-31", Elementary::Date, -86_400_000),
        ("d#2024-02-29", Elementary::Date, FEB_29_MS),
        ("LDATE#2024-02-29", Elementary::LDate, FEB_29_MS * 1_000_000),
        ("LD#2024-02-29", Elementary::LDate, FEB_29_MS * 1_000_000),
        ("D#2024-02-29", Elementary::LDate, FEB_29_MS * 1_000_000),
        ("TOD#12:30:15.25", Elementary::Tod, CLOCK_S * 1_000 + 250),
        ("TIME_OF_DAY#00:00:00", Elementary::Tod, 0),
        (
            "LTOD#12:30:15.000000001",
            Elementary::LTod,
            CLOCK_S * 1_000_000_000 + 1,
        ),
        (
            "LTOD#12:00:00.0005",
            Elementary::LTod,
            43_200_000_000_000 + 500_000,
        ),
        (
            "LTIME_OF_DAY#12:00:00.1234567890",
            Elementary::LTod,
            43_200_123_456_789,
        ),
        (
            "DT#2024-02-29-12:30:15",
            Elementary::Dt,
            FEB_29_MS + CLOCK_S * 1_000,
        ),
        (
            "DATE_AND_TIME#2024-02-29-12:30:15.1_5",
            Elementary::Dt,
            FEB_29_MS + CLOCK_S * 1_000 + 150,
        ),
        (
            "LDT#2024-02-29-12:30:15.5",
            Elementary::Ldt,
            (FEB_29_MS + CLOCK_S * 1_000) * 1_000_000 + 500_000_000,
        ),
        (
            "LDATE_AND_TIME#2024-02-29-12:30:15",
            Elementary::Ldt,
            (FEB_29_MS + CLOCK_S * 1_000) * 1_000_000,
        ),
    ] {
        assert_eq!(value(text, ty), Some(expected), "{text} as {ty:?}");
    }
}

#[test]
fn a_date_that_does_not_exist_or_does_not_fit_is_refused() {
    // RU-26: a literal is of the variable's kind, names a date and a time of day that exist, and
    // keeps no fraction finer than the type holds. Catches 2023-02-29 taken for 2023-03-01, an
    // hour 24, a TOD rounded to its millisecond, an LDATE past 2262 wrapped, and a literal of
    // another kind accepted.
    for (text, ty) in [
        ("D#2023-02-29", Elementary::Date),
        ("D#2024-13-01", Elementary::Date),
        ("D#2024-04-31", Elementary::Date),
        ("D#2024-02-29-12:00:00", Elementary::Date),
        ("T#1s", Elementary::Date),
        ("TOD#24:00:00", Elementary::Tod),
        ("TOD#12:60:00", Elementary::Tod),
        ("TOD#12:00:60", Elementary::Tod),
        ("TOD#12:00", Elementary::Tod),
        ("TOD#12:00:00.0005", Elementary::Tod),
        ("TOD#12:00:00.", Elementary::Tod),
        ("LTOD#12:00:00.1234567891", Elementary::LTod),
        ("DT#2024-02-29", Elementary::Dt),
        ("D#2024-02-29", Elementary::Dt),
        ("DT#2024-02-29 12:00:00", Elementary::Dt),
        ("LDATE#2300-01-01", Elementary::LDate),
        ("LDT#2262-04-12-00:00:00", Elementary::Ldt),
    ] {
        assert_eq!(value(text, ty), None, "{text} as {ty:?}");
    }
}

#[test]
fn a_date_value_is_written_as_its_types_literal() {
    // RU-26: the generated declaration carries the value as the literal of the variable's own
    // type, which reads back to the same value. Catches a long form written with a short prefix, a
    // date before 1970 written as the next day, and a fraction's trailing zeros kept or digits
    // lost.
    for (ty, stored, text) in [
        (Elementary::Date, FEB_29_MS, "DATE#2024-02-29"),
        (Elementary::Date, -86_400_000, "DATE#1969-12-31"),
        (Elementary::LDate, FEB_29_MS * 1_000_000, "LDATE#2024-02-29"),
        (Elementary::Tod, CLOCK_S * 1_000 + 250, "TOD#12:30:15.25"),
        (
            Elementary::LTod,
            CLOCK_S * 1_000_000_000 + 1,
            "LTOD#12:30:15.000000001",
        ),
        (Elementary::Dt, 0, "DT#1970-01-01-00:00:00"),
        (
            Elementary::Dt,
            FEB_29_MS + CLOCK_S * 1_000,
            "DT#2024-02-29-12:30:15",
        ),
        (
            Elementary::Ldt,
            (FEB_29_MS + CLOCK_S * 1_000) * 1_000_000 + 500_000_000,
            "LDT#2024-02-29-12:30:15.5",
        ),
    ] {
        assert_eq!(literal(ty, stored), text, "{ty:?}");
        assert_eq!(value(text, ty), Some(stored), "{text} reads back");
    }
}
