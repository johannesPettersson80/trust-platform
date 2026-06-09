use trust_hir::types::TypeRegistry;
use trust_hir::TypeId;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::{EnumValue, Value};

#[test]
fn enum_comparisons() {
    let lib = StandardLibrary::new();
    let mut registry = TypeRegistry::new();
    let color_type = registry.register_enum(
        "Color",
        TypeId::INT,
        vec![("RED".into(), 0), ("GREEN".into(), 1)],
    );

    let red = Value::Enum(Box::new(
        EnumValue::new(&registry, color_type, "RED").expect("RED enum value"),
    ));
    let green = Value::Enum(Box::new(
        EnumValue::new(&registry, color_type, "GREEN").expect("GREEN enum value"),
    ));

    assert_eq!(
        lib.call("EQ", &[red.clone(), red.clone()]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        lib.call("NE", &[red.clone(), green.clone()]).unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn is_valid_real_values() {
    let lib = StandardLibrary::new();
    assert_eq!(
        lib.call("IS_VALID", &[Value::Real(1.0)]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        lib.call("IS_VALID", &[Value::Real(f32::NAN)]).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        lib.call("IS_VALID", &[Value::LReal(f64::INFINITY)])
            .unwrap(),
        Value::Bool(false)
    );
    assert!(lib.call("IS_VALID", &[Value::DInt(1)]).is_err());
}

#[test]
fn is_valid_bcd_bit_strings() {
    let lib = StandardLibrary::new();
    assert_eq!(
        lib.call("IS_VALID_BCD", &[Value::Byte(0x12)]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        lib.call("IS_VALID_BCD", &[Value::Word(0x1234)]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        lib.call("IS_VALID_BCD", &[Value::DWord(0x1234_5678)])
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        lib.call("IS_VALID_BCD", &[Value::LWord(0x1234_5678_9012_3456)])
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        lib.call("IS_VALID_BCD", &[Value::Word(0x12FA)]).unwrap(),
        Value::Bool(false)
    );
    assert!(lib.call("IS_VALID_BCD", &[Value::Bool(true)]).is_err());
}
