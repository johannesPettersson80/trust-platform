use trust_hir::types::TypeRegistry;
use trust_hir::TypeId;
use trust_runtime::value::{default_value_for_type_id, DateTimeProfile, Duration, Value};

#[test]
fn default_values_table10() {
    let registry = TypeRegistry::new();
    let profile = DateTimeProfile::default();

    let cases = [
        (TypeId::BOOL, Value::Bool(false)),
        (TypeId::SINT, Value::SInt(0)),
        (TypeId::INT, Value::Int(0)),
        (TypeId::DINT, Value::DInt(0)),
        (TypeId::LINT, Value::LInt(0)),
        (TypeId::USINT, Value::USInt(0)),
        (TypeId::UINT, Value::UInt(0)),
        (TypeId::UDINT, Value::UDInt(0)),
        (TypeId::ULINT, Value::ULInt(0)),
        (TypeId::REAL, Value::Real(0.0)),
        (TypeId::LREAL, Value::LReal(0.0)),
        (TypeId::BYTE, Value::Byte(0)),
        (TypeId::WORD, Value::Word(0)),
        (TypeId::DWORD, Value::DWord(0)),
        (TypeId::LWORD, Value::LWord(0)),
        (TypeId::STRING, Value::String("".into())),
        (TypeId::WSTRING, Value::WString(String::new())),
        (TypeId::CHAR, Value::Char(0)),
        (TypeId::WCHAR, Value::WChar(0)),
    ];

    for (type_id, expected) in cases {
        let value = default_value_for_type_id(type_id, &registry, &profile).unwrap();
        assert_eq!(value, expected);
    }

    let time_value = default_value_for_type_id(TypeId::TIME, &registry, &profile).unwrap();
    match time_value {
        Value::Time(dur) => assert_eq!(dur.as_nanos(), Duration::ZERO.as_nanos()),
        _ => panic!("unexpected TIME default"),
    }

    let ltime_value = default_value_for_type_id(TypeId::LTIME, &registry, &profile).unwrap();
    match ltime_value {
        Value::LTime(dur) => assert_eq!(dur.as_nanos(), Duration::ZERO.as_nanos()),
        _ => panic!("unexpected LTIME default"),
    }

    let date_value = default_value_for_type_id(TypeId::DATE, &registry, &profile).unwrap();
    match date_value {
        Value::Date(date) => assert_eq!(date.ticks(), 0),
        _ => panic!("unexpected DATE default"),
    }

    let ldate_value = default_value_for_type_id(TypeId::LDATE, &registry, &profile).unwrap();
    match ldate_value {
        Value::LDate(date) => assert_eq!(date.nanos(), 0),
        _ => panic!("unexpected LDATE default"),
    }

    let tod_value = default_value_for_type_id(TypeId::TOD, &registry, &profile).unwrap();
    match tod_value {
        Value::Tod(tod) => assert_eq!(tod.ticks(), 0),
        _ => panic!("unexpected TOD default"),
    }

    let ltod_value = default_value_for_type_id(TypeId::LTOD, &registry, &profile).unwrap();
    match ltod_value {
        Value::LTod(tod) => assert_eq!(tod.nanos(), 0),
        _ => panic!("unexpected LTOD default"),
    }

    let dt_value = default_value_for_type_id(TypeId::DT, &registry, &profile).unwrap();
    match dt_value {
        Value::Dt(dt) => assert_eq!(dt.ticks(), 0),
        _ => panic!("unexpected DT default"),
    }

    let ldt_value = default_value_for_type_id(TypeId::LDT, &registry, &profile).unwrap();
    match ldt_value {
        Value::Ldt(dt) => assert_eq!(dt.nanos(), 0),
        _ => panic!("unexpected LDT default"),
    }
}

#[test]
fn enum_defaults() {
    let mut registry = TypeRegistry::new();
    let profile = DateTimeProfile::default();

    let enum_id = registry.register_enum(
        "Traffic",
        TypeId::INT,
        vec![("Red".into(), 0), ("Green".into(), 1)],
    );

    let value = default_value_for_type_id(enum_id, &registry, &profile).unwrap();
    match value {
        Value::Enum(enum_value) => {
            assert_eq!(enum_value.type_name(), "Traffic");
            assert_eq!(enum_value.variant_name(), "Red");
            assert_eq!(enum_value.numeric_value(), 0);
        }
        _ => panic!("unexpected enum default"),
    }
}
