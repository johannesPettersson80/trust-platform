use super::*;

use std::fmt::Debug;
use std::sync::Arc;

use crate::value::{
    ArrayValue, DateTimeValue, DateValue, Duration, LDateTimeValue, LDateValue, LTimeOfDayValue,
    TimeOfDayValue,
};

fn assert_compile_error<T: Debug>(result: Result<T, CompileError>, expected: &str) {
    let message = result.expect_err("expected coercion error").to_string();
    assert!(
        message
            .to_ascii_lowercase()
            .contains(&expected.to_ascii_lowercase()),
        "expected {message:?} to contain {expected:?}"
    );
}

fn field(name: &str, type_id: TypeId) -> StructField {
    StructField {
        name: name.into(),
        type_id,
        address: None,
        default_initializer: None,
    }
}

fn variant(name: &str, type_id: TypeId) -> UnionVariant {
    UnionVariant {
        name: name.into(),
        type_id,
        address: None,
        default_initializer: None,
    }
}

fn struct_value(type_name: &str, fields: &[(&str, Value)]) -> Value {
    let fields = fields
        .iter()
        .map(|(name, value)| ((*name).into(), value.clone()))
        .collect();
    Value::Struct(Arc::new(StructValue::from_untyped_parts(
        type_name.into(),
        fields,
    )))
}

#[test]
fn harness_coerce_contract_bool_is_exact() {
    assert_eq!(
        coerce_value_to_type(Value::Bool(true), TypeId::BOOL).unwrap(),
        Value::Bool(true)
    );
    assert_compile_error(
        coerce_value_to_type(Value::SInt(1), TypeId::BOOL),
        "expected bool",
    );
}

#[test]
fn harness_coerce_contract_signed_sources_preserve_mathematical_value() {
    for source in [
        Value::SInt(7),
        Value::Int(7),
        Value::DInt(7),
        Value::LInt(7),
        Value::USInt(7),
        Value::UInt(7),
        Value::UDInt(7),
        Value::ULInt(7),
    ] {
        assert_eq!(
            coerce_value_to_type(source, TypeId::LINT).unwrap(),
            Value::LInt(7)
        );
    }
}

#[test]
fn harness_coerce_contract_signed_destinations_check_both_bounds() {
    for (type_id, minimum, maximum, below, above) in [
        (
            TypeId::SINT,
            Value::LInt(i8::MIN as i64),
            Value::LInt(i8::MAX as i64),
            Value::LInt(i8::MIN as i64 - 1),
            Value::LInt(i8::MAX as i64 + 1),
        ),
        (
            TypeId::INT,
            Value::LInt(i16::MIN as i64),
            Value::LInt(i16::MAX as i64),
            Value::LInt(i16::MIN as i64 - 1),
            Value::LInt(i16::MAX as i64 + 1),
        ),
        (
            TypeId::DINT,
            Value::LInt(i32::MIN as i64),
            Value::LInt(i32::MAX as i64),
            Value::LInt(i32::MIN as i64 - 1),
            Value::LInt(i32::MAX as i64 + 1),
        ),
    ] {
        assert!(coerce_value_to_type(minimum, type_id).is_ok());
        assert!(coerce_value_to_type(maximum, type_id).is_ok());
        assert_compile_error(coerce_value_to_type(below, type_id), "out of");
        assert_compile_error(coerce_value_to_type(above, type_id), "out of");
    }
}

#[test]
fn harness_coerce_contract_signed_rejects_ulint_above_i64_and_noninteger() {
    assert_compile_error(
        coerce_value_to_type(Value::ULInt(i64::MAX as u64 + 1), TypeId::LINT),
        "signed range",
    );
    assert_compile_error(
        coerce_value_to_type(Value::LReal(1.0), TypeId::INT),
        "expected integer",
    );
}

#[test]
fn harness_coerce_contract_unsigned_sources_preserve_mathematical_value() {
    for source in [
        Value::SInt(7),
        Value::Int(7),
        Value::DInt(7),
        Value::LInt(7),
        Value::USInt(7),
        Value::UInt(7),
        Value::UDInt(7),
        Value::ULInt(7),
    ] {
        assert_eq!(
            coerce_value_to_type(source, TypeId::ULINT).unwrap(),
            Value::ULInt(7)
        );
    }
}

#[test]
fn harness_coerce_contract_unsigned_destinations_check_width() {
    for (type_id, maximum, above) in [
        (
            TypeId::USINT,
            Value::ULInt(u8::MAX as u64),
            u8::MAX as u64 + 1,
        ),
        (
            TypeId::UINT,
            Value::ULInt(u16::MAX as u64),
            u16::MAX as u64 + 1,
        ),
        (
            TypeId::UDINT,
            Value::ULInt(u32::MAX as u64),
            u32::MAX as u64 + 1,
        ),
    ] {
        assert!(coerce_value_to_type(maximum, type_id).is_ok());
        assert_compile_error(coerce_value_to_type(Value::ULInt(above), type_id), "out of");
    }
}

#[test]
fn harness_coerce_contract_unsigned_rejects_negative_and_noninteger() {
    for source in [
        Value::SInt(-1),
        Value::Int(-1),
        Value::DInt(-1),
        Value::LInt(-1),
    ] {
        assert_compile_error(
            coerce_value_to_type(source, TypeId::ULINT),
            "unsigned range",
        );
    }
    assert_compile_error(
        coerce_value_to_type(Value::Bool(true), TypeId::UINT),
        "expected unsigned integer",
    );
}

#[test]
fn harness_coerce_contract_bitstrings_accept_integer_and_bit_sources() {
    for source in [
        Value::SInt(7),
        Value::Int(7),
        Value::DInt(7),
        Value::LInt(7),
        Value::USInt(7),
        Value::UInt(7),
        Value::UDInt(7),
        Value::ULInt(7),
        Value::Byte(7),
        Value::Word(7),
        Value::DWord(7),
        Value::LWord(7),
    ] {
        assert_eq!(
            coerce_value_to_type(source, TypeId::LWORD).unwrap(),
            Value::LWord(7)
        );
    }
}

#[test]
fn harness_coerce_contract_bitstrings_reject_negative_width_overflow_and_noninteger() {
    assert_compile_error(
        coerce_value_to_type(Value::Int(-1), TypeId::LWORD),
        "unsigned range",
    );
    assert_compile_error(
        coerce_value_to_type(Value::Word(256), TypeId::BYTE),
        "byte range",
    );
    assert_compile_error(
        coerce_value_to_type(Value::Bool(true), TypeId::WORD),
        "expected integer",
    );
}

#[test]
fn harness_coerce_contract_real_accepts_finite_numeric_sources() {
    for source in [
        Value::SInt(-1),
        Value::Int(-2),
        Value::DInt(-3),
        Value::LInt(-4),
        Value::USInt(1),
        Value::UInt(2),
        Value::UDInt(3),
        Value::ULInt(4),
        Value::Real(1.5),
        Value::LReal(2.5),
    ] {
        assert!(coerce_value_to_type(source, TypeId::LREAL).is_ok());
    }
    assert_eq!(
        coerce_value_to_type(Value::LReal(1.25), TypeId::REAL).unwrap(),
        Value::Real(1.25)
    );
}

#[test]
fn harness_coerce_contract_real_rejects_nonfinite_values() {
    for source in [
        Value::Real(f32::NAN),
        Value::LReal(f64::NAN),
        Value::LReal(f64::INFINITY),
        Value::LReal(f64::NEG_INFINITY),
    ] {
        assert_compile_error(coerce_value_to_type(source, TypeId::LREAL), "finite");
    }
}

#[test]
fn harness_coerce_contract_real_rejects_narrowing_overflow() {
    assert_compile_error(
        coerce_value_to_type(Value::LReal(f64::MAX), TypeId::REAL),
        "real range",
    );
}

#[test]
fn harness_coerce_contract_real_rejects_nonnumeric_source() {
    assert_compile_error(
        coerce_value_to_type(Value::Bool(true), TypeId::REAL),
        "expected numeric",
    );
}

#[test]
fn harness_coerce_contract_string_and_wstring_conversions_are_explicit() {
    assert_eq!(
        coerce_value_to_type(Value::WString("wide".to_string()), TypeId::STRING).unwrap(),
        Value::String("wide".into())
    );
    assert_eq!(
        coerce_value_to_type(Value::String("narrow".into()), TypeId::WSTRING).unwrap(),
        Value::WString("narrow".to_string())
    );
    assert_eq!(
        coerce_value_to_type(Value::Char(b'Z'), TypeId::STRING).unwrap(),
        Value::String("Z".into())
    );
    assert_eq!(
        coerce_value_to_type(Value::WChar('å' as u16), TypeId::WSTRING).unwrap(),
        Value::WString("å".to_string())
    );
}

#[test]
fn harness_coerce_contract_strings_reject_unowned_source_tags() {
    assert_compile_error(
        coerce_value_to_type(Value::Int(7), TypeId::STRING),
        "expected string",
    );
    assert_compile_error(
        coerce_value_to_type(Value::Bool(true), TypeId::WSTRING),
        "expected wstring",
    );
}

#[test]
fn harness_coerce_contract_char_accepts_exact_or_single_ascii_string() {
    assert_eq!(
        coerce_value_to_type(Value::Char(b'A'), TypeId::CHAR).unwrap(),
        Value::Char(b'A')
    );
    assert_eq!(
        coerce_value_to_type(Value::String("Z".into()), TypeId::CHAR).unwrap(),
        Value::Char(b'Z')
    );
    assert_eq!(
        coerce_value_to_type(Value::WString("Q".to_string()), TypeId::CHAR).unwrap(),
        Value::Char(b'Q')
    );
}

#[test]
fn harness_coerce_contract_wchar_accepts_exact_or_single_bmp_string() {
    assert_eq!(
        coerce_value_to_type(Value::WChar('å' as u16), TypeId::WCHAR).unwrap(),
        Value::WChar('å' as u16)
    );
    assert_eq!(
        coerce_value_to_type(Value::String("å".into()), TypeId::WCHAR).unwrap(),
        Value::WChar('å' as u16)
    );
}

#[test]
fn harness_coerce_contract_character_arity_is_checked() {
    for text in ["", "AB"] {
        assert_compile_error(
            coerce_value_to_type(Value::String(text.into()), TypeId::CHAR),
            "single character",
        );
    }
    assert_compile_error(
        coerce_value_to_type(Value::Int(65), TypeId::CHAR),
        "expected char",
    );
}

#[test]
fn harness_coerce_contract_char_rejects_non_ascii_scalar() {
    assert_compile_error(
        coerce_value_to_type(Value::String("å".into()), TypeId::CHAR),
        "char range",
    );
}

#[test]
fn harness_coerce_contract_wchar_rejects_non_bmp_scalar() {
    assert_compile_error(
        coerce_value_to_type(Value::WString("😀".to_string()), TypeId::WCHAR),
        "wchar range",
    );
}

#[test]
fn harness_coerce_contract_time_widths_convert_bidirectionally() {
    let duration = Duration::from_nanos(123);
    assert_eq!(
        coerce_value_to_type(Value::LTime(duration), TypeId::TIME).unwrap(),
        Value::Time(duration)
    );
    assert_eq!(
        coerce_value_to_type(Value::Time(duration), TypeId::LTIME).unwrap(),
        Value::LTime(duration)
    );
    assert_compile_error(
        coerce_value_to_type(Value::Int(123), TypeId::TIME),
        "expected time",
    );
}

#[test]
fn harness_coerce_contract_calendar_families_require_exact_width() {
    for (value, type_id) in [
        (Value::Date(DateValue::new(1)), TypeId::DATE),
        (Value::LDate(LDateValue::new(2)), TypeId::LDATE),
        (Value::Tod(TimeOfDayValue::new(3)), TypeId::TOD),
        (Value::LTod(LTimeOfDayValue::new(4)), TypeId::LTOD),
        (Value::Dt(DateTimeValue::new(5)), TypeId::DT),
        (Value::Ldt(LDateTimeValue::new(6)), TypeId::LDT),
    ] {
        assert_eq!(coerce_value_to_type(value.clone(), type_id).unwrap(), value);
    }
    assert_compile_error(
        coerce_value_to_type(Value::LDate(LDateValue::new(1)), TypeId::DATE),
        "expected date",
    );
    assert_compile_error(
        coerce_value_to_type(Value::LTod(LTimeOfDayValue::new(1)), TypeId::TOD),
        "expected tod",
    );
    assert_compile_error(
        coerce_value_to_type(Value::Ldt(LDateTimeValue::new(1)), TypeId::DT),
        "expected dt",
    );
}

#[test]
fn harness_coerce_contract_alias_and_subrange_delegate_to_storage_type() {
    let mut registry = TypeRegistry::new();
    let alias = registry.register(
        "Counter",
        Type::Alias {
            name: "Counter".into(),
            target: TypeId::DINT,
        },
    );
    let subrange = registry.register(
        "Small",
        Type::Subrange {
            base: TypeId::INT,
            lower: 0,
            upper: 10,
        },
    );
    let profile = DateTimeProfile::default();

    assert_eq!(
        coerce_initializer_value_to_type(Value::SInt(7), alias, &registry, &profile).unwrap(),
        Value::DInt(7)
    );
    assert_eq!(
        coerce_initializer_value_to_type(Value::SInt(7), subrange, &registry, &profile).unwrap(),
        Value::Int(7)
    );
}

#[test]
fn harness_coerce_contract_bounded_strings_truncate_by_unicode_scalar() {
    let mut registry = TypeRegistry::new();
    let string = registry.register_string_with_length(2);
    let wstring = registry.register_wstring_with_length(2);
    let profile = DateTimeProfile::default();

    assert_eq!(
        coerce_initializer_value_to_type(
            Value::String("aå😀".into()),
            string,
            &registry,
            &profile,
        )
        .unwrap(),
        Value::String("aå".into())
    );
    assert_eq!(
        coerce_initializer_value_to_type(
            Value::WString("aå😀".to_string()),
            wstring,
            &registry,
            &profile,
        )
        .unwrap(),
        Value::WString("aå".to_string())
    );
}

#[test]
fn harness_coerce_contract_partial_array_defaults_omitted_elements() {
    let mut registry = TypeRegistry::new();
    let array_type = registry.register_array(TypeId::INT, vec![(1, 3)]);
    let input = Value::Array(Box::new(
        ArrayValue::from_untyped_parts(vec![Value::SInt(7)], vec![(0, 0)]).unwrap(),
    ));
    let profile = DateTimeProfile::default();

    let Value::Array(array) =
        coerce_initializer_value_to_type(input, array_type, &registry, &profile).unwrap()
    else {
        panic!("expected array");
    };
    assert_eq!(array.dimensions(), &[(1, 3)]);
    assert_eq!(
        array.elements(),
        &[Value::Int(7), Value::Int(0), Value::Int(0)]
    );
}

#[test]
fn harness_coerce_contract_array_rejects_wrong_shape_category_and_ignores_excess() {
    let mut registry = TypeRegistry::new();
    let array_type = registry.register_array(TypeId::INT, vec![(1, 2)]);
    let profile = DateTimeProfile::default();
    assert_compile_error(
        coerce_initializer_value_to_type(Value::Int(1), array_type, &registry, &profile),
        "expected array",
    );
    let excess = Value::Array(Box::new(
        ArrayValue::from_untyped_parts(
            vec![Value::Int(1), Value::Int(2), Value::Int(3)],
            vec![(0, 2)],
        )
        .unwrap(),
    ));
    let Value::Array(excess) =
        coerce_initializer_value_to_type(excess, array_type, &registry, &profile).unwrap()
    else {
        panic!("expected array");
    };
    assert_eq!(excess.elements(), &[Value::Int(1), Value::Int(2)]);
}

#[test]
fn harness_coerce_contract_struct_defaults_omissions_and_canonicalizes_names() {
    let mut registry = TypeRegistry::new();
    let record = registry.register_struct(
        "Record",
        vec![field("Count", TypeId::DINT), field("Enabled", TypeId::BOOL)],
    );
    let profile = DateTimeProfile::default();
    let input = struct_value("input", &[("count", Value::SInt(7))]);

    let Value::Struct(value) =
        coerce_initializer_value_to_type(input, record, &registry, &profile).unwrap()
    else {
        panic!("expected struct");
    };
    assert_eq!(value.type_name(), "Record");
    assert_eq!(
        value
            .fields()
            .iter()
            .map(|(name, value)| (name.as_str(), value))
            .collect::<Vec<_>>(),
        vec![("Count", &Value::DInt(7)), ("Enabled", &Value::Bool(false))]
    );
}

#[test]
fn harness_coerce_contract_struct_rejects_unknown_field_and_nonaggregate_input() {
    let mut registry = TypeRegistry::new();
    let record = registry.register_struct("Record", vec![field("Count", TypeId::DINT)]);
    let profile = DateTimeProfile::default();
    assert_compile_error(
        coerce_initializer_value_to_type(
            struct_value("input", &[("missing", Value::Int(1))]),
            record,
            &registry,
            &profile,
        ),
        "unknown aggregate field",
    );
    assert_compile_error(
        coerce_initializer_value_to_type(Value::Int(1), record, &registry, &profile),
        "expected struct",
    );
}

#[test]
fn harness_coerce_contract_union_defaults_omissions_and_canonicalizes_names() {
    let mut registry = TypeRegistry::new();
    let choice = registry.register_union(
        "Choice",
        vec![
            variant("Number", TypeId::INT),
            variant("Flag", TypeId::BOOL),
        ],
    );
    let profile = DateTimeProfile::default();

    let Value::Struct(value) = coerce_initializer_value_to_type(
        struct_value("input", &[("flag", Value::Bool(true))]),
        choice,
        &registry,
        &profile,
    )
    .unwrap() else {
        panic!("expected union representation");
    };
    assert_eq!(value.type_name(), "Choice");
    assert_eq!(value.field("Number"), Some(&Value::Int(0)));
    assert_eq!(value.field("Flag"), Some(&Value::Bool(true)));
}

#[test]
fn harness_coerce_contract_union_rejects_unknown_field_and_nonaggregate_input() {
    let mut registry = TypeRegistry::new();
    let choice = registry.register_union("Choice", vec![variant("Number", TypeId::INT)]);
    let profile = DateTimeProfile::default();
    assert_compile_error(
        coerce_initializer_value_to_type(
            struct_value("input", &[("missing", Value::Int(1))]),
            choice,
            &registry,
            &profile,
        ),
        "unknown aggregate field",
    );
    assert_compile_error(
        coerce_initializer_value_to_type(Value::Int(1), choice, &registry, &profile),
        "expected union",
    );
}
