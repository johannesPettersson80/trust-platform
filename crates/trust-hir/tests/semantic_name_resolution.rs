mod common;
use common::*;

// Name Resolution Tests
#[test]
// IEC 61131-3 Ed.3 Section 6.5.2.2 (scope rules)
fn test_undefined_variable() {
    check_has_error(
        r#"
PROGRAM Test
    VAR x : INT; END_VAR
    y := 10;
END_PROGRAM
"#,
        DiagnosticCode::UndefinedVariable,
    );
}

#[test]
fn test_variable_in_scope() {
    // Use an explicit integer type in the declaration.
    check_no_errors(
        r#"
PROGRAM Test
    VAR x : DINT; END_VAR
    x := 10;
END_PROGRAM
"#,
    );
}

#[test]
fn test_variable_in_scope_test_program() {
    check_no_errors(
        r#"
TEST_PROGRAM TestSuite
    VAR x : DINT; END_VAR
    x := 10;
END_TEST_PROGRAM
"#,
    );
}

#[test]
fn test_variable_in_scope_test_function_block() {
    check_no_errors(
        r#"
TEST_FUNCTION_BLOCK FB_TestCase
    VAR x : DINT; END_VAR
    x := 10;
END_TEST_FUNCTION_BLOCK
"#,
    );
}

#[test]
// IEC 61131-3 Ed.3 Section 6.5.2.2 (duplicate declarations)
fn test_duplicate_declaration() {
    check_has_error(
        r#"
PROGRAM Test
    VAR
        x : INT;
        x : BOOL;
    END_VAR
END_PROGRAM
"#,
        DiagnosticCode::DuplicateDeclaration,
    );
}

#[test]
fn test_reserved_identifier() {
    check_has_error(
        r#"
PROGRAM Test
    VAR GET : INT; END_VAR
END_PROGRAM
"#,
        DiagnosticCode::InvalidIdentifier,
    );
}

#[test]
fn test_property_name_with_underscore_backing_var() {
    let iface = r#"
INTERFACE IPump
    PUBLIC PROPERTY Speed : INT
        GET
        END_GET
        SET
        END_SET
    END_PROPERTY
END_INTERFACE
"#;
    let fb = r#"
FUNCTION_BLOCK PumpFb IMPLEMENTS IPump
VAR
    speed_value : INT;
END_VAR
PUBLIC PROPERTY Speed : INT
    GET
        Speed := speed_value;
    END_GET
    SET
        speed_value := Speed;
    END_SET
END_PROPERTY
END_FUNCTION_BLOCK
"#;
    let mut db = Database::new();
    let iface_id = FileId(0);
    let fb_id = FileId(1);
    db.set_source_text(iface_id, iface.to_string());
    db.set_source_text(fb_id, fb.to_string());
    let diags = db.diagnostics(fb_id);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
}

#[test]
fn test_multiple_variables_same_type() {
    // Use an explicit integer type in the declaration.
    check_no_errors(
        r#"
PROGRAM Test
    VAR a, b, c : DINT; END_VAR
    a := 1;
    b := 2;
    c := 3;
END_PROGRAM
"#,
    );
}

// CODESYS qualified enum member access (`EnumType.Member`, vendor extension):
// the base names an enum type, not a value.
#[test]
fn qualified_enum_member_in_gvl_initializer_resolves() {
    check_no_errors_multi(&[
        r#"
TYPE eModeMaster : (Off := 0, Night := 2, Auto := 3, Hybrid := 2, Disabled := 4);
END_TYPE
"#,
        r#"
NAMESPACE SomeNs
VAR_GLOBAL RETAIN PERSISTENT
    aeMasterManual : ARRAY[0..7] OF eModeMaster := [eModeMaster.Hybrid, eModeMaster.Night, eModeMaster.Disabled];
END_VAR
END_NAMESPACE
"#,
    ]);
}

#[test]
fn qualified_enum_member_in_pou_body_resolves() {
    check_no_errors(
        r#"
TYPE eModeMaster : (Off := 0, Night := 2, Hybrid := 2);
END_TYPE
PROGRAM Main
VAR
    x : eModeMaster;
END_VAR
    x := eModeMaster.Hybrid;
END_PROGRAM
"#,
    );
}

#[test]
fn qualified_enum_member_with_unknown_member_still_rejected() {
    check_has_error(
        r#"
TYPE eModeMaster : (Off := 0, Night := 2);
END_TYPE
PROGRAM Main
VAR
    x : eModeMaster;
END_VAR
    x := eModeMaster.Nope;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

// s4 T10: a stub file's failed constant must be attributed to its own file,
// not leak into every other file's diagnostics (agent-serve reported stub
// spans under workspace src/ paths).
#[test]
fn const_eval_error_from_one_file_does_not_leak_into_another() {
    let mut db = Database::new();
    db.set_source_text(
        FileId(0),
        r#"
PROGRAM Main
VAR x : INT; END_VAR
    x := 1;
END_PROGRAM
"#
        .to_string(),
    );
    db.set_source_text(
        FileId(1),
        r#"
VAR_GLOBAL CONSTANT
    width : INT := MAX_UNDEFINED;
END_VAR
"#
        .to_string(),
    );

    let file0: Vec<_> = db
        .diagnostics(FileId(0))
        .iter()
        .filter(|d| d.code == DiagnosticCode::UndefinedVariable)
        .map(|d| d.code)
        .collect();
    assert!(
        file0.is_empty(),
        "clean file must not inherit another file's constant failure: {file0:?}"
    );
    let file1_has_own = db
        .diagnostics(FileId(1))
        .iter()
        .any(|d| d.code == DiagnosticCode::UndefinedVariable);
    assert!(
        file1_has_own,
        "the owning file keeps its own constant failure"
    );
}
