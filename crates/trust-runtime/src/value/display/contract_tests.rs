use super::*;

use std::sync::Arc;

use crate::memory::{InstanceId, MemoryLocation};
use crate::value::{
    ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, RefPath, StructValue, TimeOfDayValue, ValueRef,
};
use indexmap::IndexMap;

#[test]
fn booleans_use_structured_text_keywords() {
    assert_eq!(format_user_value(&Value::Bool(true)), "TRUE");
    assert_eq!(format_user_value(&Value::Bool(false)), "FALSE");
}

#[test]
fn every_integer_and_bit_string_tag_uses_unprefixed_decimal_text() {
    let cases = [
        (Value::SInt(-8), "-8"),
        (Value::Int(-16), "-16"),
        (Value::DInt(-32), "-32"),
        (Value::LInt(-64), "-64"),
        (Value::USInt(8), "8"),
        (Value::UInt(16), "16"),
        (Value::UDInt(32), "32"),
        (Value::ULInt(64), "64"),
        (Value::Byte(255), "255"),
        (Value::Word(65_535), "65535"),
        (Value::DWord(4_000_000_000), "4000000000"),
        (Value::LWord(9_000_000_000), "9000000000"),
    ];

    for (value, expected) in cases {
        assert_eq!(format_user_value(&value), expected);
    }
}

#[test]
fn real_values_keep_fractional_identity_for_integral_values() {
    let cases = [
        (Value::Real(1.0), "1.0"),
        (Value::Real(-2.0), "-2.0"),
        (Value::Real(1.25), "1.25"),
        (Value::LReal(3.0), "3.0"),
        (Value::LReal(-4.5), "-4.5"),
    ];

    for (value, expected) in cases {
        assert_eq!(format_user_value(&value), expected);
    }
}

#[test]
fn duration_formatter_chooses_shortest_exact_unit_and_preserves_sign() {
    let cases = [
        ("T", 0, "T#0s"),
        ("T", 2_000_000_000, "T#2s"),
        ("T", 1_500_000_000, "T#1500ms"),
        ("T", 1_500_000, "T#1500us"),
        ("T", 1_500, "T#1500ns"),
        ("T", -2_000_000_000, "T#-2s"),
        ("LT", -1_500_000, "LT#-1500us"),
    ];

    for (prefix, nanos, expected) in cases {
        assert_eq!(format_duration(prefix, nanos), expected);
    }
}

#[test]
fn time_and_ltime_use_distinct_prefixes_with_same_exact_unit_policy() {
    assert_eq!(
        format_user_value(&Value::Time(Duration::from_millis(250))),
        "T#250ms"
    );
    assert_eq!(
        format_user_value(&Value::LTime(Duration::from_nanos(250))),
        "LT#250ns"
    );
}

#[test]
fn calendar_families_render_stored_ticks_or_nanoseconds() {
    let cases = [
        (Value::Date(DateValue::new(-1)), "D#-1"),
        (Value::LDate(LDateValue::new(-2)), "LD#-2"),
        (Value::Tod(TimeOfDayValue::new(3)), "TOD#3"),
        (Value::LTod(LTimeOfDayValue::new(4)), "LTOD#4"),
        (Value::Dt(DateTimeValue::new(5)), "DT#5"),
        (Value::Ldt(LDateTimeValue::new(6)), "LDT#6"),
    ];

    for (value, expected) in cases {
        assert_eq!(format_user_value(&value), expected);
    }
}

#[test]
fn structured_text_string_quoting_escapes_dollars_and_quotes_in_order() {
    assert_eq!(quote_st_string("plain"), "'plain'");
    assert_eq!(quote_st_string("pump$'a"), "'pump$$$'a'");
    assert_eq!(quote_st_string("$$"), "'$$$$'");
    assert_eq!(quote_st_string("''"), "'$'$''");
}

#[test]
fn narrow_and_wide_strings_use_same_payload_escaping_with_distinct_prefix() {
    assert_eq!(format_user_value(&Value::String("a$'b".into())), "'a$$$'b'");
    assert_eq!(
        format_user_value(&Value::WString("a$'b".into())),
        "W'a$$$'b'"
    );
}

#[test]
fn char_and_wchar_render_quoted_scalars_and_invalid_wchar_is_question_mark() {
    assert_eq!(format_user_value(&Value::Char(b'A')), "'A'");
    assert_eq!(format_user_value(&Value::Char(b'\'')), "'$''");
    assert_eq!(format_user_value(&Value::WChar('Ω' as u16)), "W'Ω'");
    assert_eq!(format_user_value(&Value::WChar(0xD800)), "W'?'");
}

#[test]
fn array_structure_and_enum_render_stable_summary_identity() {
    let array = Value::Array(Box::new(
        ArrayValue::from_untyped_parts(
            vec![Value::Int(1), Value::Int(2), Value::Int(3)],
            vec![(5, 7)],
        )
        .expect("array"),
    ));
    let structure = Value::Struct(Arc::new(StructValue::from_untyped_parts(
        "PumpState".into(),
        IndexMap::from([("running".into(), Value::Bool(true))]),
    )));
    let enumeration = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "Mode".into(),
        "Automatic".into(),
        2,
    )));

    assert_eq!(format_user_value(&array), "[3]");
    assert_eq!(format_user_value(&structure), "PumpState {...}");
    assert_eq!(format_user_value(&enumeration), "Mode::Automatic");
}

#[test]
fn reference_instance_and_null_states_have_nonaddress_diagnostic_text() {
    let reference = ValueRef {
        location: MemoryLocation::Global,
        offset: 99,
        path: RefPath::new(),
    };

    assert_eq!(format_user_value(&Value::Reference(Some(reference))), "REF");
    assert_eq!(format_user_value(&Value::Reference(None)), "NULL_REF");
    assert_eq!(
        format_user_value(&Value::Instance(InstanceId(42))),
        "Instance"
    );
    assert_eq!(format_user_value(&Value::Null), "NULL");
}

#[test]
fn user_value_text_never_leaks_rust_enum_constructor_names() {
    let values = [
        Value::Bool(true),
        Value::SInt(1),
        Value::Int(2),
        Value::DInt(3),
        Value::LInt(4),
        Value::USInt(5),
        Value::UInt(6),
        Value::UDInt(7),
        Value::ULInt(8),
        Value::Real(9.0),
        Value::LReal(10.0),
        Value::Byte(11),
        Value::Word(12),
        Value::DWord(13),
        Value::LWord(14),
        Value::String("text".into()),
        Value::WString("wide".into()),
        Value::Char(b'A'),
        Value::WChar(b'B' as u16),
        Value::Reference(None),
        Value::Instance(InstanceId(1)),
        Value::Null,
    ];

    for value in values {
        let text = format_user_value(&value);
        for implementation_name in [
            "Bool(",
            "Int(",
            "Real(",
            "String(",
            "Reference(",
            "Instance(",
        ] {
            assert!(!text.contains(implementation_name), "{text}");
        }
    }
}
