use crate::common::*;

#[test]
fn expression_operator_rejects_signed_unsigned_arithmetic() {
    check_has_error(
        "PROGRAM P\nVAR a : INT; b : UINT; r : INT; END_VAR\nr := a + b;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_signed_unsigned_comparison() {
    check_has_error(
        "PROGRAM P\nVAR a : DINT; b : UDINT; r : BOOL; END_VAR\nr := a < b;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_dint_real_accuracy_loss() {
    check_has_error(
        "PROGRAM P\nVAR a : DINT; b : REAL; r : REAL; END_VAR\nr := a + b;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_lint_lreal_accuracy_loss() {
    check_has_error(
        "PROGRAM P\nVAR a : LINT; b : LREAL; r : LREAL; END_VAR\nr := a + b;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_ulint_real_without_common_type() {
    check_has_error(
        "PROGRAM P\nVAR a : ULINT; b : REAL; r : REAL; END_VAR\nr := a + b;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_bool_addition() {
    check_has_error(
        "PROGRAM P\nVAR a : BOOL; r : BOOL; END_VAR\nr := a + a;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_string_subtraction() {
    check_has_error(
        "PROGRAM P\nVAR a : STRING; r : STRING; END_VAR\nr := a - a;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_real_modulo() {
    check_has_error(
        "PROGRAM P\nVAR a : REAL; r : REAL; END_VAR\nr := a MOD a;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_bool_unary_minus() {
    check_has_error(
        "PROGRAM P\nVAR a : BOOL; r : BOOL; END_VAR\nr := -a;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_integer_not() {
    check_has_error(
        "PROGRAM P\nVAR a : INT; r : INT; END_VAR\nr := NOT a;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_integer_and() {
    check_has_error(
        "PROGRAM P\nVAR a : INT; r : INT; END_VAR\nr := a AND a;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_bool_bit_string_and() {
    check_has_error(
        "PROGRAM P\nVAR a : BOOL; b : BYTE; r : BYTE; END_VAR\nr := a AND b;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_narrow_wide_string_comparison() {
    check_has_error(
        "PROGRAM P\nVAR a : STRING; b : WSTRING; r : BOOL; END_VAR\nr := a = b;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_struct_comparison() {
    check_has_error("TYPE S : STRUCT x : INT; END_STRUCT END_TYPE\nPROGRAM P\nVAR a : S; b : S; r : BOOL; END_VAR\nr := a = b;\nEND_PROGRAM", DiagnosticCode::TypeMismatch);
}

#[test]
fn expression_operator_rejects_numeric_chained_comparison() {
    check_has_error(
        "PROGRAM P\nVAR a : INT; b : INT; c : INT; r : BOOL; END_VAR\nr := a < b < c;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_out_of_range_contextual_unsigned_literal() {
    check_has_error(
        "PROGRAM P\nVAR a : USINT; r : USINT; END_VAR\nr := a + 256;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_typed_cross_width_narrowing_result() {
    check_has_error(
        "PROGRAM P\nVAR a : INT; b : DINT; r : INT; END_VAR\nr := a + b;\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn expression_operator_rejects_string_exponentiation() {
    check_has_error(
        "PROGRAM P\nVAR a : STRING; r : STRING; END_VAR\nr := a ** INT#2;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_reference_arithmetic() {
    check_has_error("PROGRAM P\nVAR x : INT; a : REF_TO INT; r : INT; END_VAR\na := REF(x); r := a + a;\nEND_PROGRAM", DiagnosticCode::TypeMismatch);
}

#[test]
fn expression_operator_rejects_array_comparison() {
    check_has_error("PROGRAM P\nVAR a : ARRAY[0..1] OF INT; b : ARRAY[0..1] OF INT; r : BOOL; END_VAR\nr := a = b;\nEND_PROGRAM", DiagnosticCode::TypeMismatch);
}

#[test]
fn expression_operator_rejects_bit_string_arithmetic() {
    check_has_error(
        "PROGRAM P\nVAR a : WORD; r : WORD; END_VAR\nr := a + a;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_real_bitwise_or() {
    check_has_error(
        "PROGRAM P\nVAR a : REAL; r : REAL; END_VAR\nr := a OR a;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn expression_operator_rejects_unlisted_or_swapped_temporal_arithmetic() {
    let cases = [
        (
            "mixed TIME and LTIME addition",
            "PROGRAM P\nVAR t : TIME; lt : LTIME; r : TIME; END_VAR\nr := t + lt;\nEND_PROGRAM",
        ),
        (
            "swapped TOD addition",
            "PROGRAM P\nVAR t : TIME; tod : TOD; r : TOD; END_VAR\nr := t + tod;\nEND_PROGRAM",
        ),
        (
            "swapped DT addition",
            "PROGRAM P\nVAR t : TIME; dt : DT; r : DT; END_VAR\nr := t + dt;\nEND_PROGRAM",
        ),
        (
            "DATE addition",
            "PROGRAM P\nVAR t : TIME; date : DATE; r : DATE; END_VAR\nr := date + t;\nEND_PROGRAM",
        ),
        (
            "DATE minus duration",
            "PROGRAM P\nVAR t : TIME; date : DATE; r : DATE; END_VAR\nr := date - t;\nEND_PROGRAM",
        ),
        (
            "swapped duration multiplication",
            "PROGRAM P\nVAR t : TIME; n : INT; r : TIME; END_VAR\nr := n * t;\nEND_PROGRAM",
        ),
        (
            "numeric divided by duration",
            "PROGRAM P\nVAR t : TIME; n : INT; r : TIME; END_VAR\nr := n / t;\nEND_PROGRAM",
        ),
        (
            "TOD multiplication",
            "PROGRAM P\nVAR tod_value : TOD; n : INT; r : TOD; END_VAR\nr := tod_value * n;\nEND_PROGRAM",
        ),
        (
            "DT division",
            "PROGRAM P\nVAR dt_value : DT; n : INT; r : DT; END_VAR\nr := dt_value / n;\nEND_PROGRAM",
        ),
    ];
    let missing_operator_rejections = cases
        .iter()
        .filter_map(|(label, source)| {
            let errors = check_errors(source);
            (!errors.contains(&DiagnosticCode::TypeMismatch)).then_some((*label, errors))
        })
        .collect::<Vec<_>>();

    assert!(
        missing_operator_rejections.is_empty(),
        "unlisted temporal arithmetic lacked operator rejection: {missing_operator_rejections:?}"
    );
}
