use crate::common::*;

#[test]
fn accuracy_preserving_implicit_conversion_matrix_is_accepted() {
    check_no_errors(
        r#"
PROGRAM Main
VAR
    sint_value : SINT;
    int_value : INT;
    dint_value : DINT;
    lint_value : LINT;
    usint_value : USINT;
    uint_value : UINT;
    udint_value : UDINT;
    ulint_value : ULINT;
    byte_value : BYTE;
    word_value : WORD;
    dword_value : DWORD;
    lword_value : LWORD;
    real_value : REAL;
    lreal_value : LREAL;
END_VAR
int_value := sint_value;
dint_value := sint_value;
dint_value := int_value;
lint_value := sint_value;
lint_value := int_value;
lint_value := dint_value;
uint_value := usint_value;
udint_value := usint_value;
udint_value := uint_value;
ulint_value := usint_value;
ulint_value := uint_value;
ulint_value := udint_value;
word_value := byte_value;
dword_value := byte_value;
dword_value := word_value;
lword_value := byte_value;
lword_value := word_value;
lword_value := dword_value;
real_value := sint_value;
real_value := int_value;
lreal_value := sint_value;
lreal_value := int_value;
lreal_value := dint_value;
lreal_value := real_value;
END_PROGRAM
"#,
    );
}

#[test]
fn typed_integer_to_float_edges_that_can_lose_accuracy_require_explicit_conversion() {
    let errors = check_errors(
        r#"
PROGRAM Main
VAR
    dint_value : DINT := DINT#16777217;
    lint_value : LINT := LINT#9007199254740993;
    real_value : REAL;
    lreal_value : LREAL;
END_VAR
real_value := dint_value;
lreal_value := lint_value;
END_PROGRAM
"#,
    );

    assert_eq!(
        errors
            .iter()
            .filter(|code| **code == DiagnosticCode::IncompatibleAssignment)
            .count(),
        2,
        "typed DINT-to-REAL and LINT-to-LREAL assignments must require explicit conversion: {errors:?}"
    );
}

#[test]
fn mixed_numeric_expression_requires_accuracy_preserving_common_type() {
    let errors = check_errors(
        r#"
PROGRAM Main
VAR
    ulint_value : ULINT := ULINT#18446744073709551615;
    real_value : REAL := REAL#1.0;
    result : REAL;
END_VAR
result := ulint_value + real_value;
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::TypeMismatch),
        "ULINT and REAL have no accuracy-preserving common type: {errors:?}"
    );
}

#[test]
fn representable_untyped_integer_literals_use_the_other_typed_operand() {
    check_no_errors(
        r#"
PROGRAM Main
VAR
    uint_value : UINT := UINT#2;
    arithmetic_left : UINT;
    arithmetic_right : UINT;
    equal_left : BOOL;
    equal_right : BOOL;
    maximum_left : UINT;
    maximum_right : UINT;
    minimum_left : UINT;
    minimum_right : UINT;
    real_value : REAL := REAL#2.0;
    real_arithmetic_left : REAL;
    real_arithmetic_right : REAL;
    real_equal_integer : BOOL;
    real_equal_left : BOOL;
    real_equal_right : BOOL;
    real_maximum_left : REAL;
    real_maximum_right : REAL;
END_VAR
arithmetic_left := uint_value + 1;
arithmetic_right := 1 + uint_value;
equal_left := uint_value = 1;
equal_right := 1 = uint_value;
maximum_left := MAX(uint_value, 1);
maximum_right := MAX(1, uint_value);
minimum_left := MIN(uint_value, 1);
minimum_right := MIN(1, uint_value);
real_arithmetic_left := real_value + 1.0;
real_arithmetic_right := 1.0 + real_value;
real_equal_integer := real_value = 1;
real_equal_left := real_value = 1.0;
real_equal_right := 1.0 = real_value;
real_maximum_left := MAX(real_value, 1.0);
real_maximum_right := MAX(1.0, real_value);
END_PROGRAM
"#,
    );
}

#[test]
fn out_of_range_or_typed_cross_family_operands_are_not_contextual_literals() {
    let errors = check_errors(
        r#"
PROGRAM Main
VAR
    usint_value : USINT := USINT#1;
    uint_value : UINT := UINT#2;
    usint_result : USINT;
    uint_result : UINT;
END_VAR
usint_result := usint_value + 256;
usint_result := MAX(usint_value, 256);
uint_result := SINT#1 + uint_value;
uint_result := MAX(SINT#1, uint_value);
END_PROGRAM
"#,
    );

    assert!(
        errors
            .iter()
            .filter(|code| {
                matches!(
                    code,
                    DiagnosticCode::TypeMismatch | DiagnosticCode::InvalidArgumentType
                )
            })
            .count()
            >= 4,
        "every out-of-range or explicitly typed cross-family operand must be rejected: {errors:?}"
    );
}

#[test]
fn explicit_integer_to_float_conversion_and_representable_context_literals_are_accepted() {
    check_no_errors(
        r#"
PROGRAM Main
VAR
    dint_value : DINT := DINT#16777217;
    lint_value : LINT := LINT#9007199254740993;
    real_value : REAL;
    lreal_value : LREAL;
END_VAR
real_value := DINT_TO_REAL(dint_value);
lreal_value := LINT_TO_LREAL(lint_value);
real_value := 16777216;
lreal_value := 9007199254740992;
END_PROGRAM
"#,
    );
}

#[test]
fn untyped_integer_literals_must_be_exactly_representable_in_float_targets() {
    let errors = check_errors(
        r#"
PROGRAM Main
VAR
    real_value : REAL;
    lreal_value : LREAL;
END_VAR
real_value := 16777217;
lreal_value := 9007199254740993;
END_PROGRAM
"#,
    );

    assert_eq!(
        errors
            .iter()
            .filter(|code| **code == DiagnosticCode::IncompatibleAssignment)
            .count(),
        2,
        "untyped integer literals that require rounding need an explicit conversion: {errors:?}"
    );
}

#[test]
fn cross_family_and_non_numeric_implicit_assignments_are_rejected() {
    let errors = check_errors(
        r#"
PROGRAM Main
VAR
    signed_value : DINT;
    unsigned_value : UDINT;
    flag : BOOL;
    real_value : REAL;
    text : STRING[5];
    wide_text : WSTRING[5];
END_VAR
signed_value := unsigned_value;
unsigned_value := signed_value;
real_value := flag;
flag := signed_value;
text := wide_text;
wide_text := text;
END_PROGRAM
"#,
    );

    assert_eq!(
        errors
            .iter()
            .filter(|code| **code == DiagnosticCode::IncompatibleAssignment)
            .count(),
        6,
        "implicit conversion must stay within the reviewed numeric or string family: {errors:?}"
    );
}

#[test]
fn subrange_constant_initializers_and_wrong_base_assignments_fail_closed() {
    let errors = check_errors(
        r#"
TYPE
    Limited : INT (-2..2);
END_TYPE

PROGRAM Main
VAR
    below : Limited := -3;
    above : Limited := 3;
    target : Limited;
    wrong : REAL;
END_VAR
target := wrong;
END_PROGRAM
"#,
    );

    assert_eq!(
        errors
            .iter()
            .filter(|code| **code == DiagnosticCode::OutOfRange)
            .count(),
        2,
        "both initializer boundaries must be checked: {errors:?}"
    );
    assert!(
        errors.contains(&DiagnosticCode::IncompatibleAssignment),
        "a REAL value must not be assigned implicitly to an INT subrange: {errors:?}"
    );
}
