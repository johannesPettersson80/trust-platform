//! A JSON value admitted as a value of its destination's declared type (specification 11 §6.8.1,
//! "Writing variables"), over the hand-written image of [`crate::live_values::typed_fixture`]. The
//! JSON forms are those in which `variables.read` gives each type's value; every expected value is
//! worked out from the form and the type's range by hand.

use alloc::boxed::Box;
use alloc::string::String;

use serde_json::{json, Value as Json};
use smol_str::SmolStr;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::value::{
    DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue, LTimeOfDayValue,
    TimeOfDayValue, Value,
};

use super::json_value;
use crate::live_values::typed_fixture::{image, types};

/// `json` admitted as a value of type `type_index` of the fixture.
fn admit(type_index: u32, json: &Json) -> Result<Value, String> {
    let bytes = image();
    let image = Image::parse(&bytes).expect("the fixture parses");
    json_value(&image, type_index, json)
}

#[test]
fn each_type_takes_the_form_variables_read_gives_it() {
    // One value per type, in variables.read's form: integers and bit strings as JSON integers at
    // their types' ends (SINT -128, ULINT 2^64 - 1, LWORD 2^64 - 1); a REAL's 1.5 and an LREAL's
    // integer 2; a TIME of 1.5 s as 1,500,000,000 ns; DATE 2026-11-19 as 20,411 days of
    // 86,400,000 ms = 1,763,510,400,000 ms; TOD 12:00 as 43,200,000 ms; DT 2026-11-19 12:00 as
    // 1,763,510,400,000 + 43,200,000 = 1,763,553,600,000 ms; the L forms in nanoseconds; a CHAR
    // and a WCHAR as one-character strings; a STRING[5] of five characters; Green of Color, which
    // is 1. Catches a form taken by another type's rule, a date in seconds or days, and a value cut
    // to fit.
    let date_ms: i64 = 20_411 * 86_400_000;
    assert_eq!(date_ms, 1_763_510_400_000);
    let cases: [(u32, Json, Value); 21] = [
        (types::BOOL, json!(true), Value::Bool(true)),
        (types::SINT, json!(-128), Value::SInt(-128)),
        (types::INT, json!(1000), Value::Int(1000)),
        (
            types::UDINT,
            json!(4_294_967_295_u64),
            Value::UDInt(u32::MAX),
        ),
        (types::ULINT, json!(u64::MAX), Value::ULInt(u64::MAX)),
        (types::BYTE, json!(255), Value::Byte(255)),
        (types::LWORD, json!(u64::MAX), Value::LWord(u64::MAX)),
        (types::REAL, json!(1.5), Value::Real(1.5)),
        (types::LREAL, json!(2), Value::LReal(2.0)),
        (
            types::TIME,
            json!(1_500_000_000),
            Value::Time(Duration::from_nanos(1_500_000_000)),
        ),
        (
            types::LTIME,
            json!(-5),
            Value::LTime(Duration::from_nanos(-5)),
        ),
        (
            types::DATE,
            json!(date_ms),
            Value::Date(DateValue::new(date_ms)),
        ),
        (
            types::LDATE,
            json!(date_ms * 1_000_000),
            Value::LDate(LDateValue::new(date_ms * 1_000_000)),
        ),
        (
            types::TOD,
            json!(43_200_000),
            Value::Tod(TimeOfDayValue::new(43_200_000)),
        ),
        (
            types::LTOD,
            json!(43_200_000_000_000_i64),
            Value::LTod(LTimeOfDayValue::new(43_200_000_000_000)),
        ),
        (
            types::DT,
            json!(1_763_553_600_000_i64),
            Value::Dt(DateTimeValue::new(1_763_553_600_000)),
        ),
        (
            types::LDT,
            json!(1_763_553_600_000_000_000_i64),
            Value::Ldt(LDateTimeValue::new(1_763_553_600_000_000_000)),
        ),
        (types::CHAR, json!("A"), Value::Char(b'A')),
        (types::WCHAR, json!("é"), Value::WChar(0xE9)),
        (
            types::STRING5,
            json!("pumps"),
            Value::String("pumps".into()),
        ),
        (
            types::COLOR,
            json!("Green"),
            Value::Enum(Box::new(EnumValue::from_canonical_parts(
                SmolStr::new("Color"),
                SmolStr::new("Green"),
                1,
            ))),
        ),
    ];
    for (type_index, json, expected) in cases {
        assert_eq!(admit(type_index, &json), Ok(expected), "{json}");
    }
}

#[test]
fn a_real_takes_the_values_no_json_number_holds() {
    // variables.read writes a NaN or an infinity as the strings NaN, Infinity and -Infinity; a
    // write takes them back. Catches a service that cannot write back a value it read.
    assert!(matches!(admit(types::REAL, &json!("NaN")), Ok(Value::Real(value)) if value.is_nan()));
    assert_eq!(
        admit(types::LREAL, &json!("Infinity")),
        Ok(Value::LReal(f64::INFINITY))
    );
    assert_eq!(
        admit(types::REAL, &json!("-Infinity")),
        Ok(Value::Real(f32::NEG_INFINITY))
    );
}

#[test]
fn a_value_the_type_does_not_take_is_refused_with_its_reason() {
    // SINT's range ends at 127, Percent's at 100; STRING[5] holds five characters; Purple is no
    // Color; a DATE of noon is no whole day; a text for an INT, a number for a BOOL and a string
    // for a TIME are of another form; the structure Point holds no single value. Nothing is cut to
    // fit or converted. Catches a value wrapped to the type's width, a text cut to its capacity and
    // a form taken by another type's rule.
    for (type_index, json, reason) in [
        (
            types::SINT,
            json!(128),
            "'128' is outside the range of SINT (-128..127)",
        ),
        (
            types::PERCENT,
            json!(101),
            "'101' is outside the range of Percent (0..100)",
        ),
        (
            types::STRING5,
            json!("pumpss"),
            "'\"pumpss\"' is longer than STRING[5] holds",
        ),
        (
            types::COLOR,
            json!("Purple"),
            "'Purple' is not a value of Color (Red, Green, Blue)",
        ),
        (types::DATE, json!(43_200_000), "'43200000' is no whole day"),
        (
            types::INT,
            json!("12"),
            "'\"12\"' is not a value of type INT",
        ),
        (types::BOOL, json!(1), "'1' is not a value of type BOOL"),
        (
            types::TIME,
            json!("1s"),
            "'\"1s\"' is not a value of type TIME",
        ),
        (
            types::POINT,
            json!(1),
            "Point holds no single value: write one of its members or elements",
        ),
    ] {
        assert_eq!(
            admit(type_index, &json),
            Err(String::from(reason)),
            "{json}"
        );
    }
}
