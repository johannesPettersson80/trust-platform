use trust_hir::types::TypeRegistry;
use trust_hir::TypeId;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::{EnumValue, Value};

#[test]
fn enum_and_validate() {
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

    assert_eq!(
        lib.call("IS_VALID", &[Value::Real(1.0)]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        lib.call("IS_VALID", &[Value::Real(f32::NAN)]).unwrap(),
        Value::Bool(false)
    );

    assert_eq!(
        lib.call("IS_VALID_BCD", &[Value::Word(0x1234)]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        lib.call("IS_VALID_BCD", &[Value::Word(0x12FA)]).unwrap(),
        Value::Bool(false)
    );
}
