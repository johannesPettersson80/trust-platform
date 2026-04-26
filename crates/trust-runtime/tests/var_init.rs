use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn iec_table14() {
    let source = r#"
PROGRAM Main
VAR
    a : INT;
    b : INT := 4;
    c : INT := b + 1;
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("a"), Some(Value::Int(0)));
    assert_eq!(harness.get_output("b"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("c"), Some(Value::Int(5)));
}

#[test]
fn declaration_array_initializer_end_to_end() {
    let source = r#"
PROGRAM Main
VAR
    a : ARRAY[1..3] OF INT := [1, 2, 3];
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    harness.cycle();

    assert_eq!(
        harness.get_output("a"),
        Some(Value::Array(Box::new(
            trust_runtime::value::ArrayValue::from_untyped_parts(
                vec![Value::Int(1), Value::Int(2), Value::Int(3)],
                vec![(1, 3)],
            )
            .expect("valid expected array"),
        )))
    );
}

#[test]
fn declaration_partial_array_initializer_default_fills_remaining_elements() {
    let source = r#"
PROGRAM Main
VAR
    a : ARRAY[1..5] OF INT := [1, 2];
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    harness.cycle();

    assert_eq!(
        harness.get_output("a"),
        Some(Value::Array(Box::new(
            trust_runtime::value::ArrayValue::from_untyped_parts(
                vec![
                    Value::Int(1),
                    Value::Int(2),
                    Value::Int(0),
                    Value::Int(0),
                    Value::Int(0),
                ],
                vec![(1, 5)],
            )
            .expect("valid expected array"),
        )))
    );
}

#[test]
fn declaration_repetition_array_initializer_expands_group() {
    let source = r#"
PROGRAM Main
VAR
    a : ARRAY[1..6] OF INT := [3(1, 2)];
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    harness.cycle();

    assert_eq!(
        harness.get_output("a"),
        Some(Value::Array(Box::new(
            trust_runtime::value::ArrayValue::from_untyped_parts(
                vec![
                    Value::Int(1),
                    Value::Int(2),
                    Value::Int(1),
                    Value::Int(2),
                    Value::Int(1),
                    Value::Int(2),
                ],
                vec![(1, 6)],
            )
            .expect("valid expected array"),
        )))
    );
}
