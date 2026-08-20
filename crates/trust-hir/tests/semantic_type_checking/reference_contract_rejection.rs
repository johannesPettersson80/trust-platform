use crate::common::*;

#[test]
fn reference_contract_rejects_ref_without_argument() {
    check_has_error(
        "PROGRAM P\nVAR r : REF_TO INT; END_VAR\nr := REF();\nEND_PROGRAM",
        DiagnosticCode::WrongArgumentCount,
    );
}

#[test]
fn reference_contract_rejects_ref_literal() {
    check_has_error(
        "PROGRAM P\nVAR r : REF_TO INT; END_VAR\nr := REF(1);\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn reference_contract_rejects_ref_computed_expression() {
    check_has_error(
        "PROGRAM P\nVAR x : INT; r : REF_TO INT; END_VAR\nr := REF(x + 1);\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn reference_contract_rejects_ref_call_result() {
    check_has_error("FUNCTION F : INT\nRETURN 1;\nEND_FUNCTION\nPROGRAM P\nVAR r : REF_TO INT; END_VAR\nr := REF(F());\nEND_PROGRAM", DiagnosticCode::InvalidOperation);
}

#[test]
fn reference_contract_rejects_ref_constant() {
    check_has_error("PROGRAM P\nVAR CONSTANT x : INT := 1; END_VAR\nVAR r : REF_TO INT; END_VAR\nr := REF(x);\nEND_PROGRAM", DiagnosticCode::InvalidOperation);
}

#[test]
fn reference_contract_rejects_ref_temporary() {
    check_has_error("PROGRAM P\nVAR_TEMP x : INT; END_VAR\nVAR r : REF_TO INT; END_VAR\nr := REF(x);\nEND_PROGRAM", DiagnosticCode::InvalidOperation);
}

#[test]
fn reference_contract_rejects_ref_function_local() {
    check_has_error("FUNCTION F : INT\nVAR x : INT; r : REF_TO INT; END_VAR\nr := REF(x); RETURN 1;\nEND_FUNCTION", DiagnosticCode::InvalidOperation);
}

#[test]
fn reference_contract_rejects_ref_function_result() {
    check_has_error(
        "FUNCTION F : INT\nVAR r : REF_TO INT; END_VAR\nr := REF(F); RETURN 1;\nEND_FUNCTION",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn reference_contract_rejects_adr_literal() {
    check_has_error(
        "PROGRAM P\nVAR p : POINTER TO INT; END_VAR\np := ADR(1);\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn reference_contract_rejects_adr_computed_expression() {
    check_has_error(
        "PROGRAM P\nVAR x : INT; p : POINTER TO INT; END_VAR\np := ADR(x + 1);\nEND_PROGRAM",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn reference_contract_rejects_adr_call_result() {
    check_has_error("FUNCTION F : INT\nRETURN 1;\nEND_FUNCTION\nPROGRAM P\nVAR p : POINTER TO INT; END_VAR\np := ADR(F());\nEND_PROGRAM", DiagnosticCode::InvalidOperation);
}

#[test]
fn reference_contract_rejects_adr_constant() {
    check_has_error("PROGRAM P\nVAR CONSTANT x : INT := 1; END_VAR\nVAR p : POINTER TO INT; END_VAR\np := ADR(x);\nEND_PROGRAM", DiagnosticCode::InvalidOperation);
}

#[test]
fn reference_contract_rejects_deref_integer() {
    check_has_error(
        "PROGRAM P\nVAR x : INT; y : INT; END_VAR\ny := x^;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn reference_contract_rejects_deref_bool() {
    check_has_error(
        "PROGRAM P\nVAR x : BOOL; y : BOOL; END_VAR\ny := x^;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn reference_contract_rejects_reference_target_type_mismatch() {
    check_has_error(
        "PROGRAM P\nVAR x : REAL; r : REF_TO INT; END_VAR\nr := REF(x);\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn reference_contract_rejects_pointer_target_type_mismatch() {
    check_has_error(
        "PROGRAM P\nVAR x : REAL; p : POINTER TO INT; END_VAR\np := ADR(x);\nEND_PROGRAM",
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn reference_contract_rejects_reference_to_pointer_assignment() {
    check_has_error("PROGRAM P\nVAR x : INT; r : REF_TO INT; p : POINTER TO INT; END_VAR\nr := REF(x); p := r;\nEND_PROGRAM", DiagnosticCode::IncompatibleAssignment);
}

#[test]
fn reference_contract_rejects_pointer_to_reference_assignment() {
    check_has_error("PROGRAM P\nVAR x : INT; r : REF_TO INT; p : POINTER TO INT; END_VAR\np := ADR(x); r := p;\nEND_PROGRAM", DiagnosticCode::IncompatibleAssignment);
}

#[test]
fn reference_contract_rejects_base_to_derived_ordinary_assignment() {
    check_has_error("CLASS Base END_CLASS\nCLASS Derived EXTENDS Base END_CLASS\nPROGRAM P\nVAR b : Base; baseRef : REF_TO Base; derivedRef : REF_TO Derived; END_VAR\nbaseRef := REF(b); derivedRef := baseRef;\nEND_PROGRAM", DiagnosticCode::IncompatibleAssignment);
}

#[test]
fn reference_contract_rejects_attempt_with_scalar_target() {
    check_has_error(
        "PROGRAM P\nVAR x : INT; y : INT; END_VAR\nx ?= y;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn reference_contract_rejects_attempt_with_scalar_source() {
    check_has_error(
        "PROGRAM P\nVAR x : INT; r : REF_TO INT; END_VAR\nr ?= x;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn reference_contract_rejects_cross_family_reference_to_pointer_attempt() {
    check_has_error("PROGRAM P\nVAR x : INT; r : REF_TO INT; p : POINTER TO INT; END_VAR\nr := REF(x); p ?= r;\nEND_PROGRAM", DiagnosticCode::TypeMismatch);
}

#[test]
fn reference_contract_rejects_cross_family_pointer_to_reference_attempt() {
    check_has_error("PROGRAM P\nVAR x : INT; r : REF_TO INT; p : POINTER TO INT; END_VAR\np := ADR(x); r ?= p;\nEND_PROGRAM", DiagnosticCode::TypeMismatch);
}

#[test]
fn reference_contract_rejects_incompatible_elementary_reference_attempt() {
    check_has_error("PROGRAM P\nVAR x : REAL; source : REF_TO REAL; result : REF_TO INT; END_VAR\nsource := REF(x); result ?= source;\nEND_PROGRAM", DiagnosticCode::TypeMismatch);
}

#[test]
fn reference_contract_rejects_incompatible_pointer_attempt() {
    check_has_error("PROGRAM P\nVAR x : REAL; source : POINTER TO REAL; result : POINTER TO INT; END_VAR\nsource := ADR(x); result ?= source;\nEND_PROGRAM", DiagnosticCode::TypeMismatch);
}

#[test]
fn reference_contract_rejects_reference_var_in_out() {
    check_has_error(
        "FUNCTION F : INT\nVAR_IN_OUT r : REF_TO INT; END_VAR\nRETURN 1;\nEND_FUNCTION",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn reference_contract_rejects_reference_addition() {
    check_has_error(
        "PROGRAM P\nVAR a : REF_TO INT; b : REF_TO INT; x : INT; END_VAR\nx := a + b;\nEND_PROGRAM",
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn reference_contract_rejects_pointer_subtraction() {
    check_has_error("PROGRAM P\nVAR a : POINTER TO INT; b : POINTER TO INT; x : INT; END_VAR\nx := a - b;\nEND_PROGRAM", DiagnosticCode::TypeMismatch);
}

#[test]
fn reference_contract_rejects_reference_ordering() {
    check_has_error("PROGRAM P\nVAR a : REF_TO INT; b : REF_TO INT; x : BOOL; END_VAR\nx := a < b;\nEND_PROGRAM", DiagnosticCode::TypeMismatch);
}

#[test]
fn reference_contract_rejects_pointer_ordering() {
    check_has_error("PROGRAM P\nVAR a : POINTER TO INT; b : POINTER TO INT; x : BOOL; END_VAR\nx := a > b;\nEND_PROGRAM", DiagnosticCode::TypeMismatch);
}

#[test]
fn reference_contract_rejects_pointer_concrete_array_bound_mismatch() {
    check_has_error("PROGRAM P\nVAR a : ARRAY[0..2] OF INT; p : POINTER TO ARRAY[1..3] OF INT; END_VAR\np := ADR(a);\nEND_PROGRAM", DiagnosticCode::IncompatibleAssignment);
}
