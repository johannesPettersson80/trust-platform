//! The syntax of a value's IEC 61131-3 literal (IEC 61131-3 Ed.3 §6.3.2 to §6.3.5, Tables 5 to 9;
//! §6.4.4.2), as the language's tokens read it.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;

use super::{parse_literal, Literal};

fn integer(value: i128, based: bool) -> Literal {
    Literal::Integer { value, based }
}

fn text(wide: bool, text: &str) -> Literal {
    Literal::Text {
        wide,
        text: String::from(text),
    }
}

fn typed(type_name: &str, literal: Literal) -> Literal {
    Literal::Typed {
        type_name: String::from(type_name),
        literal: Box::new(literal),
    }
}

#[test]
fn numbers_read_as_table_5_writes_them() {
    // Table 5: a decimal integer may have a sign and single underscores between digits; a based
    // number (2#, 8#, 16#) has no sign and is a bit string; a real has a decimal point and may have
    // an exponent, kept as its text so that its destination reads it once. Worked: 2#1111_1111,
    // 8#377 and 16#FF are 255; 123_4 is 1234. Catches underscores taken as digits, a based number
    // read in base 10, and a real parsed through another type before its destination reads it.
    for (source, expected) in [
        ("-12", integer(-12, false)),
        ("+986", integer(986, false)),
        ("123_4", integer(1234, false)),
        ("2#1111_1111", integer(255, true)),
        ("8#377", integer(255, true)),
        ("16#ff", integer(255, true)),
        (
            "16#FFFF_FFFF_FFFF_FFFF",
            integer(i128::from(u64::MAX), true),
        ),
        ("3.14159_26", Literal::Real(String::from("3.1415926"))),
        ("-1.34E-12", Literal::Real(String::from("-1.34E-12"))),
        ("1.0e+6", Literal::Real(String::from("1.0E+6"))),
        (" TRUE ", Literal::Bool(true)),
        ("false", Literal::Bool(false)),
    ] {
        assert_eq!(parse_literal(source), Ok(expected), "{source}");
    }
}

#[test]
fn a_text_that_is_no_literal_keeps_the_stable_refusal() {
    // Specification 11: a text that is no literal answers `unsupported value '<text>'`, its own text.
    // Each one breaks a rule of Table 5: a real without a decimal point (`1e3`), a sign before a based
    // number, a doubled or trailing underscore, a digit outside the base, no digits after the point,
    // a second prefix. Catches a grammar laxer than the language's tokens.
    for source in [
        "1e3",
        "-16#FF",
        "1__0",
        "10_",
        "2#102",
        "8#8",
        "16#",
        "1.",
        ".5",
        "1.5e",
        "INT#INT#5",
        "",
        "a b",
        "3,5",
        "16#FF.0",
    ] {
        assert_eq!(
            parse_literal(source),
            Err(format!("unsupported value '{source}'")),
            "{source}"
        );
    }
}

#[test]
fn durations_read_as_table_8_writes_them() {
    // Table 8, worked by hand in nanoseconds: T#14ms is 14,000,000; T#-14ms its negative;
    // t#25h_15m is 25 h 15 min = 90,900 s; LTIME#5m_30s_500ms_100.1us is 330.5001001 s =
    // 330,500,100,100 ns; T#14.7s is 14,700,000,000; units take any case. Catches a unit read as
    // another (`m` as `ms`), the sign dropped, the long prefix forgotten, and a fraction lost.
    for (source, long, nanos) in [
        ("T#14ms", false, 14_000_000),
        ("T#-14ms", false, -14_000_000),
        ("t#25h_15m", false, 90_900_000_000_000),
        ("TIME#14.7S", false, 14_700_000_000),
        ("LTIME#5m_30s_500ms_100.1us", true, 330_500_100_100),
        ("LT#1d", true, 86_400_000_000_000),
    ] {
        assert_eq!(
            parse_literal(source),
            Ok(Literal::Duration { long, nanos }),
            "{source}"
        );
    }
    // A duration finer than a nanosecond is held by no duration, and is refused, not rounded.
    assert_eq!(
        parse_literal("T#1.5ns"),
        Err(String::from(
            "'1.5ns' is finer than the nanosecond a duration holds"
        ))
    );
    for source in ["T#", "T#5", "T#5x", "T#1h__5m", "T#.5s", "T#5s_"] {
        assert_eq!(
            parse_literal(source),
            Err(format!("unsupported value '{source}'")),
            "{source}"
        );
    }
}

#[test]
fn dates_and_times_of_day_read_as_table_9_writes_them() {
    // Table 9, worked by hand: 1984-06-25 is 5,289 days after 1970-01-01 (14 years with 3 leap
    // days, 5,113, then 152 days to June and 24 more); 15:36:55.36 is 56,215 s and 360 ms after
    // midnight; the fraction of a date and time may have underscores. Catches a date counted from
    // another day, a fraction read as a whole number of milliseconds, and a long prefix lost.
    let clock = (15 * 3600 + 36 * 60 + 55) * 1_000_000_000i64;
    for (source, expected) in [
        (
            "D#1984-06-25",
            Literal::Date {
                long: false,
                days: 5289,
            },
        ),
        (
            "ldate#1984-06-25",
            Literal::Date {
                long: true,
                days: 5289,
            },
        ),
        (
            "TOD#15:36:55.36",
            Literal::TimeOfDay {
                long: false,
                nanos: clock + 360_000_000,
            },
        ),
        (
            "LTIME_OF_DAY#15:36:55.360227400",
            Literal::TimeOfDay {
                long: true,
                nanos: clock + 360_227_400,
            },
        ),
        (
            "LDT#1984-06-25-15:36:55.360_227_400",
            Literal::DateAndTime {
                long: true,
                days: 5289,
                nanos: clock + 360_227_400,
            },
        ),
        (
            "DATE_AND_TIME#1984-06-25-15:36:55",
            Literal::DateAndTime {
                long: false,
                days: 5289,
                nanos: clock,
            },
        ),
    ] {
        assert_eq!(parse_literal(source), Ok(expected), "{source}");
    }
    // A day that does not exist and an hour past the day are no value, and are refused as such.
    assert_eq!(
        parse_literal("D#2026-02-30"),
        Err(String::from("'2026-02-30' is no date"))
    );
    assert_eq!(
        parse_literal("TOD#24:00:00"),
        Err(String::from("'24:00:00' is no time of day"))
    );
    assert_eq!(
        parse_literal("D#2026-2-3"),
        Err(String::from("unsupported value 'D#2026-2-3'"))
    );
}

#[test]
fn texts_read_as_tables_6_and_7_write_them() {
    // Tables 6 and 7: a single-byte text in single quotes takes `$hh`, a double-byte one in double
    // quotes `$hhhh`; `$$`, `$L`/`$N`, `$P`, `$R`, `$T` in any case, `$'` only in single and `$"`
    // only in double quotes; the other quote stands as itself. Catches an escape read in the wrong
    // kind of text, two hex digits taken where four are, and a quote of the other kind refused.
    for (source, expected) in [
        ("'It$'s'", text(false, "It's")),
        ("'$41$42'", text(false, "AB")),
        ("'a$nb$T$$'", text(false, "a\nb\t$")),
        ("'\"'", text(false, "\"")),
        ("\"$00C4\"", text(true, "\u{C4}")),
        ("\"'$\"\"", text(true, "'\"")),
        ("''", text(false, "")),
    ] {
        assert_eq!(parse_literal(source), Ok(expected), "{source}");
    }
    for source in ["'a$\"'", "\"a$'\"", "'it's'", "'$4'", "\"$41\"", "'open"] {
        assert_eq!(
            parse_literal(source),
            Err(format!("unsupported value '{source}'")),
            "{source}"
        );
    }
}

#[test]
fn typed_literals_and_names_keep_their_type_and_their_spelling() {
    // Table 5 feature 9, Table 6 features 3 and 4 and §6.4.4.2: a prefix names the literal's type,
    // a sign may follow the `#`, a based number too; a name is an enumeration's value. Catches a
    // prefix taken for a based number and the sign after `#` refused.
    for (source, expected) in [
        ("INT#-123", typed("INT", integer(-123, false))),
        ("INT#16#7FFF", typed("INT", integer(32_767, true))),
        (
            "REAL#3.5",
            typed("REAL", Literal::Real(String::from("3.5"))),
        ),
        ("BOOL#TRUE", typed("BOOL", Literal::Bool(true))),
        ("WSTRING#'OK'", typed("WSTRING", text(false, "OK"))),
        (
            "Color#Red",
            typed("Color", Literal::Name(String::from("Red"))),
        ),
        ("Green", Literal::Name(String::from("Green"))),
    ] {
        assert_eq!(parse_literal(source), Ok(expected), "{source}");
    }
}
