use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn function_var_stat_persists_across_calls() {
    let source = r#"
FUNCTION Counter : INT
VAR_STAT
Count : INT := 0;
END_VAR
Count := Count + INT#1;
Counter := Count;
END_FUNCTION

PROGRAM Main
VAR
    first : INT;
    second : INT;
    third : INT;
END_VAR
first := Counter();
second := Counter();
third := Counter();
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);

    assert_eq!(
        harness
            .runtime()
            .storage()
            .get_global("__STAT::Counter::Count"),
        Some(&Value::Int(3))
    );
    assert_eq!(harness.get_output("first"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("third"), Some(Value::Int(3)));
}

#[test]
fn method_var_stat_is_isolated_per_instance() {
    let source = r#"
CLASS Accumulator
METHOD PUBLIC Next : INT
VAR_STAT
Count : INT := 0;
END_VAR
Count := Count + INT#1;
Next := Count;
END_METHOD
END_CLASS

PROGRAM Main
VAR
    a : Accumulator;
    b : Accumulator;
    a_first : INT;
    a_second : INT;
    b_first : INT;
END_VAR
a_first := a.Next();
a_second := a.Next();
b_first := b.Next();
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);

    let a = match harness.get_output("a") {
        Some(Value::Instance(id)) => id,
        other => panic!("expected class instance for a, got {other:?}"),
    };
    let b = match harness.get_output("b") {
        Some(Value::Instance(id)) => id,
        other => panic!("expected class instance for b, got {other:?}"),
    };
    assert_eq!(
        harness
            .runtime()
            .storage()
            .get_instance_var(a, "__STAT::Accumulator::Next::Count"),
        Some(&Value::Int(2))
    );
    assert_eq!(
        harness
            .runtime()
            .storage()
            .get_instance_var(b, "__STAT::Accumulator::Next::Count"),
        Some(&Value::Int(1))
    );
    assert_eq!(harness.get_output("a_first"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("a_second"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("b_first"), Some(Value::Int(1)));
}
