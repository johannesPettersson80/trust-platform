use crate::common::*;

fn diagnostics(source: &str) -> Vec<trust_hir::diagnostics::Diagnostic> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .cloned()
        .collect()
}

fn assert_invalid_operation(source: &str) {
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidOperation),
        "expected InvalidOperation, got {diagnostics:?}"
    );
}

#[test]
fn edge_declaration_function_block_accepts_rising_and_falling_bool_inputs() {
    check_no_errors(
        r#"
FUNCTION_BLOCK EdgeBlock
VAR_INPUT
    Rising : BOOL R_EDGE;
    Falling : BOOL F_EDGE;
END_VAR
VAR_OUTPUT
    Combined : BOOL;
END_VAR
Combined := Rising OR Falling;
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_program_accepts_rising_and_falling_bool_inputs() {
    check_no_errors(
        r#"
PROGRAM Main
VAR_INPUT
    Rising : BOOL R_EDGE;
    Falling : BOOL F_EDGE;
END_VAR
VAR_OUTPUT
    Combined : BOOL;
END_VAR
Combined := Rising OR Falling;
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_accepts_multiple_names_with_independent_hidden_state() {
    check_no_errors(
        r#"
FUNCTION_BLOCK EdgeBlock
VAR_INPUT
    First, Second, Third : BOOL R_EDGE;
END_VAR
VAR_OUTPUT
    Combined : BOOL;
END_VAR
Combined := First OR Second OR Third;
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_accepts_retain_non_retain_and_persistent_input_sections() {
    check_no_errors(
        r#"
FUNCTION_BLOCK EdgeBlock
VAR_INPUT RETAIN
    RetainedEdge : BOOL R_EDGE;
END_VAR
VAR_INPUT NON_RETAIN
    ReinitializedEdge : BOOL F_EDGE;
END_VAR
VAR_INPUT PERSISTENT
    PersistentEdge : BOOL R_EDGE;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_hidden_identity_does_not_collide_with_user_name() {
    check_no_errors(
        r#"
FUNCTION_BLOCK EdgeBlock
VAR_INPUT
    Start : BOOL R_EDGE;
END_VAR
VAR
    Start_TRIG : BOOL;
END_VAR
VAR_OUTPUT
    Observed : BOOL;
END_VAR
Start_TRIG := NOT Start_TRIG;
Observed := Start;
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_rising_input_on_function() {
    assert_invalid_operation(
        r#"
FUNCTION InvalidFunction : BOOL
VAR_INPUT
    Signal : BOOL R_EDGE;
END_VAR
InvalidFunction := Signal;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_falling_input_on_method() {
    assert_invalid_operation(
        r#"
CLASS Container
METHOD PUBLIC InvalidMethod : BOOL
VAR_INPUT
    Signal : BOOL F_EDGE;
END_VAR
InvalidMethod := Signal;
END_METHOD
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_rising_output() {
    assert_invalid_operation(
        r#"
FUNCTION_BLOCK InvalidBlock
VAR_OUTPUT
    Signal : BOOL R_EDGE;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_falling_in_out() {
    assert_invalid_operation(
        r#"
FUNCTION_BLOCK InvalidBlock
VAR_IN_OUT
    Signal : BOOL F_EDGE;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_rising_ordinary_variable() {
    assert_invalid_operation(
        r#"
FUNCTION_BLOCK InvalidBlock
VAR
    Signal : BOOL R_EDGE;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_falling_temporary_variable() {
    assert_invalid_operation(
        r#"
PROGRAM Main
VAR_TEMP
    Signal : BOOL F_EDGE;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_rising_external_variable() {
    assert_invalid_operation(
        r#"
VAR_GLOBAL
    Signal : BOOL;
END_VAR
FUNCTION_BLOCK InvalidBlock
VAR_EXTERNAL
    Signal : BOOL R_EDGE;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_non_bool_rising_input() {
    assert_invalid_operation(
        r#"
FUNCTION_BLOCK InvalidBlock
VAR_INPUT
    Signal : INT R_EDGE;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_non_bool_falling_input() {
    assert_invalid_operation(
        r#"
PROGRAM Main
VAR_INPUT
    Signal : WORD F_EDGE;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_rising_initializer() {
    assert_invalid_operation(
        r#"
FUNCTION_BLOCK InvalidBlock
VAR_INPUT
    Signal : BOOL R_EDGE := TRUE;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_falling_initializer() {
    assert_invalid_operation(
        r#"
PROGRAM Main
VAR_INPUT
    Signal : BOOL F_EDGE := FALSE;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_duplicate_rising_suffix() {
    assert_invalid_operation(
        r#"
FUNCTION_BLOCK InvalidBlock
VAR_INPUT
    Signal : BOOL R_EDGE R_EDGE;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_duplicate_falling_suffix() {
    assert_invalid_operation(
        r#"
FUNCTION_BLOCK InvalidBlock
VAR_INPUT
    Signal : BOOL F_EDGE F_EDGE;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_mixed_edge_suffixes() {
    assert_invalid_operation(
        r#"
PROGRAM Main
VAR_INPUT
    Signal : BOOL R_EDGE F_EDGE;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_constant_input_section() {
    assert_invalid_operation(
        r#"
FUNCTION_BLOCK InvalidBlock
VAR_INPUT CONSTANT
    Signal : BOOL R_EDGE;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_rejects_suffix_on_explicit_trigger_instance() {
    assert_invalid_operation(
        r#"
FUNCTION_BLOCK InvalidBlock
VAR_INPUT
    Signal : R_TRIG R_EDGE;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn edge_declaration_function_block_method_cannot_access_transformed_input() {
    assert_invalid_operation(
        r#"
FUNCTION_BLOCK EdgeBlock
VAR_INPUT
    Signal : BOOL R_EDGE;
END_VAR
METHOD PUBLIC ReadSignal : BOOL
ReadSignal := Signal;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
}
