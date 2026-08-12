use crate::common::*;

#[test]
fn control_flow_rejects_integer_if_condition() {
    check_has_error(
        "PROGRAM Main\nIF 1 THEN ; END_IF;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_integer_elsif_condition() {
    check_has_error(
        "PROGRAM Main\nIF TRUE THEN ; ELSIF 1 THEN ; END_IF;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_real_while_condition() {
    check_has_error(
        "PROGRAM Main\nWHILE REAL#1.0 DO EXIT; END_WHILE;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_string_repeat_condition() {
    check_has_error(
        "PROGRAM Main\nREPEAT EXIT; UNTIL 'done' END_REPEAT;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_struct_case_selector() {
    check_has_error(
        r#"
TYPE Pair : STRUCT left : INT; right : INT; END_STRUCT END_TYPE
PROGRAM Main
VAR selector : Pair; END_VAR
CASE selector OF 1: ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_array_case_selector() {
    check_has_error(
        r#"
PROGRAM Main
VAR selector : ARRAY[0..1] OF INT; END_VAR
CASE selector OF 1: ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_variable_case_label() {
    check_has_error(
        r#"
PROGRAM Main
VAR selector : INT; labelValue : INT; END_VAR
CASE selector OF labelValue: ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_string_label_for_integer_selector() {
    check_has_error(
        r#"
PROGRAM Main
VAR selector : INT; END_VAR
CASE selector OF 'one': ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_integer_label_for_string_selector() {
    check_has_error(
        r#"
PROGRAM Main
VAR selector : STRING[8]; END_VAR
CASE selector OF 1: ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_string_case_subrange() {
    check_has_error(
        r#"
PROGRAM Main
VAR selector : STRING[8]; END_VAR
CASE selector OF 'a'..'z': ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_duplicate_scalar_case_label() {
    check_has_error(
        r#"
PROGRAM Main
VAR selector : INT; END_VAR
CASE selector OF 1: ; 1: ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_duplicate_case_label_in_comma_list() {
    check_has_error(
        r#"
PROGRAM Main
VAR selector : INT; END_VAR
CASE selector OF 1, 2, 1: ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_scalar_inside_prior_case_range() {
    check_has_error(
        r#"
PROGRAM Main
VAR selector : INT; END_VAR
CASE selector OF 1..5: ; 3: ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_range_containing_prior_scalar() {
    check_has_error(
        r#"
PROGRAM Main
VAR selector : INT; END_VAR
CASE selector OF 3: ; 1..5: ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_overlapping_case_ranges() {
    check_has_error(
        r#"
PROGRAM Main
VAR selector : INT; END_VAR
CASE selector OF 1..5: ; 5..9: ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_reversed_case_range() {
    check_has_error(
        r#"
PROGRAM Main
VAR selector : INT; END_VAR
CASE selector OF 9..1: ; END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_real_for_control_variable() {
    check_has_error(
        r#"
PROGRAM Main
VAR i : REAL; END_VAR
FOR i := REAL#1.0 TO REAL#3.0 DO ; END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_bool_for_initial_value() {
    check_has_error(
        r#"
PROGRAM Main
VAR i : INT; END_VAR
FOR i := TRUE TO 3 DO ; END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_real_for_final_value() {
    check_has_error(
        r#"
PROGRAM Main
VAR i : INT; END_VAR
FOR i := 1 TO REAL#3.0 DO ; END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_bool_for_step_value() {
    check_has_error(
        r#"
PROGRAM Main
VAR i : INT; END_VAR
FOR i := 1 TO 3 BY TRUE DO ; END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_typed_initial_value_mismatch() {
    check_has_error(
        r#"
PROGRAM Main
VAR i : DINT; END_VAR
FOR i := INT#1 TO DINT#3 DO ; END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_typed_final_value_mismatch() {
    check_has_error(
        r#"
PROGRAM Main
VAR i : DINT; END_VAR
FOR i := DINT#1 TO INT#3 DO ; END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_typed_step_value_mismatch() {
    check_has_error(
        r#"
PROGRAM Main
VAR i : DINT; END_VAR
FOR i := DINT#1 TO DINT#3 BY INT#1 DO ; END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn control_flow_rejects_for_control_assignment_in_body() {
    check_has_error(
        r#"
PROGRAM Main
VAR i : INT; END_VAR
FOR i := 1 TO 3 DO i := i + 1; END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_for_start_source_assignment_in_body() {
    check_has_error(
        r#"
PROGRAM Main
VAR i : INT; first : INT; END_VAR
FOR i := first TO 3 DO first := 2; END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_for_end_source_assignment_in_body() {
    check_has_error(
        r#"
PROGRAM Main
VAR i : INT; last : INT; END_VAR
FOR i := 1 TO last DO last := 2; END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_for_restricted_assignment_in_nested_if() {
    check_has_error(
        r#"
PROGRAM Main
VAR i : INT; last : INT; END_VAR
FOR i := 1 TO last DO
    IF TRUE THEN last := 2; END_IF;
END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_exit_outside_loop() {
    check_has_error(
        "PROGRAM Main\nEXIT;\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_continue_outside_loop() {
    check_has_error(
        "PROGRAM Main\nCONTINUE;\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_exit_inside_if_without_loop() {
    check_has_error(
        "PROGRAM Main\nIF TRUE THEN EXIT; END_IF;\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_continue_inside_case_without_loop() {
    check_has_error(
        "PROGRAM Main\nCASE 1 OF 1: CONTINUE; END_CASE;\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn control_flow_rejects_value_return_from_program() {
    check_has_error(
        "PROGRAM Main\nRETURN INT#1;\nEND_PROGRAM",
        DiagnosticCode::InvalidReturnType,
    );
}

#[test]
fn control_flow_rejects_value_return_from_function_block() {
    check_has_error(
        "FUNCTION_BLOCK Worker\nRETURN INT#1;\nEND_FUNCTION_BLOCK",
        DiagnosticCode::InvalidReturnType,
    );
}

#[test]
fn control_flow_rejects_wrong_function_return_type() {
    check_has_error(
        "FUNCTION Pick : INT\nRETURN 'wrong';\nEND_FUNCTION",
        DiagnosticCode::InvalidReturnType,
    );
}

#[test]
fn control_flow_rejects_bare_return_before_result_assignment() {
    check_has_error(
        "FUNCTION Pick : INT\nRETURN;\nEND_FUNCTION",
        DiagnosticCode::MissingReturn,
    );
}

#[test]
fn control_flow_rejects_missing_function_return() {
    check_has_error(
        "FUNCTION Pick : INT\n;\nEND_FUNCTION",
        DiagnosticCode::MissingReturn,
    );
}
