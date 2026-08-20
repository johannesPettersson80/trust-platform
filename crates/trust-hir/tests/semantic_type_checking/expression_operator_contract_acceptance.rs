use crate::common::*;

#[test]
fn expression_operator_accepts_same_type_signed_arithmetic() {
    check_no_errors(
        "PROGRAM P\nVAR a : DINT; r : DINT; END_VAR\nr := a + a - a * a / DINT#1;\nEND_PROGRAM",
    );
}

#[test]
fn expression_operator_accepts_same_type_unsigned_arithmetic() {
    check_no_errors(
        "PROGRAM P\nVAR a : UDINT; r : UDINT; END_VAR\nr := a + a - a * a / UDINT#1;\nEND_PROGRAM",
    );
}

#[test]
fn expression_operator_accepts_signed_accuracy_preserving_widening() {
    check_no_errors(
        "PROGRAM P\nVAR a : INT; b : DINT; r : DINT; END_VAR\nr := a + b;\nEND_PROGRAM",
    );
}

#[test]
fn expression_operator_accepts_unsigned_accuracy_preserving_widening() {
    check_no_errors(
        "PROGRAM P\nVAR a : UINT; b : ULINT; r : ULINT; END_VAR\nr := a + b;\nEND_PROGRAM",
    );
}

#[test]
fn expression_operator_accepts_real_to_lreal_widening() {
    check_no_errors(
        "PROGRAM P\nVAR a : REAL; b : LREAL; r : LREAL; END_VAR\nr := a + b;\nEND_PROGRAM",
    );
}

#[test]
fn expression_operator_accepts_exact_integer_to_real_widening() {
    check_no_errors(
        "PROGRAM P\nVAR a : INT; b : REAL; r : REAL; END_VAR\nr := a + b;\nEND_PROGRAM",
    );
}

#[test]
fn expression_operator_accepts_exact_dint_to_lreal_widening() {
    check_no_errors(
        "PROGRAM P\nVAR a : DINT; b : LREAL; r : LREAL; END_VAR\nr := a + b;\nEND_PROGRAM",
    );
}

#[test]
fn expression_operator_accepts_contextual_untyped_integer_rhs() {
    check_no_errors("PROGRAM P\nVAR a : UINT; r : UINT; END_VAR\nr := a + 1;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_contextual_untyped_integer_lhs() {
    check_no_errors("PROGRAM P\nVAR a : UINT; r : UINT; END_VAR\nr := 1 + a;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_integer_division_and_modulo() {
    check_no_errors("PROGRAM P\nVAR a : LINT; q : LINT; m : LINT; END_VAR\nq := a / LINT#3; m := a MOD LINT#3;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_real_exponentiation() {
    check_no_errors("PROGRAM P\nVAR a : REAL; r : REAL; END_VAR\nr := a ** INT#2;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_reviewed_integer_exponent_extension() {
    check_no_errors("PROGRAM P\nVAR r : INT; END_VAR\nr := INT#2 ** INT#3;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_bool_short_circuit_families() {
    check_no_errors("PROGRAM P\nVAR a : BOOL; b : BOOL; r : BOOL; END_VAR\nr := a AND b; r := a & b; r := a OR b; r := a XOR b; r := NOT a;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_same_width_bitwise_families() {
    check_no_errors("PROGRAM P\nVAR a : WORD; b : WORD; r : WORD; END_VAR\nr := a AND b; r := a & b; r := a OR b; r := a XOR b; r := NOT a;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_bit_string_width_widening() {
    check_no_errors("PROGRAM P\nVAR a : BYTE; b : DWORD; r : DWORD; END_VAR\nr := a AND b; r := a OR b; r := a XOR b;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_signed_numeric_comparisons() {
    check_no_errors("PROGRAM P\nVAR a : INT; b : DINT; r : BOOL; END_VAR\nr := a < b; r := a <= b; r := a = b; r := a <> b; r := a >= b; r := a > b;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_unsigned_numeric_comparisons() {
    check_no_errors("PROGRAM P\nVAR a : UINT; b : UDINT; r : BOOL; END_VAR\nr := a < b; r := a = b; r := a > b;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_narrow_string_comparisons() {
    check_no_errors("PROGRAM P\nVAR a : STRING[8]; b : STRING[16]; r : BOOL; END_VAR\nr := a < b; r := a = b; r := a <> b;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_wide_string_comparisons() {
    check_no_errors("PROGRAM P\nVAR a : WSTRING[8]; b : WSTRING[16]; r : BOOL; END_VAR\nr := a < b; r := a = b; r := a <> b;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_parenthesized_numeric_range_check() {
    check_no_errors("PROGRAM P\nVAR a : INT; b : INT; c : INT; r : BOOL; END_VAR\nr := (a < b) AND (b < c);\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_left_associative_equal_precedence() {
    check_no_errors("PROGRAM P\nVAR a : INT; b : INT; c : INT; r : INT; END_VAR\nr := a - b - c; r := a / b / c;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_unary_plus_and_minus_numeric() {
    check_no_errors("PROGRAM P\nVAR a : LREAL; r : LREAL; END_VAR\nr := -a; r := +a;\nEND_PROGRAM");
}

#[test]
fn expression_operator_accepts_complete_temporal_arithmetic_matrix() {
    check_no_errors(
        r#"
PROGRAM P
VAR
    t1 : TIME; t2 : TIME; tr : TIME;
    lt1 : LTIME; lt2 : LTIME; ltr : LTIME;
    date1 : DATE; date2 : DATE;
    ldate1 : LDATE; ldate2 : LDATE;
    tod1 : TOD; tod2 : TOD; todr : TOD;
    ltod1 : LTOD; ltod2 : LTOD; ltodr : LTOD;
    dt1 : DT; dt2 : DT; dtr : DT;
    ldt1 : LDT; ldt2 : LDT; ldtr : LDT;
    factor : LREAL;
END_VAR
tr := t1 + t2;
tr := t1 - t2;
ltr := lt1 + lt2;
ltr := lt1 - lt2;
todr := tod1 + t1;
todr := tod1 - t1;
tr := tod1 - tod2;
ltodr := ltod1 + lt1;
ltodr := ltod1 - lt1;
ltr := ltod1 - ltod2;
dtr := dt1 + t1;
dtr := dt1 - t1;
tr := dt1 - dt2;
ldtr := ldt1 + lt1;
ldtr := ldt1 - lt1;
ltr := ldt1 - ldt2;
tr := date1 - date2;
ltr := ldate1 - ldate2;
tr := t1 * factor;
tr := t1 / factor;
ltr := lt1 * factor;
ltr := lt1 / factor;
END_PROGRAM
"#,
    );
}
