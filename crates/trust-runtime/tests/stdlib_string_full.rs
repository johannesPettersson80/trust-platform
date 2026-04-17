use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

#[test]
fn string_full() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("LEFT", &[Value::String("ASTR".into()), Value::Int(3)])
            .unwrap(),
        Value::String("AST".into())
    );

    assert_eq!(
        lib.call("RIGHT", &[Value::String("ASTR".into()), Value::Int(3)])
            .unwrap(),
        Value::String("STR".into())
    );

    assert_eq!(
        lib.call(
            "MID",
            &[Value::String("ASTR".into()), Value::Int(2), Value::Int(2)]
        )
        .unwrap(),
        Value::String("ST".into())
    );

    assert_eq!(
        lib.call(
            "CONCAT",
            &[
                Value::String("AB".into()),
                Value::String("CD".into()),
                Value::String("E".into())
            ]
        )
        .unwrap(),
        Value::String("ABCDE".into())
    );

    assert_eq!(
        lib.call(
            "INSERT",
            &[
                Value::String("ABC".into()),
                Value::String("XY".into()),
                Value::Int(2)
            ]
        )
        .unwrap(),
        Value::String("ABXYC".into())
    );

    assert_eq!(
        lib.call(
            "DELETE",
            &[Value::String("ABXYC".into()), Value::Int(2), Value::Int(3)]
        )
        .unwrap(),
        Value::String("ABC".into())
    );

    assert_eq!(
        lib.call(
            "REPLACE",
            &[
                Value::String("ABCDE".into()),
                Value::String("X".into()),
                Value::Int(2),
                Value::Int(3)
            ]
        )
        .unwrap(),
        Value::String("ABXE".into())
    );

    assert_eq!(
        lib.call(
            "FIND",
            &[Value::String("ABCBC".into()), Value::String("BC".into())]
        )
        .unwrap(),
        Value::Int(2)
    );
}

#[test]
fn string_full_non_ascii_uses_character_elements() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("LEN", &[Value::String("ÄBC".into())]).unwrap(),
        Value::Int(3)
    );

    assert_eq!(
        lib.call("LEFT", &[Value::String("ÄBC".into()), Value::Int(1)])
            .unwrap(),
        Value::String("Ä".into())
    );

    assert_eq!(
        lib.call(
            "MID",
            &[Value::String("ÄBC".into()), Value::Int(1), Value::Int(2)]
        )
        .unwrap(),
        Value::String("B".into())
    );

    assert_eq!(
        lib.call(
            "FIND",
            &[Value::String("ÄBC".into()), Value::String("B".into())]
        )
        .unwrap(),
        Value::Int(2)
    );

    assert_eq!(
        lib.call("LEN", &[Value::WString("ÄBC".into())]).unwrap(),
        Value::Int(3)
    );
}
