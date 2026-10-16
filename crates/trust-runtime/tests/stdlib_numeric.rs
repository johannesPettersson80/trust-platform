use trust_runtime::error::RuntimeError;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

#[test]
fn numeric_functions() {
    let lib = StandardLibrary::new();

    assert_eq!(lib.call("ABS", &[Value::Int(-5)]).unwrap(), Value::Int(5));

    assert_eq!(
        lib.call("SQRT", &[Value::Real(9.0)]),
        Ok(Value::Real(3.0)),
        "SQRT(9.0) is exactly 3.0"
    );

    let sin = lib.call("SIN", &[Value::Real(0.0)]).unwrap();
    match sin {
        Value::Real(value) => assert!(value.abs() < 1e-6),
        _ => panic!("expected REAL result"),
    }
}

#[test]
fn real_named_numeric_functions_reject_single_width_overflow() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("EXP", &[Value::Real(100.0)]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("EXPT", &[Value::Real(10.0), Value::Real(100.0)]),
        Err(RuntimeError::Overflow)
    );
}
