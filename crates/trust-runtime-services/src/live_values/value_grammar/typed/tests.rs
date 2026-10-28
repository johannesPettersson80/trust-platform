//! A value text admitted as a value of its destination's declared type (specification 11, "Runtime
//! control I/O and variable mutation contract"), over the hand-written image of
//! [`crate::live_values::typed_fixture`]. Each expected value is worked out from the literal and the
//! type's range by hand; each refusal names the reason the specification gives.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;

use smol_str::SmolStr;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::value::{
    DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue, LTimeOfDayValue,
    TimeOfDayValue, Value,
};

use super::{address_value, typed_value};
use crate::live_values::typed_fixture::{image, types};

/// `text` admitted as a value of type `type_index` of the fixture.
fn admit(type_index: u32, text: &str) -> Result<Value, String> {
    let bytes = image();
    let image = Image::parse(&bytes).expect("the fixture parses");
    typed_value(&image, type_index, text)
}

fn color(name: &str, number: i64) -> Value {
    Value::Enum(Box::new(EnumValue::from_canonical_parts(
        SmolStr::new("Color"),
        SmolStr::new(name),
        number,
    )))
}

#[test]
fn integers_take_their_type_when_they_fit_and_are_refused_when_they_do_not() {
    // Table 5 and Table 10: an integer literal takes the declared integer type when its value lies
    // in the type's range: SINT -128..127, USINT 0..255, ULINT up to 2^64 - 1; a based number is the
    // same number (16#7F is 127, 16#FF is 255 and outside SINT). A value outside the range is refused
    // with the range, never cut to fit. Catches a literal wrapped to the type's width (16#FF as SINT
    // -1) and a signed bit pattern taken for an unsigned type.
    for (type_index, text, expected) in [
        (types::SINT, "-128", Value::SInt(-128)),
        (types::SINT, "16#7F", Value::SInt(127)),
        (types::INT, "1_000", Value::Int(1000)),
        (types::INT, "INT#-123", Value::Int(-123)),
        (types::INT, "INT#16#7FFF", Value::Int(32_767)),
        (types::DINT, "-2147483648", Value::DInt(i32::MIN)),
        (types::LINT, "-9223372036854775808", Value::LInt(i64::MIN)),
        (types::USINT, "255", Value::USInt(255)),
        (types::UINT, "65535", Value::UInt(u16::MAX)),
        (types::UDINT, "4294967295", Value::UDInt(u32::MAX)),
        (types::ULINT, "18446744073709551615", Value::ULInt(u64::MAX)),
        (types::BYTE, "2#1111_1111", Value::Byte(255)),
        (types::BYTE, "8#377", Value::Byte(255)),
        (types::WORD, "WORD#16#AFF", Value::Word(0x0AFF)),
        (types::DWORD, "16#FFFF_FFFF", Value::DWord(u32::MAX)),
        (
            types::LWORD,
            "16#FFFF_FFFF_FFFF_FFFF",
            Value::LWord(u64::MAX),
        ),
    ] {
        assert_eq!(admit(type_index, text), Ok(expected), "{text}");
    }
    for (type_index, text, range) in [
        (types::SINT, "128", "SINT (-128..127)"),
        (types::SINT, "16#FF", "SINT (-128..127)"),
        (types::USINT, "-1", "USINT (0..255)"),
        (types::INT, "40000", "INT (-32768..32767)"),
        (types::BYTE, "256", "BYTE (0..255)"),
    ] {
        assert_eq!(
            admit(type_index, text),
            Err(format!("'{text}' is outside the range of {range}")),
            "{text}"
        );
    }
}

#[test]
fn a_boolean_takes_true_false_zero_and_one() {
    // Table 5 features 7 to 9: TRUE, FALSE, 0, 1, BOOL#1 and BOOL#FALSE are Boolean literals; 2 and
    // a based 2#1 are not. Catches 1 refused for a BOOL and any other integer taken as TRUE.
    for (text, expected) in [
        ("TRUE", true),
        ("false", false),
        ("1", true),
        ("0", false),
        ("BOOL#1", true),
        ("BOOL#FALSE", false),
    ] {
        assert_eq!(
            admit(types::BOOL, text),
            Ok(Value::Bool(expected)),
            "{text}"
        );
    }
    for text in ["2", "2#1", "'TRUE'"] {
        assert_eq!(
            admit(types::BOOL, text),
            Err(format!("'{text}' is not a value of type BOOL")),
            "{text}"
        );
    }
}

#[test]
fn reals_are_read_once_from_their_text_and_integers_only_when_held_exactly() {
    // Table 5 features 2 and 3: a REAL is read from its text as binary32, an LREAL as binary64; an
    // integer is a REAL only when binary32 holds it exactly: 16,777,216 (2^24) is, 16,777,217 is
    // not, though binary64 holds it. A real beyond binary32's range is refused, not made infinite.
    // Catches 3.5 refused, a REAL rounded through binary64, an integer rounded silently and an
    // infinity stored.
    for (type_index, text, expected) in [
        (types::REAL, "3.5", Value::Real(3.5)),
        (types::REAL, "-1.34E-12", Value::Real(-1.34e-12)),
        (types::REAL, "1.0e+6", Value::Real(1.0e6)),
        (types::REAL, "REAL#0.1", Value::Real(0.1)),
        (types::REAL, "3", Value::Real(3.0)),
        (types::REAL, "16777216", Value::Real(16_777_216.0)),
        (types::LREAL, "16777217", Value::LReal(16_777_217.0)),
        (types::LREAL, "1.23456_78", Value::LReal(1.234_567_8)),
        (types::LREAL, "REAL#2.5", Value::Real(2.5)),
    ] {
        assert_eq!(admit(type_index, text), Ok(expected), "{text}");
    }
    assert_eq!(
        admit(types::REAL, "16777217"),
        Err(String::from("'16777217' cannot be held exactly by REAL"))
    );
    assert_eq!(
        admit(types::REAL, "1.0E39"),
        Err(String::from("'1.0E39' is outside the range of REAL"))
    );
    assert_eq!(
        admit(types::INT, "3.5"),
        Err(String::from("'3.5' is not a value of type INT"))
    );
    assert_eq!(
        admit(types::REAL, "LREAL#2.5"),
        Err(String::from("'LREAL#2.5' is not a value of type REAL"))
    );
}

#[test]
fn durations_dates_and_times_of_day_take_their_own_kind() {
    // Tables 8 and 9 with specification 44 §6.8: a TIME and an LTIME hold nanoseconds; a DATE, a
    // TIME_OF_DAY and a DATE_AND_TIME milliseconds since 1970-01-01 or midnight, their long forms
    // nanoseconds. Worked: 2026-10-26 is day 20,752 (56 years with 14 leap days, 20,454, then 298
    // days of 2026), so 1,792,972,800,000 ms; 15:36:55.36 is 56,215,360 ms. An LTIME takes a TIME
    // literal, a TIME no LTIME one; a TIME_OF_DAY refuses a time finer than its millisecond. Catches
    // a date in days or seconds, a long value in milliseconds, and a fraction rounded away.
    let clock_ms = 56_215_360;
    for (type_index, text, expected) in [
        (
            types::TIME,
            "T#5s",
            Value::Time(Duration::from_nanos(5_000_000_000)),
        ),
        (
            types::TIME,
            "t#1h_30m",
            Value::Time(Duration::from_nanos(5_400_000_000_000)),
        ),
        (
            types::TIME,
            "TIME#-14ms",
            Value::Time(Duration::from_nanos(-14_000_000)),
        ),
        (
            types::LTIME,
            "T#5s",
            Value::LTime(Duration::from_nanos(5_000_000_000)),
        ),
        (
            types::DATE,
            "D#2026-10-26",
            Value::Date(DateValue::new(20_752 * 86_400_000)),
        ),
        (
            types::LDATE,
            "D#2026-10-26",
            Value::LDate(LDateValue::new(20_752 * 86_400_000_000_000)),
        ),
        (
            types::TOD,
            "TOD#15:36:55.36",
            Value::Tod(TimeOfDayValue::new(clock_ms)),
        ),
        (
            types::LTOD,
            "LTOD#15:36:55.360227400",
            Value::LTod(LTimeOfDayValue::new(56_215_360_227_400)),
        ),
        (
            types::DT,
            "DT#1984-06-25-15:36:55.36",
            Value::Dt(DateTimeValue::new(5289 * 86_400_000 + clock_ms)),
        ),
        (
            types::LDT,
            "LDT#1984-06-25-15:36:55.360_227_400",
            Value::Ldt(LDateTimeValue::new(
                5289 * 86_400_000_000_000 + 56_215_360_227_400,
            )),
        ),
    ] {
        assert_eq!(admit(type_index, text), Ok(expected), "{text}");
    }
    assert_eq!(
        admit(types::TIME, "LTIME#5s"),
        Err(String::from("'LTIME#5s' is not a value of type TIME"))
    );
    assert_eq!(
        admit(types::TOD, "TOD#12:00:00.0005"),
        Err(String::from(
            "'TOD#12:00:00.0005' is finer than the millisecond TIME_OF_DAY holds"
        ))
    );
    assert_eq!(
        admit(types::DATE, "T#5s"),
        Err(String::from("'T#5s' is not a value of type DATE"))
    );
}

#[test]
fn texts_and_characters_take_their_own_quotes_and_never_are_cut() {
    // Tables 6 and 7: a STRING takes a text in single quotes and a WSTRING one in double quotes or
    // a WSTRING# one in single quotes; a CHAR one character in single quotes or its code
    // (CHAR#16#41), a WCHAR one in double quotes or a WCHAR# one in single quotes. A text longer
    // than the type holds is refused, never cut: 'hello' fills STRING[5], 'hello!' does not. Each
    // reason quotes the request's text as it came, its own quotes included. Catches a text cut to
    // its capacity, a double-quoted text taken for a STRING, and a two-character text taken for a
    // CHAR.
    for (type_index, text, expected) in [
        (
            types::STRING5,
            "'hello'",
            Value::String(SmolStr::new("hello")),
        ),
        (
            types::STRING5,
            "'It$'s'",
            Value::String(SmolStr::new("It's")),
        ),
        (
            types::STRING5,
            "STRING#'OK'",
            Value::String(SmolStr::new("OK")),
        ),
        (
            types::WSTRING3,
            "\"a$00C4c\"",
            Value::WString(String::from("a\u{C4}c")),
        ),
        (
            types::WSTRING3,
            "WSTRING#'ab'",
            Value::WString(String::from("ab")),
        ),
        (types::CHAR, "'A'", Value::Char(b'A')),
        (types::CHAR, "CHAR#16#41", Value::Char(b'A')),
        (types::WCHAR, "\"\u{C4}\"", Value::WChar(0xC4)),
        (types::WCHAR, "WCHAR#'X'", Value::WChar(u16::from(b'X'))),
    ] {
        assert_eq!(admit(type_index, text), Ok(expected), "{text}");
    }
    assert_eq!(
        admit(types::STRING5, "'hello!'"),
        Err(String::from("''hello!'' is longer than STRING[5] holds"))
    );
    assert_eq!(
        admit(types::WSTRING3, "\"abcd\""),
        Err(String::from("'\"abcd\"' is longer than WSTRING[3] holds"))
    );
    for (type_index, text, name) in [
        (types::STRING5, "\"hello\"", "STRING[5]"),
        (types::WSTRING3, "'abc'", "WSTRING[3]"),
        (types::CHAR, "'AB'", "CHAR"),
        (types::CHAR, "\"A\"", "CHAR"),
        (types::STRING5, "hello", "STRING[5]"),
    ] {
        assert_eq!(
            admit(type_index, text),
            Err(format!("'{text}' is not a value of type {name}")),
            "{text}"
        );
    }
}

#[test]
fn an_enumeration_takes_its_values_by_name_bare_or_typed() {
    // §6.4.4.2: `Green` and `Color#Blue` are values of `Color` (Red = 0, Green = 1, Blue = 2),
    // names compared without case; a name it does not declare is refused with the names it does,
    // a value of another enumeration and a number are refused. Catches a value taken by its position
    // instead of its name, and a number written into an enumeration.
    assert_eq!(admit(types::COLOR, "Green"), Ok(color("Green", 1)));
    assert_eq!(admit(types::COLOR, "Color#Blue"), Ok(color("Blue", 2)));
    assert_eq!(admit(types::COLOR, "color#red"), Ok(color("Red", 0)));
    assert_eq!(
        admit(types::COLOR, "Purple"),
        Err(String::from(
            "'Purple' is not a value of Color (Red, Green, Blue)"
        ))
    );
    assert_eq!(
        admit(types::COLOR, "Mode#Red"),
        Err(String::from("'Mode#Red' is not a value of type Color"))
    );
    assert_eq!(
        admit(types::COLOR, "1"),
        Err(String::from("'1' is not a value of type Color"))
    );
}

#[test]
fn a_subrange_and_a_compound_type_are_checked_as_the_boundary_checks_them() {
    // A subrange is its base type within its bounds: Percent is INT(0..100), so 100 fits and 101 is
    // refused with the bounds; a structure and an array hold no single value. Catches a subrange
    // checked against its base type's range alone.
    assert_eq!(admit(types::PERCENT, "100"), Ok(Value::Int(100)));
    assert_eq!(
        admit(types::PERCENT, "101"),
        Err(String::from(
            "'101' is outside the range of Percent (0..100)"
        ))
    );
    assert_eq!(
        admit(types::POINT, "1"),
        Err(String::from(
            "Point holds no single value: write one of its members or elements"
        ))
    );
    assert_eq!(
        admit(types::INTS, "1"),
        Err(String::from(
            "ARRAY[1..3] OF INT holds no single value: write one of its members or elements"
        ))
    );
    assert_eq!(
        admit(types::INT, "abc def"),
        Err(String::from("unsupported value 'abc def'"))
    );
}

#[test]
fn a_bound_address_takes_its_variable_s_literal_in_the_image_s_form() {
    // Specification 11 (typed values at an address) and specification 44 §5.6: at %MD4, bound to the
    // REAL `Setpoint`, 3.5 is written as the bits of binary32 3.5, 16#40600000; at %MW2, bound to the
    // INT `Level`, -1 is its two's complement 16#FFFF and 40000 is refused; at %MX0.1, bound to the
    // BOOL `Lamp`, 1 is TRUE; at %QD0, bound to the TIME `Delay` in whole milliseconds, T#1.5s is
    // 1,500 and T#1.5ms finer than a millisecond. An address no variable is bound to keeps the
    // untyped grammar. Catches a REAL written as the integer 3, a TIME written in nanoseconds, and an
    // unbound address typed.
    let bytes = image();
    let image = Image::parse(&bytes).expect("the fixture parses");
    let at = |address: &str, text: &str| {
        address_value(
            &image,
            &IoAddress::parse(address).expect("an address"),
            text,
        )
    };
    assert_eq!(at("%MD4", "3.5"), Some(Ok(Value::DWord(0x4060_0000))));
    assert_eq!(at("%MW2", "-1"), Some(Ok(Value::Word(0xFFFF))));
    assert_eq!(
        at("%MW2", "40000"),
        Some(Err(String::from(
            "'40000' is outside the range of INT (-32768..32767)"
        )))
    );
    assert_eq!(at("%MX0.1", "1"), Some(Ok(Value::Bool(true))));
    assert_eq!(at("%QD0", "T#1.5s"), Some(Ok(Value::DWord(1500))));
    assert_eq!(
        at("%QD0", "T#1.5ms"),
        Some(Err(String::from(
            "'T#1.5ms' is finer than the millisecond its address holds"
        )))
    );
    assert_eq!(at("%MB7", "3.5"), None);
    assert_eq!(at("%MX0.2", "TRUE"), None);
}
