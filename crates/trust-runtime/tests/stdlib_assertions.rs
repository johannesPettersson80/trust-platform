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

    let err = lib
        .call("ASSERT_EQUAL", &[Value::Int(2), Value::Int(3)])
        .unwrap_err();
    match err {
        RuntimeError::AssertionFailed(message) => {
            assert!(message.contains("ASSERT_EQUAL"));
            assert!(message.contains("expected"));
            assert!(message.contains("actual"));
        }
        other => panic!("expected AssertionFailed, got {other:?}"),
    }

    let err = lib
        .call("ASSERT_NOT_EQUAL", &[Value::Int(3), Value::Int(3)])
        .unwrap_err();
    match err {
        RuntimeError::AssertionFailed(message) => {
            assert!(message.contains("ASSERT_NOT_EQUAL"));
            assert!(message.contains("differ"));
        }
        other => panic!("expected AssertionFailed, got {other:?}"),
    }

    let err = lib
        .call("ASSERT_GREATER", &[Value::Int(1), Value::Int(2)])
        .unwrap_err();
    match err {
        RuntimeError::AssertionFailed(message) => {
            assert!(message.contains("ASSERT_GREATER"));
            assert!(message.contains("bound"));
        }
        other => panic!("expected AssertionFailed, got {other:?}"),
    }

    let err = lib
        .call("ASSERT_LESS", &[Value::Int(2), Value::Int(1)])
        .unwrap_err();
    match err {
        RuntimeError::AssertionFailed(message) => {
            assert!(message.contains("ASSERT_LESS"));
            assert!(message.contains("bound"));
        }
        other => panic!("expected AssertionFailed, got {other:?}"),
    }

    let err = lib
        .call("ASSERT_GREATER_OR_EQUAL", &[Value::Int(1), Value::Int(2)])
        .unwrap_err();
    match err {
        RuntimeError::AssertionFailed(message) => {
            assert!(message.contains("ASSERT_GREATER_OR_EQUAL"));
            assert!(message.contains("bound"));
        }
        other => panic!("expected AssertionFailed, got {other:?}"),
    }

    let err = lib
        .call("ASSERT_LESS_OR_EQUAL", &[Value::Int(3), Value::Int(2)])
        .unwrap_err();
    match err {
        RuntimeError::AssertionFailed(message) => {
            assert!(message.contains("ASSERT_LESS_OR_EQUAL"));
            assert!(message.contains("bound"));
        }
        other => panic!("expected AssertionFailed, got {other:?}"),
    }

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
fn assertion_comparison_functions_coerce_numeric_types() {
    let lib = StandardLibrary::new();
    let value = lib.call("ASSERT_GREATER", &[Value::Int(5), Value::DInt(3)]);
    assert_eq!(value.unwrap(), Value::Null);
}
