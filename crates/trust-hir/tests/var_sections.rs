mod common;

use common::*;

#[test]
fn iec_table13() {
    check_no_errors(
        r#"
FUNCTION_BLOCK DemoFb
VAR_INPUT
    i : INT;
END_VAR
VAR_OUTPUT
    o : INT;
END_VAR
VAR_IN_OUT
    io : INT;
END_VAR
VAR_TEMP
    t : INT;
END_VAR
VAR
    v : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR_EXTERNAL
    g : INT;
END_VAR
END_PROGRAM

CONFIGURATION Conf
VAR_GLOBAL
    g : INT;
END_VAR
END_CONFIGURATION
"#,
    );
}

#[test]
fn program_var_global_is_accepted() {
    check_no_errors(
        r#"
PROGRAM Main
VAR_GLOBAL
    G : INT;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn file_scope_var_global_is_accepted_across_files() {
    let gvl = r#"
VAR_GLOBAL
    G : INT;
END_VAR
"#;
    let consumer = r#"
PROGRAM Main
VAR_EXTERNAL
    G : INT;
END_VAR
END_PROGRAM
"#;
    check_no_errors_multi(&[gvl, consumer]);
}

#[test]
fn multiple_file_scope_gvls_are_aggregated() {
    let gvl_a = r#"
VAR_GLOBAL
    G_A : INT;
END_VAR
"#;
    let gvl_b = r#"
VAR_GLOBAL
    G_B : INT;
END_VAR
"#;
    let consumer = r#"
PROGRAM Main
VAR_EXTERNAL
    G_A : INT;
    G_B : INT;
END_VAR
END_PROGRAM
    "#;
    check_no_errors_multi(&[gvl_a, gvl_b, consumer]);
}

#[test]
fn duplicate_global_names_across_scopes_are_rejected() {
    check_has_error(
        r#"
VAR_GLOBAL
    G : INT;
END_VAR

PROGRAM Main
VAR_GLOBAL
    G : INT;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::DuplicateDeclaration,
    );
}

#[test]
fn duplicate_file_scope_global_names_are_rejected_by_collector_path() {
    check_has_error(
        r#"
VAR_GLOBAL
    G : INT;
END_VAR

VAR_GLOBAL
    G : DINT;
END_VAR
"#,
        DiagnosticCode::DuplicateDeclaration,
    );
}
