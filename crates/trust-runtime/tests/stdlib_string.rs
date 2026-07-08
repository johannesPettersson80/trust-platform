use trust_runtime::harness::TestHarness;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

#[test]
fn string_functions() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("LEN", &[Value::String("abc".into())]).unwrap(),
        Value::Int(3)
    );

    assert_eq!(
        lib.call(
            "CONCAT",
            &[Value::String("foo".into()), Value::String("bar".into())]
        )
        .unwrap(),
        Value::String("foobar".into())
    );
}

#[test]
fn bounded_string_assignment_and_concat_respect_declared_capacity() {
    let source = r#"
PROGRAM Main
VAR
    text : STRING[3] := 'A';
    source : STRING[6] := 'BCDE';
END_VAR
text := CONCAT(text, source);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "bounded string assignment must not fault the cycle: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("text"),
        Some(Value::String("ABC".into()))
    );
}
