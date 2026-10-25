//! VAR_STAT of functions and methods: kept from call to call, per instance for a method; the calls'
//! results show it.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

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

    assert_eq!(harness.get_output("a_first"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("a_second"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("b_first"), Some(Value::Int(1)));
}
