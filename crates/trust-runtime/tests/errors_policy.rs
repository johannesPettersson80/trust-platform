use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn error_policy() {
    let source = r#"
PROGRAM Main
VAR
    x : DINT := 0;
END_VAR
x := 1 / 0;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.contains(&RuntimeError::DivisionByZero));
}

#[test]
fn real_arithmetic_overflow_faults_before_assignment_store() {
    let source = r#"
PROGRAM Main
VAR
    result : REAL := REAL#7.0;
END_VAR
result := REAL#1.0E38 * REAL#4.0;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile harness");
    assert_eq!(harness.get_output("result"), Some(Value::Real(7.0)));

    let cycle = harness.cycle();

    assert_eq!(cycle.errors, vec![RuntimeError::Overflow]);
    assert_eq!(harness.get_output("result"), Some(Value::Real(7.0)));
}

#[test]
fn real_named_numeric_overflow_faults_before_assignment_store() {
    for expression in ["EXP(REAL#100.0)", "EXPT(REAL#10.0, REAL#100.0)"] {
        let source = format!(
            r#"
PROGRAM Main
VAR
    result : REAL := REAL#7.0;
END_VAR
result := {expression};
END_PROGRAM
"#
        );

        let mut harness = TestHarness::from_source(&source).expect("compile harness");
        assert_eq!(harness.get_output("result"), Some(Value::Real(7.0)));

        let cycle = harness.cycle();

        assert_eq!(cycle.errors, vec![RuntimeError::Overflow], "{expression}");
        assert_eq!(
            harness.get_output("result"),
            Some(Value::Real(7.0)),
            "{expression}"
        );
    }
}
