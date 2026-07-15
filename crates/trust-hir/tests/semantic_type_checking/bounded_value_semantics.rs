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
