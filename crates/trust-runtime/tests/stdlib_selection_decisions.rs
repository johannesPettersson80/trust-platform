//! LIMIT is MIN(MAX(IN, MN), MX) (spec 07 section 6.1, IEC 61131-3 Table 32).

use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

#[test]
fn limit_with_inverted_bounds_returns_the_upper_bound() {
    let lib = StandardLibrary::new();

    // MN := 5, IN := 3, MX := 0: the raise to MN is capped by MX.
    assert_eq!(
        lib.call("LIMIT", &[Value::Int(5), Value::Int(3), Value::Int(0)]),
        Ok(Value::Int(0))
    );
    // MN := 5, IN := 9, MX := 0: every input resolves to MX.
    assert_eq!(
        lib.call("LIMIT", &[Value::Int(5), Value::Int(9), Value::Int(0)]),
        Ok(Value::Int(0))
    );
    assert_eq!(
        lib.call(
            "LIMIT",
            &[Value::LReal(2.0), Value::LReal(1.0), Value::LReal(1.5)]
        ),
        Ok(Value::LReal(1.5))
    );
}

#[test]
fn limit_with_ordered_bounds_clamps_both_sides() {
    let lib = StandardLibrary::new();
    let limit = |mn: i16, input: i16, mx: i16| {
        lib.call(
            "LIMIT",
            &[Value::Int(mn), Value::Int(input), Value::Int(mx)],
        )
    };

    assert_eq!(limit(0, 5, 3), Ok(Value::Int(3)));
    assert_eq!(limit(0, -5, 3), Ok(Value::Int(0)));
    assert_eq!(limit(0, 2, 3), Ok(Value::Int(2)));
    assert_eq!(limit(0, 3, 3), Ok(Value::Int(3)));
    assert_eq!(limit(3, 3, 3), Ok(Value::Int(3)));
}
