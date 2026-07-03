use trust_runtime::error::RuntimeError;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

#[test]
fn assertion_functions_pass_when_conditions_hold() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("ASSERT_TRUE", &[Value::Bool(true)]).unwrap(),
        Value::Null
    );
    assert_eq!(
        lib.call("ASSERT_FALSE", &[Value::Bool(false)]).unwrap(),
        Value::Null
    );
    assert_eq!(
        lib.call("ASSERT_EQUAL", &[Value::Int(2), Value::DInt(2)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        lib.call("ASSERT_EQUAL", &[Value::Char(b'B'), Value::Char(b'B')])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        lib.call(
            "ASSERT_EQUAL",
            &[Value::WChar('Y' as u16), Value::WChar('Y' as u16)]
        )
        .unwrap(),
        Value::Null
    );
    assert_eq!(
        lib.call("ASSERT_NOT_EQUAL", &[Value::Int(2), Value::Int(3)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        lib.call("ASSERT_GREATER", &[Value::Int(5), Value::Int(3)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        lib.call("ASSERT_LESS", &[Value::Int(3), Value::Int(5)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        lib.call("ASSERT_GREATER_OR_EQUAL", &[Value::Int(5), Value::DInt(5)],)
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        lib.call("ASSERT_LESS_OR_EQUAL", &[Value::Int(5), Value::Int(10)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        lib.call(
            "ASSERT_NEAR",
            &[Value::Real(1.0), Value::LReal(1.09), Value::Real(0.1)],
        )
        .unwrap(),
        Value::Null
    );
}

#[test]
fn assertion_functions_fail_with_assertion_error() {
    let lib = StandardLibrary::new();

    assert_assertion_message(
        lib.call("ASSERT_EQUAL", &[Value::Int(2), Value::Int(3)])
            .unwrap_err(),
        "ASSERT_EQUAL failed: expected 2, actual 3",
    );
    assert_assertion_message(
        lib.call("ASSERT_NOT_EQUAL", &[Value::Int(3), Value::Int(3)])
            .unwrap_err(),
        "ASSERT_NOT_EQUAL failed: values should differ, left 3, right 3",
    );
    assert_assertion_message(
        lib.call("ASSERT_GREATER", &[Value::Int(1), Value::Int(2)])
            .unwrap_err(),
        "ASSERT_GREATER failed: value 1 is not greater than bound 2",
    );
    assert_assertion_message(
        lib.call("ASSERT_LESS", &[Value::Int(2), Value::Int(1)])
            .unwrap_err(),
        "ASSERT_LESS failed: value 2 is not less than bound 1",
    );
    assert_assertion_message(
        lib.call("ASSERT_GREATER_OR_EQUAL", &[Value::Int(1), Value::Int(2)])
            .unwrap_err(),
        "ASSERT_GREATER_OR_EQUAL failed: value 1 is not >= bound 2",
    );
    assert_assertion_message(
        lib.call("ASSERT_LESS_OR_EQUAL", &[Value::Int(3), Value::Int(2)])
            .unwrap_err(),
        "ASSERT_LESS_OR_EQUAL failed: value 3 is not <= bound 2",
    );

    let err = lib
        .call(
            "ASSERT_NEAR",
            &[Value::LReal(1.0), Value::LReal(1.2), Value::LReal(0.1)],
        )
        .unwrap_err();
    match err {
        RuntimeError::AssertionFailed(message) => {
            assert!(message.contains("ASSERT_NEAR"));
            assert!(message.contains("delta"));
        }
        other => panic!("expected AssertionFailed, got {other:?}"),
    }
}

#[test]
fn assertion_failure_messages_use_user_facing_value_strings() {
    let lib = StandardLibrary::new();

    assert_assertion_message(
        lib.call("ASSERT_EQUAL", &[Value::Real(1.0), Value::Real(1.5)])
            .unwrap_err(),
        "ASSERT_EQUAL failed: expected 1.0, actual 1.5",
    );
    assert_assertion_message(
        lib.call("ASSERT_EQUAL", &[Value::Bool(true), Value::Bool(false)])
            .unwrap_err(),
        "ASSERT_EQUAL failed: expected TRUE, actual FALSE",
    );
    assert_assertion_message(
        lib.call("ASSERT_EQUAL", &[Value::Char(b'A'), Value::Char(b'B')])
            .unwrap_err(),
        "ASSERT_EQUAL failed: expected 'A', actual 'B'",
    );
}

fn assert_assertion_message(error: RuntimeError, expected: &str) {
    match error {
        RuntimeError::AssertionFailed(message) => {
            assert_eq!(message.as_ref(), expected);
            assert!(!message.contains("Int("), "{message}");
            assert!(!message.contains("Real("), "{message}");
            assert!(!message.contains("Bool("), "{message}");
            assert!(!message.contains("Char("), "{message}");
        }
        other => panic!("expected AssertionFailed, got {other:?}"),
    }
}

#[test]
fn assertion_comparison_functions_coerce_numeric_types() {
    let lib = StandardLibrary::new();
    let value = lib.call("ASSERT_GREATER", &[Value::Int(5), Value::DInt(3)]);
    assert_eq!(value.unwrap(), Value::Null);
}
