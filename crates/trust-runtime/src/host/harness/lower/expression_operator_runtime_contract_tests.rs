use crate::error::RuntimeError;
use crate::harness::TestHarness;
use crate::value::Value;

fn operator_output(source: &str, name: &str) -> Value {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("operator fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness.try_get_output(name).unwrap()
}

fn assert_operator_fault_preserves(source: &str, error: RuntimeError, name: &str, expected: Value) {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|compile_error| panic!("fault fixture must compile: {compile_error}"));
    let cycle = harness.cycle();
    assert_eq!(cycle.errors, [error]);
    assert_eq!(harness.try_get_output(name).unwrap(), expected);
}

#[test]
fn expression_operator_runtime_applies_multiplication_before_addition() {
    assert_eq!(
        operator_output(
            "PROGRAM Main\nVAR result : INT; END_VAR\nresult := 2 + 3 * 4;\nEND_PROGRAM",
            "result"
        ),
        Value::Int(14)
    );
}

#[test]
fn expression_operator_runtime_parentheses_override_precedence() {
    assert_eq!(
        operator_output(
            "PROGRAM Main\nVAR result : INT; END_VAR\nresult := (2 + 3) * 4;\nEND_PROGRAM",
            "result"
        ),
        Value::Int(20)
    );
}

#[test]
fn expression_operator_runtime_subtraction_is_left_associative() {
    assert_eq!(
        operator_output(
            "PROGRAM Main\nVAR result : INT; END_VAR\nresult := 20 - 5 - 2;\nEND_PROGRAM",
            "result"
        ),
        Value::Int(13)
    );
}

#[test]
fn expression_operator_runtime_division_is_left_associative() {
    assert_eq!(
        operator_output(
            "PROGRAM Main\nVAR result : INT; END_VAR\nresult := 20 / 5 / 2;\nEND_PROGRAM",
            "result"
        ),
        Value::Int(2)
    );
}

#[test]
fn expression_operator_runtime_integer_division_truncates_positive_toward_zero() {
    assert_eq!(
        operator_output(
            "PROGRAM Main\nVAR result : INT; END_VAR\nresult := 7 / 3;\nEND_PROGRAM",
            "result"
        ),
        Value::Int(2)
    );
}

#[test]
fn expression_operator_runtime_integer_division_truncates_negative_toward_zero() {
    assert_eq!(
        operator_output(
            "PROGRAM Main\nVAR result : INT; END_VAR\nresult := -7 / 3;\nEND_PROGRAM",
            "result"
        ),
        Value::Int(-2)
    );
}

#[test]
fn expression_operator_runtime_modulo_tracks_positive_dividend() {
    assert_eq!(
        operator_output(
            "PROGRAM Main\nVAR result : INT; END_VAR\nresult := 7 MOD 3;\nEND_PROGRAM",
            "result"
        ),
        Value::Int(1)
    );
}

#[test]
fn expression_operator_runtime_modulo_tracks_negative_dividend() {
    assert_eq!(
        operator_output(
            "PROGRAM Main\nVAR result : INT; END_VAR\nresult := -7 MOD 3;\nEND_PROGRAM",
            "result"
        ),
        Value::Int(-1)
    );
}

#[test]
fn expression_operator_runtime_signed_widening_uses_common_type() {
    assert_eq!(operator_output("PROGRAM Main\nVAR a : INT := 2; b : DINT := 3; result : DINT; END_VAR\nresult := a + b;\nEND_PROGRAM", "result"), Value::DInt(5));
}

#[test]
fn expression_operator_runtime_real_widening_uses_lreal() {
    assert_eq!(operator_output("PROGRAM Main\nVAR a : REAL := REAL#1.5; b : LREAL := LREAL#2.5; result : LREAL; END_VAR\nresult := a + b;\nEND_PROGRAM", "result"), Value::LReal(4.0));
}

#[test]
fn expression_operator_runtime_integer_power_extension_returns_int() {
    assert_eq!(
        operator_output(
            "PROGRAM Main\nVAR result : INT; END_VAR\nresult := INT#2 ** INT#3;\nEND_PROGRAM",
            "result"
        ),
        Value::Int(8)
    );
}

#[test]
fn expression_operator_runtime_real_power_preserves_fraction() {
    assert_eq!(operator_output("PROGRAM Main\nVAR result : REAL; END_VAR\nresult := REAL#4.0 ** REAL#0.5;\nEND_PROGRAM", "result"), Value::Real(2.0));
}

#[test]
fn expression_operator_runtime_bool_and_short_circuits_false_left() {
    assert_eq!(operator_output("PROGRAM Main\nVAR result : BOOL; END_VAR\nresult := FALSE AND ((INT#1 / INT#0) = INT#0);\nEND_PROGRAM", "result"), Value::Bool(false));
}

#[test]
fn expression_operator_runtime_bool_ampersand_short_circuits_false_left() {
    assert_eq!(operator_output("PROGRAM Main\nVAR result : BOOL; END_VAR\nresult := FALSE & ((INT#1 / INT#0) = INT#0);\nEND_PROGRAM", "result"), Value::Bool(false));
}

#[test]
fn expression_operator_runtime_bool_or_short_circuits_true_left() {
    assert_eq!(operator_output("PROGRAM Main\nVAR result : BOOL; END_VAR\nresult := TRUE OR ((INT#1 / INT#0) = INT#0);\nEND_PROGRAM", "result"), Value::Bool(true));
}

#[test]
fn expression_operator_runtime_bool_xor_evaluates_right_operand() {
    assert_operator_fault_preserves(
        "PROGRAM Main\nVAR one : INT := 1; zero : INT; result : BOOL := TRUE; END_VAR\nresult := TRUE XOR ((one / zero) = 0);\nEND_PROGRAM",
        RuntimeError::DivisionByZero,
        "result",
        Value::Bool(true),
    );
}

#[test]
fn expression_operator_runtime_bitwise_and_preserves_value_width() {
    assert_eq!(operator_output("PROGRAM Main\nVAR result : WORD; END_VAR\nresult := WORD#16#FF00 AND WORD#16#0F0F;\nEND_PROGRAM", "result"), Value::Word(0x0f00));
}

#[test]
fn expression_operator_runtime_bitwise_or_preserves_value_width() {
    assert_eq!(operator_output("PROGRAM Main\nVAR result : WORD; END_VAR\nresult := WORD#16#F000 OR WORD#16#0F00;\nEND_PROGRAM", "result"), Value::Word(0xff00));
}

#[test]
fn expression_operator_runtime_bitwise_xor_preserves_value_width() {
    assert_eq!(operator_output("PROGRAM Main\nVAR result : WORD; END_VAR\nresult := WORD#16#FF00 XOR WORD#16#0FF0;\nEND_PROGRAM", "result"), Value::Word(0xf0f0));
}

#[test]
fn expression_operator_runtime_bitwise_not_preserves_value_width() {
    assert_eq!(
        operator_output(
            "PROGRAM Main\nVAR result : BYTE; END_VAR\nresult := NOT BYTE#16#F0;\nEND_PROGRAM",
            "result"
        ),
        Value::Byte(0x0f)
    );
}

#[test]
fn expression_operator_runtime_numeric_comparison_uses_common_type() {
    assert_eq!(operator_output("PROGRAM Main\nVAR a : INT := 2; b : DINT := 3; result : BOOL; END_VAR\nresult := a < b;\nEND_PROGRAM", "result"), Value::Bool(true));
}

#[test]
fn expression_operator_runtime_string_comparison_is_lexicographic() {
    assert_eq!(operator_output("PROGRAM Main\nVAR a : STRING[2] := 'A'; b : STRING[2] := 'B'; result : BOOL; END_VAR\nresult := a < b;\nEND_PROGRAM", "result"), Value::Bool(true));
}

#[test]
fn expression_operator_runtime_add_overflow_preserves_destination() {
    assert_operator_fault_preserves(
        "PROGRAM Main\nVAR a : SINT := SINT#127; result : SINT := SINT#7; END_VAR\nresult := a + 1;\nEND_PROGRAM",
        RuntimeError::Overflow,
        "result",
        Value::SInt(7),
    );
}

#[test]
fn expression_operator_runtime_multiply_overflow_preserves_destination() {
    assert_operator_fault_preserves(
        "PROGRAM Main\nVAR a : INT := INT#300; result : INT := INT#7; END_VAR\nresult := a * a;\nEND_PROGRAM",
        RuntimeError::Overflow,
        "result",
        Value::Int(7),
    );
}

#[test]
fn expression_operator_runtime_unary_min_negation_preserves_destination() {
    assert_operator_fault_preserves(
        "PROGRAM Main\nVAR a : SINT := SINT#-128; result : SINT := SINT#7; END_VAR\nresult := -a;\nEND_PROGRAM",
        RuntimeError::Overflow,
        "result",
        Value::SInt(7),
    );
}

#[test]
fn expression_operator_runtime_division_by_zero_preserves_destination() {
    assert_operator_fault_preserves(
        "PROGRAM Main\nVAR a : INT := 7; zero : INT; result : INT := 9; END_VAR\nresult := a / zero;\nEND_PROGRAM",
        RuntimeError::DivisionByZero,
        "result",
        Value::Int(9),
    );
}

#[test]
fn expression_operator_runtime_modulo_by_zero_preserves_destination() {
    assert_operator_fault_preserves(
        "PROGRAM Main\nVAR a : INT := 7; zero : INT; result : INT := 9; END_VAR\nresult := a MOD zero;\nEND_PROGRAM",
        RuntimeError::ModuloByZero,
        "result",
        Value::Int(9),
    );
}
